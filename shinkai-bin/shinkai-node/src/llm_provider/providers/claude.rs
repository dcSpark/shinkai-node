use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_db::schemas::ws_types::ToolMetadata;
use shinkai_db::schemas::ws_types::ToolStatus;
use shinkai_db::schemas::ws_types::ToolStatusType;
use shinkai_db::schemas::ws_types::WSMessageType;
use shinkai_db::schemas::ws_types::WSMetadata;
use shinkai_db::schemas::ws_types::WSUpdateHandler;
use shinkai_db::schemas::ws_types::WidgetMetadata;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName,
        job_config::JobConfig,
        llm_providers::serialized_llm_provider::{Claude, LLMProviderInterface},
        prompts::Prompt,
    },
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::llm_provider::execution::chains::inference_chain_trait::FunctionCall;
use crate::llm_provider::{
    error::LLMProviderError, execution::chains::inference_chain_trait::LLMInferenceResponse, llm_stopper::LLMStopper,
};
use crate::managers::model_capabilities_manager::PromptResultEnum;

use super::openai::truncate_image_url_in_payload;
use super::shared::claude_api::claude_prepare_messages;
use super::LLMService;

#[async_trait]
impl LLMService for Claude {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
        model: LLMProviderInterface,
        inbox_name: Option<InboxName>,
        ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        config: Option<JobConfig>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        let session_id = Uuid::new_v4().to_string();
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let base_url = if base_url.ends_with('/') {
                    base_url.to_string()
                } else {
                    format!("{}/", base_url)
                };

