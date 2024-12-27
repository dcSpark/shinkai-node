use super::error::LLMProviderError;
use super::execution::prompts::general_prompts::JobPromptGenerator;
use super::job_manager::JobManager;
use super::llm_stopper::LLMStopper;
use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use shinkai_fs::simple_parser::file_parser_helper::ShinkaiFileParser;
use shinkai_fs::simple_parser::text_group::TextGroup;
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_sqlite::SqliteManager;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ParsingHelper {}

impl ParsingHelper {
    // TODO: maybe rescue this one
    /// Given a list of TextGroup, generates a description using the Agent's LLM
    pub async fn generate_description(
        text_groups: &Vec<TextGroup>,
        agent: ProviderOrAgent,
        max_node_text_size: u64,
        db: Arc<SqliteManager>,
    ) -> Result<String, LLMProviderError> {
        let descriptions = ShinkaiFileParser::process_groups_into_descriptions_list(text_groups, 10000, 300);
        let prompt = JobPromptGenerator::simple_doc_description(descriptions);

        let mut extracted_answer: Option<String> = None;
        let llm_stopper = Arc::new(LLMStopper::new());
        for _ in 0..5 {
            let response_json = match JobManager::inference_with_llm_provider(
                agent.clone(),
                prompt.clone(),
                None,
                None,
                None,
                llm_stopper.clone(),
                db.clone(),
            )
            .await
            {
                Ok(json) => json,
                Err(_e) => {
                    continue; // Continue to the next iteration on error
                }
            };
            extracted_answer = Some(response_json.response_string);
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
}
