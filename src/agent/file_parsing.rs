use super::error::AgentError;
use super::execution::chains::tool_execution_chain;
use super::execution::job_prompts::{JobPromptGenerator, Prompt};
use super::job_manager::JobManager;
use csv::Reader;
use lazy_static::lazy_static;
use regex::Regex;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::source::{DistributionInfo, SourceFile, SourceFileMap, TextChunkingStrategy};
use shinkai_vector_resources::unstructured::unstructured_api::{self, UnstructuredAPI};
use shinkai_vector_resources::unstructured::unstructured_parser::UnstructuredParser;
use shinkai_vector_resources::unstructured::unstructured_types::UnstructuredElement;
use shinkai_vector_resources::vector_resource::{BaseVectorResource, SourceFileType, VRKai, VRPath};
use shinkai_vector_resources::{data_tags::DataTag, source::VRSource};
use std::collections::HashMap;
use std::io::Cursor;

impl JobManager {
    /// Given a list of UnstructuredElements generates a description using the Agent's LLM
    // TODO: the 2000 should be dynamic depending on the agent model capabilities
    pub async fn generate_description(
        elements: &Vec<UnstructuredElement>,
        agent: SerializedAgent,
        max_node_size: u64,
    ) -> Result<String, AgentError> {
        let prompt = ParsingHelper::process_elements_into_description_prompt(&elements, 2000);

        let mut extracted_answer: Option<String> = None;
        for _ in 0..5 {
            let response_json = match JobManager::inference_agent(agent.clone(), prompt.clone()).await {
                Ok(json) => json,
                Err(e) => {
                    continue; // Continue to the next iteration on error
                }
            };
            let (answer, _new_resp_json) = match JobManager::advanced_extract_key_from_inference_response(
                agent.clone(),
                response_json,
                prompt.clone(),
                vec!["summary".to_string(), "answer".to_string()],
                1,
            )
            .await
            {
                Ok(result) => result,
                Err(e) => {
                    continue; // Continue to the next iteration on error
                }
            };
            extracted_answer = Some(answer.clone());
            break; // Exit the loop if successful
        }

        if let Some(answer) = extracted_answer {
            let desc = ParsingHelper::ending_stripper(&answer);
            Ok(desc)
        } else {
            eprintln!(
                "Failed to generate VR description after multiple attempts. Defaulting to text from first N nodes."
            );

            let concat_text = ParsingHelper::concatenate_elements_up_to_max_size(&elements, max_node_size as usize);
            let desc = ParsingHelper::ending_stripper(&concat_text);
            Ok(desc)
        }
    }

    /// Processes the list of files into VRKai structs ready to be used/saved/etc.
    /// Supports both `.vrkai` files, and standard doc/html/etc which get generated into VRs.
    pub async fn process_files_into_vrkai(
        files: Vec<(String, Vec<u8>, DistributionInfo)>,
        generator: &dyn EmbeddingGenerator,
        agent: Option<SerializedAgent>,
        unstructured_api: UnstructuredAPI,
    ) -> Result<Vec<(String, VRKai)>, AgentError> {
        let (vrkai_files, other_files): (
            Vec<(String, Vec<u8>, DistributionInfo)>,
            Vec<(String, Vec<u8>, DistributionInfo)>,
        ) = files
            .into_iter()
            .partition(|(name, _, _dist_info)| name.ends_with(".vrkai"));
        let mut processed_vrkais = vec![];

        // Parse the `.vrkai` files
        for vrkai_file in vrkai_files {
            let filename = vrkai_file.0;
            eprintln!("Processing file: {}", filename);
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                &format!("Processing file: {}", filename),
            );

            processed_vrkais.push((filename, VRKai::from_bytes(&vrkai_file.1)?))
        }

        // Parse the other files by generating a Vector Resource from scratch
        for file in other_files {
            let filename = file.0.clone();
            eprintln!("Processing file: {}", filename);
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                &format!("Processing file: {}", filename),
            );

