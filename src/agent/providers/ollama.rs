use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};

use super::super::{error::AgentError, execution::job_prompts::Prompt};
use super::shared::ollama::OllamaAPIResponse;
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, Ollama};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};

fn truncate_image_content_in_payload(payload: &mut JsonValue) {
    if let Some(images) = payload.get_mut("images") {
        if let Some(array) = images.as_array_mut() {
            for image in array {
                if let Some(str_image) = image.as_str() {
                    let truncated_image = format!("{}...", &str_image[0..20.min(str_image.len())]);
                    *image = JsonValue::String(truncated_image);
                }
            }
        }
    }
}

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
            if let Some(_) = api_key {
                let url = format!("{}{}", base_url, "/api/generate");
                let ollama = Ollama {
                    model_type: self.model_type.clone(),
                };
                let model = AgentLLMInterface::Ollama(ollama);
                let messages_result = ModelCapabilitiesManager::route_prompt_with_model(prompt, &model).await?;
                let (messages_string, asset_content) = match messages_result.value {
                    PromptResultEnum::Text(v) => (v, None),
                    PromptResultEnum::ImageAnalysis(v, i) => (v, Some(i)),
                    _ => {
                        return Err(AgentError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Info,
                    format!("Messages JSON: {:?}", messages_string).as_str(),
                );

                let mut payload = json!({
                    "model": self.model_type,
                    "prompt": messages_string,
                    "format": "json",
                    "stream": false,
                    // Include any other optional parameters as needed
                    // https://github.com/jmorganca/ollama/blob/main/docs/api.md#request-json-mode
                });

                if let Some(asset_content) = asset_content {
                    let asset_content_str = asset_content.to_string();
                    payload["images"] = json!([asset_content_str]);
                }

                let mut payload_log = payload.clone();
                truncate_image_content_in_payload(&mut payload_log);

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", payload_log).as_str(),
                );

                let res = client
                    .post(url)
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
                        let response_string = data.response.as_str().unwrap_or("");
                        match serde_json::from_str::<JsonValue>(&response_string) {
                            Ok(deserialized_json) => {
                                let response_string = deserialized_json.to_string();
                                Self::extract_first_json_object(&response_string)
                            }
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
