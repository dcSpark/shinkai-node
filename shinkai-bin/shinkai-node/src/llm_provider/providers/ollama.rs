use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, LLMInferenceResponse};
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::llm_provider::providers::shared::ollama_api::{
    ollama_conversation_prepare_messages_with_tooling, ollama_prepare_messages, OllamaAPIStreamingResponse,
};
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};

use super::super::error::LLMProviderError;
use super::LLMService;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_db::schemas::ws_types::{
    ToolMetadata, ToolStatus, ToolStatusType, WSMessageType, WSMetadata, WSUpdateHandler, WidgetMetadata,
};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{LLMProviderInterface, Ollama};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::env;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub fn truncate_image_content_in_payload(payload: &mut JsonValue) {
    if let Some(messages) = payload.get_mut("messages") {
        if let Some(array) = messages.as_array_mut() {
            for message in array {
                if let Some(images) = message.get_mut("images") {
                    if let Some(image_array) = images.as_array_mut() {
                        for (_index, image) in image_array.iter_mut().enumerate() {
                            if let Some(str_image) = image.as_str() {
                                let truncated_image = format!("{}...", &str_image[0..100.min(str_image.len())]);
                                *image = JsonValue::String(truncated_image);
                            }
                        }
                    }
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

            let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);
            let messages_result = if is_stream {
                ollama_prepare_messages(&model, prompt)?
            } else {
                ollama_conversation_prepare_messages_with_tooling(&model, prompt)?
            };

            let messages_json = match messages_result.messages {
                PromptResultEnum::Value(v) => v,
                _ => {
                    return Err(LLMProviderError::UnexpectedPromptResultVariant(
                        "Expected Value variant in PromptResultEnum".to_string(),
                    ))
                }
            };

            // Extract tools_json from the result
            let tools_json = messages_result.functions.unwrap_or_else(Vec::new);

            match serde_json::to_string_pretty(&tools_json) {
                Ok(pretty_json) => eprintln!("Tools JSON: {}", pretty_json),
                Err(e) => eprintln!("Failed to serialize tools_json: {:?}", e),
            };

            let mut payload = json!({
                "model": self.model_type,
                "messages": messages_json,
                // Include any other optional parameters as needed
                // https://github.com/jmorganca/ollama/blob/main/docs/api.md
                // https://github.com/ollama/ollama/blob/main/docs/modelfile.md#valid-parameters-and-values
            });

            // Modify payload to add options if needed
            add_options_to_payload(&mut payload, config.as_ref(), &model);

            // Ollama path: if stream is true, then we the response is in Chinese for minicpm-v so if stream is true, then we need to remove to remove it
            if is_stream {
                if self.model_type.starts_with("minicpm-v") {
                    payload.as_object_mut().unwrap().remove("stream");
                }
            }

            // Conditionally add functions to the payload if tools_json is not empty
            if !tools_json.is_empty() {
                // Create a new vector to store modified tools
                let mut modified_tools = Vec::new();

                // Iterate over each tool and remove the "tool_router_key" if it exists
                for tool in &tools_json {
                    if let Some(mut tool_object) = tool.as_object().cloned() {
                        tool_object.remove("tool_router_key");
                        modified_tools.push(serde_json::Value::Object(tool_object));
                    } else {
                        modified_tools.push(tool.clone());
                    }
                }

                payload["tools"] = serde_json::Value::Array(modified_tools);
            }

            let mut payload_log = payload.clone();
            truncate_image_content_in_payload(&mut payload_log);

            match serde_json::to_string_pretty(&payload_log) {
                Ok(pretty_json) => {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Info,
                        format!("Messages JSON: {}", pretty_json).as_str(),
                    );
                    eprintln!("Messages JSON: {}", pretty_json);
                }
                Err(e) => shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    format!("Failed to serialize messages_json: {:?}", e).as_str(),
                ),
            };

            if is_stream {
                handle_streaming_response(
                    client,
                    url,
                    payload,
                    inbox_name,
                    ws_manager_trait,
                    llm_stopper,
                    session_id,
                    Some(tools_json),
                )
                .await
            } else {
                handle_non_streaming_response(
                    client,
                    url,
                    payload,
                    inbox_name,
                    ws_manager_trait,
                    llm_stopper,
                    Some(tools_json),
                )
                .await
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}

async fn handle_streaming_response(
    client: &Client,
    url: String,
    payload: JsonValue,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    llm_stopper: Arc<LLMStopper>,
    session_id: String,
    _tools: Option<Vec<JsonValue>>,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let res = client.post(url).json(&payload).send().await?;

    let mut stream = res.bytes_stream();
    let mut response_text = String::new();
    let mut previous_json_chunk: String = String::new();
    let mut final_eval_count = None;
    let mut final_eval_duration = None;

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

                // Send WS message indicating the job is done
                if let Some(ref manager) = ws_manager_trait {
                    let m = manager.lock().await;
                    let inbox_name_string = inbox_name.to_string();

                    let metadata = WSMetadata {
                        id: Some(session_id.clone()),
                        is_done: true,
                        done_reason: Some("Stopped by user request".to_string()),
                        total_duration: None,
                        eval_count: None,
                    };

                    let ws_message_type = WSMessageType::Metadata(metadata);

                    let _ = m
                        .queue_message(
                            WSTopic::Inbox,
                            inbox_name_string,
                            response_text.clone(),
                            ws_message_type,
                            true,
                        )
                        .await;
                }

                return Ok(LLMInferenceResponse::new(response_text, json!({}), None, None));
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

                        // Capture eval_count and eval_duration from the final response
                        if data.done {
                            final_eval_count = data.eval_count;
                            final_eval_duration = data.eval_duration;
                        }

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
                        eprintln!("Error while receiving chunk: {:?}", _e);
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Error while receiving chunk: {:?}", _e).as_str(),
                        );
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

    // Calculate tps
    let tps = if let (Some(eval_count), Some(eval_duration)) = (final_eval_count, final_eval_duration) {
        if eval_duration > 0 {
            Some(eval_count as f64 / eval_duration as f64 * 1e9)
        } else {
            None
        }
    } else {
        None
    };

    Ok(LLMInferenceResponse::new(response_text, json!({}), None, tps))
}

async fn handle_non_streaming_response(
    client: &Client,
    url: String,
    payload: JsonValue,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    llm_stopper: Arc<LLMStopper>,
    tools: Option<Vec<JsonValue>>,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    let response_fut = client.post(url).json(&payload).send();
    let mut response_fut = Box::pin(response_fut);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Some(ref inbox_name) = inbox_name {
                    if llm_stopper.should_stop(&inbox_name.to_string()) {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Info,
                            "LLM job stopped by user request",
                        );
                        llm_stopper.reset(&inbox_name.to_string());

                        return Ok(LLMInferenceResponse::new("".to_string(), json!({}), None, None));
                    }
                }
            },
            response = &mut response_fut => {
                let res = response?;
                let response_body = res.text().await?;
                let response_json: serde_json::Value = serde_json::from_str(&response_body)?;

                if let Some(message) = response_json.get("message") {
                    if let Some(content) = message.get("content") {
                        if let Some(content_str) = content.as_str() {
                            let function_call = message
                                .get("tool_calls")
                                .and_then(|tool_calls| {
                                    tool_calls.as_array().and_then(|calls| {
                                        calls.iter().find_map(|call| {
                                            call.get("function").map(|function| {
                                                let name = function.get("name")?.as_str()?.to_string();
                                                let arguments = function.get("arguments")
                                                    .and_then(|args_value| args_value.as_object().cloned())
                                                    .unwrap_or_else(|| serde_json::Map::new());

                                                // Search for the tool_router_key in the tools array
                                                let tool_router_key = tools.as_ref().and_then(|tools_array| {
                                                    tools_array.iter().find_map(|tool| {
                                                        if tool.get("name")?.as_str()? == name {
                                                            tool.get("tool_router_key").and_then(|key| key.as_str().map(|s| s.to_string()))
                                                        } else {
                                                            None
                                                        }
                                                    })
                                                });

                                                Some(FunctionCall { name, arguments, tool_router_key })
                                            })
                                        })
                                    })
                                })
                                .flatten();

                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Info,
                                format!("Function Call: {:?}", function_call).as_str(),
                            );


                            // Send WS message if a function call is detected
                            if let Some(ref manager) = ws_manager_trait {
                                if let Some(ref inbox_name) = inbox_name {
                                    if let Some(ref function_call) = function_call {
                                        let m = manager.lock().await;
                                        let inbox_name_string = inbox_name.to_string();

                                        // Serialize FunctionCall to JSON value
                                        let function_call_json = serde_json::to_value(function_call)
                                            .unwrap_or_else(|_| serde_json::json!({}));

                                        // Prepare ToolMetadata
                                        let tool_metadata = ToolMetadata {
                                            tool_name: function_call.name.clone(),
                                            tool_router_key: None,
                                            args: function_call_json
                                                .as_object()
                                                .cloned()
                                                .unwrap_or_default(),
                                            result: None,
                                            status: ToolStatus {
                                                type_: ToolStatusType::Running,
                                                reason: None,
                                            },
                                        };

                                        let ws_message_type = WSMessageType::Widget(WidgetMetadata::ToolRequest(tool_metadata));

                                        let _ = m
                                            .queue_message(
                                                WSTopic::Inbox,
                                                inbox_name_string,
                                                serde_json::to_string(&function_call)
                                                    .unwrap_or_else(|_| "{}".to_string()),
                                                ws_message_type,
                                                true,
                                            )
                                            .await;
                                    }
                                }
                            }

                            // Calculate tps
                            let eval_count = response_json.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
                            let eval_duration =
                                response_json.get("eval_duration").and_then(|v| v.as_u64()).unwrap_or(1); // Avoid division by zero
                            let tps = if eval_duration > 0 {
                                Some(eval_count as f64 / eval_duration as f64 * 1e9)
                            } else {
                                None
                            };

                            break Ok(LLMInferenceResponse::new(
                                content_str.to_string(),
                                json!({}),
                                function_call,
                                tps,
                            ));
                        } else {
                            break Err(LLMProviderError::UnexpectedResponseFormat(
                                "Content is not a string".to_string(),
                            ));
                        }
                    } else {
                        break Err(LLMProviderError::UnexpectedResponseFormat(
                            "No content field in message".to_string(),
                        ));
                    }
                } else {
                    break Err(LLMProviderError::UnexpectedResponseFormat(
                        "No message field in response".to_string(),
                    ));
                }
            }
        }
    }
}

