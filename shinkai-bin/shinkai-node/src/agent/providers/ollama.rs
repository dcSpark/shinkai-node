use crate::agent::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::agent::providers::shared::ollama::OllamaAPIStreamingResponse;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};

use super::super::{error::AgentError, execution::prompts::prompts::Prompt};
use super::shared::openai::openai_prepare_messages;
use super::shared::shared_model_logic::parse_markdown_to_json;
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
        _api_key: Option<&String>, // Note: not required
        prompt: Prompt,
        model: AgentLLMInterface,
    ) -> Result<LLMInferenceResponse, AgentError> {
        if let Some(base_url) = url {
            let url = format!("{}{}", base_url, "/api/chat");

            let messages_result = openai_prepare_messages(&model, prompt)?;
            println!("Messages Result: {:?}", messages_result);
            let messages_json = match messages_result.value {
                PromptResultEnum::Value(v) => {
                    // Transform the messages to the expected JSON format
                    let transformed_messages: Vec<JsonValue> = v.as_array().unwrap().iter().map(|message| {
                        let role = message.get("role").unwrap().as_str().unwrap();
                        let content = message.get("content").unwrap().as_array().unwrap().iter()
                            .map(|c| c.get("text").unwrap().as_str().unwrap())
                            .collect::<Vec<&str>>()
                            .join(" ");
                        json!({
                            "role": role,
                            "content": content
                        })
                    }).collect();
                    JsonValue::Array(transformed_messages)
                },
                _ => {
                    return Err(AgentError::UnexpectedPromptResultVariant(
                        "Expected Value variant in PromptResultEnum".to_string(),
                    ))
                }
            };
            // let messages_result = ModelCapabilitiesManager::route_prompt_with_model(prompt, &model).await?;
            // let (messages_string, asset_content) = match messages_result.value {
            //     PromptResultEnum::Text(v) => (v, None),
            //     PromptResultEnum::ImageAnalysis(v, i) => (v, Some(i)),
            //     _ => {
            //         return Err(AgentError::UnexpectedPromptResultVariant(
            //             "Expected Value variant in PromptResultEnum".to_string(),
            //         ))
            //     }
            // };

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                format!("Messages JSON: {:?}", messages_json).as_str(),
            );

            let mut payload = json!({
                "model": self.model_type,
                "messages": messages_json,
                "stream": true, // Yeah let's go wild and stream the response
                // Include any other optional parameters as needed
                // https://github.com/jmorganca/ollama/blob/main/docs/api.md#request-json-mode
            });

            // if let Some(asset_content) = asset_content {
            //     let asset_content_str = asset_content.to_string();
            //     payload["images"] = json!([asset_content_str]);
            // }

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
            let mut previous_json_chunk: String = String::new();
            while let Some(item) = stream.next().await {
                match item {
                    Ok(chunk) => {
                        let mut chunk_str = String::from_utf8_lossy(&chunk).to_string();
                        if !previous_json_chunk.is_empty() {
                            chunk_str = previous_json_chunk.clone() + chunk_str.as_str();
                        }
                        let data_resp: Result<OllamaAPIStreamingResponse, _> = serde_json::from_str(&chunk_str);
                        match data_resp {
                            Ok(data) => {
                                previous_json_chunk = "".to_string();
                                response_text.push_str(&data.message.content);
                            }
                            Err(e) => {
                                previous_json_chunk += chunk_str.as_str();
                                // Handle JSON parsing error here...
                            }
                        }
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Error while receiving chunk: {:?}, Error Source: {:?}", e, e.source()).as_str(),
                        );
                        return Err(AgentError::NetworkError(e.to_string()));
                    }
                }
            }

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Cleaned Response Text: {:?}", response_text).as_str(),
            );
            eprintln!("Cleaned Response Text: {:?}", response_text);

            match parse_markdown_to_json(&response_text) {
                Ok(json) => {
                    eprintln!("Parsed JSON from Markdown: {:?}", json);
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Debug,
                        format!("Parsed JSON from Markdown: {:?}", json).as_str(),
                    );
                    Ok(LLMInferenceResponse::new(response_text, json))
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
            Err(AgentError::UrlNotSet)
        }
    }
}
