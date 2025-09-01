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
        inbox_name::InboxName, job_config::JobConfig, llm_providers::serialized_llm_provider::{Claude, LLMProviderInterface}, prompts::Prompt
    }, shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption}
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::llm_provider::execution::chains::inference_chain_trait::FunctionCall;
use crate::llm_provider::{
    error::LLMProviderError, execution::chains::inference_chain_trait::LLMInferenceResponse, llm_stopper::LLMStopper
};
use crate::managers::model_capabilities_manager::PromptResultEnum;

use super::shared::claude_api::claude_prepare_messages;
use super::LLMService;

pub fn truncate_image_content_in_claude_payload(payload: &mut JsonValue) {
    if let Some(messages) = payload.get_mut("messages") {
        if let Some(array) = messages.as_array_mut() {
            for message in array {
                if let Some(content) = message.get_mut("content") {
                    if let Some(content_array) = content.as_array_mut() {
                        for content_item in content_array {
                            if let Some(content_type) = content_item.get("type") {
                                if content_type == "image" {
                                    if let Some(source) = content_item.get_mut("source") {
                                        if let Some(data) = source.get_mut("data") {
                                            if let Some(data_str) = data.as_str() {
                                                let truncated_data = format!("{}...", &data_str[0..100.min(data_str.len())]);
                                                *data = JsonValue::String(truncated_data);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

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
        db: Arc<SqliteManager>,
        tracing_message_id: Option<String>,
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

                // If model is Opus 4.1, remove top_p parameter from payload
                if self.model_type.starts_with("claude-opus-4-1") {
                    payload.as_object_mut().unwrap().remove("top_p");
                }

                // If thinking is enabled, subtract the thinking budget from the max_tokens
                if let Some(thinking) = payload.get("thinking") {
                    if let Some(thinking_type) = thinking.get("type") {
                        if thinking_type.as_str() == Some("enabled") {
                            if let (Some(max_tokens), Some(budget_tokens)) = (
                                payload["max_tokens"].as_u64(),
                                thinking["budget_tokens"].as_u64()
                            ) {
                                // Ensure we don't underflow and maintain a minimum of 1 token for actual output
                                let adjusted_max_tokens = if budget_tokens >= max_tokens {
                                    1 // Minimum token count for output
                                } else {
                                    max_tokens - budget_tokens
                                };
                                
                                payload["max_tokens"] = serde_json::json!(adjusted_max_tokens);
                            }
                        }
                    }
                }

                // Print payload as a pretty JSON string
                let mut payload_log = payload.clone();
                truncate_image_content_in_claude_payload(&mut payload_log);
                match serde_json::to_string_pretty(&payload_log) {
                    Ok(pretty_json) => eprintln!("cURL Payload: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
                };

                if let Some(ref msg_id) = tracing_message_id {
                    if let Err(e) = db.add_tracing(
                        msg_id,
                        inbox_name.as_ref().map(|i| i.get_value()).as_deref(),
                        "llm_payload",
                        &payload_log,
                    ) {
                        eprintln!("failed to add payload trace: {:?}", e);
                    }
                }

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

    // Check if it's an error response
    if !res.status().is_success() {
        let error_json: serde_json::Value = res.json().await?;
        if let Some(error) = error_json.get("error") {
            let error_message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            return Err(LLMProviderError::APIError(
                "AI Provider API Error: ".to_string() + error_message,
            ));
        }
        return Err(LLMProviderError::APIError(
            "AI Provider API Error: Unknown error occurred".to_string(),
        ));
    }

    // Check content type to determine if it's a stream
    let content_type = res.headers().get("content-type").and_then(|v| v.to_str().ok());
    let is_stream = match content_type {
        Some(ct) => {
            ct.contains("text/event-stream")
                || (ct.contains("application/json") && res.headers().contains_key("transfer-encoding"))
        }
        None => false,
    };

    if !is_stream {
        // Handle as regular JSON response
        let response_json: serde_json::Value = res.json().await?;
        if let Some(error) = response_json.get("error") {
            let error_message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            return Err(LLMProviderError::APIError(
                "AI Provider API Error: ".to_string() + error_message,
            ));
        }
        return Err(LLMProviderError::APIError(
            "AI Provider API Error: Expected streaming response but received regular JSON".to_string(),
        ));
    }

    let mut stream = res.bytes_stream();
    let mut response_text = String::new();
    let mut thinking_text = String::new();
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

                return Ok(LLMInferenceResponse::new(response_text, json!({}), Vec::new(), Vec::new(), None));
            }
        }

        match item {
            Ok(chunk) => {
                let new_data = String::from_utf8_lossy(&chunk);
                buffer.push_str(&new_data);

                // Parse events in a loop until we either:
                // - see incomplete JSON ("EOF while parsing"), or
                // - run out of parseable data in buffer
                loop {
                    match parse_one_event(&buffer) {
                        Ok((processed_chunk, consumed_bytes)) => {
                            // Remove only the processed part from the buffer
                            buffer.drain(..consumed_bytes);

                            // Update response text
                            response_text.push_str(&processed_chunk.partial_text);
                            thinking_text.push_str(&processed_chunk.thinking_text);

                            // Handle tool use
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

                            // Handle function calls when tool is complete
                            if processed_chunk.is_done && processed_tool.is_some() {
                                let name = processed_tool.as_ref().unwrap().tool_name.clone();
                                let arguments = serde_json::from_str::<JsonValue>(
                                    &processed_tool.as_ref().unwrap().partial_tool_arguments,
                                )
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
                                    index: function_calls.len() as u64,
                                    id: None,
                                    call_type: Some("function".to_string()),
                                };

                                function_calls.push(function_call.clone());

                                eprintln!("Function Call: {:?}", function_call);

                                // Reset processed_tool to prevent duplicate function calls
                                // since is_done can be set true multiple times (message_delta + message_stop)
                                processed_tool = None;

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
                                            index: function_call.index,
                                        };

                                        let ws_message_type =
                                            WSMessageType::Widget(WidgetMetadata::ToolRequest(tool_metadata));

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

                            // Send WS update
                            if let Some(ref manager) = ws_manager_trait {
                                if let Some(ref inbox_name) = inbox_name {
                                    let m = manager.lock().await;
                                    let inbox_name_string = inbox_name.to_string();
                                    let metadata = WSMetadata {
                                        id: Some(session_id.clone()),
                                        is_done: function_calls.is_empty() && processed_chunk.is_done,
                                        done_reason: if function_calls.is_empty() && processed_chunk.is_done {
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

                            // If buffer is empty, break to get next chunk
                            if buffer.is_empty() {
                                break;
                            }
                        }
                        Err(LLMProviderError::ContentParseFailed) => {
                            // Incomplete event - wait for more data
                            break;
                        }
                        Err(e) => {
                            // Any other error should be propagated
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                &format!("Error processing event: {}", e),
                            );
                            return Err(e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error while receiving chunk: {:?}, Error Source: {:?}", e, e.source());
                return Err(LLMProviderError::NetworkError(e.to_string()));
            }
        }
    }

    let final_response = if !thinking_text.is_empty() {
        format!("<think>{}</think>{}", thinking_text, response_text)
    } else {
        response_text
    };

    Ok(LLMInferenceResponse::new(
        final_response,
        json!({}),
        function_calls,
        Vec::new(),
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

                        return Ok(LLMInferenceResponse::new("".to_string(), json!({}), Vec::new(), Vec::new(), None));
                    }
                }
            },
            response = &mut response_fut => {
                let res = response?;

                // Check if it's an error response
                if !res.status().is_success() {
                    let error_json: serde_json::Value = res.json().await?;
                    if let Some(error) = error_json.get("error") {
                        let error_message = error.get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown error");
                        return Err(LLMProviderError::APIError(error_message.to_string()));
                    }
                    return Err(LLMProviderError::APIError("Unknown error occurred".to_string()));
                }

                let response_body = res.text().await?;
                let response_json: serde_json::Value = serde_json::from_str(&response_body)?;

                if let Some(content) = response_json.get("content") {
                    let mut thinking_text = String::new();
                    let mut response_text = String::new();
                    let mut function_calls = Vec::new();

                    for content_block in content.as_array().unwrap_or(&vec![]) {
                        if let Some(content_type) = content_block.get("type") {
                            match content_type.as_str().unwrap_or("") {
                                "thinking" => {
                                    if let Some(thinking) = content_block.get("thinking") {
                                        thinking_text.push_str(thinking.as_str().unwrap_or(""));
                                    }
                                }
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
                                        index: function_calls.len() as u64,
                                        id: None,
                                        call_type: Some("function".to_string()),
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
                                                index: function_call.index,
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

                    let final_response = if !thinking_text.is_empty() {
                        format!("<think>{}</think>{}", thinking_text, response_text)
                    } else {
                        response_text
                    };

                    break Ok(LLMInferenceResponse::new(
                        final_response,
                        json!({}),
                        function_calls,
                        Vec::new(),
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

fn has_tool_calls_in_messages(messages: &serde_json::Value) -> bool {
    if let Some(messages_array) = messages.as_array() {
        for message in messages_array {
            if let Some(content) = message.get("content") {
                // Check if content is an array of content blocks
                if let Some(content_array) = content.as_array() {
                    for content_block in content_array {
                        if let Some(content_type) = content_block.get("type").and_then(|t| t.as_str()) {
                            if content_type == "tool_use" || content_type == "tool_result" {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
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
    if let Some(thinking) = get_value("LLM_THINKING", config.and_then(|c| c.thinking.as_ref())) {
        // Check if there are actual tool calls in the messages - if so, disable thinking to avoid API errors
        // Claude's extended thinking feature requires specific message formatting when tools are used,
        // which can cause "Expected thinking or redacted_thinking, but found tool_use" errors
        let has_tool_calls = payload.get("messages")
            .map(|messages| has_tool_calls_in_messages(messages))
            .unwrap_or(false);

        if thinking && !has_tool_calls {
            let reasoning_effort = get_value("LLM_REASONING_EFFORT", config.and_then(|c| c.reasoning_effort.as_ref()));
            let budget_tokens = match reasoning_effort {
                Some(effort) => match effort.to_string().as_str() {
                    "low" => 1024,
                    "medium" => 2048,
                    "high" => 4096,
                    _ => 1024,
                },
                _ => 1024,
            };

            payload["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": budget_tokens,
            });

            // Claude is very restrictive with temperature, top_p, and top_k when using extended thinking.
            if let Some(obj) = payload.as_object_mut() {
                obj.remove("top_p");
                obj.remove("top_k");
                obj.remove("temperature");
            }
        } else {
            // Disable thinking if tool calls are present in messages or if thinking is explicitly disabled
            payload["thinking"] = serde_json::json!({
                "type": "disabled",
            });
        }
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
    thinking_text: String,
    tool_use: Option<ProcessedTool>,
    is_done: bool,
    done_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct ProcessedTool {
    tool_name: String,
    partial_tool_arguments: String,
}

/// Try to parse exactly one SSE event from the start of `buf`.
/// Returns `Ok((processed_event, consumed_bytes))` if one event
/// was successfully parsed; returns `Err(...)` if parse fails.
fn parse_one_event(buf: &str) -> Result<(ProcessedChunk, usize), LLMProviderError> {
    if let Some(double_newline_pos) = buf.find("\n\n") {
        let (this_block, _remainder) = buf.split_at(double_newline_pos + 2);

        // Check if this is a valid event block
        if !this_block.starts_with("event: ") {
            return Err(LLMProviderError::ContentParseFailed);
        }

        let parsed = parse_entire_sse_block(this_block)?;
        let consumed_bytes = this_block.len();
        Ok((parsed, consumed_bytes))
    } else {
        Err(LLMProviderError::ContentParseFailed)
    }
}

/// Parse a string containing exactly one SSE "event: ...\n data: ...\n\n" block.
fn parse_entire_sse_block(block: &str) -> Result<ProcessedChunk, LLMProviderError> {
    let mut text_blocks = Vec::new();
    let mut is_done = false;
    let mut done_reason = None;
    let mut content_block_type = String::new();
    let mut _content_block_index: Option<u64> = None;
    let mut current_tool: Option<ProcessedTool> = None;
    let mut current_text = String::new();
    let mut thinking_text = String::new();
    let mut accumulated_text = String::new();

    let event_rows: Vec<&str> = block.lines().collect();

    if event_rows.len() < 2 {
        return Ok(ProcessedChunk {
            partial_text: String::new(),
            thinking_text: String::new(),
            tool_use: None,
            is_done: false,
            done_reason: None,
        });
    }

    let event_type = event_rows[0].trim_start_matches("event: ");
    let event_data = event_rows[1].trim_start_matches("data: ");

    match event_type {
        "content_block_start" => {
            if let Ok(data_json) = serde_json::from_str::<serde_json::Value>(event_data) {
                // Extract index from the event data
                _content_block_index = data_json.get("index").and_then(|i| i.as_u64());
                
                if let Some(content_block) = data_json.get("content_block") {
                    content_block_type = content_block
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();

                    if content_block_type == "text" {
                        if let Some(text) = content_block.get("text").and_then(|t| t.as_str()) {
                            current_text.push_str(text);
                            accumulated_text.push_str(text);
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
        }
        "content_block_delta" => {
            if let Ok(data_json) = serde_json::from_str::<serde_json::Value>(event_data) {
                if let Some(delta) = data_json.get("delta") {
                    match delta.get("type").and_then(|t| t.as_str()) {
                        Some("text_delta") => {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                current_text.push_str(text);
                                accumulated_text.push_str(text);
                            }
                        }
                        Some("thinking_delta") => {
                            if let Some(thinking) = delta.get("thinking").and_then(|t| t.as_str()) {
                                thinking_text.push_str(thinking);
                            }
                        }
                        Some("input_json_delta") => {
                            if let Some(input_json) = delta.get("partial_json").and_then(|t| t.as_str()) {
                                if current_tool.is_none() {
                                    current_tool = Some(ProcessedTool {
                                        tool_name: String::new(),
                                        partial_tool_arguments: String::new(),
                                    });
                                }

                                if let Some(ref mut tool) = current_tool {
                                    tool.partial_tool_arguments.push_str(input_json);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        "content_block_stop" => {
            if content_block_type == "text" && !current_text.is_empty() {
                text_blocks.push(current_text.clone());
                current_text.clear();
            }
        }
        "message_delta" => {
            if let Ok(data_json) = serde_json::from_str::<serde_json::Value>(event_data) {
                if let Some(delta) = data_json.get("delta") {
                    if let Some(stop_reason) = delta.get("stop_reason").and_then(|r| r.as_str()) {
                        done_reason = Some(stop_reason.to_string());
                        is_done = true;
                    }
                }
            }
        }
        "message_stop" => {
            is_done = true;
        }
        "error" => {
            // Handle error events by returning empty chunk
            return Ok(ProcessedChunk {
                partial_text: String::new(),
                thinking_text: String::new(),
                tool_use: None,
                is_done: false,
                done_reason: None,
            });
        }
        _ => {}
    }

    Ok(ProcessedChunk {
        partial_text: accumulated_text,
        thinking_text,
        tool_use: current_tool,
        is_done,
        done_reason,
    })
}

#[allow(dead_code)]
fn process_chunk(chunk: &[u8]) -> Result<ProcessedChunk, LLMProviderError> {
    let chunk_str = String::from_utf8_lossy(chunk).to_string();
    let mut buffer = chunk_str.clone();
    let mut final_partial_text = String::new();
    let mut final_thinking_text = String::new();
    let mut final_tool_use: Option<ProcessedTool> = None;
    let mut final_is_done = false;
    let mut final_done_reason = None;

    // Process each event in the chunk
    while !buffer.is_empty() {
        match parse_one_event(&buffer) {
            Ok((parsed_block, consumed_bytes)) => {
                buffer.drain(..consumed_bytes);

                // Accumulate text
                final_partial_text.push_str(&parsed_block.partial_text);
                final_thinking_text.push_str(&parsed_block.thinking_text);

                // Handle tool use
                if let Some(tu) = parsed_block.tool_use {
                    match &mut final_tool_use {
                        Some(existing_tool) => {
                            if !tu.tool_name.is_empty() {
                                existing_tool.tool_name = tu.tool_name;
                            }
                            if !tu.partial_tool_arguments.is_empty() {
                                existing_tool
                                    .partial_tool_arguments
                                    .push_str(&tu.partial_tool_arguments);
                            }
                        }
                        None => {
                            final_tool_use = Some(tu);
                        }
                    }
                }

                // Update done status
                if parsed_block.is_done {
                    final_is_done = true;
                    if parsed_block.done_reason.is_some() {
                        final_done_reason = parsed_block.done_reason;
                    }
                }
            }
            Err(LLMProviderError::ContentParseFailed) => {
                // If we can't parse any more events, break
                break;
            }
            Err(e) => return Err(e),
        }
    }

    // Check for message_stop in the remaining buffer
    if buffer.contains("event: message_stop") {
        final_is_done = true;
    }

    Ok(ProcessedChunk {
        partial_text: final_partial_text,
        thinking_text: final_thinking_text,
        tool_use: final_tool_use,
        is_done: final_is_done,
        done_reason: final_done_reason,
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
data: {}"#
            .as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "Hello world!");
        assert_eq!(result.thinking_text, "");
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
data: {}"#
            .as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "");
        assert_eq!(result.thinking_text, "");
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
data: {}"#
            .as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "Let me help you with that.");
        assert_eq!(result.thinking_text, "");
        assert!(result.tool_use.is_some());
        let tool = result.tool_use.unwrap();
        assert_eq!(tool.tool_name, "search_tool");
        assert_eq!(tool.partial_tool_arguments, "{\"query\":\"test\"}");
        assert!(result.is_done);
    }

    #[tokio::test]
    async fn test_process_chunk_error_handling() {
        let chunk = r#"event: error
data: {"error":{"type":"invalid_request_error","message":"Invalid request"}}"#
            .as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "");
        assert_eq!(result.thinking_text, "");
        assert!(result.tool_use.is_none());
        assert!(!result.is_done);
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
data: {}"#
            .as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "Complete response");
        assert_eq!(result.thinking_text, "");
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
data: {}"#
            .as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "");
        assert_eq!(result.thinking_text, "");
        assert!(result.tool_use.is_some());
        let tool = result.tool_use.unwrap();
        assert_eq!(tool.tool_name, "complex_tool");
        assert_eq!(tool.partial_tool_arguments, "{\"key1\":\"value1\",\"key2\":42}");
        assert!(result.is_done);
    }

    #[tokio::test]
    async fn test_process_chunk_thinking_streaming() {
        let chunk = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_015onJzKgRMJHn28uVzuUtt7","type":"message","role":"assistant","model":"claude-opus-4-20250514","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":93,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":4,"service_tier":"standard"}}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}

event: ping
data: {"type": "ping"}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"The user is"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" asking me to repeat"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" back only"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" the word \"dog"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"cat\" with"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" no other words."}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" This is straight"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"forward - I"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" should just respon"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"d with \"dogcat"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"\" an"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"d nothing else."}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"EsoCCkYIBhgCKkA/mylU2TQ1s4HkOwnSA+bYpG9u+xF5p71sqMzfvDMy1YI9ouFQ0AO/mxPsRRSfLkCHKziTRI1IY4Hi4K0oM7qOEgxwnNPTothmQ4wKxfkaDE0Yz5zRcx6vhTL1QiIwOIvrXpateCp7JdfpS39d2Deb6rJGBMUBQB690T9uLcKuaR2bMbv2jPyZsx49qo6vKrEBTRNSrSwNBKmPCdyRc2S+kN7UlAuIyR79WAj7KGbt+4VHxHKuThMbvFzAX5U2+anvDMwoJYi2Y/RiSg4EeikWgyys4SGpGuXNxm+vHtvjsej2MYCkF2E3dFSGhnb02AJ5Uj9mFD1GoRh+QUtBlvkDnQlsKn1L2vOWkjYY5JKc7k1byyw5eaUWBMi7bybRhpaRBZLHp+1o+GqKZNRaHUrG4sLPp3k1GKYR1S+BGRDBXS2wGAE="}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"dogcat"}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":52}}

event: message_stop
data: {"type":"message_stop"}

"#
            .as_bytes();

        let result = process_chunk(chunk).unwrap();
        
        // Verify thinking text is captured correctly
        assert_eq!(result.thinking_text, "The user is asking me to repeat back only the word \"dogcat\" with no other words. This is straightforward - I should just respond with \"dogcat\" and nothing else.");
        
        // Verify regular text response is captured
        assert_eq!(result.partial_text, "dogcat");
        
        // Verify no tool use
        assert!(result.tool_use.is_none());
        
        // Verify completion status
        assert!(result.is_done);
        assert_eq!(result.done_reason.unwrap(), "end_turn");
    }

    #[tokio::test]
    async fn test_no_duplicate_function_calls_with_message_delta_and_stop() {
        let chunk = r#"event: content_block_start
data: {"content_block":{"type":"tool_use","name":"test_tool"}}

event: content_block_delta
data: {"delta":{"type":"input_json_delta","partial_json":"{\"message\":\"hello\"}"}}

event: content_block_stop
data: {"index":0}

event: message_delta
data: {"delta":{"stop_reason":"tool_use"}}

event: message_stop
data: {}"#
            .as_bytes();

        let result = process_chunk(chunk).unwrap();
        assert_eq!(result.partial_text, "");
        assert_eq!(result.thinking_text, "");
        assert!(result.tool_use.is_some());
        let tool = result.tool_use.unwrap();
        assert_eq!(tool.tool_name, "test_tool");
        assert_eq!(tool.partial_tool_arguments, "{\"message\":\"hello\"}");
        assert!(result.is_done);
        assert_eq!(result.done_reason.unwrap(), "tool_use");
    }

    #[tokio::test]
    async fn test_process_chunk_text_and_tool_use_streaming() {
        let chunks = vec![
            // First chunk: Message start
            r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_01GCh8jZFGBXeDhu4avMiYMX","type":"message","role":"assistant","model":"claude-3-haiku-20240307","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":686,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":4}}}

"#,
            // Second chunk: Tool use start
            r#"event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_01VPxk1XXTjpMN41kxnXykCx","name":"duckduckgo_search","input":{}}}

event: ping
data: {"type": "ping"}

"#,
            // Third chunk: First part of JSON
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"mes"}}

"#,
            // Fourth chunk: Second part of JSON
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"sage\":"}}

"#,
            // Fifth chunk: Third part of JSON
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":" \"mov"}}

"#,
            // Sixth chunk: Fourth part of JSON
            r#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"ies\"}"}}

"#,
            // Seventh chunk: Content block stop
            r#"event: content_block_stop
data: {"type":"content_block_stop","index":0}

"#,
            // Eighth chunk: Message delta with stop reason
            r#"event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":57}}

"#,
            // Ninth chunk: Message stop
            r#"event: message_stop
data: {"type":"message_stop"}

"#,
        ];

        let mut buffer = String::new();
        let mut accumulated_tool: Option<ProcessedTool> = None;
        let mut is_done = false;
        let mut done_reason = None;

        // Process each chunk and verify the accumulation
        for (i, chunk) in chunks.iter().enumerate() {
            // 1) Append new chunk data to buffer
            buffer.push_str(chunk);

            // 2) Parse as many complete SSE blocks as possible
            loop {
                match parse_one_event(&buffer) {
                    Ok((parsed, consumed_bytes)) => {
                        // Remove the processed part from the buffer
                        buffer.drain(..consumed_bytes);

                        // Update our accumulated state
                        if let Some(tool) = parsed.tool_use {
                            match accumulated_tool.as_mut() {
                                Some(acc_tool) => {
                                    // Accumulate arguments if we already have a tool
                                    if !tool.partial_tool_arguments.is_empty() {
                                        acc_tool.partial_tool_arguments.push_str(&tool.partial_tool_arguments);
                                    }
                                }
                                None => {
                                    // Initialize tool if we don't have one yet
                                    accumulated_tool = Some(tool);
                                }
                            }
                        }

                        // Update completion status
                        is_done = parsed.is_done;
                        if parsed.done_reason.is_some() {
                            done_reason = parsed.done_reason;
                        }

                        // Add assertions for specific chunks to verify the accumulation process
                        match i {
                            1 => {
                                // After tool_use start, we should have a tool with empty arguments
                                assert!(accumulated_tool.is_some());
                                assert_eq!(accumulated_tool.as_ref().unwrap().tool_name, "duckduckgo_search");
                                assert_eq!(accumulated_tool.as_ref().unwrap().partial_tool_arguments, "");
                            }
                            2 => {
                                // After first JSON part
                                assert_eq!(accumulated_tool.as_ref().unwrap().partial_tool_arguments, "{\"mes");
                            }
                            3 => {
                                // After second JSON part
                                assert_eq!(
                                    accumulated_tool.as_ref().unwrap().partial_tool_arguments,
                                    "{\"message\":"
                                );
                            }
                            4 => {
                                // After third JSON part
                                assert_eq!(
                                    accumulated_tool.as_ref().unwrap().partial_tool_arguments,
                                    "{\"message\": \"mov"
                                );
                            }
                            5 => {
                                // After fourth JSON part - complete JSON
                                assert_eq!(
                                    accumulated_tool.as_ref().unwrap().partial_tool_arguments,
                                    "{\"message\": \"movies\"}"
                                );
                            }
                            7 => {
                                // After message_delta
                                assert!(is_done);
                                assert_eq!(done_reason.as_deref(), Some("tool_use"));
                            }
                            _ => {}
                        }

                        // If buffer is empty, break to get next chunk
                        if buffer.is_empty() {
                            break;
                        }
                    }
                    Err(LLMProviderError::ContentParseFailed) => {
                        // Partial data => wait for next chunk
                        break;
                    }
                    Err(other) => {
                        panic!("Unexpected parse error: {other}");
                    }
                }
            }
        }

        // Final verification
        assert!(is_done);
        assert_eq!(done_reason.as_deref(), Some("tool_use"));
        assert!(accumulated_tool.is_some());
        let final_tool = accumulated_tool.unwrap();
        assert_eq!(final_tool.tool_name, "duckduckgo_search");
        assert_eq!(final_tool.partial_tool_arguments, "{\"message\": \"movies\"}");
    }
}
