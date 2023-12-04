use crate::managers::agents_capabilities_manager::{AgentsCapabilitiesManager, PromptResultEnum};

use super::super::{error::AgentError, execution::job_prompts::Prompt};
use super::shared::openai::{openai_prepare_messages, MessageContent, OpenAIResponse};
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self, Map};
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, OpenAI, ShinkaiBackend};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tiktoken_rs::model::get_context_size;

#[async_trait]
impl LLMProvider for ShinkaiBackend {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
    ) -> Result<JsonValue, AgentError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                // TODO: Update URL
                let url = format!("{}/ai-proxy/{}", base_url, "https://api.openai.com/v1/chat/completions");
                eprintln!("URL: {}", url);
                let open_ai = OpenAI {
                    model_type: self.model_type.clone(),
                };
                let model = AgentLLMInterface::OpenAI(open_ai);
                let max_tokens = AgentsCapabilitiesManager::get_max_tokens(&model);
                // Note(Nico): we can use prepare_messages directly or we could had called AgentsCapabilitiesManager
                let result = openai_prepare_messages(&model, self.model_type.clone(), prompt, max_tokens)?;
                let messages_json = match result.value {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(AgentError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                let mut payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "temperature": 0.7,
                    "max_tokens": result.remaining_tokens,
                });

                // Openai doesn't support json_object response format for vision models. wut?
                if !self.model_type.contains("vision") {
                    payload["response_format"] = json!({ "type": "json_object" });
                }

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", payload).as_str(),
                );

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

                // TODO: refactor parsing logic so it's reusable
                match data_resp {
                    Ok(value) => {
                        if self.model_type.contains("vision") {
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
                            Self::extract_first_json_object(&response_string)
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
                            Self::extract_first_json_object(&response_string)
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

    /// Returns the maximum number of tokens supported based on
    /// the provided model string
    fn get_max_tokens(s: &str) -> usize {
        // Custom added, since not supported by Tiktoken atm
        if s == "gpt-4-1106-preview" {
            128_000
        } else {
            let normalized_model = Self::normalize_model(s);
            get_context_size(normalized_model.as_str())
        }
    }

    /// Returns a maximum number of output tokens
    fn get_max_output_tokens(s: &str) -> usize {
        4096
    }

    /// Normalizes the model string to one that is supported by Tiktoken crate
    fn normalize_model(s: &str) -> String {
        if s.to_string().starts_with("gpt-4") {
            "gpt-4-32k".to_string()
        } else if s.to_string().starts_with("gpt-3.5") {
            "gpt-3.5-turbo-16k".to_string()
        } else {
            "gpt-4".to_string()
        }
    }
}
