use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::openai::{add_options_to_payload, handle_non_streaming_response, handle_streaming_response};
use super::shared::openai_api::{openai_prepare_messages, MessageContent, OpenAIResponse};
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{Gemini, LLMProviderInterface};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use tokio::sync::Mutex;
use uuid::Uuid;

#[async_trait]
impl LLMService for Gemini {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
        model: LLMProviderInterface,
        inbox_name: Option<InboxName>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        let session_id = Uuid::new_v4().to_string();
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let session_id = Uuid::new_v4().to_string();
                let url = format!("https://generativelanguage.googleapis.com/v1beta/openai/chat/completions");
                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);

                let result = openai_prepare_messages(&model, prompt)?;
                let messages_json = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                // Extract tools_json from the result
                let tools_json = result.functions.unwrap_or_else(Vec::new);

                let mut payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "max_tokens": result.remaining_output_tokens,
                    "stream": is_stream,
                });

                if !tools_json.is_empty() {
                    payload["functions"] = serde_json::Value::Array(tools_json.clone());
                }

                // Add options to payload
                add_options_to_payload(&mut payload, config.as_ref());

                // Print payload as a pretty JSON string only if IS_TESTING is true
                if std::env::var("LOG_ALL").unwrap_or_default() == "true"
                    || std::env::var("LOG_ALL").unwrap_or_default() == "1"
                {
                    match serde_json::to_string_pretty(&payload) {
                        Ok(pretty_json) => eprintln!("cURL Payload: {}", pretty_json),
                        Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
                    };
                }

                if is_stream {
                    handle_streaming_response(
                        client,
                        url,
                        payload,
                        key.clone(),
                        inbox_name,
                        ws_manager_trait,
                        llm_stopper,
                        session_id,
                        Some(tools_json),
                    )
                    .await
                } else {
                    handle_non_streaming_response(
                        client,
                        url,
                        payload,
                        key.clone(),
                        inbox_name,
                        llm_stopper,
                        ws_manager_trait,
                        Some(tools_json),
                    )
                    .await
                }
            } else {
                Err(LLMProviderError::ApiKeyNotSet)
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}
