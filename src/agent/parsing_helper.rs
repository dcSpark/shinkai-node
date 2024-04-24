use super::error::AgentError;
use super::execution::chains::tool_execution_chain;
use super::execution::prompts::prompts::{JobPromptGenerator, Prompt};
use super::job_manager::JobManager;


use regex::Regex;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::file_parser::file_parser::ShinkaiFileParser;
use shinkai_vector_resources::file_parser::file_parser_types::TextGroup;
use shinkai_vector_resources::file_parser::unstructured_api::{UnstructuredAPI};



use shinkai_vector_resources::source::{DistributionInfo, SourceFile, SourceFileMap, TextChunkingStrategy};
use shinkai_vector_resources::vector_resource::{BaseVectorResource, SourceFileType, VRKai, VRPath};
use shinkai_vector_resources::{data_tags::DataTag, source::VRSourceReference};
use std::collections::HashMap;


pub struct ParsingHelper {}

impl ParsingHelper {
    /// Given a list of TextGroup, generates a description using the Agent's LLM
    pub async fn generate_description(
        text_groups: &Vec<TextGroup>,
        agent: SerializedAgent,
        max_node_text_size: u64,
    ) -> Result<String, AgentError> {
        let descriptions = ShinkaiFileParser::process_groups_into_descriptions_list(text_groups, 10000, 300);
        let prompt = JobPromptGenerator::simple_doc_description(descriptions);

        let mut extracted_answer: Option<String> = None;
        for _ in 0..5 {
            let response_json = match JobManager::inference_agent_json(agent.clone(), prompt.clone()).await {
                Ok(json) => json,
                Err(_e) => {
                    continue; // Continue to the next iteration on error
                }
            };
            let (answer, _new_resp_json) = match JobManager::advanced_extract_key_from_inference_response_with_json(
                agent.clone(),
                response_json,
                prompt.clone(),
                vec!["summary".to_string(), "answer".to_string()],
                1,
            )
            .await
            {
                Ok(result) => result,
                Err(_e) => {
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

            let desc = ShinkaiFileParser::process_groups_into_description(
                &text_groups,
                max_node_text_size as usize,
                max_node_text_size.checked_div(2).unwrap_or(100) as usize,
            );
            let desc = ParsingHelper::ending_stripper(&desc);
            Ok(desc)
        }
    }

    ///  Processes the file buffer through Unstructured, our hierarchical structuring algo,
    ///  generates all embeddings, uses LLM to generate desc and improve overall structure quality,
    ///  and returns a finalized BaseVectorResource. If no agent is provided, description defaults to first text in elements.
    /// Note: Requires file_name to include the extension ie. `*.pdf` or url `http://...`
    pub async fn process_file_into_resource_gen_desc(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        file_name: String,
        parsing_tags: &Vec<DataTag>,
        agent: Option<SerializedAgent>,
        max_node_text_size: u64,
        unstructured_api: UnstructuredAPI,
        distribution_info: DistributionInfo,
    ) -> Result<BaseVectorResource, AgentError> {
        let cleaned_name = ShinkaiFileParser::clean_name(&file_name);
        let source = VRSourceReference::from_file(&file_name, TextChunkingStrategy::V1)?;
        let text_groups = ShinkaiFileParser::process_file_into_text_groups(
            file_buffer,
            file_name,
            max_node_text_size,
            source.clone(),
            unstructured_api,
        )
        .await?;

        let mut desc = None;
        if let Some(actual_agent) = agent {
            desc = Some(Self::generate_description(&text_groups, actual_agent, max_node_text_size).await?);
        } else {
            let description_text = ShinkaiFileParser::process_groups_into_description(
                &text_groups,
                max_node_text_size as usize,
                max_node_text_size.checked_div(2).unwrap_or(100) as usize,
            );
            if !description_text.trim().is_empty() {
                desc = Some(description_text);
            }
        }

        Ok(ShinkaiFileParser::process_groups_into_resource(
            text_groups,
            generator,
            cleaned_name,
            desc,
            source,
            parsing_tags,
            max_node_text_size,
            distribution_info,
        )
        .await?)
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
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                &format!("Processing file: {}", filename),
            );

            let resource = ParsingHelper::process_file_into_resource_gen_desc(
                file.1.clone(),
                generator,
                filename.clone(),
                &vec![],
                agent.clone(),
                (generator.model_type().max_input_token_count() - 20) as u64,
                unstructured_api.clone(),
                file.2.clone(),
            )
            .await?;

            let file_type = SourceFileType::detect_file_type(&file.0)?;
            let source = SourceFile::new_standard_source_file(file.0, file_type, file.1, None);
            let mut source_map = SourceFileMap::new(HashMap::new());
            source_map.add_source_file(VRPath::root(), source);

            processed_vrkais.push((filename, VRKai::new(resource, Some(source_map))))
        }

        Ok(processed_vrkais)
    }

