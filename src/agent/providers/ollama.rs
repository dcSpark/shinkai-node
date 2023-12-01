use crate::managers::agents_capabilities_manager::{AgentsCapabilitiesManager, PromptResult, PromptResultEnum};

use super::super::{error::AgentError, execution::job_prompts::Prompt};
use super::shared::ollama::OllamaAPIResponse;
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, GenericAPI, Ollama};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tiktoken_rs::get_chat_completion_max_tokens;
use tiktoken_rs::num_tokens_from_messages;

#[async_trait]
impl LLMProvider for Ollama {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>, // Note: not required
        prompt: Prompt,
    ) -> Result<JsonValue, AgentError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let url = format!("{}{}", base_url, "/api/generate");
                let ollama = Ollama {
                    model_type: self.model_type.clone(),
                };
                let model = AgentLLMInterface::Ollama(ollama);
                let messages_result = AgentsCapabilitiesManager::route_prompt_with_model(prompt, &model).await?;
                let messages_string = match messages_result.value {
                    PromptResultEnum::Text(v) => v,
                    _ => return Err(AgentError::UnexpectedPromptResultVariant("Expected Value variant in PromptResultEnum".to_string())),
                };

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Info,
                    format!("Messages JSON: {:?}", messages_string).as_str(),
                );

                let payload = json!({
                    "model": self.model_type,
                    "prompt": messages_string,
                    "format": "json",
                    "stream": false,
                    // Include any other optional parameters as needed
                    // https://github.com/jmorganca/ollama/blob/main/docs/api.md#request-json-mode
                });

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", payload).as_str(),
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
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Info,
                    format!("Call API Response Text: {:?}", response_text).as_str(),
                );

                let data_resp: Result<OllamaAPIResponse, _> = serde_json::from_str(&response_text);

                match data_resp {
                    Ok(data) => {
                        let response_string = data.response.to_string();
                        match serde_json::from_str::<JsonValue>(&response_string) {
                            Ok(deserialized_json) => {
                                let response_string = deserialized_json.to_string();
                                eprintln!("response_string: {:?}", response_string);
                                Self::extract_first_json_object(&response_string)
                            },
                            Err(e) => {
                                shinkai_log(
                                    ShinkaiLogOption::JobExecution,
                                    ShinkaiLogLevel::Error,
                                    format!("Failed to deserialize response: {:?}", e).as_str(),
                                );
                                Err(AgentError::SerdeError(e))
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

    fn normalize_model(s: &str) -> String {
        s.to_string()
    }

    fn get_max_tokens(s: &str) -> usize {
        if s.to_string().starts_with("Open-Orca/Mistral-7B-OpenOrca") {
            8000
        } else {
            4096
        }
    }

    fn get_max_output_tokens(s: &str) -> usize {
        Self::get_max_tokens(s)
    }
}
