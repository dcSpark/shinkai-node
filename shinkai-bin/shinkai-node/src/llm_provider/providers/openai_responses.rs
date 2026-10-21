use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::openai_api::openai_prepare_messages;
use super::shared::shared_model_logic::{send_tool_ws_update, send_ws_update};
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, LLMInferenceResponse};
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenAI};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_sqlite::SqliteManager;
use std::collections::HashMap;
use tokio::sync::Mutex;
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
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
        _llm_stopper: Arc<LLMStopper>,
        db: Arc<SqliteManager>,
        tracing_message_id: Option<String>,
    ) -> Result<LLMInferenceResponse, LLMProviderError> {
        // Generate a per-request session ID so websocket chunks can be grouped by clients
        let session_id = Uuid::new_v4().to_string();
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                // Use the Responses API endpoint
                let url = openai_endpoint(base_url, "responses");

                // Enable streaming if requested (SSE)
                let is_stream = config.as_ref().and_then(|c| c.stream).unwrap_or(true);

                // Prepare messages as before (role/content), then map to Responses 'input'
                let result = openai_prepare_messages(&model, prompt)?;
                let messages_json = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                // Transform Chat-style messages into Responses-style input blocks
                let input_messages = transform_input_messages_for_responses(messages_json);

                // Extract tools_json from the result
                let tools_json = result.functions.unwrap_or_else(Vec::new);

                // Responses API prefers 'max_output_tokens'
                let mut payload = json!({
                    "model": self.model_type,
                    "input": input_messages,
                    "max_output_tokens": result.remaining_output_tokens,
                    "stream": is_stream,
                    "store": false,
                });

                if !tools_json.is_empty() {
                    // Normalize tool definitions for Responses API: {type:function, name, description?, parameters?}
                    let mut tools_for_responses: Vec<JsonValue> = Vec::new();
                    for tool in tools_json.iter() {
                        // Try to read name/description/parameters from either top-level or nested under "function"
                        let fn_obj = tool.get("function");
                        let name = tool
                            .get("name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                fn_obj
                                    .and_then(|f| f.get("name"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                            });

                        // Skip malformed tool entries with no name to avoid API errors
                        let Some(name) = name else { continue };

                        let description = tool
                            .get("description")
                            .cloned()
                            .or_else(|| fn_obj.and_then(|f| f.get("description")).cloned());
                        let parameters = tool
                            .get("parameters")
                            .cloned()
                            .or_else(|| fn_obj.and_then(|f| f.get("parameters")).cloned())
                            .unwrap_or_else(|| json!({"type":"object","properties":{}}));

                        let mut obj = serde_json::Map::new();
                        obj.insert("type".to_string(), json!("function"));
                        obj.insert("name".to_string(), json!(name));
                        if let Some(desc) = description {
                            obj.insert("description".to_string(), desc);
                        }
                        obj.insert("parameters".to_string(), parameters);
                        // Existing tools can have optional fields and additional properties.
                        obj.insert("strict".to_string(), json!(false));
                        tools_for_responses.push(JsonValue::Object(obj));
                    }

                    if !tools_for_responses.is_empty() {
                        payload["tools"] = serde_json::Value::Array(tools_for_responses);
                        payload["tool_choice"] = json!("auto");
                    }
                }

                // Add common sampling/options, reusing existing helper semantics
                let is_reasoning_model = ModelCapabilitiesManager::has_reasoning_capabilities(&model);
                add_options_to_payload_responses(&mut payload, config.as_ref(), is_reasoning_model);

                let mut payload_log = payload.clone();
                truncate_image_url_in_payload(&mut payload_log);

                if let Some(ref msg_id) = tracing_message_id {
                    if let Err(e) = db.add_tracing(
                        msg_id,
                        inbox_name.as_ref().map(|i| i.get_value()).as_deref(),
                        "llm_payload",
                        &payload_log,
                    ) {
                        // ignore tracing failures
                    }
                }

                if is_stream {
                    handle_streaming_response_responses(
                        client,
                        url,
                        payload,
                        key.to_string(),
                        inbox_name,
                        ws_manager_trait,
                        _llm_stopper,
                        session_id,
                    )
                    .await
                } else {
                    handle_non_streaming_response_responses(
                        client,
                        url,
                        payload,
                        key.to_string(),
                        inbox_name,
                        ws_manager_trait,
                        session_id,
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

fn validate_response_completion(response: &JsonValue) -> Result<(), LLMProviderError> {
    let error = response.get("error").filter(|value| !value.is_null());
    let incomplete = response.get("incomplete_details").filter(|value| !value.is_null());
    let status = response.get("status").and_then(JsonValue::as_str);
    if error.is_some() || incomplete.is_some() || status.is_some_and(|status| status != "completed") {
        let details = error
            .or(incomplete)
            .map(ToString::to_string)
            .unwrap_or_else(|| "Response did not complete successfully".into());
        return Err(LLMProviderError::APIError(format!(
            "OpenAI Responses {}: {}",
            status.unwrap_or("error"),
            details
        )));
    }
    Ok(())
}

async fn handle_non_streaming_response_responses(
    client: &Client,
    url: String,
    payload: JsonValue,
    api_key: String,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    session_id: String,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let res = client
        .post(url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        // Try to parse error structure from Responses API
        let error_json: serde_json::Value = res.json().await.unwrap_or(json!({"error":"Unknown error"}));
        if let Some(err) = error_json.get("error") {
            // Could be object or string
            if let Some(msg) = err.get("message").and_then(|m| m.as_str()) {
                return Err(LLMProviderError::APIError(format!("AI Provider API Error: {}", msg)));
            }
            if let Some(msg) = err.as_str() {
                return Err(LLMProviderError::APIError(format!("AI Provider API Error: {}", msg)));
            }
        }
        return Err(LLMProviderError::APIError(
            "AI Provider API Error: Unknown error occurred".to_string(),
        ));
    }

    let response_json: serde_json::Value = res.json().await?;
    validate_response_completion(&response_json)?;

    // Only extract from output.content[].text where type="output_text" - no fallbacks
    // 1) Text content
    let mut response_text = String::new();
    if let Some(output_items) = response_json.get("output").and_then(|v| v.as_array()) {
        for item in output_items {
            if item.get("type").and_then(|t| t.as_str()) == Some("message") {
                if let Some(content_blocks) = item.get("content").and_then(|c| c.as_array()) {
                    for block in content_blocks {
                        if block.get("type").and_then(|t| t.as_str()) == Some("output_text") {
                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                response_text.push_str(text);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2) Reasoning text - only from output.summary.text path
    let mut reasoning_text = String::new();
    if let Some(output_items) = response_json.get("output").and_then(|v| v.as_array()) {
        for item in output_items {
            if item.get("type").and_then(|t| t.as_str()) == Some("reasoning") {
                if let Some(summary) = item.get("summary").and_then(|s| s.as_array()) {
                    for summary_item in summary {
                        if summary_item.get("type").and_then(|t| t.as_str()) == Some("summary_text") {
                            if let Some(text) = summary_item.get("text").and_then(|t| t.as_str()) {
                                reasoning_text.push_str(text);
                            }
                        }
                    }
                }
            }
        }
    }

    let function_calls = extract_native_function_calls(
        response_json
            .get("output")
            .and_then(JsonValue::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]),
    );

    // Send a final WebSocket update if needed
    if let Some(inbox_name) = inbox_name.as_ref() {
        // If we have a function call, emit the tool update as well (use the last one)
        if let Some(last_function_call) = function_calls.last() {
            let _ = send_tool_ws_update(&ws_manager_trait, Some(inbox_name.clone()), last_function_call).await;
        }
        // Send final message with the per-request session id
        let _ = send_ws_update(
            &ws_manager_trait,
            Some(inbox_name.clone()),
            &session_id,
            response_text.clone(),
            false,
            function_calls.is_empty(),
            None,
        )
        .await;
    }

    Ok(LLMInferenceResponse::new(
        response_text,
        if reasoning_text.is_empty() {
            None
        } else {
            Some(reasoning_text)
        },
        json!({}),
        function_calls,
        Vec::new(),
        None,
    ))
}

async fn handle_streaming_response_responses(
    client: &Client,
    url: String,
    payload: JsonValue,
    api_key: String,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    llm_stopper: Arc<crate::llm_provider::llm_stopper::LLMStopper>,
    session_id: String,
) -> Result<LLMInferenceResponse, LLMProviderError> {
    let res = client
        .post(url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&payload)
        .send()
        .await?;

    if !res.status().is_success() {
        // Surface API error body
        let text = res.text().await.unwrap_or_else(|_| "".to_string());
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(err) = v.get("error") {
                if let Some(msg) = err.get("message").and_then(|m| m.as_str()) {
                    return Err(LLMProviderError::APIError(format!("AI Provider API Error: {}", msg)));
                }
            }
        }
        return Err(LLMProviderError::APIError(
            "AI Provider API Error: Unknown error occurred".to_string(),
        ));
    }

    let mut stream = res.bytes_stream();
    let mut buffer = String::new();
    let mut response_text = String::new();
    let mut reasoning_text = String::new();

    // Accumulate tool calls by id
    #[derive(Debug)]
    struct ToolAccum {
        name: Option<String>,
        arguments: String,
        call_type: Option<String>,
    }
    let mut tools_map: HashMap<String, ToolAccum> = HashMap::new();
    let mut function_calls: Vec<FunctionCall> = Vec::new();
    let mut native_output: Vec<JsonValue> = Vec::new();
    let mut completed = false;
    let mut stopped_by_user = false;

    // SSE parsing loop
    while let Some(item) = stream.next().await {
        // Stopping support
        if let Some(ref inbox_name) = inbox_name {
            if llm_stopper.should_stop(&inbox_name.to_string()) {
                llm_stopper.reset(&inbox_name.to_string());
                stopped_by_user = true;
                // notify done
                super::shared::shared_model_logic::send_ws_update(
                    &ws_manager_trait,
                    Some(inbox_name.clone()),
                    &session_id,
                    response_text.clone(),
                    false,
                    true,
                    Some("Stopped by user request".to_string()),
                )
                .await?;
                break;
            }
        }

        let chunk = match item {
            Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
            Err(e) => return Err(LLMProviderError::NetworkError(e.to_string())),
        };
        buffer.push_str(&chunk);

        // process complete SSE blocks separated by \n\n
        loop {
            if let Some(pos) = buffer.find("\n\n") {
                let block = buffer.drain(..pos + 2).collect::<String>();
                // parse event and data lines
                let mut event_type: Option<String> = None;
                let mut data_buf = String::new();
                for line in block.lines() {
                    if let Some(rest) = line.strip_prefix("event: ") {
                        event_type = Some(rest.trim().to_string());
                    } else if let Some(rest) = line.strip_prefix("data: ") {
                        if !data_buf.is_empty() {
                            data_buf.push('\n');
                        }
                        data_buf.push_str(rest);
                    }
                }

                if let Some(ev) = event_type {
                    // handle event
                    match ev.as_str() {
                        "response.failed" | "response.incomplete" | "error" => {
                            let details = serde_json::from_str::<JsonValue>(&data_buf)
                                .unwrap_or_else(|_| json!({"message":data_buf}));
                            let provider_details = details
                                .pointer("/response/error")
                                .filter(|value| !value.is_null())
                                .or_else(|| details.pointer("/response/incomplete_details"))
                                .or_else(|| details.get("error"))
                                .unwrap_or(&details);
                            return Err(LLMProviderError::APIError(format!(
                                "OpenAI Responses stream {}: {}",
                                ev, provider_details
                            )));
                        }
                        // Text deltas
                        "response.output_text.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(delta) = v.get("delta").and_then(|d| d.as_str()) {
                                    response_text.push_str(delta);
                                    if let Some(ref inbox) = inbox_name {
                                        super::shared::shared_model_logic::send_ws_update(
                                            &ws_manager_trait,
                                            Some(inbox.clone()),
                                            &session_id,
                                            delta.to_string(),
                                            false,
                                            false,
                                            None,
                                        )
                                        .await?;
                                    }
                                }
                            }
                        }
                        // Reasoning / summary deltas
                        "response.reasoning_summary_part.added" => {
                            if let Some(ref inbox) = inbox_name {
                                super::shared::shared_model_logic::send_ws_update(
                                    &ws_manager_trait,
                                    Some(inbox.clone()),
                                    &session_id,
                                    "".to_string(),
                                    true,
                                    false,
                                    None,
                                )
                                .await?;
                            }
                        }
                        "response.reasoning_summary_text.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(delta) = v.get("delta").and_then(|d| d.as_str()) {
                                    reasoning_text.push_str(delta);
                                    if let Some(ref inbox) = inbox_name {
                                        super::shared::shared_model_logic::send_ws_update(
                                            &ws_manager_trait,
                                            Some(inbox.clone()),
                                            &session_id,
                                            delta.to_string(),
                                            true,
                                            false,
                                            None,
                                        )
                                        .await?;
                                    }
                                }
                            }
                        }
                        "response.reasoning_summary_text.done" => {
                            // Informational event, reasoning content is handled by response.reasoning.delta
                        }
                        "response.reasoning_summary_part.done" => {
                            if let Some(ref inbox) = inbox_name {
                                super::shared::shared_model_logic::send_ws_update(
                                    &ws_manager_trait,
                                    Some(inbox.clone()),
                                    &session_id,
                                    "".to_string(),
                                    false,
                                    false,
                                    None,
                                )
                                .await?;
                            }
                        }
                        // Tool call lifecycle
                        "response.tool_call.created" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v
                                    .get("call_id")
                                    .and_then(|s| s.as_str())
                                    .map(|s| s.to_string())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()))
                                {
                                    let name =
                                        v.get("name")
                                            .and_then(|s| s.as_str())
                                            .map(|s| s.to_string())
                                            .or_else(|| {
                                                v.get("function")
                                                    .and_then(|f| f.get("name"))
                                                    .and_then(|s| s.as_str())
                                                    .map(|s| s.to_string())
                                            });
                                    tools_map.entry(id).or_insert(ToolAccum {
                                        name,
                                        arguments: String::new(),
                                        call_type: Some("function".to_string()),
                                    });
                                }
                            }
                        }
                        "response.tool_call.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v
                                    .get("call_id")
                                    .and_then(|s| s.as_str())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()))
                                {
                                    let entry = tools_map.entry(id.to_string()).or_insert(ToolAccum {
                                        name: None,
                                        arguments: String::new(),
                                        call_type: Some("function".to_string()),
                                    });
                                    // name may arrive here too
                                    if entry.name.is_none() {
                                        entry.name =
                                            v.get("name").and_then(|s| s.as_str()).map(|s| s.to_string()).or_else(
                                                || {
                                                    v.get("function")
                                                        .and_then(|f| f.get("name"))
                                                        .and_then(|s| s.as_str())
                                                        .map(|s| s.to_string())
                                                },
                                            );
                                    }
                                    // delta may be in {"delta":{"arguments":"..."}}
                                    if let Some(delta) = v.get("delta") {
                                        if let Some(args) = delta.get("arguments").and_then(|a| a.as_str()) {
                                            entry.arguments.push_str(args);
                                        }
                                    }
                                }
                            }
                        }
                        "response.tool_call.done" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v
                                    .get("call_id")
                                    .and_then(|s| s.as_str())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()))
                                {
                                    if let Some(acc) = tools_map.remove(id) {
                                        if let Some(name) = acc.name {
                                            let args_map = serde_json::from_str::<serde_json::Value>(&acc.arguments)
                                                .ok()
                                                .and_then(|v| v.as_object().cloned())
                                                .unwrap_or_default();
                                            function_calls.push(FunctionCall {
                                                provider_metadata: None,
                                                name,
                                                arguments: args_map,
                                                tool_router_key: None,
                                                response: None,
                                                index: function_calls.len() as u64,
                                                id: Some(id.to_string()),
                                                call_type: acc.call_type.or(Some("function".to_string())),
                                            });
                                        }
                                    }
                                    if let Some(ref inbox) = inbox_name {
                                        if let Some(last) = function_calls.last() {
                                            super::shared::shared_model_logic::send_tool_ws_update(
                                                &ws_manager_trait,
                                                Some(inbox.clone()),
                                                last,
                                            )
                                            .await?;
                                        }
                                    }
                                }
                            }
                        }
                        // Function call lifecycle (OpenAI Responses API uses function_call instead of tool_call)
                        "response.function_call.created" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v
                                    .get("call_id")
                                    .and_then(|s| s.as_str())
                                    .map(|s| s.to_string())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()))
                                {
                                    let name =
                                        v.get("name")
                                            .and_then(|s| s.as_str())
                                            .map(|s| s.to_string())
                                            .or_else(|| {
                                                v.get("function")
                                                    .and_then(|f| f.get("name"))
                                                    .and_then(|s| s.as_str())
                                                    .map(|s| s.to_string())
                                            });
                                    tools_map.entry(id).or_insert(ToolAccum {
                                        name,
                                        arguments: String::new(),
                                        call_type: Some("function".to_string()),
                                    });
                                }
                            }
                        }
                        "response.function_call.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v
                                    .get("call_id")
                                    .and_then(|s| s.as_str())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()))
                                {
                                    let entry = tools_map.entry(id.to_string()).or_insert(ToolAccum {
                                        name: None,
                                        arguments: String::new(),
                                        call_type: Some("function".to_string()),
                                    });
                                    // name may arrive here too
                                    if entry.name.is_none() {
                                        entry.name =
                                            v.get("name").and_then(|s| s.as_str()).map(|s| s.to_string()).or_else(
                                                || {
                                                    v.get("function")
                                                        .and_then(|f| f.get("name"))
                                                        .and_then(|s| s.as_str())
                                                        .map(|s| s.to_string())
                                                },
                                            );
                                    }
                                    // delta may be in {"delta":{"arguments":"..."}}
                                    if let Some(delta) = v.get("delta") {
                                        if let Some(args) = delta.get("arguments").and_then(|a| a.as_str()) {
                                            entry.arguments.push_str(args);
                                        }
                                    }
                                }
                            }
                        }
                        "response.function_call_arguments.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                // For OpenAI Responses API, the ID is in item_id field
                                if let Some(id) = v
                                    .get("item_id")
                                    .and_then(|s| s.as_str())
                                    .or_else(|| v.get("call_id").and_then(|s| s.as_str()))
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()))
                                {
                                    let entry = tools_map.entry(id.to_string()).or_insert(ToolAccum {
                                        name: None,
                                        arguments: String::new(),
                                        call_type: Some("function".to_string()),
                                    });
                                    // OpenAI Responses API puts the delta directly in the "delta" field
                                    if let Some(delta) = v.get("delta").and_then(|d| d.as_str()) {
                                        entry.arguments.push_str(delta);
                                    }
                                }
                            }
                        }
                        "response.function_call_arguments.done" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                // For OpenAI Responses API, the ID is in item_id field
                                if let Some(id) = v
                                    .get("item_id")
                                    .and_then(|s| s.as_str())
                                    .or_else(|| v.get("call_id").and_then(|s| s.as_str()))
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()))
                                {
                                    // Get the complete arguments from the event (fallback to accumulated)
                                    let final_arguments = v
                                        .get("arguments")
                                        .and_then(|a| a.as_str())
                                        .map(|s| s.to_string())
                                        .or_else(|| tools_map.get(id).map(|acc| acc.arguments.clone()))
                                        .unwrap_or_default();

                                    if let Some(acc) = tools_map.remove(id) {
                                        if let Some(name) = acc.name {
                                            let args_map = serde_json::from_str::<serde_json::Value>(&final_arguments)
                                                .ok()
                                                .and_then(|v| v.as_object().cloned())
                                                .unwrap_or_default();
                                            function_calls.push(FunctionCall {
                                                provider_metadata: None,
                                                name,
                                                arguments: args_map,
                                                tool_router_key: None,
                                                response: None,
                                                index: function_calls.len() as u64,
                                                id: Some(id.to_string()),
                                                call_type: acc.call_type.or(Some("function".to_string())),
                                            });
                                            // Send WebSocket update for this function call
                                            if let Some(ref inbox) = inbox_name {
                                                if let Some(last) = function_calls.last() {
                                                    let _ = super::shared::shared_model_logic::send_tool_ws_update(
                                                        &ws_manager_trait,
                                                        Some(inbox.clone()),
                                                        last,
                                                    )
                                                    .await;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        "response.function_call.done" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v
                                    .get("call_id")
                                    .and_then(|s| s.as_str())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()))
                                {
                                    if let Some(acc) = tools_map.remove(id) {
                                        if let Some(name) = acc.name {
                                            let args_map = serde_json::from_str::<serde_json::Value>(&acc.arguments)
                                                .ok()
                                                .and_then(|v| v.as_object().cloned())
                                                .unwrap_or_default();
                                            function_calls.push(FunctionCall {
                                                provider_metadata: None,
                                                name,
                                                arguments: args_map,
                                                tool_router_key: None,
                                                response: None,
                                                index: function_calls.len() as u64,
                                                id: Some(id.to_string()),
                                                call_type: acc.call_type.or(Some("function".to_string())),
                                            });
                                        }
                                    }
                                    if let Some(ref inbox) = inbox_name {
                                        if let Some(last) = function_calls.last() {
                                            super::shared::shared_model_logic::send_tool_ws_update(
                                                &ws_manager_trait,
                                                Some(inbox.clone()),
                                                last,
                                            )
                                            .await?;
                                        }
                                    }
                                }
                            }
                        }
                        // Completion end
                        "response.completed" => {
                            let completion: JsonValue = serde_json::from_str(&data_buf).map_err(|_| {
                                LLMProviderError::APIError("OpenAI Responses completion event is invalid".into())
                            })?;
                            let response = completion
                                .get("response")
                                .filter(|response| response.is_object())
                                .ok_or_else(|| {
                                    LLMProviderError::APIError("OpenAI Responses completion response is missing".into())
                                })?;
                            validate_response_completion(response)?;
                            if response["status"] != "completed" {
                                return Err(LLMProviderError::APIError(
                                    "OpenAI Responses completion status is missing".into(),
                                ));
                            }
                            completed = true;
                            if let Some(output) = response.get("output").and_then(JsonValue::as_array) {
                                if !output.is_empty() {
                                    native_output = output.clone();
                                }
                            }
                            // finalize any remaining tool calls
                            let ids: Vec<String> = tools_map.keys().cloned().collect();
                            for id in ids {
                                if let Some(acc) = tools_map.remove(&id) {
                                    if let Some(name) = acc.name {
                                        let args_map = serde_json::from_str::<serde_json::Value>(&acc.arguments)
                                            .ok()
                                            .and_then(|v| v.as_object().cloned())
                                            .unwrap_or_default();
                                        function_calls.push(FunctionCall {
                                            provider_metadata: None,
                                            name,
                                            arguments: args_map,
                                            tool_router_key: None,
                                            response: None,
                                            index: function_calls.len() as u64,
                                            id: Some(id.to_string()),
                                            call_type: acc.call_type.or(Some("function".to_string())),
                                        });
                                    }
                                }
                            }

                            // Send tool updates for any finalized function calls
                            if let Some(ref inbox) = inbox_name {
                                if let Some(last_function_call) = function_calls.last() {
                                    let _ = super::shared::shared_model_logic::send_tool_ws_update(
                                        &ws_manager_trait,
                                        Some(inbox.clone()),
                                        last_function_call,
                                    )
                                    .await;
                                }
                            }

                            if let Some(ref inbox) = inbox_name {
                                super::shared::shared_model_logic::send_ws_update(
                                    &ws_manager_trait,
                                    Some(inbox.clone()),
                                    &session_id,
                                    "".to_string(),
                                    false,
                                    true,
                                    Some("Completed".to_string()),
                                )
                                .await?;
                            }
                        }
                        // Response lifecycle events (informational)
                        "response.created" => {
                            // Response has been created, no specific action needed
                        }
                        "response.in_progress" => {
                            // Response is being processed, no specific action needed
                        }
                        "response.output_item.added" => {
                            // Check if this is a function call being added
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(item) = v.get("item") {
                                    if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                                        if let Some(id) = item.get("id").and_then(|id| id.as_str()) {
                                            // Extract function name from various possible locations
                                            let name = item
                                                .get("name")
                                                .and_then(|n| n.as_str())
                                                .map(|s| s.to_string())
                                                .or_else(|| {
                                                    item.get("function_name")
                                                        .and_then(|n| n.as_str())
                                                        .map(|s| s.to_string())
                                                })
                                                .or_else(|| {
                                                    item.get("function")
                                                        .and_then(|f| f.get("name"))
                                                        .and_then(|n| n.as_str())
                                                        .map(|s| s.to_string())
                                                });
                                            tools_map.entry(id.to_string()).or_insert(ToolAccum {
                                                name,
                                                arguments: String::new(),
                                                call_type: Some("function".to_string()),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        "response.output_item.done" => {
                            if let Ok(v) = serde_json::from_str::<JsonValue>(&data_buf) {
                                if let Some(item) = v.get("item") {
                                    native_output.push(item.clone());
                                }
                            }
                            // Check if this is a completed function call
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(item) = v.get("item") {
                                    if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                                        if let Some(id) = item.get("id").and_then(|id| id.as_str()) {
                                            // Try to extract function name from the completed item
                                            let name = item
                                                .get("name")
                                                .and_then(|n| n.as_str())
                                                .map(|s| s.to_string())
                                                .or_else(|| {
                                                    item.get("function_name")
                                                        .and_then(|n| n.as_str())
                                                        .map(|s| s.to_string())
                                                })
                                                .or_else(|| {
                                                    item.get("function")
                                                        .and_then(|f| f.get("name"))
                                                        .and_then(|n| n.as_str())
                                                        .map(|s| s.to_string())
                                                });
                                            // Update the tools_map entry with the function name if we found it
                                            if let Some(name) = name {
                                                if let Some(entry) = tools_map.get_mut(id) {
                                                    if entry.name.is_none() {
                                                        entry.name = Some(name);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                break;
            }
        }
    }

    // A stop can arrive while waiting for EOF, including a stream with no chunks.
    if !completed && !stopped_by_user {
        if let Some(ref inbox) = inbox_name {
            if llm_stopper.should_stop(&inbox.to_string()) {
                llm_stopper.reset(&inbox.to_string());
                stopped_by_user = true;
                send_ws_update(
                    &ws_manager_trait,
                    Some(inbox.clone()),
                    &session_id,
                    response_text.clone(),
                    false,
                    true,
                    Some("Stopped by user request".to_string()),
                )
                .await?;
            }
        }
    }
    if !completed && !stopped_by_user {
        return Err(LLMProviderError::APIError(
            "OpenAI Responses stream ended before response.completed".into(),
        ));
    }

    // Completed native items contain the final arguments, call IDs, and encrypted reasoning.
    let native_calls = extract_native_function_calls(&native_output);
    if native_output.iter().any(|item| item["type"] == "function_call") {
        function_calls = native_calls;
    }

    Ok(LLMInferenceResponse::new(
        response_text,
        if reasoning_text.is_empty() {
            None
        } else {
            Some(reasoning_text)
        },
        json!({}),
        function_calls,
        Vec::new(),
        None,
    ))
}
fn native_function_identity(item: &JsonValue) -> Option<(&str, &str)> {
    if item["type"] != "function_call" {
        return None;
    }
    let name = item["name"].as_str().filter(|name| !name.is_empty())?;
    let call_id = item["call_id"].as_str().filter(|id| !id.is_empty())?;
    Some((name, call_id))
}

// Keep the native response together so parallel calls replay their shared reasoning once.
fn extract_native_function_calls(output: &[JsonValue]) -> Vec<FunctionCall> {
    output
        .iter()
        .filter(|item| item["type"] == "function_call")
        .filter_map(|item| {
            let (name, call_id) = native_function_identity(item)?;
            let arguments = match item.get("arguments") {
                Some(JsonValue::String(args)) => serde_json::from_str::<JsonValue>(args)
                    .map_err(|error| format!("Tool arguments are not valid JSON: {error}")),
                Some(args) => Ok(args.clone()),
                None => Err("Tool arguments are missing".to_string()),
            }
            .and_then(|args| {
                args.as_object()
                    .cloned()
                    .ok_or_else(|| "Tool arguments must be a JSON object".to_string())
            });
            let mut provider_metadata = json!({"openai_responses": {"output": output}});
            let arguments = match arguments {
                Ok(arguments) => arguments,
                Err(error) => {
                    // Retain the call for validation feedback, but prevent an empty map
                    // from executing a tool whose schema has no required arguments.
                    provider_metadata["function_argument_error"] = JsonValue::String(error);
                    Default::default()
                }
            };
            Some(FunctionCall {
                name: name.to_owned(),
                arguments,
                tool_router_key: None,
                response: None,
                index: 0,
                id: Some(call_id.to_owned()),
                call_type: Some("function".to_string()),
                // Keep signed reasoning and every original call. Replay supplies an explicit
                // error output for any call that has no recorded execution result.
                provider_metadata: Some(provider_metadata),
            })
        })
        .enumerate()
        .map(|(index, mut call)| {
            call.index = index as u64;
            call
        })
        .collect()
}

pub(super) fn openai_endpoint(base_url: &str, endpoint: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/{endpoint}")
    } else {
        format!("{base}/v1/{endpoint}")
    }
}

fn add_options_to_payload_responses(
    payload: &mut serde_json::Value,
    config: Option<&JobConfig>,
    is_reasoning_model: bool,
) {
    let prepared_budget = payload.get("max_output_tokens").and_then(JsonValue::as_u64);
    // Helper to read env var or config value
    fn read_env_var<T: std::str::FromStr>(key: &str) -> Option<T> {
        std::env::var(key).ok().and_then(|val| val.parse::<T>().ok())
    }
    fn get_value<T: Clone + std::str::FromStr>(env_key: &str, config_value: Option<&T>) -> Option<T> {
        config_value.cloned().or_else(|| read_env_var::<T>(env_key))
    }

    // Encrypted reasoning is a model capability, not a general text response field.
    if is_reasoning_model {
        payload["include"] = json!(["reasoning.encrypted_content"]);
    } else if let Some(include) = payload.get_mut("include").and_then(JsonValue::as_array_mut) {
        include.retain(|field| field != "reasoning.encrypted_content");
    }

    // Disable sampling params for reasoning-capable models
    if !is_reasoning_model {
        if let Some(temp) = get_value("LLM_TEMPERATURE", config.and_then(|c| c.temperature.as_ref())) {
            payload["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = get_value("LLM_TOP_P", config.and_then(|c| c.top_p.as_ref())) {
            payload["top_p"] = serde_json::json!(top_p);
        }
    } else if let Some(obj) = payload.as_object_mut() {
        obj.remove("temperature");
        obj.remove("top_p");
        obj.remove("top_k");
    }
    if let Some(max_tokens) = get_value("LLM_MAX_TOKENS", config.and_then(|c| c.max_tokens.as_ref())) {
        // Responses API uses max_output_tokens
        payload["max_output_tokens"] = serde_json::json!(max_tokens);
    }

    // Reasoning effort (only for reasoning-capable models)
    if is_reasoning_model {
        let thinking_enabled = config.and_then(|c| c.thinking).unwrap_or(false);
        if thinking_enabled {
            let mut effort = config
                .and_then(|c| c.reasoning_effort.clone())
                .unwrap_or_else(|| "medium".to_string());
            let model = payload["model"].as_str().unwrap_or_default();
            if ModelCapabilitiesManager::is_gpt6(model) && effort == "minimal"
                || matches!(model, "gpt-6-astra" | "gpt-6.1-sol") && effort == "none"
            {
                effort = "low".to_string();
            }
            payload["reasoning"] = json!({"effort": effort, "summary": "detailed"});
        } else if config.and_then(|c| c.thinking) == Some(false)
            && matches!(payload["model"].as_str(), Some("gpt-6-sol" | "gpt-6-luna"))
        {
            // An omitted effort uses the API default, so send none for an explicit disable.
            payload["reasoning"] = json!({"effort": "none"});
        } else if let Some(obj) = payload.as_object_mut() {
            obj.remove("reasoning");
        }
    } else if let Some(obj) = payload.as_object_mut() {
        // Ensure we don't send reasoning for non-reasoning models
        obj.remove("reasoning");
    }

    if let Some(other_params) = config.and_then(|c| c.other_model_params.as_ref()) {
        if let Some(obj) = other_params.as_object() {
            for (key, value) in obj {
                match key.as_str() {
                    // Responses has a different schema from Chat Completions.
                    "service_tier" | "parallel_tool_calls" | "store" | "truncation" => payload[key] = value.clone(),
                    "response_format" => {
                        let mut format = value.clone();
                        if value["type"] == "json_schema" {
                            if let Some(schema) = value.get("json_schema").and_then(|v| v.as_object()) {
                                format = JsonValue::Object(schema.clone());
                                format["type"] = json!("json_schema");
                            }
                        }
                        payload["text"]["format"] = format;
                    }
                    "temperature" | "top_p" if !is_reasoning_model => payload[key] = value.clone(),
                    "top_logprobs" if !is_reasoning_model => payload[key] = value.clone(),
                    _ => (),
                };
            }
        }
    }
    if let (Some(budget), Some(requested)) = (prepared_budget, payload["max_output_tokens"].as_u64()) {
        payload["max_output_tokens"] = json!(requested.min(budget));
    }
}

pub fn truncate_image_url_in_payload(payload: &mut JsonValue) {
    if let Some(messages) = payload.get_mut("input") {
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

fn missing_tool_output(call_id: &str) -> JsonValue {
    json!({
        "type": "function_call_output",
        "call_id": call_id,
        "output": json!({"error": {
            "type": "missing_tool_result",
            "message": "No execution result is recorded for this tool call."
        }}).to_string()
    })
}

fn transform_input_messages_for_responses(messages_json: serde_json::Value) -> serde_json::Value {
    // Expected input: Array of chat-style messages: [{role, content, images?, tool_calls? ...}]
    // Output: Array of Responses messages: [{role, content: [ {type: 'input_text'|'input_image'|...} ] }]
    use std::collections::{HashMap, HashSet};
    let mut out: Vec<serde_json::Value> = Vec::new();
    // Track assistant-emitted tool calls by id so we can pair them with outputs
    let mut call_map: HashMap<String, (String, String)> = HashMap::new(); // id -> (name, args)
    let mut emitted_call_ids: HashSet<String> = HashSet::new();
    // Tagged turns and legacy payload identities have separate namespaces.
    type NativeTurn = (bool, String);
    fn native_turn(message: &JsonValue) -> Option<(&Vec<JsonValue>, NativeTurn)> {
        if message["role"] != "assistant" {
            return None;
        }
        message.get("tool_calls")?.as_array()?.iter().find_map(|call| {
            let metadata = call
                .get("provider_metadata")
                .or_else(|| call.pointer("/function/provider_metadata"))?;
            let output = metadata.pointer("/openai_responses/output")?.as_array()?;
            let identity = match metadata["inference_turn_id"].as_str() {
                Some(turn) => (true, turn.to_owned()),
                None => (false, serde_json::to_string(output).unwrap()),
            };
            Some((output, identity))
        })
    }
    let mut emitted_native_outputs: HashSet<NativeTurn> = HashSet::new();
    let mut missing_result_ids: HashSet<(NativeTurn, String)> = HashSet::new();
    if let Some(arr) = messages_json.as_array() {
        // Parallel call/result pairs can be separated in Prompt history. Check the
        // whole history before deciding whether a native call has an execution result.
        let mut recorded_outputs: HashSet<(NativeTurn, String)> = HashSet::new();
        let mut active_turn = None;
        for message in arr {
            if let Some((_, identity)) = native_turn(message) {
                active_turn = Some(identity);
            } else if message["role"] == "tool" {
                if let (Some(turn), Some(id)) = (&active_turn, message["tool_call_id"].as_str()) {
                    recorded_outputs.insert((turn.clone(), id.to_owned()));
                }
            } else {
                active_turn = None;
            }
        }
        for msg in arr {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            if role == "assistant" {
                if let Some((items, turn)) = native_turn(msg) {
                    // Prompt history can split parallel calls across assistant/tool pairs.
                    // Each pair carries the same complete native response.
                    if !emitted_native_outputs.insert(turn.clone()) {
                        continue;
                    }
                    // Metadata is repeated on each call, but the response is replayed only once.
                    for item in items {
                        if item["type"] == "function_call" {
                            let Some((_, id)) = native_function_identity(item) else {
                                continue;
                            };
                            emitted_call_ids.insert(id.to_owned());
                        }
                        out.push(item.clone());
                    }
                    // Preserve opaque native reasoning and calls even when argument
                    // validation or execution skipped a call. A missing result is an
                    // error, not a successful tool execution or an orphaned call.
                    for item in items {
                        if let Some((_, id)) = native_function_identity(item) {
                            let result = (turn.clone(), id.to_owned());
                            if !recorded_outputs.contains(&result) && missing_result_ids.insert(result) {
                                out.push(missing_tool_output(id));
                            }
                        }
                    }
                    continue;
                }
            }
            // Skip legacy 'function' role
            if role == "function" {
                continue;
            }

            // Convert tool role messages into function_call_output items
            if role == "tool" {
                if let Some(call_id) = msg.get("tool_call_id").and_then(|v| v.as_str()) {
                    // Build output string from message content
                    let mut output_text = String::new();
                    if let Some(content) = msg.get("content") {
                        match content {
                            serde_json::Value::String(s) => {
                                output_text = s.clone();
                            }
                            serde_json::Value::Array(items) => {
                                for item in items {
                                    if let Some(itype) = item.get("type").and_then(|t| t.as_str()) {
                                        match itype {
                                            "text" | "input_text" | "output_text" => {
                                                if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                                                    if !output_text.is_empty() {
                                                        output_text.push_str("\n");
                                                    }
                                                    output_text.push_str(t);
                                                }
                                            }
                                            _ => {}
                                        }
                                    } else if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                                        if !output_text.is_empty() {
                                            output_text.push_str("\n");
                                        }
                                        output_text.push_str(t);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    // Emit the corresponding function_call (if captured) immediately before the output
                    if let Some((name, args)) = call_map.get(call_id).cloned() {
                        out.push(json!({
                            "type": "function_call",
                            "call_id": call_id,
                            "name": name,
                            "arguments": args,
                            "status": "completed"
                        }));
                        emitted_call_ids.insert(call_id.to_string());
                    }

                    out.push(json!({
                        "type": "function_call_output",
                        "call_id": call_id,
                        "output": output_text,
                    }));
                    continue;
                }
                // If no tool_call_id, fall through and treat as a regular message
            }

            // Capture assistant tool calls for later emission paired with their outputs
            if role == "assistant" {
                if let Some(tcs) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                    for tc in tcs {
                        if let (Some(id), Some(func)) = (tc.get("id").and_then(|i| i.as_str()), tc.get("function")) {
                            if let (Some(name), Some(args)) = (
                                func.get("name").and_then(|n| n.as_str()),
                                func.get("arguments").and_then(|a| a.as_str()),
                            ) {
                                call_map.insert(id.to_string(), (name.to_string(), args.to_string()));
                            }
                        }
                    }
                }
            }

            // Responses API message roles
            let normalized_role = role;

            let content = msg.get("content");
            if content.is_none() || content == Some(&serde_json::Value::Null) {
                // No assistant text to emit; function_call will be emitted when tool output arrives
                continue;
            }

            let mut content_blocks: Vec<serde_json::Value> = Vec::new();
            match content.unwrap() {
                serde_json::Value::String(s) => {
                    let block_type = if normalized_role == "assistant" {
                        "output_text"
                    } else {
                        "input_text"
                    };
                    content_blocks.push(json!({"type": block_type, "text": s}));
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        if let Some(itype) = item.get("type").and_then(|t| t.as_str()) {
                            match itype {
                                "text" => {
                                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                        let block_type = if normalized_role == "assistant" {
                                            "output_text"
                                        } else {
                                            "input_text"
                                        };
                                        content_blocks.push(json!({"type": block_type, "text": text}));
                                    }
                                }
                                "image_url" => {
                                    if let Some(image_url) = item.get("image_url") {
                                        // Extract the URL string from the image_url object
                                        if let Some(url) = image_url.get("url").and_then(|u| u.as_str()) {
                                            content_blocks.push(json!({"type":"input_image","image_url": url}));
                                        }
                                    }
                                }
                                // Allow already-correct inputs to pass through
                                "input_text" | "input_image" | "input_file" | "computer_screenshot" => {
                                    content_blocks.push(item.clone());
                                }
                                // Allow already-correct assistant outputs to pass through
                                "output_text" | "refusal" => {
                                    content_blocks.push(item.clone());
                                }
                                _ => {
                                    // Ignore unsupported types silently
                                }
                            }
                        } else if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            let block_type = if role == "assistant" {
                                "output_text"
                            } else {
                                "input_text"
                            };
                            content_blocks.push(json!({"type": block_type, "text": text}));
                        }
                    }
                }
                _ => {
                    // Not a supported format; skip this message
                    continue;
                }
            }

            if content_blocks.is_empty() {
                continue;
            }

            let mut new_msg = serde_json::Map::new();
            new_msg.insert("role".into(), json!(normalized_role));
            new_msg.insert("content".into(), serde_json::Value::Array(content_blocks));

            // For non-tool messages, just push the message

            out.push(serde_json::Value::Object(new_msg));

            // function_calls will be emitted when their outputs arrive
        }
    }
    // Emit any function_calls that never received outputs (fallback)
    for (id, (name, args)) in call_map.into_iter() {
        if !emitted_call_ids.contains(&id) {
            out.push(json!({
                "type": "function_call",
                "call_id": id,
                "name": name,
                "arguments": args,
                "status": "completed"
            }));
        }
    }
    serde_json::Value::Array(out)
}

#[cfg(test)]
mod tests {
    use super::JobConfig;
    use serde_json::json;

    fn replay_recorded_native_calls(output: &serde_json::Value, executed: &[&str]) -> serde_json::Value {
        let results = executed
            .iter()
            .map(|id| (*id, "actual tool result".to_string()))
            .collect::<Vec<_>>();
        replay_native_calls_with_results(output, &results)
    }

    #[test]
    fn saved_identical_native_turns_keep_parallel_calls_and_results_separate() {
        let native = json!([
            {"type":"reasoning","id":"rs_1","summary":[],"encrypted_content":"signed-opaque"},
            {"type":"function_call","id":"fc_A","call_id":"A","name":"first","arguments":"{}"},
            {"type":"function_call","id":"fc_B","call_id":"B","name":"second","arguments":"{}"}
        ]);
        // The first tag deliberately equals the legacy payload identity.
        let tag = serde_json::to_string(&native).unwrap();
        let mut prompt = super::Prompt::new();
        let mut expected = Vec::new();
        for (index, turn) in [Some(tag.as_str()), Some("second-turn"), None].into_iter().enumerate() {
            for call in super::extract_native_function_calls(native.as_array().unwrap()) {
                let mut metadata = call.provider_metadata.unwrap();
                if let Some(turn) = turn {
                    metadata["inference_turn_id"] = json!(turn);
                }
                prompt.add_function_call(
                    json!({"name":call.name,"call_id":call.id,
                    "arguments":call.arguments,"provider_metadata":metadata}),
                    100,
                );
            }
            expected.extend(native.as_array().unwrap().iter().cloned());
            if index == 1 {
                expected.push(super::missing_tool_output("B"));
            }
            for (id, name) in [("A", "first"), ("B", "second")] {
                if index == 1 && id == "B" {
                    continue;
                }
                let result = format!("turn {index}: {id}");
                prompt
                    .add_function_call_response(json!({"function_call":{"name":name,"id":id},"response":result}), 100);
                expected.push(json!({"type":"function_call_output","call_id":id,"output":result}));
            }
        }
        let saved: super::Prompt = serde_json::from_str(&serde_json::to_string(&prompt).unwrap()).unwrap();
        let model = super::LLMProviderInterface::OpenAI(super::OpenAI {
            model_type: "gpt-6.1-sol".into(),
        });
        let prepared = super::openai_prepare_messages(&model, saved).unwrap();
        let super::PromptResultEnum::Value(messages) = prepared.messages else {
            panic!("Expected messages")
        };
        let replay = super::transform_input_messages_for_responses(messages);
        assert_eq!(replay, json!(expected));
        assert!(!replay.to_string().contains("provider_metadata"));
        assert!(!replay.to_string().contains("inference_turn_id"));
    }

    fn replay_native_calls_with_results(output: &serde_json::Value, results: &[(&str, String)]) -> serde_json::Value {
        let calls = super::extract_native_function_calls(output.as_array().unwrap());
        let mut prompt = super::Prompt::new();
        for call in calls {
            prompt.add_function_call(
                json!({
                    "name": call.name, "arguments": call.arguments, "call_id": call.id,
                    "provider_metadata": call.provider_metadata
                }),
                100,
            );
            if let Some((_, result)) = results.iter().find(|(id, _)| Some(*id) == call.id.as_deref()) {
                prompt.add_function_call_response(
                    json!({
                        "function_call": {"name": call.name, "id": call.id}, "response": result
                    }),
                    100,
                );
            }
        }
        let model = super::LLMProviderInterface::OpenAI(super::OpenAI {
            model_type: "gpt-6.1-sol".into(),
        });
        // Exercise persisted metadata through the shared message converter as well.
        let prompt = serde_json::from_str(&serde_json::to_string(&prompt).unwrap()).unwrap();
        let prepared = super::openai_prepare_messages(&model, prompt).unwrap();
        let super::PromptResultEnum::Value(messages) = prepared.messages else {
            panic!("Expected messages")
        };
        super::transform_input_messages_for_responses(messages)
    }

    #[tokio::test]
    async fn responses_prepared_request_caps_output_for_direct_and_legacy_gpt6() {
        use shinkai_embedding::model_type::EmbeddingModelType;
        use shinkai_message_primitives::schemas::{
            llm_providers::serialized_llm_provider::OpenAILegacy, subprompts::SubPromptType,
        };
        let temp = tempfile::tempdir().unwrap();
        let db = std::sync::Arc::new(
            super::SqliteManager::new(
                temp.path().join("budget.db"),
                String::new(),
                EmbeddingModelType::default(),
            )
            .unwrap(),
        );
        for model in [
            super::LLMProviderInterface::OpenAI(super::OpenAI {
                model_type: "gpt-6.1-sol".into(),
            }),
            super::LLMProviderInterface::OpenAILegacy(OpenAILegacy {
                model_type: "gpt-6.1-sol".into(),
            }),
        ] {
            let mut prompt = super::Prompt::new();
            prompt.add_content("Reply briefly.".into(), SubPromptType::UserLastMessage, 100);
            let budget = super::openai_prepare_messages(&model, prompt.clone())
                .unwrap()
                .remaining_output_tokens as u64;
            assert!(budget > 1024);
            for requested in [1024, budget + 1] {
                let mut server = mockito::Server::new_async().await;
                let mock = server
                    .mock("POST", "/v1/responses")
                    .match_body(mockito::Matcher::PartialJson(
                        json!({"max_output_tokens":requested.min(budget)}),
                    ))
                    .with_header("content-type", "application/json")
                    .with_body(json!({"status":"completed","output":[]}).to_string())
                    .create_async()
                    .await;
                let mut config = super::JobConfig::empty();
                config.stream = Some(false);
                config.max_tokens = Some(requested);
                // Chat-style aliases remain unforwarded by the Responses schema allowlist.
                config.other_model_params =
                    Some(json!({"max_tokens":u64::MAX,"max_completion_tokens":u64::MAX,"max_output_tokens":u64::MAX}));
                let service: &dyn super::LLMService = match &model {
                    super::LLMProviderInterface::OpenAI(provider) => provider,
                    super::LLMProviderInterface::OpenAILegacy(provider) => provider,
                    _ => unreachable!(),
                };
                service
                    .call_api(
                        &reqwest::Client::new(),
                        Some(&server.url()),
                        Some(&"test-key".into()),
                        prompt.clone(),
                        model.clone(),
                        None,
                        None,
                        Some(config),
                        std::sync::Arc::new(super::LLMStopper::new()),
                        db.clone(),
                        None,
                    )
                    .await
                    .unwrap();
                mock.assert_async().await;
            }
        }
    }

    #[tokio::test]
    async fn responses_http_terminal_status_is_checked_before_returning_output() {
        for (status, error, incomplete, expected) in [
            (
                "failed",
                json!({"message":"provider failed"}),
                json!(null),
                Some("provider failed"),
            ),
            (
                "incomplete",
                json!(null),
                json!({"reason":"max_output_tokens"}),
                Some("max_output_tokens"),
            ),
            (
                "completed",
                json!({"message":"provider error"}),
                json!(null),
                Some("provider error"),
            ),
            ("completed", json!(null), json!(null), None),
        ] {
            let mut server = mockito::Server::new_async().await;
            let mock = server
                .mock("POST", "/responses")
                .with_header("content-type", "application/json")
                .with_body(
                    json!({"status":status,"error":error,"incomplete_details":incomplete,
                    "output":[{"type":"message","content":[{"type":"output_text","text":"answer"}]}]})
                    .to_string(),
                )
                .create_async()
                .await;
            let result = super::handle_non_streaming_response_responses(
                &reqwest::Client::new(),
                format!("{}/responses", server.url()),
                json!({}),
                "test-key".into(),
                None,
                None,
                "test".into(),
            )
            .await;
            mock.assert_async().await;
            if let Some(expected) = expected {
                let error = result.unwrap_err().to_string();
                assert!(error.contains(expected), "{error}");
            } else {
                assert_eq!(result.unwrap().response_string, "answer");
            }
        }
    }

    #[tokio::test]
    async fn responses_http_failure_without_details_omits_native_output_from_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/responses")
            .with_header("content-type", "application/json")
            .with_body(
                json!({"status":"failed", "error":null, "incomplete_details":null,
                "output":[{"type":"reasoning", "encrypted_content":"private-native-output"}]})
                .to_string(),
            )
            .create_async()
            .await;
        let error = super::handle_non_streaming_response_responses(
            &reqwest::Client::new(),
            format!("{}/responses", server.url()),
            json!({}),
            "test-key".into(),
            None,
            None,
            "test".into(),
        )
        .await
        .unwrap_err()
        .to_string();
        mock.assert_async().await;
        assert!(error.contains("failed"));
        assert!(error.contains("did not complete successfully"));
        assert!(!error.contains("private-native-output"));
        assert!(!error.contains("encrypted_content"));
        assert!(!error.contains("reasoning"));
    }

    #[tokio::test]
    async fn responses_sse_empty_completed_output_preserves_finished_native_items() {
        let mut server = mockito::Server::new_async().await;
        let reasoning = json!({"type":"reasoning","id":"rs_1","summary":[],"encrypted_content":"signed-opaque"});
        let call = json!({"type":"function_call","id":"fc_1","call_id":"call_1","name":"lookup","arguments":"{\"city\":\"Santiago\"}","status":"completed"});
        let body = format!(
            "event: response.output_item.done\ndata: {}\n\nevent: response.output_item.done\ndata: {}\n\nevent: response.completed\ndata: {}\n\n",
            json!({"item":reasoning}), json!({"item":call}),
            json!({"response":{"status":"completed","output":[]}}),
        );
        let mock = server
            .mock("POST", "/responses")
            .with_header("content-type", "text/event-stream")
            .with_body(body)
            .create_async()
            .await;
        let response = super::handle_streaming_response_responses(
            &reqwest::Client::new(),
            format!("{}/responses", server.url()),
            json!({}),
            "test-key".into(),
            None,
            None,
            std::sync::Arc::new(super::LLMStopper::new()),
            "test".into(),
        )
        .await
        .unwrap();
        mock.assert_async().await;
        assert_eq!(response.function_calls.len(), 1);
        assert_eq!(response.function_calls[0].id.as_deref(), Some("call_1"));
        assert_eq!(response.function_calls[0].arguments["city"], "Santiago");
        assert_eq!(
            response.function_calls[0].provider_metadata.as_ref().unwrap()["openai_responses"]["output"],
            json!([reasoning, call])
        );
    }

    #[tokio::test]
    async fn responses_sse_terminal_errors_do_not_return_partial_success() {
        for (event, details, expected) in [
            (
                "response.failed",
                json!({"type":"response.failed", "response":{"error":{"code":"server_error","message":"provider failed"}}}),
                "provider failed",
            ),
            (
                "response.incomplete",
                json!({"type":"response.incomplete", "response":{"incomplete_details":{"reason":"max_output_tokens"}}}),
                "max_output_tokens",
            ),
            (
                "error",
                json!({"type":"error", "code":"rate_limit_exceeded","message":"provider limit"}),
                "provider limit",
            ),
        ] {
            let mut server = mockito::Server::new_async().await;
            let body = format!("event: response.output_text.delta\ndata: {{\"delta\":\"partial\"}}\n\nevent: {event}\ndata: {details}\n\n");
            let mock = server
                .mock("POST", "/responses")
                .with_header("content-type", "text/event-stream")
                .with_body(body)
                .create_async()
                .await;
            let result = super::handle_streaming_response_responses(
                &reqwest::Client::new(),
                format!("{}/responses", server.url()),
                json!({"model":"gpt-6.1-sol","stream":true}),
                "test-key".into(),
                None,
                None,
                std::sync::Arc::new(super::LLMStopper::new()),
                "test".into(),
            )
            .await;
            mock.assert_async().await;
            let error = result.unwrap_err().to_string();
            assert!(error.contains(event), "{error}");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[tokio::test]
    async fn responses_sse_clean_eof_without_completion_rejects_partial_output() {
        for body in [
            "event: response.output_text.delta\ndata: {\"delta\":\"partial\"}\n\n".to_string(),
            format!(
                "event: response.output_item.done\ndata: {}\n\n",
                json!({"item":{
                    "type":"function_call","id":"fc_1","call_id":"call_1","name":"lookup","arguments":"{}","status":"completed"
                }})
            ),
        ] {
            let mut server = mockito::Server::new_async().await;
            let mock = server
                .mock("POST", "/responses")
                .with_header("content-type", "text/event-stream")
                .with_body(body)
                .create_async()
                .await;
            let result = super::handle_streaming_response_responses(
                &reqwest::Client::new(),
                format!("{}/responses", server.url()),
                json!({}),
                "test-key".into(),
                None,
                None,
                std::sync::Arc::new(super::LLMStopper::new()),
                "test".into(),
            )
            .await;
            mock.assert_async().await;
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("ended before response.completed"));
        }
    }

    #[tokio::test]
    async fn responses_sse_empty_body_requires_completion_unless_stopped() {
        for stopped in [false, true] {
            let mut server = mockito::Server::new_async().await;
            let mock = server
                .mock("POST", "/responses")
                .with_header("content-type", "text/event-stream")
                .with_body("")
                .create_async()
                .await;
            let inbox = super::InboxName::get_job_inbox_name_from_params("empty-stop-test".into()).unwrap();
            let stopper = std::sync::Arc::new(super::LLMStopper::new());
            if stopped {
                stopper.stop(&inbox.to_string());
            }
            let result = super::handle_streaming_response_responses(
                &reqwest::Client::new(),
                format!("{}/responses", server.url()),
                json!({}),
                "test-key".into(),
                Some(inbox.clone()),
                None,
                stopper.clone(),
                "test".into(),
            )
            .await;
            mock.assert_async().await;
            if stopped {
                let response = result.unwrap();
                assert!(response.response_string.is_empty());
                assert!(response.function_calls.is_empty());
                assert!(!stopper.should_stop(&inbox.to_string()));
            } else {
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains("ended before response.completed"));
            }
        }
    }

    #[tokio::test]
    async fn responses_sse_explicit_user_stop_does_not_require_completion() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/responses")
            .with_header("content-type", "text/event-stream")
            .with_body("event: response.output_text.delta\ndata: {\"delta\":\"not consumed\"}\n\n")
            .create_async()
            .await;
        let inbox = super::InboxName::get_job_inbox_name_from_params("stop-test".into()).unwrap();
        let stopper = std::sync::Arc::new(super::LLMStopper::new());
        stopper.stop(&inbox.to_string());
        let response = super::handle_streaming_response_responses(
            &reqwest::Client::new(),
            format!("{}/responses", server.url()),
            json!({}),
            "test-key".into(),
            Some(inbox.clone()),
            None,
            stopper.clone(),
            "test".into(),
        )
        .await
        .unwrap();
        mock.assert_async().await;
        assert!(response.response_string.is_empty());
        assert!(response.function_calls.is_empty());
        assert!(!stopper.should_stop(&inbox.to_string()));
    }

    #[tokio::test]
    async fn responses_sse_completed_stream_returns_success() {
        let mut server = mockito::Server::new_async().await;
        let body = "event: response.output_text.delta\ndata: {\"delta\":\"complete answer\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",\"output\":[]}}\n\n";
        let mock = server
            .mock("POST", "/responses")
            .with_header("content-type", "text/event-stream")
            .with_body(body)
            .create_async()
            .await;
        let response = super::handle_streaming_response_responses(
            &reqwest::Client::new(),
            format!("{}/responses", server.url()),
            json!({"model":"gpt-6.1-sol","stream":true}),
            "test-key".into(),
            None,
            None,
            std::sync::Arc::new(super::LLMStopper::new()),
            "test".into(),
        )
        .await
        .unwrap();
        mock.assert_async().await;
        assert_eq!(response.response_string, "complete answer");
    }

    #[test]
    fn native_calls_with_unusable_identity_are_not_replayed() {
        for invalid in [
            json!({"type": "function_call", "name": "lookup", "arguments": "{}"}),
            json!({"type": "function_call", "call_id": "", "name": "lookup", "arguments": "{}"}),
            json!({"type": "function_call", "call_id": "bad", "arguments": "{}"}),
            json!({"type": "function_call", "call_id": "bad", "name": "", "arguments": "{}"}),
        ] {
            let reasoning = json!({"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "opaque"});
            let valid = json!({"type": "function_call", "id": "fc_good", "call_id": "good", "name": "lookup", "arguments": "{}"});
            let output = json!([reasoning, invalid, valid]);
            let replay = replay_recorded_native_calls(&output, &["good"]);
            assert_eq!(
                replay,
                json!([
                    reasoning, valid,
                    {"type": "function_call_output", "call_id": "good", "output": "actual tool result"}
                ])
            );
        }
    }

    #[test]
    fn malformed_parallel_call_arguments_replay_as_explicit_missing_results() {
        for invalid_arguments in ["{not-json", "[]"] {
            let output = json!([
                {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "signed-opaque"},
                {"type": "function_call", "id": "fc_bad", "call_id": "bad", "name": "lookup", "arguments": invalid_arguments},
                {"type": "function_call", "id": "fc_good", "call_id": "good", "name": "lookup", "arguments": "{}"}
            ]);
            let calls = super::extract_native_function_calls(output.as_array().unwrap());
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].id.as_deref(), Some("bad"));
            assert!(calls[0].provider_metadata.as_ref().unwrap()["function_argument_error"].is_string());
            assert_eq!(calls[1].id.as_deref(), Some("good"));
            let replay = replay_recorded_native_calls(&output, &["good"]);
            let replay = replay.as_array().unwrap();
            assert_eq!(&replay[..3], output.as_array().unwrap().as_slice());
            assert_eq!(replay.len(), 5);
            assert_eq!(replay[3], super::missing_tool_output("bad"));
            assert_eq!(replay[4]["call_id"], "good");
            assert_eq!(replay[4]["output"], "actual tool result");
        }
    }

    #[tokio::test]
    async fn invalid_native_arguments_remain_calls_and_cannot_execute_zero_argument_tools() {
        use crate::llm_provider::execution::chains::generic_chain::generic_inference_chain::execute_valid_function_call;
        use crate::managers::tool_router::ToolCallFunctionResponse;

        for arguments in [json!("{not-json"), json!("[]"), json!([]), json!(null), json!("")] {
            let output = json!([
                {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "signed-opaque"},
                {"type": "function_call", "id": "fc_bad", "call_id": "bad", "name": "zero_arguments", "arguments": arguments}
            ]);
            let calls = super::extract_native_function_calls(output.as_array().unwrap());
            let response = super::LLMInferenceResponse::new(String::new(), None, json!({}), calls, vec![], None);
            assert!(!response.is_function_calls_empty());
            let call = &response.function_calls[0];
            assert_eq!(call.name, "zero_arguments");
            assert_eq!(call.id.as_deref(), Some("bad"));
            assert!(call.arguments.is_empty());
            // The marker survives storage without changing the FunctionCall schema.
            let call = serde_json::from_value::<super::FunctionCall>(serde_json::to_value(call).unwrap()).unwrap();
            let executed = std::cell::Cell::new(false);
            let result = execute_valid_function_call(&call, async {
                // A zero-argument tool would accept the empty map if this future ran.
                executed.set(true);
                Ok(ToolCallFunctionResponse {
                    function_call: call.clone(),
                    response: "must not run".into(),
                })
            })
            .await;
            assert!(!executed.get());
            let Err(super::LLMProviderError::ToolRouterError(error)) = result else {
                panic!("Invalid arguments must enter the tool validation retry path");
            };
            assert!(error.starts_with("Invalid function arguments:"));
            let replay = replay_native_calls_with_results(&output, &[("bad", error.clone())]);
            assert_eq!(
                replay,
                json!([
                    output[0], output[1],
                    {"type": "function_call_output", "call_id": "bad", "output": error}
                ])
            );
        }
    }

    #[tokio::test]
    async fn native_argument_retry_retains_earlier_parallel_result_and_later_unexecuted_call() {
        use crate::llm_provider::execution::chains::generic_chain::generic_inference_chain::execute_valid_function_call;
        use crate::managers::tool_router::ToolCallFunctionResponse;

        let output = json!([
            {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "signed-opaque"},
            {"type": "function_call", "id": "fc_good", "call_id": "good", "name": "zero_arguments", "arguments": "{}"},
            {"type": "function_call", "id": "fc_bad", "call_id": "bad", "name": "zero_arguments", "arguments": "[]"},
            {"type": "function_call", "id": "fc_later", "call_id": "later", "name": "zero_arguments", "arguments": "{}"}
        ]);
        let calls = super::extract_native_function_calls(output.as_array().unwrap());
        assert_eq!(calls.len(), 3);
        let executed = std::cell::Cell::new(0);
        let result = execute_valid_function_call(&calls[0], async {
            executed.set(executed.get() + 1);
            Ok(ToolCallFunctionResponse {
                function_call: calls[0].clone(),
                response: "actual tool result".into(),
            })
        })
        .await
        .unwrap();
        let error = execute_valid_function_call(&calls[1], async {
            executed.set(executed.get() + 1);
            Ok(ToolCallFunctionResponse {
                function_call: calls[1].clone(),
                response: "must not run".into(),
            })
        })
        .await;
        assert_eq!(executed.get(), 1);
        let Err(super::LLMProviderError::ToolRouterError(error)) = error else {
            panic!("Invalid arguments must enter the tool validation retry path");
        };
        let replay = replay_native_calls_with_results(&output, &[("good", result.response), ("bad", error.clone())]);
        let replay = replay.as_array().unwrap();
        assert_eq!(&replay[..4], output.as_array().unwrap().as_slice());
        assert_eq!(replay.len(), 7);
        assert_eq!(replay[4], super::missing_tool_output("later"));
        assert_eq!(
            replay[5],
            json!({"type": "function_call_output", "call_id": "good", "output": "actual tool result"})
        );
        assert_eq!(
            replay[6],
            json!({"type": "function_call_output", "call_id": "bad", "output": error})
        );
    }

    #[test]
    fn native_parallel_calls_with_partial_execution_have_error_outputs() {
        let output = json!([
            {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "signed-opaque"},
            {"type": "function_call", "id": "fc_a", "call_id": "a", "name": "lookup", "arguments": "{}"},
            {"type": "function_call", "id": "fc_b", "call_id": "b", "name": "lookup", "arguments": "{}"}
        ]);
        for executed in [vec!["a"], vec!["b"]] {
            let replay = replay_recorded_native_calls(&output, &executed);
            let replay = replay.as_array().unwrap();
            assert_eq!(&replay[..3], output.as_array().unwrap().as_slice());
            assert_eq!(replay.len(), 5);
            let missing = if executed[0] == "a" { "b" } else { "a" };
            assert_eq!(replay[3], super::missing_tool_output(missing));
            assert_eq!(replay[4]["call_id"], executed[0]);
            assert_eq!(replay[4]["output"], "actual tool result");
        }
    }

    #[test]
    fn encrypted_reasoning_is_requested_only_for_reasoning_models() {
        for (model, reasoning) in [("gpt-4o", false), ("gpt-4o-mini", false), ("gpt-6.1-sol", true)] {
            let mut payload = json!({"model": model, "store": false});
            super::add_options_to_payload_responses(&mut payload, None, reasoning);
            assert_eq!(payload["store"], false);
            if reasoning {
                assert_eq!(payload["include"], json!(["reasoning.encrypted_content"]));
            } else {
                assert!(payload.get("include").is_none());
            }
        }
        let mut payload =
            json!({"model": "gpt-4o", "include": ["reasoning.encrypted_content", "message.output_text.logprobs"]});
        super::add_options_to_payload_responses(&mut payload, None, false);
        assert_eq!(payload["include"], json!(["message.output_text.logprobs"]));
    }

    #[test]
    fn test_prompt_parallel_calls_replay_native_output_once() {
        let output = json!([
            {"type": "reasoning", "id": "rs_parallel", "summary": [], "encrypted_content": "opaque"},
            {"type": "function_call", "id": "fc_a", "call_id": "call_a", "name": "lookup",
             "arguments": "{\"key\":1}", "status": "completed"},
            {"type": "function_call", "id": "fc_b", "call_id": "call_b", "name": "lookup",
             "arguments": "{\"key\":2}", "status": "completed"}
        ]);
        let calls = super::extract_native_function_calls(output.as_array().unwrap());
        let mut prompt = super::Prompt::new();
        for call in calls {
            prompt.add_function_call(
                json!({
                    "name": call.name, "arguments": call.arguments, "call_id": call.id,
                    "provider_metadata": call.provider_metadata
                }),
                100,
            );
            prompt.add_function_call_response(
                json!({
                    "function_call": {"name": call.name, "id": call.id}, "response": "found"
                }),
                100,
            );
        }
        let prompt = serde_json::from_str(&serde_json::to_string(&prompt).unwrap()).unwrap();
        let model = super::LLMProviderInterface::OpenAI(super::OpenAI {
            model_type: "gpt-6.1-sol".to_string(),
        });
        let prepared = super::openai_prepare_messages(&model, prompt).unwrap();
        let super::PromptResultEnum::Value(messages) = prepared.messages else {
            panic!("Expected messages")
        };
        let replay = super::transform_input_messages_for_responses(messages);
        let replay = replay.as_array().unwrap();
        assert_eq!(replay.len(), 5);
        assert_eq!(&replay[..3], output.as_array().unwrap().as_slice());
        assert_eq!(replay[3]["call_id"], "call_a");
        assert_eq!(replay[4]["call_id"], "call_b");
        assert_eq!(replay[3]["type"], "function_call_output");
        assert_eq!(replay[4]["type"], "function_call_output");
    }

    #[test]
    fn test_native_reasoning_parallel_calls_round_trip() {
        let output = json!([
            {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "encrypted-state"},
            {"type": "message", "id": "msg_1", "status": "completed", "role": "assistant",
             "content": [{"type": "output_text", "text": "I will check both." , "annotations": []}]},
            {"type": "function_call", "id": "fc_1", "call_id": "call_1", "name": "lookup",
             "arguments": "{\"key\":1}", "status": "completed"},
            {"type": "function_call", "id": "fc_2", "call_id": "call_2", "name": "lookup",
             "arguments": "{\"key\":2}", "status": "completed"}
        ]);
        let calls = super::extract_native_function_calls(output.as_array().unwrap());
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id.as_deref(), Some("call_1"));
        assert_eq!(calls[1].index, 1);
        let tool_calls: Vec<_> = calls
            .iter()
            .map(|call| {
                json!({
                    "id": call.id, "type": "function", "provider_metadata": call.provider_metadata,
                    "function": {"name": call.name, "arguments": serde_json::to_string(&call.arguments).unwrap()}
                })
            })
            .collect();
        // A database serialization round trip must keep the opaque reasoning data.
        let messages = json!([
            {"role": "assistant", "content": "I will check both.", "tool_calls": tool_calls},
            {"role": "tool", "tool_call_id": "call_1", "content": "one"},
            {"role": "tool", "tool_call_id": "call_2", "content": "two"}
        ]);
        let restored = serde_json::from_str(&serde_json::to_string(&messages).unwrap()).unwrap();
        let replay = super::transform_input_messages_for_responses(restored);
        let replay = replay.as_array().unwrap();
        assert_eq!(replay.len(), 6);
        assert_eq!(&replay[..4], output.as_array().unwrap().as_slice());
        assert_eq!(
            replay[4],
            json!({"type": "function_call_output", "call_id": "call_1", "output": "one"})
        );
        assert_eq!(
            replay[5],
            json!({"type": "function_call_output", "call_id": "call_2", "output": "two"})
        );
    }

    #[test]
    fn test_current_responses_options() {
        let config: JobConfig = serde_json::from_value(json!({
            "thinking": true,
            "reasoning_effort": "none",
            "temperature": 0.3,
            "other_model_params": {
                "frequency_penalty": 1, "logprobs": true, "top_logprobs": 2,
                "n": 2, "stop": ["end"], "response_format": {"type": "json_object"}
            }
        }))
        .unwrap();
        for model in ["gpt-6-astra", "gpt-6.1-sol"] {
            let mut payload = json!({"model": model});
            super::add_options_to_payload_responses(&mut payload, Some(&config), true);
            assert_eq!(payload["reasoning"]["effort"], "low");
            assert_eq!(payload["text"]["format"]["type"], "json_object");
            for key in [
                "temperature",
                "frequency_penalty",
                "logprobs",
                "top_logprobs",
                "n",
                "stop",
                "response_format",
                "seed",
            ] {
                assert!(payload.get(key).is_none(), "unexpected Responses option: {key}");
            }
        }
    }

    #[test]
    fn test_responses_optional_reasoning_requires_explicit_disable() {
        for model in ["gpt-6-sol", "gpt-6-luna"] {
            let mut config = JobConfig::empty();
            for thinking in [None, Some(false)] {
                config.thinking = thinking;
                let mut payload = json!({"model": model, "reasoning": {"effort": "high"}});
                super::add_options_to_payload_responses(&mut payload, Some(&config), true);
                if thinking == Some(false) {
                    assert_eq!(payload["reasoning"], json!({"effort": "none"}));
                } else {
                    assert!(payload.get("reasoning").is_none());
                }
            }
            let mut payload = json!({"model": model});
            super::add_options_to_payload_responses(&mut payload, None, true);
            assert!(payload.get("reasoning").is_none());
        }
    }

    #[test]
    fn test_responses_reasoning_disable_preserves_other_models() {
        let mut config = JobConfig::empty();
        config.thinking = Some(false);
        for model in ["gpt-6-astra", "gpt-6.1-sol", "o3", "gpt-6-sol-unknown"] {
            let mut payload = json!({"model": model, "reasoning": {"effort": "high"}});
            super::add_options_to_payload_responses(&mut payload, Some(&config), true);
            assert!(payload.get("reasoning").is_none(), "{model}");
        }
        let mut payload = json!({"model": "gpt-6-sol"});
        super::add_options_to_payload_responses(&mut payload, Some(&config), false);
        assert!(payload.get("reasoning").is_none());
    }

    #[test]
    fn test_responses_current_models_preserve_xhigh_effort() {
        let mut config = JobConfig::empty();
        config.thinking = Some(true);
        config.reasoning_effort = Some("xhigh".to_string());
        for model in ["gpt-6-sol", "gpt-6-luna", "gpt-6-astra", "gpt-6.1-sol"] {
            let mut payload = json!({"model": model});
            super::add_options_to_payload_responses(&mut payload, Some(&config), true);
            assert_eq!(payload["reasoning"]["effort"], "xhigh", "{model}");
        }
    }

    #[test]
    fn test_responses_json_schema_format() {
        let mut config = JobConfig::empty();
        config.other_model_params = Some(json!({"response_format": {
            "type": "json_schema", "json_schema": {
                "name": "answer", "strict": true,
                "schema": {"type": "object", "properties": {}, "additionalProperties": false}
            }
        }}));
        let mut payload = json!({"model": "gpt-6.1-sol"});
        super::add_options_to_payload_responses(&mut payload, Some(&config), true);
        assert_eq!(payload["text"]["format"]["name"], "answer");
        assert_eq!(payload["text"]["format"]["type"], "json_schema");
        assert!(payload["text"]["format"].get("json_schema").is_none());
    }

    #[test]
    fn test_openai_endpoint() {
        for base in [
            "https://api.openai.com",
            "https://api.openai.com/",
            "https://api.openai.com/v1",
            "https://api.openai.com/v1/",
        ] {
            assert_eq!(
                super::openai_endpoint(base, "responses"),
                "https://api.openai.com/v1/responses"
            );
        }
    }

    #[test]
    fn test_extract_reasoning_summary_from_output() {
        let sample = json!({
            "id": "resp_68b9",
            "object": "response",
            "output": [
                {
                    "id": "rs_1",
                    "type": "reasoning",
                    "summary": [
                        {"type": "summary_text", "text": "Reasoning summary text here."}
                    ]
                },
                {
                    "id": "msg_1",
                    "type": "message",
                    "status": "completed",
                    "content": [
                        {"type": "output_text", "text": "Hello world"}
                    ],
                    "role": "assistant"
                }
            ]
        });

        // Test the reasoning extraction logic directly
        let mut reasoning_text = String::new();
        if let Some(output_items) = sample.get("output").and_then(|v| v.as_array()) {
            for item in output_items {
                if item.get("type").and_then(|t| t.as_str()) == Some("reasoning") {
                    if let Some(summary) = item.get("summary").and_then(|s| s.as_array()) {
                        for summary_item in summary {
                            if summary_item.get("type").and_then(|t| t.as_str()) == Some("summary_text") {
                                if let Some(text) = summary_item.get("text").and_then(|t| t.as_str()) {
                                    reasoning_text.push_str(text);
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(reasoning_text.contains("Reasoning summary text here."));
    }

    #[test]
    fn test_extract_function_call_from_output() {
        let sample = json!({
            "id": "resp_68b9c72315a48196a6a01a649f5b9a0804a6a6bd5a4c2ba5",
            "object": "response",
            "created_at": 1757005603,
            "status": "completed",
            "output": [
                {
                    "id": "rs_68b9c723fe6c8196993c85e5dac3cdff04a6a6bd5a4c2ba5",
                    "type": "reasoning",
                    "summary": []
                },
                {
                    "id": "fc_68b9c724c2588196bd96351b03ff6db304a6a6bd5a4c2ba5",
                    "type": "function_call",
                    "status": "completed",
                    "arguments": "{\"lang\":\"en\",\"url\":\"https://www.youtube.com/watch?v=a10M_i42z7M\"}",
                    "call_id": "call_m5OS4MrR1ywdFsxcDnhKcmj9",
                    "name": "youtube_transcript_extractor_2_0"
                }
            ]
        });

        // Test function call extraction
        let mut function_calls = Vec::new();
        if let Some(output_items) = sample.get("output").and_then(|v| v.as_array()) {
            for item in output_items {
                if let Some(item_type) = item.get("type").and_then(|t| t.as_str()) {
                    if item_type.eq_ignore_ascii_case("function_call") {
                        let function_obj = item.get("function");
                        let name_opt = item
                            .get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                function_obj
                                    .and_then(|f| f.get("name"))
                                    .and_then(|n| n.as_str())
                                    .map(|s| s.to_string())
                            });
                        let id_opt = item
                            .get("call_id")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| item.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()));

                        if let Some(name) = name_opt {
                            let raw_args = item
                                .get("arguments")
                                .cloned()
                                .or_else(|| function_obj.and_then(|f| f.get("arguments")).cloned());

                            let args_map = match raw_args {
                                Some(serde_json::Value::String(s)) => serde_json::from_str::<serde_json::Value>(&s)
                                    .ok()
                                    .and_then(|v| v.as_object().cloned())
                                    .unwrap_or_default(),
                                Some(serde_json::Value::Object(map)) => map,
                                _ => serde_json::Map::new(),
                            };

                            function_calls.push((name, args_map, id_opt));
                        }
                    }
                }
            }
        }

        // Assertions
        assert_eq!(function_calls.len(), 1);
        let (name, args, id) = &function_calls[0];
        assert_eq!(name, "youtube_transcript_extractor_2_0");
        assert_eq!(id, &Some("call_m5OS4MrR1ywdFsxcDnhKcmj9".to_string()));
        assert_eq!(args.get("lang").and_then(|v| v.as_str()), Some("en"));
        assert_eq!(
            args.get("url").and_then(|v| v.as_str()),
            Some("https://www.youtube.com/watch?v=a10M_i42z7M")
        );
    }

    #[test]
    fn test_transform_input_messages_for_responses_image_url() {
        use super::transform_input_messages_for_responses;

        let messages_json = json!([
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "What's in this image?"
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg=="
                        }
                    }
                ]
            }
        ]);

        let result = transform_input_messages_for_responses(messages_json);

        // Verify the structure
        let result_array = result.as_array().unwrap();
        assert_eq!(result_array.len(), 1);

        let message = &result_array[0];
        assert_eq!(message.get("role").unwrap(), "user");

        let content = message.get("content").unwrap().as_array().unwrap();
        assert_eq!(content.len(), 2);

        // Check text block
        let text_block = &content[0];
        assert_eq!(text_block.get("type").unwrap(), "input_text");
        assert_eq!(text_block.get("text").unwrap(), "What's in this image?");

        // Check image block - should have URL as string, not object
        let image_block = &content[1];
        assert_eq!(image_block.get("type").unwrap(), "input_image");
        let image_url = image_block.get("image_url").unwrap();
        // This should be a string, not an object
        assert!(image_url.is_string());
        assert_eq!(image_url.as_str().unwrap(), "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==");
    }

    #[test]
    fn test_transform_tool_to_function_call_output() {
        use super::transform_input_messages_for_responses;

        let messages_json = json!([
            {
                "role": "user",
                "content": "Use youtube tool"
            },
            {
                "role": "tool",
                "tool_call_id": "call_123",
                "content": [
                    {
                        "type": "text",
                        "text": "Tool execution failed with error"
                    }
                ]
            }
        ]);

        let result = transform_input_messages_for_responses(messages_json);

        // Verify the structure
        let result_array = result.as_array().unwrap();
        assert_eq!(result_array.len(), 2);

        // Check first message (user)
        let user_message = &result_array[0];
        assert_eq!(user_message.get("role").unwrap(), "user");

        // Check second item: function_call_output
        let fco = &result_array[1];
        assert_eq!(fco.get("type").and_then(|v| v.as_str()), Some("function_call_output"));
        assert_eq!(fco.get("call_id").and_then(|v| v.as_str()), Some("call_123"));
        assert_eq!(
            fco.get("output").and_then(|v| v.as_str()),
            Some("Tool execution failed with error")
        );
    }

    #[test]
    fn test_transform_complete_function_call_flow() {
        use super::transform_input_messages_for_responses;

        let messages_json = json!([
            {
                "role": "user",
                "content": "Use YouTube tool to get transcript"
            },
            {
                "role": "assistant",
                "content": "I'll get the transcript for you.",
                "tool_calls": [
                    {
                        "id": "call_123456",
                        "type": "function",
                        "function": {
                            "name": "youtube_transcript_extractor_2_0",
                            "arguments": "{\"url\":\"https://www.youtube.com/watch?v=abc123\",\"lang\":\"en\"}"
                        }
                    }
                ]
            },
            {
                "role": "tool",
                "tool_call_id": "call_123456",
                "content": "Transcript extracted successfully"
            }
        ]);

        let result = transform_input_messages_for_responses(messages_json);
        let result_array = result.as_array().unwrap();

        // Should have: user message, assistant message, function_call item, function_call_output
        assert_eq!(result_array.len(), 4);

        // First: user message
        assert_eq!(result_array[0].get("role").and_then(|r| r.as_str()), Some("user"));

        // Second: assistant message
        assert_eq!(result_array[1].get("role").and_then(|r| r.as_str()), Some("assistant"));

        // Third: function_call item
        let function_call = &result_array[2];
        assert_eq!(
            function_call.get("type").and_then(|t| t.as_str()),
            Some("function_call")
        );
        assert_eq!(
            function_call.get("call_id").and_then(|id| id.as_str()),
            Some("call_123456")
        );
        assert_eq!(
            function_call.get("name").and_then(|n| n.as_str()),
            Some("youtube_transcript_extractor_2_0")
        );
        assert_eq!(function_call.get("status").and_then(|s| s.as_str()), Some("completed"));

        // Fourth: function_call_output item
        assert_eq!(
            result_array[3].get("type").and_then(|t| t.as_str()),
            Some("function_call_output")
        );
        assert_eq!(
            result_array[3].get("call_id").and_then(|v| v.as_str()),
            Some("call_123456")
        );
        assert_eq!(
            result_array[3].get("output").and_then(|t| t.as_str()),
            Some("Transcript extracted successfully")
        );
    }

    #[test]
    fn test_streaming_tool_call_id_field_extraction() {
        // Test the field extraction logic used in streaming tool call events

        // Test call_id is preferred over id
        let event_with_both = json!({
            "call_id": "call_123456",
            "id": "regular_id_123",
            "name": "test_tool"
        });

        let extracted_id = event_with_both
            .get("call_id")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                event_with_both
                    .get("id")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
            });
        assert_eq!(extracted_id, Some("call_123456".to_string()));

        // Test fallback to id when call_id is missing
        let event_with_id_only = json!({
            "id": "regular_id_123",
            "name": "test_tool"
        });

        let extracted_id = event_with_id_only
            .get("call_id")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                event_with_id_only
                    .get("id")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
            });
        assert_eq!(extracted_id, Some("regular_id_123".to_string()));

        // Test no extraction when neither field is present
        let event_without_ids = json!({
            "name": "test_tool"
        });

        let extracted_id = event_without_ids
            .get("call_id")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                event_without_ids
                    .get("id")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
            });
        assert_eq!(extracted_id, None);
    }
}