    /// Generates Blake3 hash of the input data.
    fn generate_data_hash_blake3(content: &[u8]) -> String {
        ShinkaiFileParser::generate_data_hash(content)
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

    /// Attempts to clean up the answer response from the LLM for basic inferencing where the response is primarily English text
    pub fn basic_inference_text_answer_cleanup(string: &str) -> String {
        let flattened = ParsingHelper::flatten_to_content_if_json(&string);
        let stripped_string = ParsingHelper::ending_stripper(&flattened);
        let cleaned_string = ParsingHelper::add_paragraph_line_breaks(&stripped_string);
        cleaned_string
    }

    /// Splits the text into sentences based on periods, ignoring any periods inside of back ticks/code blocks.
    pub fn split_text_into_sentences_ignoring_back_ticks(text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current_sentence = String::new();
        let mut backtick_block = false;
        let mut backtick_count = 0;

        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            current_sentence.push(c);

            if c == '`' {
                backtick_count += 1;
                // Check if entering or exiting a backtick block
                if backtick_count == 1 || (backtick_block && backtick_count == 3) {
                    if let Some(&next_char) = chars.peek() {
                        if next_char != '`' {
                            // If it's a single backtick or the end of a triple backtick block
                            backtick_block = !backtick_block;
                            backtick_count = 0;
                        }
                    }
                }
                // Reset backtick count if it's a triple backtick block
                if backtick_count == 3 {
                    backtick_count = 0;
                }
            } else {
                // Reset backtick count on any character that's not a backtick
                backtick_count = 0;
            }

            // If not in a backtick block and the character is a period, consider it the end of a sentence
            if !backtick_block && c == '.' {
                sentences.push(current_sentence.trim().to_string());
                current_sentence.clear();
            }
        }

        // Add any remaining text as a sentence
        if !current_sentence.trim().is_empty() {
            sentences.push(current_sentence.trim().to_string());
        }

        sentences
    }

    /// Adds line breaks after sentences exceed a certain character limit.
    pub fn add_paragraph_line_breaks(string: &str) -> String {
        let sentences = Self::split_text_into_sentences_ignoring_back_ticks(string);
        let max_chars = 450;
        let mut result_string = String::new();
        let mut current_paragraph = String::new();

        for sentence in sentences {
            // Check if adding the next sentence exceeds the max_chars limit
            if current_paragraph.len() + sentence.len() > max_chars {
                // If the current paragraph is not empty, add it to the result with a double line break
                if !current_paragraph.is_empty() {
                    result_string.push_str(&format!("{}\n\n", current_paragraph.trim()));
                    current_paragraph.clear();
                }
                // Start a new paragraph with the current sentence
                current_paragraph.push_str(&sentence);
            } else {
                // If adding the sentence doesn't exceed the limit, just add it to the current paragraph
                current_paragraph.push_str(" ");
                current_paragraph.push_str(&sentence);
            }
        }

        // Add any remaining paragraph to the result
        if !current_paragraph.is_empty() {
            result_string.push_str(&format!("{}\n", current_paragraph.trim()));
        }

        result_string.trim().to_string()
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
