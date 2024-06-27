use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::providers::shared::ollama::{
    ollama_conversation_prepare_messages, OllamaAPIStreamingResponse,
};
use crate::managers::model_capabilities_manager::PromptResultEnum;
use crate::network::ws_manager::{WSMetadata, WSUpdateHandler};

use super::super::{error::LLMProviderError, execution::prompts::prompts::Prompt};
use super::LLMService;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{LLMProviderInterface, Ollama};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use uuid::Uuid;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

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
impl LLMService for Ollama {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        _api_key: Option<&String>, // Note: not required
        prompt: Prompt,
        model: LLMProviderInterface,
        inbox_name: Option<InboxName>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        let session_id = Uuid::new_v4().to_string();
        if let Some(base_url) = url {
            let url = format!("{}{}", base_url, "/api/chat");

            let messages_result = ollama_conversation_prepare_messages(&model, prompt)?;
            let messages_json = match messages_result.messages {
                PromptResultEnum::Value(v) => v,
                _ => {
                    return Err(LLMProviderError::UnexpectedPromptResultVariant(
                        "Expected Value variant in PromptResultEnum".to_string(),
                    ))
                }
            };

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                format!("Messages JSON: {:?}", messages_json).as_str(),
            );
            // Print messages_json as a pretty JSON string
            // match serde_json::to_string_pretty(&messages_json) {
            //     Ok(pretty_json) => eprintln!("Messages JSON: {}", pretty_json),
            //     Err(e) => eprintln!("Failed to serialize messages_json: {:?}", e),
            // };

            let payload = json!({
                "model": self.model_type,
                "messages": messages_json,
                "stream": true, // Yeah let's go wild and stream the response
                // Include any other optional parameters as needed
                // https://github.com/jmorganca/ollama/blob/main/docs/api.md#request-json-mode
            });

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

                                // Note: this is the code for enabling WS
                                if let Some(ref manager) = ws_manager_trait {
                                    if let Some(ref inbox_name) = inbox_name {
                                        let m = manager.lock().await;
                                        let inbox_name_string = inbox_name.to_string();

                                        let metadata = if data.done {
                                            Some(WSMetadata {
                                                id: Some(session_id.clone()),
                                                is_done: data.done,
                                                done_reason: data.done_reason.clone(),
                                                total_duration: data.total_duration.map(|d| d as u64),
                                                eval_count: data.eval_count.map(|c| c as u64),
                                            })
                                        } else {
                                            None
                                        };

                                        let _ = m
                                            .queue_message(
                                                WSTopic::Inbox,
                                                inbox_name_string,
                                                data.message.content,
                                                metadata,
                                            )
                                            .await;
                                    }
                                }
                            }
                            Err(_e) => {
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
                        return Err(LLMProviderError::NetworkError(e.to_string()));
                    }
                }
            }

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Cleaned Response Text: {:?}", response_text).as_str(),
            );

            // Directly return response_text with an empty JSON object
            Ok(LLMInferenceResponse::new(response_text, json!({}), None))
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}
