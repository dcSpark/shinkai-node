use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::job::JobConfig;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::llm_provider::providers::shared::ollama::{
    ollama_conversation_prepare_messages, OllamaAPIStreamingResponse,
};
use crate::managers::model_capabilities_manager::PromptResultEnum;
use crate::network::ws_manager::{WSMessageType, WSMetadata, WSUpdateHandler};

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
use std::env;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub fn truncate_image_content_in_payload(payload: &mut JsonValue) {
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
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
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

            let mut payload = json!({
                "model": self.model_type,
                "messages": messages_json,
                "stream": true,
                // Include any other optional parameters as needed
                // https://github.com/jmorganca/ollama/blob/main/docs/api.md
                // https://github.com/ollama/ollama/blob/main/docs/modelfile.md#valid-parameters-and-values
            });

            // Modify payload to add options if needed
            add_options_to_payload(&mut payload, config.as_ref());

            let mut payload_log = payload.clone();
            truncate_image_content_in_payload(&mut payload_log);

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Call API Body: {:?}", payload_log).as_str(),
            );
            eprintln!("Call API Body: {:?}", payload_log);

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
                // Check if we need to stop the LLM job
                if let Some(ref inbox_name) = inbox_name {
                    if llm_stopper.should_stop(&inbox_name.to_string()) {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Info,
                            "LLM job stopped by user request",
                        );
                        llm_stopper.reset(&inbox_name.to_string());
                        return Ok(LLMInferenceResponse::new(response_text, json!({}), None));
                    }
                }

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

                                        let metadata = WSMetadata {
                                            id: Some(session_id.clone()),
                                            is_done: data.done,
                                            done_reason: if data.done { data.done_reason.clone() } else { None },
                                            total_duration: if data.done {
                                                data.total_duration.map(|d| d as u64)
                                            } else {
                                                None
                                            },
                                            eval_count: if data.done {
                                                data.eval_count.map(|c| c as u64)
                                            } else {
                                                None
                                            },
                                        };

                                        let ws_message_type = WSMessageType::Metadata(metadata);

                                        let _ = m
                                            .queue_message(
                                                WSTopic::Inbox,
                                                inbox_name_string,
                                                data.message.content,
                                                ws_message_type,
                                                true,
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

fn add_options_to_payload(payload: &mut serde_json::Value, config: Option<&JobConfig>) {
    let mut options = serde_json::Map::new();

    // Helper function to read and parse environment variables
    fn read_env_var<T: std::str::FromStr>(key: &str) -> Option<T> {
        env::var(key).ok().and_then(|val| val.parse::<T>().ok())
    }

    // Helper function to get value from env or config
    fn get_value<T: Clone + std::str::FromStr>(env_key: &str, config_value: Option<&T>) -> Option<T> {
        config_value.cloned().or_else(|| read_env_var::<T>(env_key))
    }

    // Read options from environment variables or config
    if let Some(seed) = get_value("LLM_SEED", config.and_then(|c| c.seed.as_ref())) {
        options.insert("seed".to_string(), serde_json::json!(seed));
    }
    if let Some(temp) = get_value("LLM_TEMPERATURE", config.and_then(|c| c.temperature.as_ref())) {
        options.insert("temperature".to_string(), serde_json::json!(temp));
    }
    if let Some(top_k) = get_value("LLM_TOP_K", config.and_then(|c| c.top_k.as_ref())) {
        options.insert("top_k".to_string(), serde_json::json!(top_k));
    }
    if let Some(top_p) = get_value("LLM_TOP_P", config.and_then(|c| c.top_p.as_ref())) {
        options.insert("top_p".to_string(), serde_json::json!(top_p));
    }

    // Handle streaming option
    let streaming = get_value("LLM_STREAMING", config.and_then(|c| c.stream.as_ref())).unwrap_or(true); // Default to true if not specified
    payload["stream"] = serde_json::json!(streaming);

    // Handle other model params
    if let Some(other_params) = config.and_then(|c| c.other_model_params.as_ref()) {
        if let Some(obj) = other_params.as_object() {
            for (key, value) in obj {
                match key.as_str() {
                    "num_ctx" => options.insert("num_ctx".to_string(), value.clone()),
                    "num_predict" => options.insert("num_predict".to_string(), value.clone()),
                    "num_keep" => options.insert("num_keep".to_string(), value.clone()),
                    "repeat_last_n" => options.insert("repeat_last_n".to_string(), value.clone()),
                    "repeat_penalty" => options.insert("repeat_penalty".to_string(), value.clone()),
                    "presence_penalty" => options.insert("presence_penalty".to_string(), value.clone()),
                    "frequency_penalty" => options.insert("frequency_penalty".to_string(), value.clone()),
                    "tfs_z" => options.insert("tfs_z".to_string(), value.clone()),
                    "typical_p" => options.insert("typical_p".to_string(), value.clone()),
                    "mirostat" => options.insert("mirostat".to_string(), value.clone()),
                    "mirostat_tau" => options.insert("mirostat_tau".to_string(), value.clone()),
                    "mirostat_eta" => options.insert("mirostat_eta".to_string(), value.clone()),
                    "penalize_newline" => options.insert("penalize_newline".to_string(), value.clone()),
                    "stop" => options.insert("stop".to_string(), value.clone()),
                    "numa" => options.insert("numa".to_string(), value.clone()),
                    "num_batch" => options.insert("num_batch".to_string(), value.clone()),
                    "num_gpu" => options.insert("num_gpu".to_string(), value.clone()),
                    "main_gpu" => options.insert("main_gpu".to_string(), value.clone()),
                    "low_vram" => options.insert("low_vram".to_string(), value.clone()),
                    "f16_kv" => options.insert("f16_kv".to_string(), value.clone()),
                    "vocab_only" => options.insert("vocab_only".to_string(), value.clone()),
                    "use_mmap" => options.insert("use_mmap".to_string(), value.clone()),
                    "use_mlock" => options.insert("use_mlock".to_string(), value.clone()),
                    "rope_frequency_base" => options.insert("rope_frequency_base".to_string(), value.clone()),
                    "rope_frequency_scale" => options.insert("rope_frequency_scale".to_string(), value.clone()),
                    "num_thread" => options.insert("num_thread".to_string(), value.clone()),
                    _ => None,
                };
            }
        }
    }

    // Add options to payload if not empty
    if !options.is_empty() {
        payload["options"] = serde_json::Value::Object(options);
    }
}
