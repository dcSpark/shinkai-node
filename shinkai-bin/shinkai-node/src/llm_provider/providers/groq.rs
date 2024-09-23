use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::openai_api::{openai_prepare_messages, MessageContent, OpenAIResponse};
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self};
use shinkai_db::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{Groq, LLMProviderInterface};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tokio::sync::Mutex;

#[async_trait]
impl LLMService for Groq {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
        _model: LLMProviderInterface,
        inbox_name: Option<InboxName>,
        _ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let url = format!("{}{}", base_url, "/chat/completions");
                let groq = Groq {
                    model_type: self.model_type.clone(),
                };
                let model = LLMProviderInterface::Groq(groq);
                let max_tokens = ModelCapabilitiesManager::get_max_tokens(&model);
                // Note(Nico): we can use prepare_messages directly or we could had called ModelCapabilitiesManager
                let result = openai_prepare_messages(&model, prompt)?;
                let messages_json = match result.messages {
                    PromptResultEnum::Value(mut v) => {
                        // Assuming `v` is a serde_json::Value representing an array of messages
                        if let JsonValue::Array(ref mut messages) = v {
                            for message in messages.iter_mut() {
                                if let JsonValue::Object(ref mut obj) = message {
                                    if let Some(JsonValue::Array(contents)) = obj.get_mut("content") {
                                        // Concatenate all text fields in the content array into a single string
                                        let concatenated_content = contents
                                            .iter()
                                            .filter_map(|content| {
                                                if let JsonValue::Object(content_obj) = content {
                                                    content_obj.get("text").and_then(|t| t.as_str()).map(String::from)
                                                } else {
                                                    None
                                                }
                                            })
                                            .collect::<Vec<String>>()
                                            .join(" ");
                                        // Replace the content array with a single string
                                        obj.insert("content".to_string(), JsonValue::String(concatenated_content));
                                    }
                                }
                            }
                        }
                        v
                    }
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                let payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "temperature": 0.7,
                    "max_tokens": result.remaining_tokens,
                });

                let payload_log = payload.clone();
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
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Groq Call API Response Text: {:?}", response_text).as_str(),
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
                                Some(MessageContent::Text(text)) => Some(text.clone()),
                                _ => None,
                            })
                            .collect::<Vec<String>>()
                            .join(" ");
                        Ok(LLMInferenceResponse::new(response_string, json!({}), None, None))
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
