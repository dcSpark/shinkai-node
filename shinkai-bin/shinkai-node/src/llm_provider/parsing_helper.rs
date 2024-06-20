use super::error::LLMProviderError;
use super::execution::chains::inference_chain_trait::LLMInferenceResponse;
use super::execution::prompts::prompts::JobPromptGenerator;
use super::execution::user_message_parser::{JobTaskElement, ParsedUserMessage};
use super::job_manager::JobManager;
use regex::Regex;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;
use shinkai_vector_resources::file_parser::file_parser::ShinkaiFileParser;
use shinkai_vector_resources::file_parser::file_parser_types::TextGroup;
use shinkai_vector_resources::file_parser::unstructured_api::UnstructuredAPI;
use shinkai_vector_resources::source::{DistributionInfo, SourceFile, SourceFileMap, TextChunkingStrategy};
use shinkai_vector_resources::vector_resource::{BaseVectorResource, SourceFileType, VRKai, VRPath};
use shinkai_vector_resources::{data_tags::DataTag, source::VRSourceReference};
use std::collections::HashMap;

pub struct ParsingHelper {}

impl ParsingHelper {
    /// Given a list of TextGroup, generates a description using the Agent's LLM
    pub async fn generate_description(
        text_groups: &Vec<TextGroup>,
        agent: SerializedLLMProvider,
        max_node_text_size: u64,
    ) -> Result<String, LLMProviderError> {
        let descriptions = ShinkaiFileParser::process_groups_into_descriptions_list(text_groups, 10000, 300);
        let prompt = JobPromptGenerator::simple_doc_description(descriptions);

        let mut extracted_answer: Option<String> = None;
        for _ in 0..5 {
            let response_json = match JobManager::inference_agent_markdown(agent.clone(), prompt.clone()).await {
                Ok(json) => json,
                Err(_e) => {
                    continue; // Continue to the next iteration on error
                }
            };
            extracted_answer = Some(response_json.original_response_string);
            break; // Exit the loop if successful
        }

        if let Some(answer) = extracted_answer {
            let desc = answer.to_string();
            Ok(desc)
        } else {
            eprintln!(
                "Failed to generate VR description after multiple attempts. Defaulting to text from first N nodes."
            );

            let desc = ShinkaiFileParser::process_groups_into_description(
                text_groups,
                max_node_text_size as usize,
                max_node_text_size.checked_div(2).unwrap_or(100) as usize,
            );
            Ok(desc)
        }
    }

    ///  Processes the file buffer through Unstructured, our hierarchical structuring algo,
    ///  generates all embeddings, uses LLM to generate desc and improve overall structure quality,
    ///  and returns a finalized BaseVectorResource. If no agent is provided, description defaults to first text in elements.
    /// Note: Requires file_name to include the extension ie. `*.pdf` or url `http://...`
    #[allow(clippy::too_many_arguments)]
    pub async fn process_file_into_resource_gen_desc(
        file_buffer: Vec<u8>,
        generator: &dyn EmbeddingGenerator,
        file_name: String,
        parsing_tags: &Vec<DataTag>,
        agent: Option<SerializedLLMProvider>,
        max_node_text_size: u64,
        unstructured_api: UnstructuredAPI,
        distribution_info: DistributionInfo,
    ) -> Result<BaseVectorResource, LLMProviderError> {
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
        agent: Option<SerializedLLMProvider>,
        unstructured_api: UnstructuredAPI,
    ) -> Result<Vec<(String, VRKai)>, LLMProviderError> {
        #[allow(clippy::type_complexity)]
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

    /// Cleaning method for the LLM response JSON object, after its been parsed from the markdown string.
    /// Tries to get rid of weird visual edgecases LLMs tend to leave in the actual content
    pub fn clean_markdown_inference_response(response: LLMInferenceResponse) -> LLMInferenceResponse {
        let mut cleaned_json = response.json;
        if let JsonValue::Object(ref mut obj) = cleaned_json {
            for (_key, value) in obj.iter_mut() {
                if let JsonValue::String(ref mut str_value) = value {
                    *value = JsonValue::String(ParsingHelper::clean_markdown_result_string(str_value));
                }
            }
        }
        LLMInferenceResponse::new(response.original_response_string, cleaned_json)
    }

    /// Cleans the value string from a parsed markdown response from common LLM issues.
    fn clean_markdown_result_string(string: &str) -> String {
        let clean_llm_references = ParsingHelper::clean_llm_content_references(string);
        let link_image_cleaned = ParsingHelper::clean_markdown_urls_images(&clean_llm_references);
        let extra_cleaned = link_image_cleaned.replace("\\\\n", "\n");
        let parsed_message = ParsedUserMessage::new(extra_cleaned.to_string());

        // If there is a codeblock and it has no/disallowed content, then remove it
        if parsed_message.num_of_code_blocks() > 0 {
            let mut elements = vec![];
            for element in parsed_message.elements {
                if let JobTaskElement::CodeBlock(code_block) = &element {
                    if code_block.content_len() < 10 || code_block.content.contains("SYS") {
                        continue;
                    } else {
                        elements.push(element);
                    }
                } else {
                    elements.push(element);
                }
            }
            ParsedUserMessage::new_from_elements(elements).get_output_string()
        }
        // If there's no code blocks, then we can attempt to trim the string
        else {
            let sys_tags_regex = Regex::new(r"<</?SYS>>").unwrap();
            let mut cleaned_string = sys_tags_regex.replace_all(string, "").to_string();
            let mut done = false;
            while !done {
                done = true; // Assume no more trimming is needed, prove otherwise below.
                let trim_cases = ["```", "``` ", "```\n", "``` \n", "```md", "```md ", "```md\n", "md"];
                for case in trim_cases.iter() {
                    if cleaned_string.ends_with(case) {
                        cleaned_string = cleaned_string.strip_suffix(case).unwrap_or(&cleaned_string).to_string();
                        done = false; // Found a case, so continue trimming.
                    }
                }
            }
            cleaned_string.replace("< >", "").replace("<>", "")
        }
    }

    /// Cleans URLs and images from markdown strings.
    /// Removes markdown images entirely. Removes link syntax leaving only the link text.
    fn clean_markdown_urls_images(string: &str) -> String {
        let re_image = Regex::new(r"!\[[^\]]*\]\([^\)]*\)").unwrap(); // Matches markdown image syntax
        let cleaned_string = re_image.replace_all(string, ""); // Remove all images

        // Updated regex to handle nested parentheses in URLs
        let re_link = Regex::new(r"\[([^\]]+)\]\((?:[^()]|\([^)]*\))+\)").unwrap(); // Matches markdown link syntax
        let cleaned_string = re_link.replace_all(&cleaned_string, "$1"); // Replace link with link text

        cleaned_string.to_string()
    }

    /// Cleans content references from the LLM response.
    fn clean_llm_content_references(string: &str) -> String {
        let re_references = Regex::new(r"\[\^[0-9]+\]: http.+\n").unwrap();
        re_references.replace_all(string, "").to_string()
    }
}
