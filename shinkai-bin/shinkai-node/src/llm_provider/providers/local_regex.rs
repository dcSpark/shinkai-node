use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use serde_json::json;
use shinkai_sqlite::SqliteManager;
use std::sync::Arc;
use tokio::sync::Mutex;

use shinkai_message_primitives::schemas::{
    inbox_name::InboxName,
    job_config::JobConfig,
    llm_providers::serialized_llm_provider::{LLMProviderInterface, LocalRegex},
    prompts::Prompt,
    ws_types::WSUpdateHandler,
};

use crate::{
    llm_provider::{
        error::LLMProviderError, execution::chains::inference_chain_trait::LLMInferenceResponse,
        llm_stopper::LLMStopper, providers::shared::ollama_api::ollama_prepare_messages,
    },
    managers::model_capabilities_manager::PromptResultEnum,
};

use super::LLMService;

#[async_trait]
impl LLMService for LocalRegex {
    async fn call_api(
        &self,
        _client: &Client,
        _url: Option<&String>,
        _api_key: Option<&String>,
        prompt: Prompt,
        model: LLMProviderInterface,
        _inbox_name: Option<InboxName>,
        _ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        _config: Option<JobConfig>,
        _llm_stopper: Arc<LLMStopper>,
        db: Arc<SqliteManager>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        // Prepare messages from the prompt using Ollama's message preparation
        let messages_result = ollama_prepare_messages(&model, prompt)?;

        // Get the message content
        let messages = match &messages_result.messages {
            PromptResultEnum::Value(messages_json) => {
                // Extract the last message's content
                messages_json
                    .as_array()
                    .and_then(|arr| arr.last())
                    .and_then(|msg| msg.get("content"))
                    .and_then(|content| content.as_str())
                    .ok_or_else(|| {
                        LLMProviderError::UnexpectedPromptResultVariant("Failed to extract message content".to_string())
                    })?
            }
            _ => {
                return Err(LLMProviderError::UnexpectedPromptResultVariant(
                    "Expected Value variant in PromptResultEnum".to_string(),
                ))
            }
        };

        // Get patterns from the database for this specific provider
        let patterns = db.get_enabled_regex_patterns_for_provider(&model.model_type())
            .map_err(|e| LLMProviderError::DatabaseError(format!("Failed to get regex patterns: {}", e)))?;

        // Try to match the message against patterns
        for pattern in patterns {
            if let Ok(regex) = Regex::new(&pattern.pattern) {
                if regex.is_match(messages) {
                    return Ok(LLMInferenceResponse::new(pattern.response, json!({}), Vec::new(), None));
                }
            }
        }

        // If no pattern matches, return a default response
        Ok(LLMInferenceResponse::new(
            "No matching pattern found".to_string(),
            json!({}),
            Vec::new(),
            None,
        ))
    }
}
