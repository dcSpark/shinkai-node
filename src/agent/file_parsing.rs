use super::error::AgentError;
use super::execution::chains::tool_execution_chain;
use super::execution::job_prompts::{JobPromptGenerator, Prompt};
use super::job_manager::JobManager;
use csv::Reader;
use lazy_static::lazy_static;
use regex::Regex;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::source::TextChunkingStrategy;
use shinkai_vector_resources::unstructured::unstructured_api::{self, UnstructuredAPI};
use shinkai_vector_resources::unstructured::unstructured_parser::UnstructuredParser;
use shinkai_vector_resources::unstructured::unstructured_types::UnstructuredElement;
use shinkai_vector_resources::vector_resource::{BaseVectorResource, SourceFileType};
use shinkai_vector_resources::{data_tags::DataTag, source::VRSource};
use std::io::Cursor;

impl JobManager {
    /// Given a list of UnstructuredElements generates a description using the Agent's LLM
    pub async fn generate_description(
        elements: &Vec<UnstructuredElement>,
        agent: SerializedAgent,
    ) -> Result<String, AgentError> {
        // TODO: the 2000 should be dynamic depending on the LLM model
        let prompt = ParsingHelper::process_elements_into_description_prompt(&elements, 2000);
        let response_json = JobManager::inference_agent(agent.clone(), prompt.clone()).await?;
        let (answer, _new_resp_json) = &JobManager::extract_single_key_from_inference_response(
            agent.clone(),
            response_json,
            prompt,
            vec!["summary".to_string(), "answer".to_string()],
            1,
        )
        .await?;
        let desc = Some(ParsingHelper::ending_stripper(answer));
        eprintln!("LLM Generated File Description: {:?}", desc);
        Ok(desc.unwrap_or_else(|| "".to_string()))
    }

    ///  Processes the file buffer through Unstructured, our hierarchical structuring algo,
    ///  generates all embeddings, uses LLM to generate desc and improve overall structure quality,
    ///  and returns a finalized BaseVectorResource.
    /// Note: The file name must include the extension ie. `*.pdf`
    pub async fn parse_file_into_resource_gen_desc(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        name: String,
        parsing_tags: &Vec<DataTag>,
        agent: SerializedAgent,
        max_node_size: u64,
        unstructured_api: UnstructuredAPI,
    ) -> Result<BaseVectorResource, AgentError> {
        let (_, source, elements) =
            ParsingHelper::parse_file_helper(file_buffer.clone(), name.clone(), unstructured_api).await?;
        let desc = Self::generate_description(&elements, agent).await?;
        ParsingHelper::parse_elements_into_resource(
            elements,
            generator,
            name,
            Some(desc),
            source,
            parsing_tags,
            max_node_size,
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

    /// Removes `\n` when it's either in front or behind of a `{`, `}`, `"`, or `,`.
    pub fn clean_json_response_line_breaks(json_string: &str) -> String {
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

        let mut cleaned_string = json_string.to_string();
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
