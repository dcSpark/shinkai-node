use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::LLMService;
use super::openai::{
    add_options_to_payload, handle_non_streaming_response, handle_streaming_response,
    truncate_image_url_in_payload,
};
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, LMStudio,
};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{
    shinkai_log, ShinkaiLogLevel, ShinkaiLogOption,
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::shared::openai_api::openai_prepare_messages;

#[async_trait]
impl LLMService for LMStudio {
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
        _db: Arc<SqliteManager>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        let session_id = Uuid::new_v4().to_string();
        if let Some(base_url) = url {
            let url = format!("{}{}", base_url, "/api/v0/chat/completions");

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

            let tools_json = result.functions.unwrap_or_else(Vec::new);

            let mut payload = if ModelCapabilitiesManager::has_reasoning_capabilities(&model) {
                json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "max_completion_tokens": result.remaining_output_tokens,
                    "stream": is_stream,
                })
            } else {
                json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "max_tokens": result.remaining_output_tokens,
                    "stream": is_stream,
                })
            };

            if !tools_json.is_empty() {
                payload["tools"] = serde_json::Value::Array(tools_json.clone());
            }

            if !ModelCapabilitiesManager::has_reasoning_capabilities(&model) {
                add_options_to_payload(&mut payload, config.as_ref());
            }

            match serde_json::to_string_pretty(&payload) {
                Ok(pretty_json) => eprintln!("cURL Payload: {}", pretty_json),
                Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
            };

            let mut payload_log = payload.clone();
            truncate_image_url_in_payload(&mut payload_log);
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Call API Body: {:?}", payload_log).as_str(),
            );

            if is_stream {
                handle_streaming_response(
                    client,
                    url,
                    payload,
                    api_key.unwrap_or(&"".to_string()).to_string(),
                    inbox_name,
                    ws_manager_trait,
                    llm_stopper,
                    session_id,
                    Some(tools_json),
                    None,
                )
                .await
            } else {
                handle_non_streaming_response(
                    client,
                    url,
                    payload,
                    api_key.unwrap_or(&"".to_string()).to_string(),
                    inbox_name,
                    llm_stopper,
                    ws_manager_trait,
                    Some(tools_json),
                    None,
                )
                .await
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}

