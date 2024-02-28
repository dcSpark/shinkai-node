use crate::agent::providers::shared::ollama::OllamaAPIStreamingResponse;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};

use super::super::{error::AgentError, execution::job_prompts::Prompt};
use super::LLMProvider;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, Ollama};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::error::Error;

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
                "stream": true, // Yeah let's go wild and stream the response
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

            let res = client.post(url).json(&payload).send().await?;

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Call API Status: {:?}", res.status()).as_str(),
            );

            let mut stream = res.bytes_stream();
            let mut response_text = String::new();

            while let Some(item) = stream.next().await {
                match item {
                    Ok(chunk) => {
                        let chunk_str = String::from_utf8_lossy(&chunk);
                        let chunk_str = chunk_str.chars().filter(|c| !c.is_control()).collect::<String>();
                        let data_resp: Result<OllamaAPIStreamingResponse, _> = serde_json::from_str(&chunk_str);
                        match data_resp {
                            Ok(data) => {
                                if let Some(response) = data.response.as_str() {
                                    response_text.push_str(response);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to parse line: {:?}", e);
                                // Handle JSON parsing error here...
                            }
                        }
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Error while receiving chunk: {:?}, Error Source: {:?}", e, e.source())
                                .as_str(),
                        );
                        return Err(AgentError::NetworkError(e.to_string()));
                    }
                }
            }

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                format!("Call API Response Text: {:?}", response_text).as_str(),
            );

            match serde_json::from_str::<JsonValue>(&response_text) {
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
        } else {
            Err(AgentError::UrlNotSet)
        }
    }
}