                let url = format!("{}{}", base_url, "v1/messages");

                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);

                let messages_result = claude_prepare_messages(&model, prompt)?;
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
                    "max_tokens": messages_result.remaining_tokens,
                    "stream": is_stream,
                });

                // Conditionally add functions to the payload if tools_json is not empty
                if !tools_json.is_empty() {
                    payload["tools"] = serde_json::Value::Array(tools_json.clone());
                }

                // Add options to payload
                add_options_to_payload(&mut payload, config.as_ref());

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

                if is_stream {
                    handle_streaming_response(
                        client,
                        url,
                        payload,
                        key.clone(),
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
                        key.to_string(),
                        payload,
                        inbox_name,
                        ws_manager_trait,
                        llm_stopper,
                        Some(tools_json),
                    )
                    .await
                }
            } else {
                return Err(LLMProviderError::ApiKeyNotSet);
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
    api_key: String,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    llm_stopper: Arc<LLMStopper>,
    session_id: String,
    tools: Option<Vec<JsonValue>>,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let res = client
        .post(url)
        .header("anthropic-version", "2023-06-01")
        .header("x-api-key", api_key)
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await?;

    let mut stream = res.bytes_stream();
    let mut response_text = String::new();
    let mut previous_json_chunk: String = String::new();
    let mut function_call: Option<FunctionCall> = None;

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
                let chunk_str = String::from_utf8_lossy(&chunk).to_string();
                eprintln!("Chunk: {}", chunk_str);
                previous_json_chunk += chunk_str.as_str();
                let trimmed_chunk_str = previous_json_chunk.trim().to_string();
                let data_resp: Result<JsonValue, _> = serde_json::from_str(&trimmed_chunk_str);
                match data_resp {
                    Ok(data) => {
                        serde_json::to_string_pretty(&data)
                            .map(|pretty_json| eprintln!("Response JSON: {}", pretty_json))
                            .unwrap_or_else(|e| eprintln!("Failed to serialize response_json: {:?}", e));

                        previous_json_chunk = "".to_string();
                        if let Some(choices) = data.get("choices") {
                            for choice in choices.as_array().unwrap_or(&vec![]) {
                                if let Some(message) = choice.get("message") {
                                    if let Some(content) = message.get("content") {
                                        response_text.push_str(content.as_str().unwrap_or(""));
                                    }
                                    if let Some(fc) = message.get("function_call") {
                                        if let Some(name) = fc.get("name") {
                                            let fc_arguments = fc
                                                .get("arguments")
                                                .and_then(|args| args.as_str())
                                                .and_then(|args_str| serde_json::from_str(args_str).ok())
                                                .and_then(|args_value: serde_json::Value| {
                                                    args_value.as_object().cloned()
                                                })
                                                .unwrap_or_else(|| serde_json::Map::new());

                                            // Extract tool_router_key
                                            let tool_router_key = tools.as_ref().and_then(|tools_array| {
                                                tools_array.iter().find_map(|tool| {
                                                    if tool.get("name")?.as_str()? == name.as_str().unwrap_or("") {
                                                        tool.get("tool_router_key")
                                                            .and_then(|key| key.as_str().map(|s| s.to_string()))
                                                    } else {
                                                        None
                                                    }
                                                })
                                            });

                                            function_call = Some(FunctionCall {
                                                name: name.as_str().unwrap_or("").to_string(),
                                                arguments: fc_arguments.clone(),
                                                tool_router_key,
                                            });
                                        }
                                    }
                                }
                            }
                        }

                        // Updated WS message handling for tooling
                        if let Some(ref manager) = ws_manager_trait {
                            if let Some(ref inbox_name) = inbox_name {
                                if let Some(ref function_call) = function_call {
                                    let m = manager.lock().await;
                                    let inbox_name_string = inbox_name.to_string();

                                    // Serialize FunctionCall to JSON value
                                    let function_call_json =
                                        serde_json::to_value(function_call).unwrap_or_else(|_| serde_json::json!({}));

                                    // Prepare ToolMetadata
                                    let tool_metadata = ToolMetadata {
                                        tool_name: function_call.name.clone(),
                                        tool_router_key: function_call.tool_router_key.clone(),
                                        args: function_call_json.as_object().cloned().unwrap_or_default(),
                                        result: None,
                                        status: ToolStatus {
                                            type_: ToolStatusType::Running,
                                            reason: None,
                                        },
                                    };

                                    let ws_message_type =
                                        WSMessageType::Widget(WidgetMetadata::ToolRequest(tool_metadata));

                                    let _ = m
                                        .queue_message(
                                            WSTopic::Inbox,
                                            inbox_name_string,
                                            serde_json::to_string(&function_call).unwrap_or_else(|_| "{}".to_string()),
                                            ws_message_type,
                                            true,
                                        )
                                        .await;
                                }
                            }
                        }
                    }
                    Err(_e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Error while receiving chunk: {:?}", _e).as_str(),
                        );
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

    Ok(LLMInferenceResponse::new(response_text, json!({}), function_call, None))
}

async fn handle_non_streaming_response(
    client: &Client,
    url: String,
    api_key: String,
    payload: JsonValue,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    llm_stopper: Arc<LLMStopper>,
    tools: Option<Vec<JsonValue>>,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    let response_fut = client
        .post(url)
        .header("anthropic-version", "2023-06-01")
        .header("x-api-key", api_key)
        .header("content-type", "application/json")
        .json(&payload)
        .send();
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

                serde_json::to_string_pretty(&response_json)
                    .map(|pretty_json| eprintln!("Response JSON: {}", pretty_json))
                    .unwrap_or_else(|e| eprintln!("Failed to serialize response_json: {:?}", e));

                if let Some(content) = response_json.get("content") {
                    let content_str = content.as_array().and_then(|content_array| {
                        content_array.iter().find_map(|item| {
                            if let Some(text) = item.get("text") {
                                text.as_str()
                            } else {
                                None
                            }
                        })
                    });
                    if let Some(content_str) = content_str {
                        let function_call = response_json
                            .get("tool_use")
                            .and_then(|tool_use| {
                                tool_use.as_array().and_then(|calls| {
                                    calls.iter().find_map(|call| {
                                            let name = call.get("name")?.as_str()?.to_string();
                                            let arguments = call.get("input")
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
                            });

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
            }
        }
    }
}

fn add_options_to_payload(payload: &mut serde_json::Value, config: Option<&JobConfig>) {
    // Helper function to read and parse environment variables
    fn read_env_var<T: std::str::FromStr>(key: &str) -> Option<T> {
        std::env::var(key).ok().and_then(|val| val.parse::<T>().ok())
    }

    // Helper function to get value from env or config
    fn get_value<T: Clone + std::str::FromStr>(env_key: &str, config_value: Option<&T>) -> Option<T> {
        config_value.cloned().or_else(|| read_env_var::<T>(env_key))
    }

    // Read options from environment variables or config and add them directly to the payload
    if let Some(seed) = get_value("LLM_SEED", config.and_then(|c| c.seed.as_ref())) {
        payload["seed"] = serde_json::json!(seed);
    }
    if let Some(temp) = get_value("LLM_TEMPERATURE", config.and_then(|c| c.temperature.as_ref())) {
        payload["temperature"] = serde_json::json!(temp);
    }
    if let Some(top_p) = get_value("LLM_TOP_P", config.and_then(|c| c.top_p.as_ref())) {
        payload["top_p"] = serde_json::json!(top_p);
    }
    if let Some(max_tokens) = get_value("LLM_MAX_TOKENS", config.and_then(|c| c.max_tokens.as_ref())) {
        payload["max_completion_tokens"] = serde_json::json!(max_tokens);
    }

    // Handle other model params
    if let Some(other_params) = config.and_then(|c| c.other_model_params.as_ref()) {
        if let Some(obj) = other_params.as_object() {
            for (key, value) in obj {
                match key.as_str() {
                    "frequency_penalty" => payload["frequency_penalty"] = value.clone(),
                    "logit_bias" => payload["logit_bias"] = value.clone(),
                    "logprobs" => payload["logprobs"] = value.clone(),
                    "top_logprobs" => payload["top_logprobs"] = value.clone(),
                    "max_completion_tokens" => payload["max_completion_tokens"] = value.clone(),
                    "n" => payload["n"] = value.clone(),
                    "presence_penalty" => payload["presence_penalty"] = value.clone(),
                    "response_format" => payload["response_format"] = value.clone(),
                    "service_tier" => payload["service_tier"] = value.clone(),
                    "stop" => payload["stop"] = value.clone(),
                    "stream_options" => payload["stream_options"] = value.clone(),
                    "parallel_tool_calls" => payload["parallel_tool_calls"] = value.clone(),
                    _ => (),
                };
            }
        }
    }
}