fn add_options_to_payload(payload: &mut serde_json::Value, config: Option<&JobConfig>, model: &LLMProviderInterface) {
    eprintln!("config: {:?}", config);
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
    if let Some(max_tokens) = get_value("LLM_MAX_TOKENS", config.and_then(|c| c.max_tokens.as_ref())) {
        options.insert("num_predict".to_string(), serde_json::json!(max_tokens));
    }

    // Handle streaming option
    let streaming = get_value("LLM_STREAMING", config.and_then(|c| c.stream.as_ref())).unwrap_or(true); // Default to true if not specified
    payload["stream"] = serde_json::json!(streaming);

    // Handle num_ctx setting
    let num_ctx_from_config = config
        .and_then(|c| c.other_model_params.as_ref())
        .and_then(|params| params.get("num_ctx"));

    if num_ctx_from_config.is_none() {
        // If num_ctx is not defined in config, set it using get_max_tokens
        let max_tokens = ModelCapabilitiesManager::get_max_tokens(model);
        eprintln!("updating num_ctx to: {:?}", max_tokens);
        options.insert("num_ctx".to_string(), serde_json::json!(max_tokens));
    }

    // Handle other model params
    if let Some(other_params) = config.and_then(|c| c.other_model_params.as_ref()) {
        if let Some(obj) = other_params.as_object() {
            for (key, value) in obj {
                match key.as_str() {
                    "num_ctx" => options.insert("num_ctx".to_string(), value.clone()),
                    "num_predict" | "max_tokens" => options.insert("num_predict".to_string(), value.clone()),
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
