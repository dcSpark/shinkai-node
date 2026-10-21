use std::error::Error;
use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::openai_api::openai_prepare_messages;
use super::shared::shared_model_logic::{send_tool_ws_update, send_ws_update};
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, LLMInferenceResponse};
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenRouter};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use uuid::Uuid;

fn truncate_image_url_in_payload(payload: &mut JsonValue) {
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

#[async_trait]
impl LLMService for OpenRouter {
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
                let url = format!("{}{}", base_url, "/api/v1/chat/completions");

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

                let mut payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "max_tokens": result.remaining_output_tokens,
                    "stream": is_stream
                });

                // Conditionally add tools to the payload if tools_json is not empty
                if !tools_json.is_empty() {
                    payload["tools"] = serde_json::Value::Array(tools_json.clone());
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
                        Some(tools_json), // Add tools parameter
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

async fn handle_streaming_response(
    client: &Client,
    url: String,
    payload: JsonValue,
    api_key: String,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    llm_stopper: Arc<LLMStopper>,
    session_id: String,
    tools: Option<Vec<JsonValue>>, // Add tools parameter
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let res = client
        .post(url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .header("X-Title", "Shinkai")
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
    let mut previous_json_chunk: String = String::new();
    let mut function_calls: Vec<FunctionCall> = Vec::new();
    let mut state = OpenRouterResponseState::default();
    let mut is_done_sent = false; // Track if any WS message with is_done: true has been sent

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

                return Ok(LLMInferenceResponse::new(
                    response_text,
                    None,
                    json!({}),
                    Vec::new(),
                    Vec::new(),
                    None,
                ));
            }
        }

        match item {
            Ok(chunk) => {
                let chunk_str = String::from_utf8_lossy(&chunk).to_string();
                eprintln!("Chunk: {}", chunk_str);
                previous_json_chunk += chunk_str.as_str();

                // Process any complete SSE events in the accumulated buffer
                let mut remaining = previous_json_chunk.as_str();
                let mut processed_any = false;

                while let Some(event_end) = remaining.find("\n\n") {
                    let event_chunk = &remaining[..event_end];
                    remaining = &remaining[event_end + 2..];
                    processed_any = true;

                    // Parse SSE format: look for "data: " prefix
                    for line in event_chunk.lines() {
                        let line = line.trim();
                        if line.starts_with("data: ") {
                            let json_str = &line[6..]; // Remove "data: " prefix
                            if json_str == "[DONE]" {
                                // End of stream
                                continue;
                            }

                            let data_resp: Result<JsonValue, _> = serde_json::from_str(json_str);
                            match data_resp {
                                Ok(data) => {
                                    let new_content = process_streaming_chunk(
                                        &data,
                                        &mut response_text,
                                        &mut function_calls,
                                        &tools,
                                        &mut state,
                                    );

                                    // Check for finish_reason to determine if stream is done
                                    let is_finished = data
                                        .get("choices")
                                        .and_then(|choices| choices.as_array())
                                        .map(|choices_array| {
                                            choices_array.iter().any(|choice| {
                                                choice
                                                    .get("finish_reason")
                                                    .and_then(|fr| fr.as_str())
                                                    .map(|fr| !fr.is_empty())
                                                    .unwrap_or(false)
                                            })
                                        })
                                        .unwrap_or(false);

                                    // Send WebSocket update with incremental content (like OpenAI)
                                    let _ = send_ws_update(
                                        &ws_manager_trait,
                                        inbox_name.clone(),
                                        &session_id,
                                        new_content, // Send only new content, not entire response
                                        false,       // is_reasoning
                                        is_finished, // is_done
                                        None,        // done_reason
                                    )
                                    .await;

                                    if is_finished {
                                        is_done_sent = true;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error parsing JSON in SSE data: {:?}", e);
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        format!("Error parsing JSON in SSE data: {:?}", e).as_str(),
                                    );
                                }
                            }
                        }
                    }
                }

                // Update the buffer with any remaining incomplete data
                if processed_any {
                    previous_json_chunk = remaining.to_string();
                }

                // Fallback: if no SSE format detected, try parsing as delta JSON directly
                if !processed_any && !previous_json_chunk.trim().is_empty() {
                    let trimmed_chunk_str = previous_json_chunk.trim().to_string();
                    let data_resp: Result<JsonValue, _> = serde_json::from_str(&trimmed_chunk_str);
                    match data_resp {
                        Ok(data) => {
                            previous_json_chunk = "".to_string();
                            let new_content = process_streaming_chunk(
                                &data,
                                &mut response_text,
                                &mut function_calls,
                                &tools,
                                &mut state,
                            );

                            // Check for finish_reason to determine if stream is done
                            let is_finished = data
                                .get("choices")
                                .and_then(|choices| choices.as_array())
                                .map(|choices_array| {
                                    choices_array.iter().any(|choice| {
                                        choice
                                            .get("finish_reason")
                                            .and_then(|fr| fr.as_str())
                                            .map(|fr| !fr.is_empty())
                                            .unwrap_or(false)
                                    })
                                })
                                .unwrap_or(false);

                            // Send WebSocket update with incremental content
                            let _ = send_ws_update(
                                &ws_manager_trait,
                                inbox_name.clone(),
                                &session_id,
                                new_content, // Send only new content, not entire response
                                false,       // is_reasoning
                                is_finished, // is_done
                                None,        // done_reason
                            )
                            .await;

                            if is_finished {
                                is_done_sent = true;
                            }
                        }
                        Err(_e) => {
                            eprintln!("Error while receiving chunk: {:?}", _e);
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                format!("Error while receiving chunk: {:?}", _e).as_str(),
                            );
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

    // If no WS message with is_done: true was sent, send a final message
    if !is_done_sent {
        let _ = send_ws_update(
            &ws_manager_trait,
            inbox_name.clone(),
            &session_id,
            "".to_string(), // Empty content
            false,          // is_reasoning
            true,           // is_done
            None,           // done_reason
        )
        .await;
    }

    state.attach_reasoning(&mut function_calls);

    Ok(LLMInferenceResponse::new(
        response_text,
        None,
        json!({}),
        function_calls,
        Vec::new(),
        None,
    ))
}

async fn handle_non_streaming_response(
    client: &Client,
    url: String,
    payload: JsonValue,
    api_key: String,
    inbox_name: Option<InboxName>,
    llm_stopper: Arc<LLMStopper>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    tools: Option<Vec<JsonValue>>, // Add tools parameter
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    let response_fut = client
        .post(url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .header("X-Title", "Shinkai")
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

                        return Ok(LLMInferenceResponse::new("".to_string(), None, json!({}), Vec::new(), Vec::new(), None));
                    }
                }
            },
            response = &mut response_fut => {
                let res = response?;
                let response_text = res.text().await?;
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

                        let mut response_string = String::new();
                        let mut function_calls = Vec::new();
                        let mut state = OpenRouterResponseState::default();
                        process_streaming_chunk(&value, &mut response_string, &mut function_calls, &tools, &mut state);
                        state.attach_reasoning(&mut function_calls);
                        for function_call in &function_calls {
                            let _ = send_tool_ws_update(&ws_manager_trait, inbox_name.clone(), function_call).await;
                        }
                        return Ok(LLMInferenceResponse::new(
                            response_string, None, json!({}), function_calls, Vec::new(), None,
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

#[derive(Default)]
struct OpenRouterResponseState {
    reasoning_details: Vec<JsonValue>,
    content: String,
    arguments: std::collections::HashMap<u64, JsonValue>,
}

impl OpenRouterResponseState {
    fn update_call_arguments(&self, call: &mut FunctionCall) {
        let arguments = match self.arguments.get(&call.index) {
            Some(JsonValue::String(arguments)) => serde_json::from_str::<JsonValue>(arguments)
                .map_err(|error| format!("Tool arguments are not valid JSON: {error}")),
            Some(_) => Err("Tool arguments must be a JSON-encoded object string".to_string()),
            None => Err("Tool arguments are missing".to_string()),
        }
        .and_then(|arguments| {
            arguments
                .as_object()
                .cloned()
                .ok_or_else(|| "Tool arguments must be a JSON object".to_string())
        });
        let mut metadata = call
            .provider_metadata
            .take()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        match arguments {
            Ok(arguments) => {
                call.arguments = arguments;
                metadata.remove("function_argument_error");
            }
            Err(error) => {
                // A valid prefix must not remain executable after a corrupt suffix.
                call.arguments.clear();
                metadata.insert("function_argument_error".into(), json!(error));
            }
        }
        call.provider_metadata = if metadata.is_empty() {
            None
        } else {
            Some(JsonValue::Object(metadata))
        };
    }

    fn attach_reasoning(&self, calls: &mut [FunctionCall]) {
        if calls.is_empty() {
            return;
        }
        for call in calls.iter_mut() {
            self.update_call_arguments(call);
        }
        let native_calls: Vec<_> = calls
            .iter()
            .map(|call| {
                let mut function = json!({"name": call.name});
                if let Some(arguments) = self.arguments.get(&call.index) {
                    function["arguments"] = arguments.clone();
                }
                json!({"id": call.id, "type": call.call_type.as_deref().unwrap_or("function"), "function": function})
            })
            .collect();
        let mut assistant = json!({"role":"assistant", "content": if self.content.is_empty() { JsonValue::Null } else { json!(self.content) }, "tool_calls": native_calls});
        if !self.reasoning_details.is_empty() {
            assistant["reasoning_details"] = json!(self.reasoning_details);
        }
        for call in calls {
            let metadata = call.provider_metadata.get_or_insert_with(|| json!({}));
            metadata["reasoning_details"] = json!(self.reasoning_details);
            metadata["openrouter_assistant"] = assistant.clone();
        }
    }
}

fn process_streaming_chunk(
    data: &JsonValue,
    response_text: &mut String,
    function_calls: &mut Vec<FunctionCall>,
    tools: &Option<Vec<JsonValue>>,
    state: &mut OpenRouterResponseState,
) -> String {
    let mut new_content = String::new();
    // A request selects one completion. Do not mix reasoning from other choices.
    if let Some(choice) = data.get("choices").and_then(|c| c.as_array()).and_then(|c| c.first()) {
        if let Some(message) = choice.get("delta").or_else(|| choice.get("message")) {
            if let Some(content) = message.get("content").and_then(|c| c.as_str()) {
                state.content.push_str(content);
                response_text.push_str(content);
                new_content.push_str(content);
            }
            // OpenRouter requires the complete sequence, unchanged and in arrival order.
            // https://openrouter.ai/docs/guides/best-practices/reasoning-tokens
            if let Some(details) = message.get("reasoning_details").and_then(|r| r.as_array()) {
                state.reasoning_details.extend(details.iter().cloned());
            }
            if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
                for (position, tool_call) in tool_calls.iter().enumerate() {
                    let index = tool_call
                        .get("index")
                        .and_then(|i| i.as_u64())
                        .unwrap_or(position as u64);
                    let function = tool_call.get("function");
                    let call_position = function_calls.iter().position(|c| c.index == index).unwrap_or_else(|| {
                        function_calls.push(FunctionCall {
                            name: String::new(),
                            arguments: serde_json::Map::new(),
                            tool_router_key: None,
                            response: None,
                            index,
                            id: None,
                            call_type: Some("function".into()),
                            provider_metadata: None,
                        });
                        function_calls.len() - 1
                    });
                    let call = &mut function_calls[call_position];
                    if let Some(id) = tool_call.get("id").and_then(|id| id.as_str()) {
                        call.id = Some(id.into());
                    }
                    if let Some(kind) = tool_call.get("type").and_then(|t| t.as_str()) {
                        call.call_type = Some(kind.into());
                    }
                    if let Some(name) = function.and_then(|f| f.get("name")).and_then(|n| n.as_str()) {
                        call.name.push_str(name);
                        call.tool_router_key = tools.as_ref().and_then(|tools| {
                            tools.iter().find_map(|tool| {
                                let definition = tool.get("function").unwrap_or(tool);
                                if definition.get("name").and_then(|n| n.as_str()) == Some(call.name.as_str()) {
                                    definition
                                        .get("tool_router_key")
                                        .or_else(|| tool.get("tool_router_key"))
                                        .and_then(|key| key.as_str())
                                        .map(str::to_owned)
                                } else {
                                    None
                                }
                            })
                        });
                    }
                    if let Some(arguments) = function.and_then(|f| f.get("arguments")) {
                        if let Some(fragment) = arguments.as_str() {
                            let buffer = state.arguments.entry(index).or_insert_with(|| json!(""));
                            // A non-string chunk is invalid, even if a later string is valid JSON.
                            if let JsonValue::String(buffer) = buffer {
                                buffer.push_str(fragment);
                            }
                        } else {
                            state.arguments.insert(index, arguments.clone());
                        }
                    }
                    state.update_call_arguments(call);
                }
            }
        }
    }
    new_content
}

// Exact catalog entries keep new preset rules from changing custom or legacy models.
// Source: https://openrouter.ai/api/v1/models (2026-10-21).
fn preset_options(model: &str) -> Option<(bool, &'static [&'static str], &'static [&'static str])> {
    let model = model.strip_suffix(":batch").unwrap_or(model);
    const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];
    const OPENAI: &[&str] = &["seed", "max_completion_tokens", "response_format", "verbosity"];
    match model {
        "openai/gpt-6-astra" | "openai/gpt-6-astra-pro" | "openai/gpt-6.1-sol" | "openai/gpt-6.1-sol-pro" => {
            Some((true, EFFORTS, OPENAI))
        }
        "openai/gpt-6-luna" | "openai/gpt-6-luna-pro" | "openai/gpt-6-sol" | "openai/gpt-6-sol-pro" => {
            Some((false, &["none", "low", "medium", "high", "xhigh", "max"], OPENAI))
        }
        "anthropic/claude-sonnet-5.5" => Some((
            true,
            EFFORTS,
            &["max_completion_tokens", "response_format", "stop", "verbosity"],
        )),
        "google/gemini-4-argon" => Some((
            true,
            &["low", "medium", "high"],
            &["seed", "temperature", "top_p", "response_format", "stop"],
        )),
        "deepseek/deepseek-v4.1-flash-1017" => Some((
            false,
            &["none", "low", "high", "max"],
            &[
                "seed",
                "temperature",
                "top_p",
                "max_completion_tokens",
                "frequency_penalty",
                "logit_bias",
                "logprobs",
                "top_logprobs",
                "presence_penalty",
                "response_format",
                "stop",
                "parallel_tool_calls",
            ],
        )),
        _ => None,
    }
}

fn add_options_to_payload(payload: &mut serde_json::Value, config: Option<&JobConfig>) {
    fn read_env_var<T: std::str::FromStr>(key: &str) -> Option<T> {
        std::env::var(key).ok().and_then(|val| val.parse::<T>().ok())
    }

    fn get_value<T: Clone + std::str::FromStr>(env_key: &str, config_value: Option<&T>) -> Option<T> {
        config_value.cloned().or_else(|| read_env_var::<T>(env_key))
    }

    let options = payload
        .get("model")
        .and_then(|model| model.as_str())
        .and_then(preset_options);
    let is_batch = payload["model"].as_str().is_some_and(|model| model.ends_with(":batch"));
    let supports = |key: &str| {
        options.map_or(true, |(_, _, parameters)| {
            parameters.contains(&key) && !(is_batch && key == "max_completion_tokens")
        })
    };
    if supports("seed") {
        if let Some(seed) = get_value("LLM_SEED", config.and_then(|c| c.seed.as_ref())) {
            payload["seed"] = json!(seed);
        }
    }
    if supports("temperature") {
        if let Some(temp) = get_value("LLM_TEMPERATURE", config.and_then(|c| c.temperature.as_ref())) {
            payload["temperature"] = json!(temp);
        }
    }
    if supports("top_p") {
        if let Some(top_p) = get_value("LLM_TOP_P", config.and_then(|c| c.top_p.as_ref())) {
            payload["top_p"] = json!(top_p);
        }
    }
    if let Some(max_tokens) = get_value("LLM_MAX_TOKENS", config.and_then(|c| c.max_tokens.as_ref())) {
        let key = if supports("max_completion_tokens") {
            "max_completion_tokens"
        } else {
            "max_tokens"
        };
        payload[key] = json!(max_tokens);
    }

    if let Some(obj) = config
        .and_then(|c| c.other_model_params.as_ref())
        .and_then(|params| params.as_object())
    {
        for (key, value) in obj {
            match key.as_str() {
                "frequency_penalty"
                | "logit_bias"
                | "logprobs"
                | "top_logprobs"
                | "max_completion_tokens"
                | "n"
                | "presence_penalty"
                | "response_format"
                | "service_tier"
                | "stop"
                | "stream_options"
                | "parallel_tool_calls"
                    if supports(key) =>
                {
                    payload[key] = value.clone()
                }
                // Keep the complete custom object, including provider-specific reasoning fields.
                "reasoning" if value.is_object() => payload[key] = value.clone(),
                _ => (),
            }
        }
    }

    let thinking = config.and_then(|c| c.thinking);
    let effort = config.and_then(|c| c.reasoning_effort.as_deref());
    if thinking.is_none() && effort.is_none() && payload.get("reasoning").is_none() {
        return;
    }
    if !payload.get("reasoning").is_some_and(JsonValue::is_object) {
        payload["reasoning"] = json!({});
    }
    let reasoning = payload["reasoning"].as_object_mut().unwrap();
    if let Some(enabled) = thinking {
        reasoning.insert("enabled".into(), json!(enabled));
    }
    if let Some(effort) = effort {
        // An explicit effort replaces a custom token budget. The API accepts one of these.
        reasoning.remove("max_tokens");
        reasoning.insert("effort".into(), json!(effort));
    }
    if let Some((mandatory, efforts, _)) = options {
        if mandatory && reasoning.get("enabled") == Some(&json!(false)) {
            reasoning.insert("enabled".into(), json!(true));
        }
        if reasoning
            .get("effort")
            .is_some_and(|effort| !effort.as_str().is_some_and(|effort| efforts.contains(&effort)))
        {
            reasoning.remove("effort");
        }
    }
    if reasoning.get("enabled") == Some(&json!(false)) {
        reasoning.remove("effort");
        reasoning.remove("max_tokens");
    } else if thinking == Some(true) && reasoning.get("effort") == Some(&json!("none")) {
        reasoning.remove("effort");
    }
}

#[cfg(test)]
mod option_tests {
    use super::*;

    #[test]
    fn typed_reasoning_options_use_openrouter_shape() {
        for model in ["openai/gpt-6-astra", "openai/gpt-6.1-sol", "openai/gpt-6-luna"] {
            for effort in ["low", "medium", "high", "xhigh", "max"] {
                let mut payload = json!({"model": model});
                let mut config = JobConfig::empty();
                config.thinking = Some(true);
                config.reasoning_effort = Some(effort.into());
                add_options_to_payload(&mut payload, Some(&config));
                assert_eq!(payload["reasoning"], json!({"enabled": true, "effort": effort}));
                assert!(payload.get("thinking").is_none());
                assert!(payload.get("reasoning_effort").is_none());
            }
        }
    }

    #[test]
    fn mandatory_presets_cannot_disable_reasoning() {
        for model in [
            "openai/gpt-6-astra",
            "openai/gpt-6.1-sol",
            "anthropic/claude-sonnet-5.5",
            "google/gemini-4-argon",
        ] {
            let mut payload = json!({"model": model});
            let mut config = JobConfig::empty();
            config.thinking = Some(false);
            config.reasoning_effort = Some("none".into());
            add_options_to_payload(&mut payload, Some(&config));
            assert_eq!(payload["reasoning"], json!({"enabled": true}));
        }
    }

    #[test]
    fn optional_reasoning_can_be_disabled_without_a_conflicting_effort() {
        for model in [
            "openai/gpt-6-luna",
            "openai/gpt-6-sol",
            "deepseek/deepseek-v4.1-flash-1017",
        ] {
            let mut payload = json!({"model": model});
            let mut config = JobConfig::empty();
            config.thinking = Some(false);
            config.reasoning_effort = Some("high".into());
            config.other_model_params = Some(json!({"reasoning": {"max_tokens": 2048, "exclude": true}}));
            add_options_to_payload(&mut payload, Some(&config));
            assert_eq!(payload["reasoning"], json!({"enabled": false, "exclude": true}));
        }
    }

    #[test]
    fn effort_validation_uses_each_preset_supported_values() {
        for (model, effort, supported) in [
            ("google/gemini-4-argon", "max", false),
            ("google/gemini-4-argon", "high", true),
            ("deepseek/deepseek-v4.1-flash-1017", "medium", false),
            ("deepseek/deepseek-v4.1-flash-1017", "max", true),
            ("openai/gpt-6-luna", "minimal", false),
            ("openai/gpt-6-luna", "none", true),
            ("openai/gpt-6-astra", "none", false),
        ] {
            let mut payload = json!({"model": model});
            let mut config = JobConfig::empty();
            config.reasoning_effort = Some(effort.into());
            add_options_to_payload(&mut payload, Some(&config));
            assert_eq!(
                payload["reasoning"].get("effort").is_some(),
                supported,
                "{model}: {effort}"
            );
        }
    }

    #[test]
    fn typed_reasoning_overrides_custom_fields_and_preserves_other_fields() {
        let mut payload = json!({"model": "custom/reasoner"});
        let mut config = JobConfig::empty();
        config.thinking = Some(true);
        config.reasoning_effort = Some("high".into());
        config.other_model_params = Some(json!({"reasoning": {
            "enabled": false, "effort": "low", "max_tokens": 2048, "exclude": true, "context": "all_turns"
        }}));
        add_options_to_payload(&mut payload, Some(&config));
        assert_eq!(
            payload["reasoning"],
            json!({
                "enabled": true, "effort": "high", "exclude": true, "context": "all_turns"
            })
        );
    }

    #[test]
    fn batch_presets_use_supported_reasoning_sampling_and_token_options() {
        for model in [
            "openai/gpt-6.1-sol:batch",
            "anthropic/claude-sonnet-5.5:batch",
            "google/gemini-4-argon:batch",
        ] {
            for thinking in [false, true] {
                let mut payload = json!({"model": model, "max_tokens": 4096});
                let mut config = JobConfig::empty();
                config.thinking = Some(thinking);
                config.reasoning_effort = Some(if thinking { "max" } else { "none" }.into());
                config.temperature = Some(0.4);
                config.top_p = Some(0.8);
                config.max_tokens = Some(1024);
                config.other_model_params = Some(json!({
                    "frequency_penalty": 0.2, "logprobs": true, "max_completion_tokens": 2048
                }));
                add_options_to_payload(&mut payload, Some(&config));
                assert_eq!(payload["model"], model);
                assert_eq!(payload["reasoning"]["enabled"], true);
                let gemini = model.starts_with("google/");
                if thinking && !gemini {
                    assert_eq!(payload["reasoning"]["effort"], "max");
                } else {
                    assert!(payload["reasoning"].get("effort").is_none());
                }
                assert_eq!(payload.get("temperature").is_some(), gemini);
                assert_eq!(payload.get("top_p").is_some(), gemini);
                assert!(payload.get("frequency_penalty").is_none());
                assert!(payload.get("logprobs").is_none());
                assert!(payload.get("max_completion_tokens").is_none());
                assert_eq!(payload["max_tokens"], 1024);
            }
        }
    }

    #[test]
    fn pro_routes_use_exact_catalog_reasoning_sampling_and_token_options() {
        for (base, mandatory) in [
            ("openai/gpt-6.1-sol-pro", true),
            ("openai/gpt-6-astra-pro", true),
            ("openai/gpt-6-luna-pro", false),
            ("openai/gpt-6-sol-pro", false),
        ] {
            for suffix in ["", ":batch"] {
                let model = format!("{base}{suffix}");
                for effort in ["none", "low", "medium", "high", "xhigh", "max", "minimal", "invalid"] {
                    let mut payload = json!({"model": model});
                    let mut config = JobConfig::empty();
                    config.reasoning_effort = Some(effort.into());
                    config.temperature = Some(0.4);
                    config.top_p = Some(0.8);
                    config.seed = Some(42);
                    config.max_tokens = Some(1024);
                    config.other_model_params = Some(json!({
                        "frequency_penalty": 0.2, "logprobs": true, "response_format": {"type": "json_object"}
                    }));
                    add_options_to_payload(&mut payload, Some(&config));
                    assert_eq!(payload["model"], model);
                    let supported = matches!(effort, "low" | "medium" | "high" | "xhigh" | "max")
                        || (!mandatory && effort == "none");
                    assert_eq!(
                        payload["reasoning"].get("effort").is_some(),
                        supported,
                        "{model}: {effort}"
                    );
                    if supported {
                        assert_eq!(payload["reasoning"]["effort"], effort);
                    }
                    for key in ["temperature", "top_p", "frequency_penalty", "logprobs"] {
                        assert!(payload.get(key).is_none(), "{model}: {key}");
                    }
                    assert_eq!(payload["seed"], 42);
                    assert_eq!(payload["response_format"], json!({"type": "json_object"}));
                    if suffix.is_empty() {
                        assert_eq!(payload["max_completion_tokens"], 1024);
                    } else {
                        assert_eq!(payload["max_tokens"], 1024);
                        assert!(payload.get("max_completion_tokens").is_none());
                    }
                }
                for custom in [false, true] {
                    let mut payload = json!({"model": model});
                    let mut config = JobConfig::empty();
                    if custom {
                        config.other_model_params =
                            Some(json!({"reasoning": {"enabled": false, "effort": "none", "exclude": true}}));
                    } else {
                        config.thinking = Some(false);
                        config.reasoning_effort = Some("none".into());
                    }
                    add_options_to_payload(&mut payload, Some(&config));
                    assert_eq!(payload["reasoning"]["enabled"], mandatory, "{model}");
                    assert!(payload["reasoning"].get("effort").is_none());
                    if custom {
                        assert_eq!(payload["reasoning"]["exclude"], true);
                    }
                }
            }
        }
    }

    #[test]
    fn other_variants_and_custom_batch_models_keep_custom_options() {
        for model in [
            "openai/gpt-6.1-sol:free",
            "anthropic/claude-sonnet-5.5:nitro",
            "google/gemini-4-argon:custom",
            "custom/reasoner:batch",
            "openai/gpt-6.1-sol:free:batch",
            "openai/gpt-6.1-sol-pro-custom",
            "custom/gpt-6-astra-pro",
            "openai/gpt-6-astra-pro:free",
            "openai/gpt-6-sol-pro-preview:batch",
        ] {
            let mut payload = json!({"model": model});
            let mut config = JobConfig::empty();
            config.thinking = Some(false);
            config.temperature = Some(0.4);
            config.top_p = Some(0.8);
            config.max_tokens = Some(1024);
            add_options_to_payload(&mut payload, Some(&config));
            assert_eq!(payload["model"], model);
            assert_eq!(payload["reasoning"], json!({"enabled": false}));
            assert_eq!(payload["temperature"], 0.4);
            assert_eq!(payload["top_p"], 0.8);
            assert_eq!(payload["max_completion_tokens"], 1024);
        }
    }

    #[test]
    fn custom_reasoning_and_legacy_options_remain_available() {
        for model in ["custom/reasoner", "openai/gpt-4o", "openai/gpt-6-custom"] {
            let mut payload = json!({"model": model});
            let mut config = JobConfig::empty();
            config.temperature = Some(0.4);
            config.top_p = Some(0.8);
            config.other_model_params = Some(json!({
                "reasoning": {"max_tokens": 2048, "exclude": true}, "frequency_penalty": 0.2,
                "model": "wrong/model", "messages": []
            }));
            add_options_to_payload(&mut payload, Some(&config));
            assert_eq!(payload["reasoning"], json!({"max_tokens": 2048, "exclude": true}));
            assert_eq!(payload["temperature"], 0.4);
            assert_eq!(payload["top_p"], 0.8);
            assert_eq!(payload["frequency_penalty"], 0.2);
            assert_eq!(payload["model"], model);
            assert!(payload.get("messages").is_none());
        }
    }

    #[test]
    fn presets_only_send_supported_sampling_and_token_options() {
        for (model, sampling) in [
            ("openai/gpt-6-astra", false),
            ("openai/gpt-6.1-sol", false),
            ("openai/gpt-6-luna", false),
            ("anthropic/claude-sonnet-5.5", false),
            ("google/gemini-4-argon", true),
            ("deepseek/deepseek-v4.1-flash-1017", true),
        ] {
            let mut payload = json!({"model": model, "max_tokens": 4096});
            let mut config = JobConfig::empty();
            config.temperature = Some(0.4);
            config.top_p = Some(0.8);
            config.max_tokens = Some(1024);
            config.other_model_params = Some(json!({"frequency_penalty": 0.2, "logprobs": true}));
            add_options_to_payload(&mut payload, Some(&config));
            assert_eq!(payload.get("temperature").is_some(), sampling, "{model}");
            assert_eq!(payload.get("top_p").is_some(), sampling, "{model}");
            assert_eq!(payload.get("logprobs").is_some(), model.starts_with("deepseek/"));
            assert_eq!(
                payload.get("frequency_penalty").is_some(),
                model.starts_with("deepseek/")
            );
            if model == "google/gemini-4-argon" {
                assert_eq!(payload["max_tokens"], 1024);
                assert!(payload.get("max_completion_tokens").is_none());
            } else {
                assert_eq!(payload["max_completion_tokens"], 1024);
            }
            assert!(payload.get("reasoning").is_none());
        }
    }
}

#[cfg(test)]
mod argument_tests {
    use super::*;
    use crate::llm_provider::execution::chains::generic_chain::generic_inference_chain::execute_valid_function_call;
    use crate::managers::tool_router::ToolCallFunctionResponse;

    async fn assert_execution_allowed(call: &FunctionCall, allowed: bool) {
        let mut executed = false;
        let result = execute_valid_function_call(call, async {
            executed = true;
            Ok(ToolCallFunctionResponse {
                function_call: call.clone(),
                response: "executed".into(),
            })
        })
        .await;
        assert_eq!(executed, allowed);
        assert_eq!(result.is_ok(), allowed);
        if !allowed {
            assert!(result.unwrap_err().to_string().contains("Invalid function arguments"));
        }
    }

    #[tokio::test]
    async fn invalid_nonstreaming_arguments_cannot_execute_or_change_signed_native_data() {
        for (arguments, valid) in [
            (None, false),
            (Some(json!(null)), false),
            (Some(json!({})), false),
            (Some(json!([])), false),
            (Some(json!(42)), false),
            (Some(json!("")), false),
            (Some(json!("{not-json")), false),
            (Some(json!("[]")), false),
            (Some(json!("null")), false),
            (Some(json!("{}")), true),
            (Some(json!("{\"city\":\"Paris\"}")), true),
        ] {
            let mut function = json!({"name": "zero_arg_tool"});
            if let Some(arguments) = arguments {
                function["arguments"] = arguments;
            }
            let assistant = json!({
                "role": "assistant", "content": null,
                "reasoning_details": [{"type": "reasoning.encrypted", "data": "signed-opaque", "index": 0}],
                "tool_calls": [{"id": "native-call", "type": "function", "function": function}]
            });
            let mut calls = Vec::new();
            let mut state = OpenRouterResponseState::default();
            process_streaming_chunk(
                &json!({"choices": [{"message": assistant}]}),
                &mut String::new(),
                &mut calls,
                &None,
                &mut state,
            );
            assert_eq!(calls.len(), 1);
            calls[0].provider_metadata.get_or_insert_with(|| json!({}))["other_data"] = json!("keep");
            state.attach_reasoning(&mut calls);
            let call = &calls[0];
            assert_eq!(call.id.as_deref(), Some("native-call"));
            assert_eq!(call.name, "zero_arg_tool");
            let metadata = call.provider_metadata.as_ref().unwrap();
            assert_eq!(metadata["openrouter_assistant"], assistant);
            assert_eq!(metadata["reasoning_details"], assistant["reasoning_details"]);
            assert_eq!(metadata["other_data"], "keep");
            assert_eq!(metadata.get("function_argument_error").is_none(), valid);
            if !valid {
                assert!(call.arguments.is_empty());
            }
            assert_execution_allowed(call, valid).await;
        }
    }

    #[tokio::test]
    async fn streamed_arguments_clear_partial_errors_and_reject_a_corrupt_suffix() {
        for suffix in ["", " trailing", "{}"] {
            let details = json!([{ "type": "reasoning.encrypted", "data": "signed-opaque", "index": 0 }]);
            let mut calls = Vec::new();
            let mut state = OpenRouterResponseState::default();
            process_streaming_chunk(
                &json!({"choices": [{"delta": {
                    "reasoning_details": details,
                    "tool_calls": [{"index": 0, "id": "native-call", "type": "function", "function": {
                        "name": "lookup", "arguments": "{\"city\":"
                    }}]
                }}]}),
                &mut String::new(),
                &mut calls,
                &None,
                &mut state,
            );
            assert!(calls[0].provider_metadata.as_ref().unwrap()["function_argument_error"].is_string());
            process_streaming_chunk(
                &json!({"choices": [{"delta": {"tool_calls": [{
                    "index": 0, "function": {"arguments": "\"Paris\"}"}
                }]}}]}),
                &mut String::new(),
                &mut calls,
                &None,
                &mut state,
            );
            assert_eq!(calls[0].arguments["city"], "Paris");
            assert!(calls[0]
                .provider_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("function_argument_error"))
                .is_none());
            if !suffix.is_empty() {
                process_streaming_chunk(
                    &json!({"choices": [{"delta": {"tool_calls": [{
                        "index": 0, "function": {"arguments": suffix}
                    }]}}]}),
                    &mut String::new(),
                    &mut calls,
                    &None,
                    &mut state,
                );
                assert!(calls[0].arguments.is_empty());
            }
            state.attach_reasoning(&mut calls);
            let call = &calls[0];
            assert_eq!(call.id.as_deref(), Some("native-call"));
            assert_eq!(call.name, "lookup");
            let metadata = call.provider_metadata.as_ref().unwrap();
            assert_eq!(
                metadata["openrouter_assistant"]["tool_calls"][0]["function"]["arguments"],
                format!("{{\"city\":\"Paris\"}}{suffix}")
            );
            assert_eq!(metadata["reasoning_details"], details);
            assert_eq!(metadata.get("function_argument_error").is_none(), suffix.is_empty());
            assert_execution_allowed(call, suffix.is_empty()).await;
        }
    }
}

#[cfg(test)]
mod reasoning_details_tests {
    use super::*;

    #[test]
    fn streaming_reasoning_survives_fragmented_parallel_tool_calls() {
        let mut state = OpenRouterResponseState::default();
        let mut text = String::new();
        let mut calls = Vec::new();
        let first = json!({"type":"reasoning.text", "text":"Plan", "index":0, "signature":null});
        let second =
            json!({"type":"reasoning.encrypted", "data":"opaque-signature", "format":"google-gemini-v1", "index":1});
        for delta in [
            json!({"reasoning_details":[first.clone()]}),
            json!({"tool_calls":[{"index":0,"id":"call_0","type":"function","function":{"name":"lookup","arguments":"{\"city\":"}}]}),
            json!({"reasoning_details":[second.clone()],"tool_calls":[{"index":0,"function":{"arguments":"\"Paris\"}"}},{"index":1,"id":"call_1","function":{"name":"clock","arguments":"{}"}}]}),
        ] {
            process_streaming_chunk(
                &json!({"choices":[{"delta":delta}]}),
                &mut text,
                &mut calls,
                &None,
                &mut state,
            );
        }
        state.attach_reasoning(&mut calls);
        assert!(text.is_empty());
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].arguments.get("city"), Some(&json!("Paris")));
        assert_eq!(calls[0].id.as_deref(), Some("call_0"));
        for call in calls {
            assert_eq!(
                call.provider_metadata.as_ref().unwrap()["reasoning_details"],
                json!([first.clone(), second.clone()])
            );
            assert_eq!(
                call.provider_metadata.as_ref().unwrap()["openrouter_assistant"]["tool_calls"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
        }
    }

    #[test]
    fn parallel_tool_turn_roundtrips_through_production_prompt_once() {
        let details = json!([{"type":"reasoning.encrypted","data":"signed","index":0}]);
        let mut prompt = Prompt::new();
        // Two distinct turns intentionally have identical reasoning. Their call IDs
        // establish turn identity; equal reasoning must not combine them.
        for prefix in ["first", "second"] {
            let response = json!({"choices":[{"message":{
                "role":"assistant", "content":"I will check both.", "reasoning_details":details,
                "tool_calls":[
                    {"id":format!("{prefix}_a"),"type":"function","function":{"name":"lookup","arguments":"{\"city\":\"Paris\"}"}},
                    {"id":format!("{prefix}_b"),"type":"function","function":{"name":"clock","arguments":"{}"}}
                ]
            }}]});
            let mut state = OpenRouterResponseState::default();
            let mut calls = Vec::new();
            process_streaming_chunk(&response, &mut String::new(), &mut calls, &None, &mut state);
            state.attach_reasoning(&mut calls);
            for call in calls {
                prompt.add_function_call(json!({"call_id":call.id,"name":call.name,"arguments":call.arguments,"provider_metadata":call.provider_metadata}),100);
                prompt.add_function_call_response(
                    json!({"function_call":{"id":call.id,"name":call.name},"response":"ok"}),
                    100,
                );
            }
        }
        let provider = LLMProviderInterface::OpenRouter(OpenRouter {
            model_type: "anthropic/claude-opus-5.5".into(),
        });
        let prepared = openai_prepare_messages(&provider, prompt).unwrap();
        let PromptResultEnum::Value(messages) = prepared.messages else {
            panic!("Expected messages")
        };
        let messages = messages.as_array().unwrap();
        assert_eq!(messages.len(), 6);
        for (offset, prefix) in [(0, "first"), (3, "second")] {
            assert_eq!(messages[offset]["role"], "assistant");
            assert_eq!(messages[offset]["content"], "I will check both.");
            assert_eq!(messages[offset]["reasoning_details"], details);
            assert_eq!(messages[offset]["tool_calls"].as_array().unwrap().len(), 2);
            assert_eq!(messages[offset]["tool_calls"][0]["id"], format!("{prefix}_a"));
            assert_eq!(messages[offset]["tool_calls"][1]["id"], format!("{prefix}_b"));
            for (result_index, suffix) in [(offset + 1, "a"), (offset + 2, "b")] {
                assert_eq!(messages[result_index]["role"], "tool");
                assert_eq!(messages[result_index]["tool_call_id"], format!("{prefix}_{suffix}"));
                assert!(messages[result_index].get("reasoning_details").is_none());
            }
        }
    }

    #[test]
    fn nonstreaming_keeps_all_tool_calls_and_complete_reasoning() {
        let details = json!([
            {"type":"reasoning.text","text":"Plan","signature":"signed","index":0},
            {"type":"reasoning.encrypted","data":"opaque","index":1}
        ]);
        let response = json!({"choices":[{"message":{
            "content":null,"reasoning_details":details,
            "tool_calls":[
                {"id":"a","type":"function","function":{"name":"one","arguments":"{}"}},
                {"id":"b","type":"function","function":{"name":"two","arguments":"{\"x\":2}"}}
            ]
        }}]});
        let mut state = OpenRouterResponseState::default();
        let mut calls = Vec::new();
        process_streaming_chunk(&response, &mut String::new(), &mut calls, &None, &mut state);
        state.attach_reasoning(&mut calls);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].arguments.get("x"), Some(&json!(2)));
        assert_eq!(
            calls[0].provider_metadata.as_ref().unwrap()["reasoning_details"],
            details
        );
        assert_eq!(calls[0].provider_metadata, calls[1].provider_metadata);
    }
}
