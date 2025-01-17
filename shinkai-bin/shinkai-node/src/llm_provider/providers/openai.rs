use std::error::Error;
use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::openai_api::{openai_prepare_messages, MessageContent, OpenAIResponse};
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, LLMInferenceResponse};
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use async_trait::async_trait;
use futures::StreamExt;
use regex::Regex;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenAI};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::{
    ToolMetadata, ToolStatus, ToolStatusType, WSMessageType, WSMetadata, WSUpdateHandler, WidgetMetadata,
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tokio::sync::Mutex;
use uuid::Uuid;

pub fn truncate_image_url_in_payload(payload: &mut JsonValue) {
    if let Some(messages) = payload.get_mut("messages") {
        if let Some(array) = messages.as_array_mut() {
            for message in array {
                if let Some(content) = message.get_mut("content") {
                    if let Some(array) = content.as_array_mut() {
                        for item in array {
                            if let Some(image_url) = item.get_mut("image_url") {
                                if let Some(url) = image_url.get_mut("url") {
                                    if let Some(str_url) = url.as_str() {
                                        let truncated_url = format!("{}...", &str_url[0..20.min(str_url.len())]);
                                        *url = JsonValue::String(truncated_url);
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

#[derive(Debug, Clone)]
pub struct PartialFunctionCall {
    pub name: Option<String>,
    pub arguments: String,
    pub is_accumulating: bool,  // Track if we're currently accumulating a function call
}

#[async_trait]
impl LLMService for OpenAI {
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
                let url = format!("{}{}", base_url, "/v1/chat/completions");

                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);

                // Note: we can use prepare_messages directly or we could have called ModelCapabilitiesManager
                let result = openai_prepare_messages(&model, prompt)?;
                let messages_json = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                // Extract tools_json from the result
                let tools_json = result.functions.unwrap_or_else(Vec::new);

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
                    "max_tokens": result.remaining_output_tokens,
                    "stream": is_stream,
                });

                // Conditionally add functions to the payload if tools_json is not empty
                if !tools_json.is_empty() {
                    payload["functions"] = serde_json::Value::Array(tools_json.clone());
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
                        payload,
                        key.clone(),
                        inbox_name,
                        llm_stopper,
                        ws_manager_trait,
                        Some(tools_json),
                    )
                    .await
                }
            } else {
                Err(LLMProviderError::ApiKeyNotSet)
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}

/// A synchronous version of finalize_function_call that doesn't deal with WebSocket updates
fn finalize_function_call_sync(
    partial_fc: &mut PartialFunctionCall,
    function_calls: &mut Vec<FunctionCall>,
    tools: &Option<Vec<JsonValue>>,
) {
    if let Some(ref name) = partial_fc.name {
        if !name.is_empty() {
            // Attempt to parse the accumulated arguments
            let fc_arguments = if partial_fc.arguments.is_empty() {
                serde_json::Map::new()
            } else {
                match serde_json::from_str::<serde_json::Value>(&partial_fc.arguments) {
                    Ok(value) => {
                        value.as_object().cloned().unwrap_or_else(|| {
                            serde_json::Map::new()
                        })
                    }
                    Err(e) => {
                        eprintln!("Failed to parse arguments: {}", e);
                        serde_json::Map::new()
                    }
                }
            };

            // Look up the optional tool_router_key
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

            // Build and add to the function_calls vector
            let new_function_call = FunctionCall {
                name: name.clone(),
                arguments: fc_arguments,
                tool_router_key,
                response: None,
            };
            function_calls.push(new_function_call);
        }
    }

    // Clear partial so we can accumulate a new function call in subsequent chunks
    partial_fc.name = None;
    partial_fc.arguments.clear();
}

pub async fn parse_openai_stream_chunk(
    buffer: &mut String,
    response_text: &mut String,
    function_calls: &mut Vec<FunctionCall>,
    partial_fc: &mut PartialFunctionCall,
    tools: &Option<Vec<JsonValue>>,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: Option<InboxName>,
    session_id: &str,
) -> Result<Option<String>, LLMProviderError> {
    let mut error_message: Option<String> = None;

    loop {
        // Look for a newline in `buffer`; if none is found, break.
        let Some(newline_pos) = buffer.find('\n') else {
            // No complete line yet, so we can't parse anything. We'll wait for more data.
            break;
        };

        // Extract this line (including the '\n') from the buffer:
        let line_with_newline = buffer.drain(..=newline_pos).collect::<String>();
        // Trim trailing whitespace from it:
        let line = line_with_newline.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }


        // If the line doesn't start with "data: ", check if it's an array-formatted error
        if !line.starts_with("data: ") {
            // If it is literally [DONE], skip
            if line == "[DONE]" {
                // If we were accumulating a function call, finalize it
                if partial_fc.is_accumulating && partial_fc.name.is_some() {
                    finalize_function_call_sync(partial_fc, function_calls, tools);
                }
                continue;
            }

            // Check if it's an array-formatted error response
            if line.starts_with("[") {
                match serde_json::from_str::<Vec<JsonValue>>(line) {
                    Ok(array) => {
                        if let Some(first_item) = array.first() {
                            if let Some(error) = first_item.get("error") {
                                let code = error
                                    .get("code")
                                    .and_then(|c| {
                                        c.as_u64()
                                            .map(|n| n.to_string())
                                            .or_else(|| c.as_str().map(|s| s.to_string()))
                                    })
                                    .unwrap_or_else(|| "Unknown code".to_string());
                                let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
                                error_message = Some(format!("{}: {}", code, msg));
                                continue;
                            }
                        }
                    }
                    Err(_) => {
                        // Not a valid array JSON, ignore or log
                        continue;
                    }
                }
            }

            // Otherwise, ignore or log, but do not parse.
            continue;
        }

        // Slice out whatever came after "data: "
        let chunk = &line["data: ".len()..];

        // If it's "[DONE]", send final update and skip
        if chunk.trim() == "[DONE]" {
            // If we were accumulating a function call, finalize it
            if partial_fc.is_accumulating && partial_fc.name.is_some() {
                finalize_function_call_sync(partial_fc, function_calls, tools);
            }
            if let Some(inbox_name) = inbox_name.as_ref() {
                send_ws_update(
                    ws_manager_trait,
                    Some(inbox_name.clone()),
                    session_id,
                    "".to_string(),
                    true,
                    None,
                )
                .await?;
            }
            continue;
        }

        // Extract any function call arguments before parsing JSON
        let (maybe_args, cleaned_chunk) = extract_and_remove_arguments(chunk);

        // If we extracted arguments and we're accumulating a function call, append them
        if let Some(args) = maybe_args {
            if partial_fc.is_accumulating {
                partial_fc.arguments.push_str(&args);
            }
        }

        // Attempt to parse the cleaned JSON
        match serde_json::from_str::<JsonValue>(&cleaned_chunk) {
            Ok(json_data) => {
                // If there's an error object, record it
                if let Some(error_obj) = json_data.get("error") {
                    let code = error_obj
                        .get("code")
                        .and_then(|c| c.as_str())
                        .unwrap_or("Unknown code")
                        .to_string();
                    let msg = error_obj
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");
                    error_message = Some(format!("{}: {}", code, msg));
                    continue;
                }

                // Otherwise, look for "choices"
                if let Some(choices) = json_data.get("choices") {
                    // Each item in "choices" may have "delta": { "content": "..."} or "function_call": ...
                    for choice in choices.as_array().unwrap_or(&vec![]) {
                        let finish_reason = choice
                            .get("finish_reason")
                            .and_then(|fr| fr.as_str())
                            .unwrap_or_default();

                        if let Some(delta) = choice.get("delta") {
                            // If there's text content, append it and send WS update
                            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                response_text.push_str(content);

                                // Send WS update for the new content
                                if let Some(inbox_name) = inbox_name.as_ref() {
                                    send_ws_update(
                                        ws_manager_trait,
                                        Some(inbox_name.clone()),
                                        session_id,
                                        content.to_string(),
                                        // if finish_reason is empty, we are not at the end of the stream
                                        !finish_reason.is_empty(),
                                        Some(finish_reason.to_string()),
                                    )
                                    .await?;
                                }
                            }

                            // If there's function_call
                            if let Some(fc) = delta.get("function_call") {
                                if let Some(name) = fc.get("name").and_then(|n| n.as_str()) {
                                    // If partial_fc already had a different name, finalize that first
                                    if let Some(old_name) = &partial_fc.name {
                                        if !old_name.is_empty() && old_name != name {
                                            finalize_function_call_sync(partial_fc, function_calls, tools);
                                        }
                                    }
                                    partial_fc.name = Some(name.to_string());
                                    partial_fc.is_accumulating = true;
                                }
                            }
                        }

                        // If finish_reason == "function_call", finalize the partial
                        // so that we stop collecting arguments for this function.
                        // (OpenAI signals that we've received all the arguments.)
                        if finish_reason == "function_call" {
                            finalize_function_call_sync(partial_fc, function_calls, tools);
                        } else if finish_reason == "stop" {
                            // If the user or model stops, we can finalize
                            // any function call that wasn't yet finished.
                            if partial_fc.name.is_some() {
                                finalize_function_call_sync(partial_fc, function_calls, tools);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                // If we're accumulating a function call, keep accumulating
                if partial_fc.is_accumulating {
                    continue;
                }
                // Otherwise, this might be a partial line that got split up
                // Put it back into `buffer` so next chunk can finish it
                buffer.insert_str(0, &(line.to_string() + "\n"));
                break;
            }
        }
    }

    Ok(error_message)
}

pub async fn handle_streaming_response(
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
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    let mut stream = res.bytes_stream();
    let mut response_text = String::new();
    let mut buffer = String::new();
    let mut function_calls: Vec<FunctionCall> = Vec::new();
    let mut error_message: Option<String> = None;
    let mut partial_fc = PartialFunctionCall {
        name: None,
        arguments: String::new(),
        is_accumulating: false,
    };

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

                send_ws_update(
                    &ws_manager_trait,
                    Some(inbox_name.clone()),
                    &session_id,
                    response_text.clone(),
                    true,
                    Some("Stopped by user request".to_string()),
                )
                .await?;

                // Process complete messages in the buffer
                if let Ok(Some(err)) = parse_openai_stream_chunk(
                    &mut buffer,
                    &mut response_text,
                    &mut function_calls,
                    &mut partial_fc,
                    &tools,
                    &ws_manager_trait,
                    Some(inbox_name.clone()),
                    &session_id,
                )
                .await
                {
                    error_message = Some(err);
                }

                // Handle WebSocket updates for function calls
                if let Some(ref _manager) = ws_manager_trait {
                    if let Some(last_function_call) = function_calls.last() {
                        send_tool_ws_update(&ws_manager_trait, Some(inbox_name.clone()), last_function_call).await?;
                    }
                }

                return Ok(LLMInferenceResponse::new(response_text, json!({}), Vec::new(), None));
            }
        }

        match item {
            Ok(chunk) => {
                let chunk_str = String::from_utf8_lossy(&chunk).to_string();
                eprintln!("Raw server response chunk: {}", chunk_str);
                buffer.push_str(&chunk_str);

                // Process complete messages in the buffer
                if let Ok(Some(err)) = parse_openai_stream_chunk(
                    &mut buffer,
                    &mut response_text,
                    &mut function_calls,
                    &mut partial_fc,
                    &tools,
                    &ws_manager_trait,
                    inbox_name.clone(),
                    &session_id,
                )
                .await
                {
                    error_message = Some(err);
                }

                // Handle WebSocket updates for function calls
                if let Some(ref manager) = ws_manager_trait {
                    if let Some(ref inbox_name) = inbox_name {
                        if let Some(last_function_call) = function_calls.last() {
                            send_tool_ws_update(&ws_manager_trait, Some(inbox_name.clone()), last_function_call)
                                .await?;
                        }
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

    // If there's an unfinalized function call at the end, finalize it
    if partial_fc.name.is_some() && !partial_fc.arguments.is_empty() {
        finalize_function_call_sync(&mut partial_fc, &mut function_calls, &tools);
    }

    if let Some(ref error_message) = error_message {
        if response_text.is_empty() {
            return Err(LLMProviderError::LLMServiceUnexpectedError(error_message.to_string()));
        }
    }

    Ok(LLMInferenceResponse::new(
        response_text,
        json!({}),
        function_calls,
        None,
    ))
}

pub async fn handle_non_streaming_response(
    client: &Client,
    url: String,
    payload: JsonValue,
    api_key: String,
    inbox_name: Option<InboxName>,
    llm_stopper: Arc<LLMStopper>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    tools: Option<Vec<JsonValue>>,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    let response_fut = client
        .post(url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
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

                        return Ok(LLMInferenceResponse::new("".to_string(), json!({}), Vec::new(), None));
                    }
                }
            },
            response = &mut response_fut => {
                let res = response?;
                let response_text = res.text().await?;
                eprintln!("Raw server response: {}", response_text);
                let data_resp: Result<JsonValue, _> = serde_json::from_str(&response_text);

                match data_resp {
                    Ok(value) => {
                        if let Some(error) = value.get("error") {
                            let code = error.get("code").and_then(|c| c.as_str());
                            let formatted_error =
                                if let (Some(code), Some(message)) = (code, error.get("message").and_then(|m| m.as_str())) {
                                    format!("{}: {}", code, message)
                                } else {
                                    serde_json::to_string(&error).unwrap_or_default()
                                };

                            return Err(match code {
                                Some("rate_limit_exceeded") => {
                                    LLMProviderError::LLMServiceInferenceLimitReached(formatted_error.to_string())
                                }
                                _ => LLMProviderError::LLMServiceUnexpectedError(formatted_error.to_string()),
                            });
                        }

                        let data: OpenAIResponse = serde_json::from_value(value).map_err(LLMProviderError::SerdeError)?;

                        let response_string: String = data
                            .choices
                            .iter()
                            .filter_map(|choice| match &choice.message.content {
                                Some(MessageContent::Text(text)) => {
                                    // Unescape the JSON string
                                    let cleaned_json_str = text.replace("\\\"", "\"").replace("\\n", "\n");
                                    Some(cleaned_json_str)
                                }
                                Some(MessageContent::ImageUrl { .. }) => None,
                                Some(MessageContent::FunctionCall(_)) => None,
                                None => None,
                            })
                            .collect::<Vec<String>>()
                            .join(" ");

                        let function_call: Option<FunctionCall> = data.choices.iter().find_map(|choice| {
                            choice.message.function_call.clone().map(|fc| {
                                let arguments = serde_json::from_str::<serde_json::Value>(&fc.arguments)
                                    .ok()
                                    .and_then(|args_value: serde_json::Value| args_value.as_object().cloned())
                                    .unwrap_or_else(|| serde_json::Map::new());

                                // Extract tool_router_key
                                let tool_router_key = tools.as_ref().and_then(|tools_array| {
                                    tools_array.iter().find_map(|tool| {
                                        if tool.get("name")?.as_str()? == fc.name {
                                            tool.get("tool_router_key").and_then(|key| key.as_str().map(|s| s.to_string()))
                                        } else {
                                            None
                                        }
                                    })
                                });

                                FunctionCall {
                                    name: fc.name,
                                    arguments,
                                    tool_router_key,
                                    response: None,
                                }
                            })
                        });
                        eprintln!("Function Call: {:?}", function_call);
                        eprintln!("Response String: {:?}", response_string);

                        // Updated WS message handling for tooling
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
                                        tool_router_key: function_call.tool_router_key.clone(),
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

                        return Ok(LLMInferenceResponse::new(
                            response_string,
                            json!({}),
                            function_call.map_or_else(Vec::new, |fc| vec![fc]),
                            None,
                        ));
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Failed to parse response: {:?}", e).as_str(),
                        );
                        return Err(LLMProviderError::SerdeError(e));
                    }
                }
            }
        }
    }
}

pub fn add_options_to_payload(payload: &mut serde_json::Value, config: Option<&JobConfig>) {
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

// Add helper function for sending WS updates
async fn send_ws_update(
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: Option<InboxName>,
    session_id: &str,
    content: String,
    is_done: bool,
    done_reason: Option<String>,
) -> Result<(), LLMProviderError> {
    if let Some(ref manager) = ws_manager_trait {
        if let Some(inbox_name) = inbox_name {
            let m = manager.lock().await;
            let inbox_name_string = inbox_name.to_string();

            let metadata = WSMetadata {
                id: Some(session_id.to_string()),
                is_done,
                done_reason,
                total_duration: None,
                eval_count: None,
            };

            let ws_message_type = WSMessageType::Metadata(metadata);

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Websocket content: {}", content).as_str(),
            );

            let _ = m
                .queue_message(WSTopic::Inbox, inbox_name_string, content, ws_message_type, true)
                .await;
        }
    }
    Ok(())
}

async fn send_tool_ws_update(
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: Option<InboxName>,
    function_call: &FunctionCall,
) -> Result<(), LLMProviderError> {
    if let Some(ref manager) = ws_manager_trait {
        if let Some(inbox_name) = inbox_name {
            let m = manager.lock().await;
            let inbox_name_string = inbox_name.to_string();

            let function_call_json = serde_json::to_value(function_call).unwrap_or_else(|_| serde_json::json!({}));

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

            let ws_message_type = WSMessageType::Widget(WidgetMetadata::ToolRequest(tool_metadata));

            eprintln!("Websocket content (function_call): {}", serde_json::to_string(function_call).unwrap_or_else(|_| "{}".to_string()));

            let _ = m
                .queue_message(
                    WSTopic::Inbox,
                    inbox_name_string,
                    serde_json::to_string(function_call).unwrap_or_else(|_| "{}".to_string()),
                    ws_message_type,
                    true,
                )
                .await;
        }
    }
    Ok(())
}

pub fn extract_and_remove_arguments(json_str: &str) -> (Option<String>, String) {
    // Find the start of arguments value
    let args_prefix = r#""function_call":{"arguments":""#;
    if let Some(args_start_pos) = json_str.find(args_prefix) {
        let content_start = args_start_pos + args_prefix.len();
        
        // Find the end of arguments value by looking for the closing quotes and braces
        // We look for the first quote that's followed by }}, which indicates the end of the function_call object
        if let Some(mut content_end) = json_str[content_start..].find(r#""}}"#) {
            content_end += content_start; // Adjust position to be relative to the full string
            
            // Extract the arguments content
            let content = json_str[content_start..content_end].to_string();
            
            // Build the cleaned JSON by replacing the arguments content with empty string
            let cleaned_json = format!(
                "{}{}{}",
                &json_str[..content_start],  // everything up to the content
                "",                          // empty string for arguments
                &json_str[content_end..]     // everything after the content
            );
            
            (Some(content), cleaned_json)
        } else {
            (None, json_str.to_string())
        }
    } else {
        (None, json_str.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_openai_stream_chunk_basic_content() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
        };
        let tools = None;
        let ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;

        // Test basic content streaming
        buffer.push_str("data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &ws_manager,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(response_text, "Hello ");

        buffer.push_str("data: {\"choices\":[{\"delta\":{\"content\":\"world!\"}}]}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &ws_manager,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(response_text, "Hello world!");
    }

    #[tokio::test]
    async fn test_parse_openai_stream_chunk_function_call() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
        };
        let tools = Some(vec![serde_json::json!({
            "name": "test_function",
            "tool_router_key": "test_router"
        })]);
        let ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;

        buffer.push_str("data: {\"choices\":[{\"delta\":{\"function_call\":{\"name\":\"test_function\"}}}]}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &ws_manager,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(partial_fc.name, Some("test_function".to_string()));

        buffer.push_str("data: {\"choices\":[{\"delta\":{\"function_call\":{\"arguments\":\"{\\\"arg\\\":\"}}}]}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &ws_manager,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());

        buffer.push_str("data: {\"choices\":[{\"delta\":{\"function_call\":{\"arguments\":\"\\\"value\\\"}\"}}}, {\"finish_reason\":\"function_call\"}]}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &ws_manager,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(function_calls.len(), 1);
        assert_eq!(function_calls[0].name, "test_function");
        assert_eq!(function_calls[0].tool_router_key, Some("test_router".to_string()));
    }

    #[tokio::test]
    async fn test_parse_openai_stream_chunk_error_handling() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
        };
        let tools = None;

        // Test error response
        buffer.push_str(
            "data: {\"error\":{\"code\":\"rate_limit_exceeded\",\"message\":\"You exceeded your current quota\"}}\n",
        );
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_parse_openai_stream_complete_response() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
        };
        let tools = None;

        // Initial message with role
        buffer.push_str("data: {\"id\":\"chatcmpl-AqTyN7bHxp10cCuIMNoPv9DuT4v0L\",\"object\":\"chat.completion.chunk\",\"created\":1737071635,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_72ed7ab54c\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\",\"refusal\":null},\"logprobs\":null,\"finish_reason\":null}]}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());

        // Content chunks
        let content_chunks = vec![
            "Yes", ",", " I'm", " here", "!", " How", " can", " I", " assist", " you", " today", "?",
        ];

        for chunk in content_chunks {
            buffer.push_str(&format!("data: {{\"id\":\"chatcmpl-AqTyN7bHxp10cCuIMNoPv9DuT4v0L\",\"object\":\"chat.completion.chunk\",\"created\":1737071635,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_72ed7ab54c\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{}\"}},\"logprobs\":null,\"finish_reason\":null}}]}}\n", chunk));
            let result = parse_openai_stream_chunk(
                &mut buffer,
                &mut response_text,
                &mut function_calls,
                &mut partial_fc,
                &tools,
                &None,
                None,
                "session_id",
            )
            .await;
            assert!(result.is_ok());
        }

        // Empty delta with finish_reason
        buffer.push_str("data: {\"id\":\"chatcmpl-AqTyN7bHxp10cCuIMNoPv9DuT4v0L\",\"object\":\"chat.completion.chunk\",\"created\":1737071635,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_72ed7ab54c\",\"choices\":[{\"index\":0,\"delta\":{},\"logprobs\":null,\"finish_reason\":\"stop\"}]}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());

        // [DONE] message
        buffer.push_str("data: [DONE]\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());

        // Verify final response text
        assert_eq!(response_text, "Yes, I'm here! How can I assist you today?");
        assert!(function_calls.is_empty());
    }

    #[tokio::test]
    async fn test_parse_gemini_openai_compatibility_stream() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
        };
        let tools = None;

        // First chunk with initial content
        buffer.push_str("data: {\"choices\":[{\"delta\":{\"content\":\"Yes\",\"role\":\"assistant\"},\"index\":0}],\"created\":1736746675,\"model\":\"gemini-1.5-flash\",\"object\":\"chat.completion.chunk\"}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(response_text, "Yes");

        // Second chunk with middle content
        buffer.push_str("data: {\"choices\":[{\"delta\":{\"content\":\", I'm here and ready to assist you.  How can I help\",\"role\":\"assistant\"},\"index\":0}],\"created\":1736746675,\"model\":\"gemini-1.5-flash\",\"object\":\"chat.completion.chunk\"}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());
        assert_eq!(response_text, "Yes, I'm here and ready to assist you.  How can I help");

        // Final chunk with finish_reason
        buffer.push_str("data: {\"choices\":[{\"delta\":{\"content\":\"?\\n\",\"role\":\"assistant\"},\"finish_reason\":\"stop\",\"index\":0}],\"created\":1736746675,\"model\":\"gemini-1.5-flash\",\"object\":\"chat.completion.chunk\"}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());

        // [DONE] message
        buffer.push_str("data: [DONE]\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());

        // Verify final response text
        assert_eq!(
            response_text,
            "Yes, I'm here and ready to assist you.  How can I help?\n"
        );
        assert!(function_calls.is_empty());
    }

    #[tokio::test]
    async fn test_parse_gemini_openai_compatibility_non_stream() {
        // Create test response JSON
        let response_json = r#"{
            "choices": [
                {
                    "finish_reason": "stop",
                    "index": 0,
                    "message": {
                        "content": "Yes, I'm here and ready to assist you.  How can I help?\n",
                        "role": "assistant"
                    }
                }
            ],
            "created": 1737072854,
            "model": "gemini-1.5-flash",
            "object": "chat.completion",
            "usage": {
                "completion_tokens": 19,
                "prompt_tokens": 45,
                "total_tokens": 64
            }
        }"#;

        // Parse response
        let data: OpenAIResponse = serde_json::from_str(response_json).unwrap();

        // Verify the parsed data
        assert_eq!(data.choices.len(), 1);
        let choice = &data.choices[0];
        assert_eq!(choice.finish_reason.clone().unwrap(), "stop");
        assert_eq!(choice.index, 0);

        match &choice.message.content {
            Some(MessageContent::Text(text)) => {
                assert_eq!(text, "Yes, I'm here and ready to assist you.  How can I help?\n");
            }
            _ => panic!("Expected text content"),
        }

        assert_eq!(choice.message.role, "assistant");
        assert!(choice.message.function_call.is_none());
    }

    #[tokio::test]
    async fn test_parse_openai_stream_chunk_array_error() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
        };
        let tools = None;

        // Test array-formatted error response
        buffer.push_str("[{\"error\":{\"code\":400,\"message\":\"Unable to submit request because it has an empty text parameter. Add a value to the parameter and try again. Learn more: https://cloud.google.com/vertex-ai/generative-ai/docs/model-reference/gemini\",\"status\":\"INVALID_ARGUMENT\"}}]\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;

        assert!(result.is_ok());
        let error = result.unwrap();
        assert!(error.is_some());
        assert_eq!(
            error.unwrap(),
            "400: Unable to submit request because it has an empty text parameter. Add a value to the parameter and try again. Learn more: https://cloud.google.com/vertex-ai/generative-ai/docs/model-reference/gemini"
        );
        assert!(response_text.is_empty());
        assert!(function_calls.is_empty());
    }

    #[tokio::test]
    async fn test_parse_openai_stream_chunk_riddle_response() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
        };
        let tools = None;

        // First chunk with split system_fingerprint
        buffer.push_str("data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"syste");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());

        // Second chunk completing the initial message
        buffer.push_str("m_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"\",\"refusal\":null},\"logprobs\":null,\"finish_reason\":null}]}\n\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &None,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());
        assert!(response_text.is_empty());

        // Add each chunk exactly as it appeared in the log
        let chunks = vec![
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Why\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" did\"},\"logprobs\":null,\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" the\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" scare\"},\"logprobs\":null,\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"crow\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" win\"},\"logprobs\":null,\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" an\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" award\"},\"logprobs\":null,\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"?\\n\\n\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Because\"},\"logprobs\":null,\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" he\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" was\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" outstanding\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" in\"},\"logprobs\":null,\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" his\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" field\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"!\"},\"logprobs\":null,\"finish_reason\":null}]}\n",
            "data: {\"id\":\"chatcmpl-AqUiMlZBEj4bSQKmwhXPjGR0HStpQ\",\"object\":\"chat.completion.chunk\",\"created\":1737074486,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_bd83329f63\",\"choices\":[{\"index\":0,\"delta\":{},\"logprobs\":null,\"finish_reason\":\"stop\"}]}\n",
            "data: [DONE]\n"
        ];

        for chunk in chunks {
            buffer.push_str(chunk);
            let result = parse_openai_stream_chunk(
                &mut buffer,
                &mut response_text,
                &mut function_calls,
                &mut partial_fc,
                &tools,
                &None,
                None,
                "session_id",
            )
            .await;
            assert!(result.is_ok());
        }

        // Verify final response text
        assert_eq!(
            response_text,
            "Why did the scarecrow win an award?\n\nBecause he was outstanding in his field!"
        );
        assert!(function_calls.is_empty());
    }

    #[tokio::test]
    async fn test_parse_openai_stream_chunk_smtp_function_call() {
        let mut buffer = String::new();
        let mut response_text = String::new();
        let mut function_calls = Vec::new();
        let mut partial_fc = PartialFunctionCall {
            name: None,
            arguments: String::new(),
            is_accumulating: false,
        };
        let tools = Some(vec![serde_json::json!({
            "name": "shinkai_tool_config_updater",
            "tool_router_key": "test_router"
        })]);
        let ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;

        // Initial message with role and function call name
        buffer.push_str("data: {\"id\":\"chatcmpl-ApllfOJ8EuDsd9Qe6J1j3EMGPhywA\",\"object\":\"chat.completion.chunk\",\"created\":1736901711,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_72ed7ab54c\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":null,\"function_call\":{\"name\":\"shinkai_tool_config_updater\",\"arguments\":\"\"},\"refusal\":null},\"logprobs\":null,\"finish_reason\":null}]}\n");

        // Function call argument chunks
        let argument_chunks = vec![
            "{\"", "tool", "_router", "_key", "\":\"", "local", "::", "none", "\",\"",
            "config", "\":{\"", "smtp", "_server", "\":\"", "smtp", ".", "zo", "ho", ".com",
            "\",\"", "port", "\":", "465", ",\"", "sender", "_email", "\":\"", "bat", "ata",
            "@", "z", "oh", "om", "ail", ".com", "\",\"", "sender", "_password", "\":\"",
            "ber", "emu", "\",\"", "ssl", "\":", "true", "}}"
        ];

        for chunk in argument_chunks {
            buffer.push_str(&format!("data: {{\"id\":\"chatcmpl-ApllfOJ8EuDsd9Qe6J1j3EMGPhywA\",\"object\":\"chat.completion.chunk\",\"created\":1736901711,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_72ed7ab54c\",\"choices\":[{{\"index\":0,\"delta\":{{\"function_call\":{{\"arguments\":\"{}\"}}}},\"logprobs\":null,\"finish_reason\":null}}]}}\n", chunk));
            let result = parse_openai_stream_chunk(
                &mut buffer,
                &mut response_text,
                &mut function_calls,
                &mut partial_fc,
                &tools,
                &ws_manager,
                None,
                "session_id",
            )
            .await;
            assert!(result.is_ok());
        }

        // Final message with finish_reason
        buffer.push_str("data: {\"id\":\"chatcmpl-ApllfOJ8EuDsd9Qe6J1j3EMGPhywA\",\"object\":\"chat.completion.chunk\",\"created\":1736901711,\"model\":\"gpt-4o-mini-2024-07-18\",\"service_tier\":\"default\",\"system_fingerprint\":\"fp_72ed7ab54c\",\"choices\":[{\"index\":0,\"delta\":{},\"logprobs\":null,\"finish_reason\":\"function_call\"}]}\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &ws_manager,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());

        // [DONE] message
        buffer.push_str("data: [DONE]\n");
        let result = parse_openai_stream_chunk(
            &mut buffer,
            &mut response_text,
            &mut function_calls,
            &mut partial_fc,
            &tools,
            &ws_manager,
            None,
            "session_id",
        )
        .await;
        assert!(result.is_ok());
        eprintln!("Result: {:?}", result);

        // Verify the function call
        assert_eq!(function_calls.len(), 1);
        let fc = &function_calls[0];
        assert_eq!(fc.name, "shinkai_tool_config_updater");
        assert_eq!(fc.tool_router_key, Some("test_router".to_string()));

        // Verify the arguments
        let expected_args = serde_json::json!({
            "tool_router_key": "local::none",
            "config": {
                "smtp_server": "smtp.zoho.com",
                "port": 465,
                "sender_email": "batata@zohomail.com",
                "sender_password": "beremu",
                "ssl": true
            }
        });
        assert_eq!(serde_json::to_value(&fc.arguments).unwrap(), expected_args);
    }

    #[test]
    fn test_extraction_stream_chunk() {
        let json_str = r#"{"id":"chatcmpl-ApllfOJ8EuDsd9Qe6J1j3EMGPhywA","object":"chat.completion.chunk","created":1736901711,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_72ed7ab54c","choices":[{"index":0,"delta":{"function_call":{"arguments":"{\""}},"logprobs":null,"finish_reason":null}]}"#;

        let (maybe_args, cleaned) = extract_and_remove_arguments(json_str);

        // Check we extracted the inner content - in this case just the start of a JSON object
        assert_eq!(maybe_args, Some("{\\\"".to_string()));  // This is what's actually in the JSON

        // The cleaned JSON should have empty arguments but maintain structure
        assert!(cleaned.contains(r#""function_call""#));
        assert!(cleaned.contains(r#""arguments""#));
        assert!(cleaned.contains(r#""""#)); // Empty string

        // The rest of the structure should be unchanged
        assert!(cleaned.contains(r#""id":"chatcmpl-ApllfOJ8EuDsd9Qe6J1j3EMGPhywA""#));
        assert!(cleaned.contains(r#""object":"chat.completion.chunk""#));
        assert!(cleaned.contains(r#""model":"gpt-4o-mini-2024-07-18""#));

        eprintln!("Cleaned: {}", cleaned);

        // Verify the cleaned JSON is valid and has the expected structure
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(parsed["choices"][0]["delta"]["function_call"]["arguments"].as_str().unwrap(), "");
    }

    #[test]
    fn test_extraction_stream_chunk_colon() {
        // Test case 1: Normal colon in arguments
        let json_str = r#"{"id":"chatcmpl-ApllfOJ8EuDsd9Qe6J1j3EMGPhywA","object":"chat.completion.chunk","created":1736901711,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_72ed7ab54c","choices":[{"index":0,"delta":{"function_call":{"arguments":":"}},"logprobs":null,"finish_reason":null}]}"#;

        eprintln!("\nTesting case 1 - Colon as argument");
        let (maybe_args, cleaned) = extract_and_remove_arguments(json_str);

        // Check we extracted the inner content - in this case just a colon
        eprintln!("Extracted arguments: {:?}", maybe_args);
        assert_eq!(maybe_args, Some(":".to_string()));

        // The cleaned JSON should have empty arguments but maintain structure
        eprintln!("Checking cleaned JSON structure...");
        assert!(cleaned.contains(r#""function_call""#));
        assert!(cleaned.contains(r#""arguments""#));
        assert!(cleaned.contains(r#""""#)); // Empty string

        // The rest of the structure should be unchanged
        assert!(cleaned.contains(r#""id":"chatcmpl-ApllfOJ8EuDsd9Qe6J1j3EMGPhywA""#));
        assert!(cleaned.contains(r#""object":"chat.completion.chunk""#));
        assert!(cleaned.contains(r#""model":"gpt-4o-mini-2024-07-18""#));

        // Verify the cleaned JSON is valid and has the expected structure
        eprintln!("Attempting to parse cleaned JSON...");
        match serde_json::from_str::<serde_json::Value>(&cleaned) {
            Ok(parsed) => {
                eprintln!("Successfully parsed JSON");
                assert_eq!(parsed["choices"][0]["delta"]["function_call"]["arguments"].as_str().unwrap(), "");
            }
            Err(e) => {
                eprintln!("Failed to parse JSON: {}", e);
                eprintln!("Cleaned JSON was: {}", cleaned);
                panic!("JSON parsing failed: {}", e);
            }
        }

        // Test case 2: Empty arguments with trailing comma
        eprintln!("\nTesting case 2 - Empty arguments with trailing comma");
        let empty_args_json = r#"{"id":"chatcmpl-ApllfOJ8EuDsd9Qe6J1j3EMGPhywA","object":"chat.completion.chunk","created":1736901711,"model":"gpt-4o-mini-2024-07-18","service_tier":"default","system_fingerprint":"fp_72ed7ab54c","choices":[{"index":0,"delta":{"function_call":{"arguments":"",""}},"logprobs":null,"finish_reason":null}]}"#;

        let (maybe_args, cleaned) = extract_and_remove_arguments(empty_args_json);

        // Check we extracted the inner content - in this case an empty string
        eprintln!("Extracted arguments: {:?}", maybe_args);
        assert_eq!(maybe_args, Some("\",\"".to_string()));

        // The cleaned JSON should have empty arguments but maintain structure
        eprintln!("Checking cleaned JSON structure...");
        assert!(cleaned.contains(r#""function_call""#));
        assert!(cleaned.contains(r#""arguments""#));
        assert!(cleaned.contains(r#""""#)); // Empty string

        // The rest of the structure should be unchanged
        assert!(cleaned.contains(r#""id":"chatcmpl-ApllfOJ8EuDsd9Qe6J1j3EMGPhywA""#));
        assert!(cleaned.contains(r#""object":"chat.completion.chunk""#));
        assert!(cleaned.contains(r#""model":"gpt-4o-mini-2024-07-18""#));

        // Verify the cleaned JSON is valid and has the expected structure
        eprintln!("Attempting to parse cleaned JSON...");
        match serde_json::from_str::<serde_json::Value>(&cleaned) {
            Ok(parsed) => {
                eprintln!("Successfully parsed JSON");
                assert_eq!(parsed["choices"][0]["delta"]["function_call"]["arguments"].as_str().unwrap(), "");
            }
            Err(e) => {
                eprintln!("Failed to parse JSON: {}", e);
                eprintln!("Cleaned JSON was: {}", cleaned);
                panic!("JSON parsing failed: {}", e);
            }
        }
    }
}