            let resource = JobManager::parse_file_into_resource_gen_desc(
                file.1.clone(),
                generator,
                filename.clone(),
                &vec![],
                agent.clone(),
                400,
                unstructured_api.clone(),
                file.2.clone(),
            )
            .await?;

            let file_type = SourceFileType::detect_file_type(&file.0)?;
            let source = SourceFile::new_standard_source_file(file.0, file_type, file.1, None);
            let mut source_map = SourceFileMap::new(HashMap::new());
            source_map.add_source_file(VRPath::root(), source);

            processed_vrkais.push((
                filename,
                VRKai::from_base_vector_resource(resource, Some(source_map), None),
            ))
        }

        Ok(processed_vrkais)
    }

    ///  Processes the file buffer through Unstructured, our hierarchical structuring algo,
    ///  generates all embeddings, uses LLM to generate desc and improve overall structure quality,
    ///  and returns a finalized BaseVectorResource. If no agent is provided, description defaults to first text in elements.
    /// Note: The file name must include the extension ie. `*.pdf`
    pub async fn parse_file_into_resource_gen_desc(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        parsing_tags: &Vec<DataTag>,
        agent: Option<SerializedAgent>,
        max_node_size: u64,
        unstructured_api: UnstructuredAPI,
        distribution_info: DistributionInfo,
    ) -> Result<BaseVectorResource, AgentError> {
        let (_, source, elements) =
            ParsingHelper::parse_file_helper(file_buffer.clone(), name.clone(), unstructured_api).await?;
        let mut desc = String::new();
        if let Some(actual_agent) = agent {
            desc = Self::generate_description(&elements, actual_agent, max_node_size).await?;
        } else {
            desc = ParsingHelper::concatenate_elements_up_to_max_size(&elements, max_node_size as usize);
        }

        ParsingHelper::parse_elements_into_resource(
            elements,
            generator,
            name,
            Some(desc),
            source,
            parsing_tags,
            max_node_size,
            distribution_info,
        )
        .await
    }
}

pub struct ParsingHelper {}

impl ParsingHelper {
    /// Generates Blake3 hash of the input data.
    fn generate_data_hash_blake3(content: &[u8]) -> String {
        UnstructuredParser::generate_data_hash(content)
    }

    /// Concatenate elements text up to a maximum size.
    pub fn concatenate_elements_up_to_max_size(elements: &[UnstructuredElement], max_size: usize) -> String {
        let mut desc = String::new();
        for e in elements {
            if desc.len() + e.text.len() + 1 > max_size {
                break; // Stop appending if adding the next element would exceed max_size
            }
            desc.push_str(&e.text);
            desc.push('\n'); // Add a line break after each element's text
        }
        desc.trim_end().to_string() // Trim any trailing space before returning
    }

    ///  Processes the file buffer through Unstructured, our hierarchical structuring algo,
    ///  generates all embeddings,  and returns a finalized BaseVectorResource.
    /// Note: The file name must include the extension ie. `*.pdf`
    pub async fn parse_file_into_resource(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        file_name: String,
        desc: Option<String>,
        parsing_tags: &Vec<DataTag>,
        max_node_size: u64,
        unstructured_api: UnstructuredAPI,
    ) -> Result<BaseVectorResource, AgentError> {
        let (_, source, elements) =
            ParsingHelper::parse_file_helper(file_buffer.clone(), file_name.clone(), unstructured_api).await?;

        // Cleans out the file extension from the file_name
        let cleaned_name = SourceFileType::clean_string_of_extension(&file_name);

        Self::parse_elements_into_resource(
            elements,
            generator,
            cleaned_name,
            desc,
            source,
            parsing_tags,
            max_node_size,
        )
        .await
    }

