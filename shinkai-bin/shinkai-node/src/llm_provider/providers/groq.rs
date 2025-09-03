use std::error::Error;
use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::openai_api_deprecated::{MessageContent, OpenAIResponse};
use super::shared::shared_model_logic::send_tool_ws_update;
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, LLMInferenceResponse};
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::llm_provider::providers::shared::groq_api::groq_prepare_messages;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{Groq, LLMProviderInterface};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::{
    WSMessageType, WSMetadata, WSUpdateHandler
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;
use uuid::Uuid;

#[async_trait]
impl LLMService for Groq {
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
                let url = format!("{}{}", base_url, "/chat/completions");
                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);

                let result = groq_prepare_messages(&model, prompt)?;
                let messages_json = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                // Extract tools_json from the result and keep original for matching
                let tools_json = result.functions.clone().unwrap_or_else(Vec::new);
                let original_tools = tools_json.clone(); // Keep original for matching

                let mut payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    // "max_tokens": result.remaining_tokens,
                    "stream": is_stream,
                });

                // Add tools to payload if they exist, but remove tool_router_key
                if !tools_json.is_empty() {
                    let tools: Vec<JsonValue> = tools_json
                        .iter()
                        .map(|tool| {
                            let mut tool_copy = tool.clone();
                            // Remove tool_router_key from the function object
                            if let Some(function_obj) = tool_copy.get_mut("function") {
                                if let Some(obj) = function_obj.as_object_mut() {
                                    obj.remove("tool_router_key");
                                }
                            }
                            tool_copy
                        })
                        .collect();
                    payload["tools"] = serde_json::Value::Array(tools);
                    payload["tool_choice"] = json!("auto");
                }

                // Clean up message content if needed
                if let Some(messages) = payload.get_mut("messages") {
                    if let Some(messages_array) = messages.as_array_mut() {
                        for message in messages_array {
                            if let Some(content) = message.get_mut("content") {
                                if let Some(content_array) = content.as_array() {
                                    // If content is an array with a single text element, simplify it
                                    if content_array.len() == 1 {
                                        if let Some(text_obj) = content_array[0].get("text") {
                                            *content = text_obj.clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Add options to payload
                add_options_to_payload(&mut payload, config.as_ref());

                let payload_log = payload.clone();
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
                        ws_manager_trait.clone(),
                        llm_stopper,
                        session_id,
                        Some(original_tools), // Pass original tools with router keys
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
                        ws_manager_trait.clone(),
                        Some(original_tools), // Pass original tools with router keys
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
    let mut buffer = String::new();
    let mut function_calls: Vec<FunctionCall> = Vec::new();

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

                return Ok(LLMInferenceResponse::new(response_text, None, json!({}), Vec::new(), Vec::new(), None));
            }
        }

        match item {
            Ok(chunk) => {
                let chunk_str = String::from_utf8_lossy(&chunk).to_string();
                buffer.push_str(&chunk_str);

                while let Some(pos) = buffer.find("\n\n") {
                    // Clone the message to own the data, avoiding borrow conflicts
                    let message = buffer[..pos].trim().to_string();
                    buffer = buffer[pos + 2..].to_string(); // Update buffer

                    if message.starts_with("data: ") {
                        let data = &message[6..]; // Skip "data: "
                        if data == "[DONE]" {
                            if let Some(ref manager) = ws_manager_trait {
                                if let Some(ref inbox_name) = inbox_name {
                                    let m = manager.lock().await;
                                    let inbox_name_string = inbox_name.to_string();

                                    let metadata = WSMetadata {
                                        id: Some(session_id.clone()),
                                        is_reasoning: false,
                                        is_done: function_calls.is_empty(),
                                        done_reason: Some("streaming completed".to_string()),
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
                            }
                            break;
                        } else {
                            match serde_json::from_str::<JsonValue>(data) {
                                Ok(data_json) => {
                                    // Check if the data contains an error
                                    if let Some(error) = data_json.get("error") {
                                        let message =
                                            error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
                                        let status_code =
                                            error.get("status_code").and_then(|c| c.as_u64()).unwrap_or(500);
                                        return Err(LLMProviderError::LLMServiceUnexpectedError(format!(
                                            "Error {}: {}",
                                            status_code, message
                                        )));
                                    }

                                    if let Some(choices) = data_json.get("choices") {
                                        for choice in choices.as_array().unwrap_or(&vec![]) {
                                            if let Some(delta) = choice.get("delta") {
                                                if let Some(fc) = delta.get("tool_calls") {
                                                    if let Some(tool_calls_array) = fc.as_array() {
                                                        for tool_call in tool_calls_array {
                                                            if let Some(function) = tool_call.get("function") {
                                                                if let Some(name) = function.get("name") {
                                                                    let fc_arguments = function
                                                                        .get("arguments")
                                                                        .and_then(|args| args.as_str())
                                                                        .and_then(|args_str| {
                                                                            serde_json::from_str(args_str).ok()
                                                                        })
                                                                        .and_then(|args_value: serde_json::Value| {
                                                                            args_value.as_object().cloned()
                                                                        })
                                                                        .unwrap_or_else(|| serde_json::Map::new());

                                                                    // Search for the tool_router_key in the tools array
                                                                    let tool_router_key =
                                                                        tools.as_ref().and_then(|tools_array| {
                                                                            tools_array.iter().find_map(|tool| {
                                                                                if let Some(function) =
                                                                                    tool.get("function")
                                                                                {
                                                                                    if function.get("name")?.as_str()?
                                                                                        == name.as_str().unwrap_or("")
                                                                                    {
                                                                                        function
                                                                                            .get("tool_router_key")
                                                                                            .and_then(|key| {
                                                                                                key.as_str().map(|s| {
                                                                                                    s.to_string()
                                                                                                })
                                                                                            })
                                                                                    } else {
                                                                                        None
                                                                                    }
                                                                                } else {
                                                                                    None
                                                                                }
                                                                            })
                                                                        });

                                                                    let function_call = FunctionCall {
                                                                        name: name.as_str().unwrap_or("").to_string(),
                                                                        arguments: fc_arguments.clone(),
                                                                        tool_router_key,
                                                                        response: None,
                                                                        index: function_calls.len() as u64,
                                                                        id: None,
                                                                        call_type: Some("function".to_string()),
                                                                    };
                                                                    function_calls.push(function_call.clone());

                                                                    let _ = send_tool_ws_update(&ws_manager_trait, inbox_name.clone(), &function_call).await;
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                if let Some(content) = delta.get("content") {
                                                    let response_text_chunk = content.as_str().unwrap_or("");
                                                    response_text.push_str(response_text_chunk);

                                                    // Handle WebSocket updates
                                                    if let Some(ref manager) = ws_manager_trait {
                                                        if let Some(ref inbox_name) = inbox_name {
                                                            let m = manager.lock().await;
                                                            let inbox_name_string = inbox_name.to_string();

                                                            let metadata = WSMetadata {
                                                                id: Some(session_id.clone()),
                                                                is_reasoning: false,
                                                                is_done: function_calls.is_empty()
                                                                    && data_json
                                                                        .get("done")
                                                                        .and_then(|d| d.as_bool())
                                                                        .unwrap_or(false),
                                                                done_reason: data_json
                                                                    .get("done_reason")
                                                                    .and_then(|d| d.as_str())
                                                                    .map(|s| s.to_string()),
                                                                total_duration: data_json
                                                                    .get("total_duration")
                                                                    .and_then(|d| d.as_u64()),
                                                                eval_count: data_json
                                                                    .get("eval_count")
                                                                    .and_then(|c| c.as_u64()),
                                                            };

                                                            let ws_message_type = WSMessageType::Metadata(metadata);

                                                            let _ = m
                                                                .queue_message(
                                                                    WSTopic::Inbox,
                                                                    inbox_name_string,
                                                                    response_text_chunk.to_string(),
                                                                    ws_message_type,
                                                                    true,
                                                                )
                                                                .await;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error parsing JSON: {:?}", e);
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        format!("Error parsing JSON: {:?}", e).as_str(),
                                    );
                                }
                            }
                        }
                    } else {
                        eprintln!("Received unexpected message format: {:?}", message);
                    }
                }
            }
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    format!("Error while receiving chunk: {:?}, Error Source: {:?}", e, e.source()).as_str(),
                );
            }
        }
    }

    // Send final WS message to indicate completion
    if let Some(ref manager) = ws_manager_trait {
        if let Some(ref inbox_name) = inbox_name {
            let m = manager.lock().await;
            let inbox_name_string = inbox_name.to_string();

            let metadata = WSMetadata {
                id: Some(session_id.clone()),
                is_reasoning: false,
                is_done: function_calls.is_empty(),
                done_reason: Some("finished".to_string()),
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
    }

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
    _ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
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

                        let function_calls: Vec<FunctionCall> = data.choices.iter().flat_map(|choice| {
                            let mut calls = Vec::new();

                            // Handle tool_calls
                            if let Some(tool_calls) = &choice.message.tool_calls {
                                for (index, tool_call) in tool_calls.iter().enumerate() {
                                    let arguments = serde_json::from_str::<serde_json::Value>(&tool_call.function.arguments)
                                        .ok()
                                        .and_then(|args_value: serde_json::Value| args_value.as_object().cloned())
                                        .unwrap_or_else(|| serde_json::Map::new());

                                    // Find matching tool and extract router key
                                    let tool_router_key = tools.as_ref().and_then(|tools_array| {
                                        tools_array.iter().find_map(|tool| {
                                            if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                                                if name == tool_call.function.name {
                                                    tool.get("tool_router_key")
                                                        .and_then(|key| key.as_str())
                                                        .map(|s| s.to_string())
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        })
                                    });

                                    calls.push(FunctionCall {
                                        name: tool_call.function.name.clone(),
                                        arguments,
                                        tool_router_key,
                                        response: None,
                                        index: index as u64,
                                        id: None,
                                        call_type: Some("function".to_string()),
                                    });
                                }
                            }

                            calls
                        }).collect();

                        return Ok(LLMInferenceResponse::new(
                            response_string,
                            None,
                            json!({}),
                            function_calls,
                            Vec::new(),
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
