use crate::agent::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::agent::job_manager::JobManager;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};

use super::super::{error::AgentError, execution::prompts::prompts::Prompt};
use super::shared::openai::{openai_prepare_messages, MessageContent, OpenAIResponse};
use super::shared::shared_model_logic::parse_markdown_to_json;
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self};
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, OpenAI, ShinkaiBackend};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};

fn truncate_image_url_in_payload(payload: &mut JsonValue) {
    if let Some(messages) = payload.get_mut("messages") {
        if let Some(array) = messages.as_array_mut() {
            for message in array {
                if let Some(content) = message.get_mut("content") {
                    if let Some(array) = content.as_array_mut() {
                        for item in array {
                            if let Some(image_url) = item.get_mut("image_url") {
                                if let Some(url) = image_url.get_mut("url") {
                                    if let Some(str_url) = url.as_str() {
                                        let truncated_url = format!("{}...", &str_url[0..20.min(str_url.len())]);
                                        *url = JsonValue::String(truncated_url);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[async_trait]
impl LLMProvider for ShinkaiBackend {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
        model: AgentLLMInterface,
    ) -> Result<LLMInferenceResponse, AgentError> {
        if let Some(base_url) = url {
            let url = format!("{}/ai/chat/completions", base_url);
            if let Some(key) = api_key {
                let messages_json = match self.model_type().to_uppercase().as_str() {
                    "PREMIUM_TEXT_INFERENCE"
                    | "PREMIUM_VISION_INFERENCE"
                    | "STANDARD_TEXT_INFERENCE"
                    | "FREE_TEXT_INFERENCE" => {
                        let result = openai_prepare_messages(&model, prompt)?;
                        match result.value {
                            PromptResultEnum::Value(v) => v,
                            _ => {
                                return Err(AgentError::UnexpectedPromptResultVariant(
                                    "Expected Value variant in PromptResultEnum".to_string(),
                                ))
                            }
                        }
                    }
                    _ => {
                        return Err(AgentError::InvalidModelType(format!(
                            "Unsupported model type: {:?}",
                            self.model_type()
                        )))
                    }
                };
                // eprintln!("Messages JSON: {:?}", messages_json);

                let mut payload = json!({
                    "model": self.model_type(),
                    "messages": messages_json,
                    "temperature": 0.7,
                    // "max_tokens": result.remaining_tokens, // TODO: Check if this is necessary
                });

                let mut payload_log = payload.clone();
                truncate_image_url_in_payload(&mut payload_log);
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", payload_log).as_str(),
                );

                let _payload_string =
                    serde_json::to_string(&payload).unwrap_or_else(|_| String::from("Failed to serialize payload"));

                // eprintln!("Curl command:");
                // eprintln!("curl -X POST \\");
                // eprintln!("  -H 'Content-Type: application/json' \\");
                // eprintln!("  -H 'Authorization: Bearer {}' \\", key);
                // eprintln!("  -d '{}' \\", payload_string);
                // eprintln!("  '{}'", url);

                let request = client
                    .post(url)
                    .bearer_auth(key)
                    .header("Content-Type", "application/json")
                    .json(&payload);

                shinkai_log(
                    ShinkaiLogOption::DetailedAPI,
                    ShinkaiLogLevel::Debug,
                    format!("Request Details: {:?}", request).as_str(),
                );
                let res = request.send().await?;

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Status: {:?}", res.status()).as_str(),
                );

                let response_text = res.text().await?;
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Response Text: {:?}", response_text).as_str(),
                );

                let data_resp: Result<JsonValue, _> = serde_json::from_str(&response_text);

                match data_resp {
                    Ok(value) => {
                        if let Some(status_code) = value.get("statusCode").and_then(|code| code.as_u64()) {
                            let resp_message = value.get("message").and_then(|m| m.as_str()).unwrap_or_default();
                            return Err(match status_code {
                                401 => AgentError::ShinkaiBackendInvalidAuthentication(resp_message.to_string()),
                                403 => AgentError::ShinkaiBackendInvalidConfiguration(resp_message.to_string()),
                                429 => AgentError::ShinkaiBackendInferenceLimitReached(resp_message.to_string()),
                                500 => AgentError::ShinkaiBackendAIProviderError(resp_message.to_string()),
                                _ => AgentError::ShinkaiBackendUnexpectedStatusCode(status_code),
                            });
                        }

                        // TODO: refactor parsing logic so it's reusable
                        // If not an error, but actual response
                        if self.model_type().contains("vision") {
                            let data: OpenAIResponse = serde_json::from_value(value).map_err(AgentError::SerdeError)?;
                            let response_string: String = data
                                .choices
                                .iter()
                                .filter_map(|choice| match &choice.message.content {
                                    MessageContent::Text(text) => {
                                        // Unescape the JSON string
                                        let cleaned_json_str = text.replace("\\\"", "\"").replace("\\n", "\n");
                                        Some(cleaned_json_str)
                                    }
                                    MessageContent::ImageUrl { .. } => None,
                                })
                                .collect::<Vec<String>>()
                                .join(" ");
                            match parse_markdown_to_json(&response_string) {
                                Ok(json) => {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Debug,
                                        format!("Parsed JSON from Markdown: {:?}", json).as_str(),
                                    );
                                    Ok(LLMInferenceResponse::new(response_string, json))
                                }
                                Err(e) => {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        format!("Failed to parse Markdown to JSON: {:?}", e).as_str(),
                                    );
                                    Err(e)
                                }
                            }
                        } else {
                            let data: OpenAIResponse = serde_json::from_value(value).map_err(AgentError::SerdeError)?;
                            let response_string: String = data
                                .choices
                                .iter()
                                .filter_map(|choice| match &choice.message.content {
                                    MessageContent::Text(text) => Some(text.clone()),
                                    MessageContent::ImageUrl { .. } => None,
                                })
                                .collect::<Vec<String>>()
                                .join(" ");
                            match parse_markdown_to_json(&response_string) {
                                Ok(json) => {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Debug,
                                        format!("Parsed JSON from Markdown: {:?}", json).as_str(),
                                    );
                                    Ok(LLMInferenceResponse::new(response_string, json))
                                }
                                Err(e) => {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        format!("Failed to parse Markdown to JSON: {:?}", e).as_str(),
                                    );
                                    Err(e)
                                }
                            }
                        }
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Failed to parse response: {:?}", e).as_str(),
                        );
                        Err(AgentError::SerdeError(e))
                    }
                }
            } else {
                Err(AgentError::ApiKeyNotSet)
            }
        } else {
            Err(AgentError::UrlNotSet)
        }
    }
}
