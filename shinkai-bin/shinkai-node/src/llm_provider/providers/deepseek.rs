use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::deepseek_api::deepseek_prepare_messages;
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::llm_provider::providers::openai::{
    add_options_to_payload, handle_non_streaming_response, handle_streaming_response, truncate_image_url_in_payload
};
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{DeepSeek, LLMProviderInterface};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use uuid::Uuid;

#[async_trait]
impl LLMService for DeepSeek {
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
            if let Some(key) = api_key {
                // DeepSeek API is compatible with OpenAI API format
                let url = format!("{}{}", base_url, "/chat/completions");

                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);

                // Use the OpenAI message preparation since DeepSeek API is compatible
                let result = deepseek_prepare_messages(&model, prompt, session_id.clone())?;
                let messages_json = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                // Extract tools_json from the result
                let mut tools_json = result.functions.unwrap_or_else(Vec::new);

                // Set up initial payload with appropriate token limit field based on model capabilities
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

                // Conditionally add functions to the payload if tools_json is not empty
                if !tools_json.is_empty() {
                    let formatted_tools = tools_json
                        .iter()
                        .map(|tool| {
                            serde_json::json!({
                                "type": "function",
                                "function": tool
                            })
                        })
                        .collect::<Vec<serde_json::Value>>();
                    payload["tools"] = serde_json::Value::Array(formatted_tools);
                }

                // Only add options to payload for non-reasoning models
                if !ModelCapabilitiesManager::has_reasoning_capabilities(&model) {
                    add_options_to_payload(&mut payload, config.as_ref());
                }

                // Print payload as a pretty JSON string
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
                        key.clone(),
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
                        key.clone(),
                        inbox_name,
                        llm_stopper,
                        ws_manager_trait,
                        Some(tools_json),
                        None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptType};

    #[test]
    fn test_deepseek_provider_creation() {
        let deepseek = DeepSeek {
            model_type: "deepseek-chat".to_string(),
        };
        assert_eq!(deepseek.model_type, "deepseek-chat");
    }

    #[test]
    fn test_deepseek_serialized_provider() {
        let provider = SerializedLLMProvider {
            id: "test-id".to_string(),
            full_identity_name: ShinkaiName::new("@@test.shinkai/main/agent/deepseek_test".to_string()).unwrap(),
            model: LLMProviderInterface::DeepSeek(DeepSeek {
                model_type: "deepseek-chat".to_string(),
            }),
            api_key: Some("test-key".to_string()),
            external_url: Some("https://api.deepseek.com".to_string()),
        };

        if let LLMProviderInterface::DeepSeek(deepseek) = &provider.model {
            assert_eq!(deepseek.model_type, "deepseek-chat");
        } else {
            panic!("Expected DeepSeek provider");
        }
    }
}
