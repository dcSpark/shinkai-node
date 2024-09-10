use std::sync::Arc;

use super::super::{error::LLMProviderError, execution::prompts::prompts::Prompt};
use super::shared::openai::{openai_prepare_messages, MessageContent, OpenAIResponse};
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::job::JobConfig;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use crate::network::ws_manager::WSUpdateHandler;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenAI};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tokio::sync::Mutex;

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
impl LLMService for OpenAI {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
        model: LLMProviderInterface,
        inbox_name: Option<InboxName>,
        _ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let url = format!("{}{}", base_url, "/v1/chat/completions");

                // Note(Nico): we can use prepare_messages directly or we could had called ModelCapabilitiesManager
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

                // Print messages_json as a pretty JSON string
                match serde_json::to_string_pretty(&messages_json) {
                    Ok(pretty_json) => eprintln!("Messages JSON: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize messages_json: {:?}", e),
                };

                match serde_json::to_string_pretty(&tools_json) {
                    Ok(pretty_json) => eprintln!("Tools JSON: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize tools_json: {:?}", e),
                };

                let mut payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "max_tokens": result.remaining_tokens,
                });

                // Conditionally add functions to the payload if tools_json is not empty
                if !tools_json.is_empty() {
                    payload["functions"] = serde_json::Value::Array(tools_json);
                }

                // Add options to payload
                add_options_to_payload(&mut payload);

                // Print payload as a pretty JSON string
                match serde_json::to_string_pretty(&payload) {
                    Ok(pretty_json) => eprintln!("Payload: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
                };

                let mut payload_log = payload.clone();
                truncate_image_url_in_payload(&mut payload_log);
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", payload_log).as_str(),
                );

                let res = client
                    .post(url)
                    .bearer_auth(key)
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
                    .await?;
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Status: {:?}", res.status()).as_str(),
                );

                let response_text = res.text().await?;
                let data_resp: Result<JsonValue, _> = serde_json::from_str(&response_text);
                // eprintln!("Data Resp: {:?}", data_resp);
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Response Text: {:?}", response_text).as_str(),
                );

                match data_resp {
                    Ok(value) => {
                        if let Some(error) = value.get("error") {
                            let code = error.get("code").and_then(|c| c.as_str());
                            let formatted_error = if let (Some(code), Some(message)) =
                                (code, error.get("message").and_then(|m| m.as_str()))
                            {
                                format!("{}: {}", code, message)
                            } else {
                                serde_json::to_string(&error).unwrap_or_default()
                            };

                            return Err(match code {
                                Some("rate_limit_exceeded") => {
                                    LLMProviderError::LLMServiceInferenceLimitReached(formatted_error.to_string())
                                }
                                _ => LLMProviderError::LLMServiceUnexpectedError(formatted_error.to_string()),
                            });
                        }

                        let data: OpenAIResponse =
                            serde_json::from_value(value).map_err(LLMProviderError::SerdeError)?;

                        let response_string: String = data
                            .choices
                            .iter()
                            .filter_map(|choice| match &choice.message.content {
                                Some(MessageContent::Text(text)) => {
                                    // Unescape the JSON string
                                    let cleaned_json_str = text.replace("\\\"", "\"").replace("\\n", "\n");
                                    Some(cleaned_json_str)
                                }
                                Some(MessageContent::ImageUrl { .. }) => None,
                                Some(MessageContent::FunctionCall(_)) => None,
                                None => None,
                            })
                            .collect::<Vec<String>>()
                            .join(" ");

                        let function_call = data.choices.iter().find_map(|choice| {
                            if let Some(mut fc) = choice.message.function_call.clone() {
                                if let Some(args_str) = fc.arguments.as_str() {
                                    fc.arguments = serde_json::from_str(args_str).unwrap_or_else(|_| json!({}));
                                }
                                Some(fc)
                            } else {
                                None
                            }
                        });
                        eprintln!("Function Call: {:?}", function_call);
                        eprintln!("Response String: {:?}", response_string);
                        Ok(LLMInferenceResponse::new(response_string, json!({}), function_call, None))
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Failed to parse response: {:?}", e).as_str(),
                        );
                        Err(LLMProviderError::SerdeError(e))
                    }
                }
            } else {
                Err(LLMProviderError::ApiKeyNotSet)
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}

fn add_options_to_payload(payload: &mut serde_json::Value) {
    // Helper function to read and parse environment variables
    fn read_env_var<T: std::str::FromStr>(key: &str) -> Option<T> {
        std::env::var(key).ok().and_then(|val| val.parse::<T>().ok())
    }

    // Read and add options from environment variables
    if let Some(seed) = read_env_var::<u64>("LLM_SEED") {
        payload["seed"] = serde_json::json!(seed);
    }
    if let Some(temp) = read_env_var::<f64>("LLM_TEMPERATURE") {
        payload["temperature"] = serde_json::json!(temp);
    }
    // Add more options as needed...
}