    /// Helper method which keeps core logic related to parsing elements into a BaseVectorResource
    pub async fn parse_elements_into_resource(
        elements: Vec<UnstructuredElement>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        desc: Option<String>,
        source: VRSource,
        parsing_tags: &Vec<DataTag>,
        max_node_size: u64,
        distribution_info: DistributionInfo,
    ) -> Result<BaseVectorResource, AgentError> {
        let name = Self::clean_name(&name);
        let resource = UnstructuredParser::process_elements_into_resource(
            elements,
            generator,
            name,
            desc,
            source,
            parsing_tags,
            max_node_size,
        )
        .await?;

        Ok(resource)
    }

    /// Cleans the JSON response string using regex, including replacing `\_` with `_` and removing unnecessary line breaks.
    pub fn clean_json_response_via_regex(json_string: &str) -> String {
        // First, replace `\_` with `_` to avoid parsing issues.
        let mut cleaned_string = json_string.replace("\\_", "_");

        // Patterns for removing unnecessary line breaks and spaces around JSON structural characters.
        let patterns = vec![
            (r#"\n\s*\{"#, "{"),
            (r#"\{\s*\n"#, "{"),
            (r#"\n\s*\}"#, "}"),
            (r#"\}\s*\n"#, "}"),
            (r#"\n\s*\""#, "\""),
            (r#"\"\s*\n"#, "\""),
            (r#"\n\s*,"#, ","),
            (r#",\s*\n"#, ","),
        ];

        for (pattern, replacement) in patterns {
            let re = Regex::new(pattern).unwrap();
            cleaned_string = re.replace_all(&cleaned_string, replacement).to_string();
        }

        cleaned_string
    }

    /// Clean's the file name of auxiliary data (file extension, url in front of file name, etc.)
    fn clean_name(name: &str) -> String {
        // Decode URL-encoded characters to simplify processing.
        let decoded_name = urlencoding::decode(name).unwrap_or_else(|_| name.into());

        // Check if the name ends with ".htm" or ".html" and calculate the position to avoid deletion.
        let avoid_deletion_position = if decoded_name.ends_with(".htm") || decoded_name.ends_with(".html") {
            decoded_name.len().saturating_sub(4) // Position before ".htm" or ".html"
        } else {
            decoded_name.len() // Use the full length if not ending with ".htm" or ".html"
        };
        // Find the last occurrence of "/" or "%2F" that is not too close to the ".htm" extension.
        let last_relevant_slash_position = decoded_name.rmatch_indices(&['/', '%']).find_map(|(index, _)| {
            if index + 3 < avoid_deletion_position && decoded_name[index..].starts_with("%2F") {
                Some(index)
            } else if index + 1 < avoid_deletion_position && decoded_name[index..].starts_with("/") {
                Some(index)
            } else {
                None
            }
        });
        // If a relevant slash is found, slice the string from the character immediately following this slash.
        let http_cleaned = match last_relevant_slash_position {
            Some(index) => decoded_name
                .get((index + if decoded_name[index..].starts_with("%2F") { 3 } else { 1 })..)
                .unwrap_or(&decoded_name),
            None => &decoded_name,
        };

        let http_cleaned = if http_cleaned.is_empty() || http_cleaned == ".html" || http_cleaned == ".htm" {
            decoded_name.to_string()
        } else {
            http_cleaned.to_string()
        };

        // Remove extension
        let cleaned_name = SourceFileType::clean_string_of_extension(&http_cleaned);

        cleaned_name
    }

    /// Basic helper method which parses file into needed data for generating a BaseVectorResource
    async fn parse_file_helper(
        file_buffer: Vec<u8>,
        file_name: String,
        unstructured_api: UnstructuredAPI,
    ) -> Result<(String, VRSource, Vec<UnstructuredElement>), AgentError> {
        let resource_id = UnstructuredParser::generate_data_hash(&file_buffer);
        let source = VRSource::from_file(&file_name, None, TextChunkingStrategy::V1)?;
        let elements = unstructured_api.file_request(file_buffer, &file_name).await?;
        Ok((resource_id, source, elements))
    }

    /// Takes the provided elements and creates a description prompt ready to be used
    /// to inference with an LLM.
    pub fn process_elements_into_description_prompt(elements: &Vec<UnstructuredElement>, max_size: usize) -> Prompt {
        let max_node_size = 300;
        let mut descriptions = Vec::new();
        let mut description = String::new();
        let mut total_size = 0;

        for element in elements {
            let element_text = &element.text;
            if description.len() + element_text.len() > max_node_size {
                descriptions.push(description.clone());
                total_size += description.len();
                description.clear();
            }
            if total_size + element_text.len() > max_size {
                break;
            }
            description.push_str(element_text);
            description.push(' ');
        }
        if !description.is_empty() {
            descriptions.push(description);
        }
        JobPromptGenerator::simple_doc_description(descriptions)
    }

    /// Given an input string, if the whole string parses into a JSON Value, then
    /// reads through every key, and concatenates all of their values into a single output string.
    /// If not parsable into JSON Value, then return original string as a copy.
    /// To be used when inferencing with dumb LLMs.
    pub fn flatten_to_content_if_json(string: &str) -> String {
        match serde_json::from_str::<serde_json::Value>(string) {
            Ok(serde_json::Value::Object(obj)) => obj
                .values()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect::<Vec<String>>()
                .join(". "),
            _ => string.to_owned(),
        }
    }

    /// Removes last sentence from a string if it contains any of the unwanted phrases.
    /// This is used because the LLM sometimes answers properly, but then adds useless last sentence such as
    /// "However, specific details are not provided in the content." at the end.
    pub fn ending_stripper(string: &str) -> String {
        let mut sentences: Vec<&str> = string.split('.').collect();

        let unwanted_phrases = [
            "however,",
            "unfortunately",
            "additional research",
            "futher research",
            "may be required",
            "i do not",
            "further information",
            "specific details",
            "provided content",
            "more information",
            "not available",
        ];

        while let Some(last_sentence) = sentences.pop() {
            if last_sentence.trim().is_empty() {
                continue;
            }
            let sentence = last_sentence.trim_start().to_lowercase();
            if !unwanted_phrases.iter().any(|&phrase| sentence.contains(phrase)) {
                sentences.push(last_sentence);
            }
            break;
        }

        sentences.join(".")
    }
}

impl ParsingHelper {
    /// Parse CSV data from a buffer and attempt to automatically detect
    /// headers.
    pub fn parse_csv_auto(buffer: &[u8]) -> Result<Vec<String>, VRError> {
        let mut reader = Reader::from_reader(Cursor::new(buffer));
        let headers = reader
            .headers()
            .map_err(|_| VRError::FailedCSVParsing)?
            .iter()
            .map(String::from)
            .collect::<Vec<String>>();

        let likely_header = headers.iter().all(|s| {
            let is_alphabetic = s.chars().all(|c| c.is_alphabetic() || c.is_whitespace());
            let no_duplicates = headers.iter().filter(|&item| item == s).count() == 1;
            let no_prohibited_chars = !s.contains(&['@', '#', '$', '%', '^', '&', '*'][..]);

            is_alphabetic && no_duplicates && no_prohibited_chars
        });

        Self::parse_csv(&buffer, likely_header)
    }

    /// Parse CSV data from a buffer.
    /// * `header` - A boolean indicating whether to prepend column headers to
    ///   values.
    pub fn parse_csv(buffer: &[u8], header: bool) -> Result<Vec<String>, VRError> {
        let mut reader = Reader::from_reader(Cursor::new(buffer));
        let headers = if header {
            reader
                .headers()
                .map_err(|_| VRError::FailedCSVParsing)?
                .iter()
                .map(String::from)
                .collect::<Vec<String>>()
        } else {
            Vec::new()
        };

        let mut result = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|_| VRError::FailedCSVParsing)?;
            let row: Vec<String> = if header {
                record
                    .iter()
                    .enumerate()
                    .map(|(i, e)| format!("{}: {}", headers[i], e))
                    .collect()
            } else {
                record.iter().map(String::from).collect()
            };
            let row_string = row.join(", ");
            result.push(row_string);
        }

        Ok(result)
    }
}
