use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::ws_types::ToolMetadata;
use shinkai_message_primitives::schemas::ws_types::ToolStatus;
use shinkai_message_primitives::schemas::ws_types::ToolStatusType;
use shinkai_message_primitives::schemas::ws_types::WSMessageType;
use shinkai_message_primitives::schemas::ws_types::WSMetadata;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::schemas::ws_types::WidgetMetadata;
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

                let (messages_result, system_messages) = claude_prepare_messages(&model, prompt)?;
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
                let tools_json = tools_json
                    .into_iter()
                    .map(|mut tool| {
                        if let Some(input_schema) = tool.get_mut("parameters") {
                            tool["input_schema"] = input_schema.clone();
                            tool.as_object_mut().unwrap().remove("parameters");
                        }
                        tool
                    })
                    .collect::<Vec<JsonValue>>();

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
                    "max_tokens": messages_result.remaining_output_tokens,
                    "stream": is_stream,
                    "system": system_messages.into_iter().map(|m| m.content.unwrap_or_default()).collect::<Vec<String>>().join(""),
                });

                // Conditionally add functions to the payload if tools_json is not empty
                if !tools_json.is_empty() {
                    let tools_payload = tools_json
                        .clone()
                        .into_iter()
                        .map(|mut tool| {
                            tool.as_object_mut().unwrap().remove("tool_router_key");
                            tool
                        })
                        .collect::<Vec<JsonValue>>();

                    payload["tools"] = serde_json::Value::Array(tools_payload);
                }

                // Add options to payload
                add_options_to_payload(&mut payload, config.as_ref());

                // Print payload as a pretty JSON string
                match serde_json::to_string_pretty(&payload) {
                    Ok(pretty_json) => eprintln!("cURL Payload: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
                };

                let mut payload_log = payload.clone();
                truncate_image_url_in_payload(&mut payload_log);
                eprintln!("Call API Body: {:?}", payload_log);

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
    let mut processed_tool: Option<ProcessedTool> = None;
    let mut function_calls = Vec::new();
    let mut buffer = String::new();

    while let Some(item) = stream.next().await {
        // Check if we need to stop the LLM job
        if let Some(ref inbox_name) = inbox_name {
            if llm_stopper.should_stop(&inbox_name.to_string()) {
                eprintln!("LLM job stopped by user request");
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

                return Ok(LLMInferenceResponse::new(response_text, json!({}), Vec::new(), None));
            }
        }

        match item {
            Ok(chunk) => {
                // Append new chunk to buffer
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                
                // Try to process the accumulated buffer
                let process_result = process_chunk(buffer.as_bytes());
                match process_result {
                    Ok(processed_chunk) => {
                        // Successfully processed, clear buffer
                        buffer.clear();
                        
                        response_text.push_str(&processed_chunk.partial_text);

                        if let Some(tool_use) = processed_chunk.tool_use {
                            match processed_tool {
                                Some(ref mut tool) => {
                                    if !tool_use.tool_name.is_empty() {
                                        tool.tool_name = tool_use.tool_name;
                                    }
                                    tool.partial_tool_arguments.push_str(&tool_use.partial_tool_arguments);
                                }
                                None => {
                                    processed_tool = Some(tool_use);
                                }
                            }
                        }

                        if processed_chunk.is_done && processed_tool.is_some() {
                            let name = processed_tool.as_ref().unwrap().tool_name.clone();
                            let arguments =
                                serde_json::from_str::<JsonValue>(&processed_tool.as_ref().unwrap().partial_tool_arguments)
                                    .ok()
                                    .and_then(|args_value| args_value.as_object().cloned())
                                    .unwrap_or_else(|| serde_json::Map::new());
                            let tool_router_key = tools.as_ref().and_then(|tools_array| {
                                tools_array.iter().find_map(|tool| {
                                    if tool.get("name")?.as_str()? == name {
                                        tool.get("tool_router_key")
                                            .and_then(|key| key.as_str().map(|s| s.to_string()))
                                    } else {
                                        None
                                    }
                                })
                            });

                            let function_call = FunctionCall {
                                name,
                                arguments,
                                tool_router_key,
                                response: None,
                            };

                            function_calls.push(function_call.clone());

                            eprintln!("Function Call: {:?}", function_call);

                            if let Some(ref manager) = ws_manager_trait {
                                if let Some(ref inbox_name) = inbox_name {
                                    let m = manager.lock().await;
                                    let inbox_name_string = inbox_name.to_string();

                                    // Serialize FunctionCall to JSON value
                                    let function_call_json = serde_json::to_value(&function_call)
                                        .unwrap_or_else(|_| serde_json::json!({}));

                                    // Prepare ToolMetadata
                                    let tool_metadata = ToolMetadata {
                                        tool_name: function_call.name.clone(),
                                        tool_router_key: None,
                                        args: function_call_json.as_object().cloned().unwrap_or_default(),
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

                        if let Some(ref manager) = ws_manager_trait {
                            if let Some(ref inbox_name) = inbox_name {
                                let m = manager.lock().await;
                                let inbox_name_string = inbox_name.to_string();
                                let metadata = WSMetadata {
                                    id: Some(session_id.clone()),
                                    is_done: processed_chunk.is_done,
                                    done_reason: if processed_chunk.is_done {
                                        processed_chunk.done_reason.clone()
                                    } else {
                                        None
                                    },
                                    total_duration: None,
                                    eval_count: None,
                                };

                                let ws_message_type = WSMessageType::Metadata(metadata);

                                let _ = m
                                    .queue_message(
                                        WSTopic::Inbox,
                                        inbox_name_string,
                                        processed_chunk.partial_text.clone(),
                                        ws_message_type,
                                        true,
                                    )
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        // If it's a JSON parsing error, continue to accumulate more chunks
                        if let LLMProviderError::SerdeError(ref serde_err) = e {
                            if serde_err.to_string().contains("EOF while parsing") {
                                eprintln!("Incomplete chunk, continuing to accumulate...");
                                continue;
                            }
                        }
                        // If it's any other error, return it
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error while receiving chunk: {:?}, Error Source: {:?}", e, e.source());
                return Err(LLMProviderError::NetworkError(e.to_string()));
            }
        }
    }

    Ok(LLMInferenceResponse::new(
        response_text,
        json!({}),
        function_calls,
        None,
    ))
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
                        eprintln!("LLM job stopped by user request");
                        llm_stopper.reset(&inbox_name.to_string());

                        return Ok(LLMInferenceResponse::new("".to_string(), json!({}), Vec::new(), None));
                    }
                }
            },
            response = &mut response_fut => {
                let res = response?;
                let response_body = res.text().await?;
                let response_json: serde_json::Value = serde_json::from_str(&response_body)?;

                if let Some(content) = response_json.get("content") {
                    let mut response_text = String::new();
                    let mut function_calls = Vec::new();

                    for content_block in content.as_array().unwrap_or(&vec![]) {
                        if let Some(content_type) = content_block.get("type") {
                            match content_type.as_str().unwrap_or("") {
                                "text" => {
                                    if let Some(text) = content_block.get("text") {
                                        response_text.push_str(text.as_str().unwrap_or(""));
                                    }
                                }
                                "tool_use" => {
                                    let name = content_block["name"].as_str().unwrap_or_default().to_string();
                                    let arguments = content_block.get("input")
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

                                    let function_call = FunctionCall {
                                        name,
                                        arguments,
                                        tool_router_key,
                                        response: None,
                                    };

                                    function_calls.push(function_call.clone());

                                    eprintln!("Function Call: {:?}", function_call);

                                    // Send WS message if a function call is detected
                                    if let Some(ref manager) = ws_manager_trait {
                                        if let Some(ref inbox_name) = inbox_name {
                                            let m = manager.lock().await;
                                            let inbox_name_string = inbox_name.to_string();

                                            // Serialize FunctionCall to JSON value
                                            let function_call_json = serde_json::to_value(&function_call)
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
                                _ => {}
                            }
                        }
                    }

                    break Ok(LLMInferenceResponse::new(
                        response_text,
                        json!({}),
                        function_calls,
                        None,
                    ));
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
    if let Some(top_k) = get_value("LLM_TOP_K", config.and_then(|c| c.top_k.as_ref())) {
        payload["top_k"] = serde_json::json!(top_k);
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

#[derive(Debug, Clone)]
struct ProcessedChunk {
    partial_text: String,
    tool_use: Option<ProcessedTool>,
    is_done: bool,
    done_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct ProcessedTool {
    tool_name: String,
    partial_tool_arguments: String,
}

// Claude streams chunk of events. Each pack can contain text deltas, name of the tool used or partial JSON of tool arguments.
fn process_chunk(chunk: &[u8]) -> Result<ProcessedChunk, LLMProviderError> {
    let chunk_str = String::from_utf8_lossy(chunk).to_string();
    
    shinkai_log(
        ShinkaiLogOption::Node,
        ShinkaiLogLevel::Info,
        &format!("=== START RAW CHUNK ===\n{}\n=== END RAW CHUNK ===", chunk_str)
    );

    let mut text_blocks = Vec::new();
    let mut is_done = false;
    let mut done_reason = None;

    let mut content_block_type = String::new();
    let mut content_block_data = String::new();
    let mut current_tool: Option<ProcessedTool> = None;
    let mut current_text = String::new();

    let events = chunk_str.split("\n\n").collect::<Vec<&str>>();
    
    for event in events.iter() {
        let event_rows = event.split("\n").collect::<Vec<&str>>();

        if event_rows.len() < 2 {
            continue;
        }

        let event_type = event_rows[0];
        let event_data = event_rows[1];

        if event_type.starts_with("event: ") {
            let event_type = event_type.trim_start_matches("event: ");

            match event_type {
                "content_block_start" => {
                    let data_str = event_data.trim_start_matches("data: ");
                    let data_json: serde_json::Value = match serde_json::from_str(data_str) {
                        Ok(json) => json,
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to parse content_block_start JSON: {}", e)
                            );
                            return Err(LLMProviderError::SerdeError(e));
                        }
                    };

                    if let Some(content_block) = data_json.get("content_block") {
                        content_block_type = content_block.get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();

                        if content_block_type == "text" {
                            current_text.clear();
                            if let Some(text) = content_block.get("text").and_then(|t| t.as_str()) {
                                current_text.push_str(text);
                            }
                        }

                        if content_block_type == "tool_use" {
                            let tool_name = content_block["name"].as_str().unwrap_or("").to_string();
                            current_tool = Some(ProcessedTool {
                                tool_name,
                                partial_tool_arguments: String::new(),
                            });
                        }
                    }
                }
                "content_block_delta" => {
                    let data_str = event_data.trim_start_matches("data: ");
                    let data_json: serde_json::Value = match serde_json::from_str(data_str) {
                        Ok(json) => json,
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to parse content_block_delta JSON: {}", e)
                            );
                            return Err(LLMProviderError::SerdeError(e));
                        }
                    };

                    if let Some(delta) = data_json.get("delta") {
                        match delta.get("type").and_then(|t| t.as_str()) {
                            Some("text_delta") => {
                                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                    current_text.push_str(text);
                                }
                            }
                            Some("input_json_delta") => {
                                if let Some(input_json) = delta.get("partial_json").and_then(|t| t.as_str()) {
                                    content_block_data.push_str(input_json);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "content_block_stop" => {
                    if content_block_type == "text" && !current_text.is_empty() {
                        text_blocks.push(current_text.clone());
                        current_text.clear();
                    } else if content_block_type == "tool_use" {
                        if current_tool.is_none() {
                            current_tool = Some(ProcessedTool {
                                tool_name: "".to_string(),
                                partial_tool_arguments: "".to_string(),
                            });
                        }
                        current_tool.as_mut().map(|tool| {
                            tool.partial_tool_arguments = content_block_data.clone();
                        });
                    }

                    content_block_type = String::new();
                    content_block_data = String::new();
                }
                "message_delta" => {
                    let data_str = event_data.trim_start_matches("data: ");
                    let data_json: serde_json::Value = match serde_json::from_str(data_str) {
                        Ok(json) => json,
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to parse message_delta JSON: {}", e)
                            );
                            return Err(LLMProviderError::SerdeError(e));
                        }
                    };

                    if let Some(delta) = data_json.get("delta") {
                        if let Some(stop_reason) = delta.get("stop_reason").and_then(|r| r.as_str()) {
                            done_reason = Some(stop_reason.to_string());
                            is_done = true;
                        }
                    }
                }
                "message_stop" => {
                    is_done = true;
                }
                "error" => {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Error event received: {}", event_data)
                    );
                }
                _ => {}
            }
        }
    }

    // If there's any remaining text in the current_text buffer, add it
    if !current_text.is_empty() {
        text_blocks.push(current_text);
    }

    Ok(ProcessedChunk {
        partial_text: text_blocks.join(""),
        tool_use: current_tool,
        is_done,
        done_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_chunk_basic_text() {
        let chunk = r#"event: content_block_start
data: {"content_block":{"type":"text"},"index":0}

event: content_block_delta
data: {"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"delta":{"type":"text_delta","text":" world!"}}

event: content_block_stop
data: {"index":0}

event: message_stop
data: {}"#.as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "Hello world!");
        assert!(result.tool_use.is_none());
        assert!(result.is_done);
    }

    #[tokio::test]
    async fn test_process_chunk_tool_use() {
        let chunk = r#"event: content_block_start
data: {"content_block":{"type":"tool_use","name":"test_tool"}}

event: content_block_delta
data: {"delta":{"type":"input_json_delta","partial_json":"{\"arg\""}}

event: content_block_delta
data: {"delta":{"type":"input_json_delta","partial_json":":\"value\"}"}}

event: content_block_stop
data: {"index":0}

event: message_stop
data: {}"#.as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "");
        assert!(result.tool_use.is_some());
        let tool = result.tool_use.unwrap();
        assert_eq!(tool.tool_name, "test_tool");
        assert_eq!(tool.partial_tool_arguments, "{\"arg\":\"value\"}");
        assert!(result.is_done);
    }

    #[tokio::test]
    async fn test_process_chunk_mixed_content() {
        let chunk = r#"event: content_block_start
data: {"content_block":{"type":"text"},"index":0}

event: content_block_delta
data: {"delta":{"type":"text_delta","text":"Let me help you with that."}}

event: content_block_stop
data: {"index":0}

event: content_block_start
data: {"content_block":{"type":"tool_use","name":"search_tool"}}

event: content_block_delta
data: {"delta":{"type":"input_json_delta","partial_json":"{\"query\":\"test\"}"}}

event: content_block_stop
data: {"index":1}

event: message_stop
data: {}"#.as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "Let me help you with that.");
        assert!(result.tool_use.is_some());
        let tool = result.tool_use.unwrap();
        assert_eq!(tool.tool_name, "search_tool");
        assert_eq!(tool.partial_tool_arguments, "{\"query\":\"test\"}");
        assert!(result.is_done);
    }

    #[tokio::test]
    async fn test_process_chunk_error_handling() {
        let chunk = r#"event: error
data: {"error":{"type":"invalid_request_error","message":"Invalid request"}}"#.as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "");
        assert!(result.tool_use.is_none());
        assert!(!result.is_done);
    }

    #[tokio::test]
    async fn test_process_chunk_invalid_json() {
        let chunk = r#"event: content_block_start
data: {invalid_json"#.as_bytes();

        let result = process_chunk(chunk);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_chunk_with_done_reason() {
        let chunk = r#"event: content_block_start
data: {"content_block":{"type":"text"},"index":0}

event: content_block_delta
data: {"delta":{"type":"text_delta","text":"Complete response"}}

event: content_block_stop
data: {"index":0}

event: message_delta
data: {"delta":{"stop_reason":"stop_sequence"}}

event: message_stop
data: {}"#.as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "Complete response");
        assert!(result.tool_use.is_none());
        assert!(result.is_done);
        assert_eq!(result.done_reason.unwrap(), "stop_sequence");
    }

    #[tokio::test]
    async fn test_process_chunk_streaming_tool_arguments() {
        let chunk = r#"event: content_block_start
data: {"content_block":{"type":"tool_use","name":"complex_tool"}}

event: content_block_delta
data: {"delta":{"type":"input_json_delta","partial_json":"{\"key1\":"}}

event: content_block_delta
data: {"delta":{"type":"input_json_delta","partial_json":"\"value1\","}}

event: content_block_delta
data: {"delta":{"type":"input_json_delta","partial_json":"\"key2\":42}"}}

event: content_block_stop
data: {"index":0}

event: message_stop
data: {}"#.as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "");
        assert!(result.tool_use.is_some());
        let tool = result.tool_use.unwrap();
        assert_eq!(tool.tool_name, "complex_tool");
        assert_eq!(tool.partial_tool_arguments, "{\"key1\":\"value1\",\"key2\":42}");
        assert!(result.is_done);
    }

    #[tokio::test]
    async fn test_process_chunk_claude3_response() {
        let chunk = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_017dNmt3cx8HWm6u2fhVLwea","type":"message","role":"assistant","model":"claude-3-haiku-20240307","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":79,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":1}}            }

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}  }

event: ping
data: {"type": "ping"}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Yes"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":", I'm here. How"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" can I assist you today?"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0          }

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":16}       }

event: message_stop
data: {"type":"message_stop"          }"#.as_bytes();

        let result = process_chunk(chunk).unwrap();
        
        // Verify the complete text response
        assert_eq!(result.partial_text, "Yes, I'm here. How can I assist you today?");
        
        // Verify no tool use was involved
        assert!(result.tool_use.is_none());
        
        // Verify the message is complete
        assert!(result.is_done);
        
        // Verify the stop reason
        assert_eq!(result.done_reason.unwrap(), "end_turn");
    }

    #[tokio::test]
    async fn test_process_chunk_incomplete_json() {
        // Simulate getting chunks in parts
        let chunk1 = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_01AnJdoPCKGTHPrDkwhdUWzB","type":"message","role":"assistant","model":"claude-3-haiku-20240307","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":79,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":1}}      }

event: content_block_start
data: {"type":"content_block_start","index":0,"cont"#;

        let chunk2 = r#"ent_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Yes"}}

"#;

        let chunk3 = r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":", I'm here. How"}}

"#;

        // Test processing incomplete chunk
        let result1 = process_chunk(chunk1.as_bytes());
        assert!(result1.is_err());
        let error = result1.unwrap_err();
        let error_string = error.to_string();
        assert!(error_string.contains("EOF while parsing"), "Expected EOF error, got: {}", error_string);

        // Now simulate the streaming accumulation that should happen in handle_streaming_response
        let mut buffer = String::new();
        
        // Add first chunk
        buffer.push_str(chunk1);
        let result = process_chunk(buffer.as_bytes());
        assert!(result.is_err());
        
        // Add second chunk
        buffer.push_str(chunk2);
        let result = process_chunk(buffer.as_bytes());
        assert!(result.is_ok());
        let processed = result.unwrap();
        assert_eq!(processed.partial_text, "Yes");
        
        // Add third chunk
        buffer.push_str(chunk3);
        let result = process_chunk(buffer.as_bytes());
        assert!(result.is_ok());
        let processed = result.unwrap();
        assert_eq!(processed.partial_text, "Yes, I'm here. How");
    }
}
