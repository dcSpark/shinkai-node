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
    arguments: std::collections::HashMap<u64, String>,
}

impl OpenRouterResponseState {
    fn attach_reasoning(&self, calls: &mut [FunctionCall]) {
        if calls.is_empty() {
            return;
        }
        let native_calls: Vec<_> = calls.iter().map(|call| json!({
            "id": call.id, "type": call.call_type.as_deref().unwrap_or("function"),
            "function": {"name": call.name, "arguments": self.arguments.get(&call.index).cloned().unwrap_or_else(|| serde_json::to_string(&call.arguments).unwrap_or_default())}
        })).collect();
        let mut assistant = json!({"role":"assistant", "content": if self.content.is_empty() { JsonValue::Null } else { json!(self.content) }, "tool_calls": native_calls});
        if !self.reasoning_details.is_empty() {
            assistant["reasoning_details"] = json!(self.reasoning_details);
        }
        for call in calls {
            call.provider_metadata =
                Some(json!({"reasoning_details":self.reasoning_details, "openrouter_assistant":assistant}));
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
                    if let Some(arguments) = function.and_then(|f| f.get("arguments")).and_then(|a| a.as_str()) {
                        let buffer = state.arguments.entry(index).or_default();
                        buffer.push_str(arguments);
                        if let Ok(JsonValue::Object(arguments)) = serde_json::from_str(buffer) {
                            call.arguments = arguments;
                        }
                    }
                }
            }
        }
    }
    new_content
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
