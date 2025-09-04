use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::openai_api::openai_prepare_messages;
use super::shared::shared_model_logic::{send_tool_ws_update, send_ws_update};
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, LLMInferenceResponse};
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResultEnum};
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
use tokio::sync::Mutex;
use futures::StreamExt;
use std::collections::HashMap;

// Note: This is an initial implementation of OpenAI's new Responses API.
// It intentionally focuses on non-streaming responses first for stability.

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

#[allow(clippy::too_many_arguments)]
pub async fn call_api(
    openai: &OpenAI,
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
    if let Some(base_url) = url {
        if let Some(key) = api_key {
            // Use the Responses API endpoint
            let url = format!("{}{}", base_url, "/v1/responses");

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
                "model": openai.model_type,
                "input": input_messages,
                "max_output_tokens": result.remaining_output_tokens,
                "stream": is_stream,
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
                        .or_else(|| fn_obj.and_then(|f| f.get("name")).and_then(|v| v.as_str()).map(|s| s.to_string()));

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
                    if let Some(desc) = description { obj.insert("description".to_string(), desc); }
                    obj.insert("parameters".to_string(), parameters);
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
                    eprintln!("failed to add payload trace: {:?}", e);
                }
            }

            eprintln!("Call API Body: {}", serde_json::to_string_pretty(&payload).unwrap());

            if is_stream {
                handle_streaming_response_responses(
                    client,
                    url,
                    payload,
                    key.to_string(),
                    inbox_name,
                    ws_manager_trait,
                    _llm_stopper,
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

async fn handle_non_streaming_response_responses(
    client: &Client,
    url: String,
    payload: JsonValue,
    api_key: String,
    inbox_name: Option<InboxName>,
    ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
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

    eprintln!("Response JSON: {}", serde_json::to_string_pretty(&response_json).unwrap());

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

    // 3) Tool/function calls (Responses API)
    let mut function_calls: Vec<FunctionCall> = Vec::new();

    // Responses API may include an `output` array with items of various types
    if let Some(output_items) = response_json.get("output").and_then(|v| v.as_array()) {
        for item in output_items {
            // Heuristic: check common fields
            if let Some(item_type) = item.get("type").and_then(|t| t.as_str()) {
                if item_type.eq_ignore_ascii_case("tool_call") || item_type.eq_ignore_ascii_case("function_call") {
                    // Example expected shapes (best-effort):
                    // - {"type":"tool_call","id":"...","name":"foo","arguments":{...} | "{...}"}
                    // - {"type":"function_call","id":"...","function":{"name":"foo","arguments":{...} | "{...}"}}
                    let function_obj = item.get("function");
                    let name_opt = item
                        .get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| function_obj.and_then(|f| f.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string()));
                    let id_opt = item.get("call_id").and_then(|id| id.as_str()).map(|s| s.to_string())
                        .or_else(|| item.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()));

                    if let Some(name) = name_opt {
                        let raw_args = item
                            .get("arguments")
                            .cloned()
                            .or_else(|| function_obj.and_then(|f| f.get("arguments")).cloned());

                        let args_map = match raw_args {
                            Some(JsonValue::String(s)) => serde_json::from_str::<serde_json::Value>(&s)
                                .ok()
                                .and_then(|v| v.as_object().cloned())
                                .unwrap_or_default(),
                            Some(JsonValue::Object(map)) => map,
                            _ => serde_json::Map::new(),
                        };

                        function_calls.push(FunctionCall {
                            name,
                            arguments: args_map,
                            tool_router_key: None,
                            response: None,
                            index: function_calls.len() as u64,
                            id: id_opt,
                            call_type: Some("function".to_string()),
                        });
                    }
                }
            }
        }
    }

    // Send a final WebSocket update if needed
    if let Some(inbox_name) = inbox_name.as_ref() {
        // If we have a function call, emit the tool update as well (use the last one)
        if let Some(last_function_call) = function_calls.last() {
            let _ = send_tool_ws_update(&ws_manager_trait, Some(inbox_name.clone()), last_function_call).await;
        }
        
        // Responses API does not use session ids here; reuse empty session
        let _ = send_ws_update(
            &ws_manager_trait,
            Some(inbox_name.clone()),
            "",
            response_text.clone(),
            false,
            function_calls.is_empty(),
            None,
        )
        .await;
    }

    Ok(LLMInferenceResponse::new(
        response_text,
        if reasoning_text.is_empty() { None } else { Some(reasoning_text) },
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
        return Err(LLMProviderError::APIError("AI Provider API Error: Unknown error occurred".to_string()));
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

    // SSE parsing loop
    while let Some(item) = stream.next().await {
        // Stopping support
        if let Some(ref inbox_name) = inbox_name {
            if llm_stopper.should_stop(&inbox_name.to_string()) {
                llm_stopper.reset(&inbox_name.to_string());
                // notify done
                super::shared::shared_model_logic::send_ws_update(
                    &ws_manager_trait,
                    Some(inbox_name.clone()),
                    "",
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
                        if !data_buf.is_empty() { data_buf.push('\n'); }
                        data_buf.push_str(rest);
                    }
                }

                if let Some(ev) = event_type {
                    // Debug: log all events to understand what we're receiving  
                    eprintln!("DEBUG Streaming event: '{}' with data: '{}'", ev, data_buf.chars().take(200).collect::<String>());
                    
                    // handle event
                    match ev.as_str() {
                        // Text deltas
                        "response.output_text.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(delta) = v.get("delta").and_then(|d| d.as_str()) {
                                    response_text.push_str(delta);
                                    if let Some(ref inbox) = inbox_name {
                                        super::shared::shared_model_logic::send_ws_update(
                                            &ws_manager_trait,
                                            Some(inbox.clone()),
                                            "",
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
                                    "",
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
                                            "",
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
                                    "",
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
                                if let Some(id) = v.get("call_id").and_then(|s| s.as_str()).map(|s| s.to_string())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string())) {
                                    let name = v.get("name").and_then(|s| s.as_str()).map(|s| s.to_string())
                                        .or_else(|| v.get("function").and_then(|f| f.get("name")).and_then(|s| s.as_str()).map(|s| s.to_string()));
                                    tools_map.entry(id).or_insert(ToolAccum { name, arguments: String::new(), call_type: Some("function".to_string()) });
                                }
                            }
                        }
                        "response.tool_call.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v.get("call_id").and_then(|s| s.as_str())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str())) {
                                    let entry = tools_map.entry(id.to_string()).or_insert(ToolAccum { name: None, arguments: String::new(), call_type: Some("function".to_string()) });
                                    // name may arrive here too
                                    if entry.name.is_none() {
                                        entry.name = v.get("name").and_then(|s| s.as_str()).map(|s| s.to_string())
                                            .or_else(|| v.get("function").and_then(|f| f.get("name")).and_then(|s| s.as_str()).map(|s| s.to_string()));
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
                                if let Some(id) = v.get("call_id").and_then(|s| s.as_str())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str())) {
                                    if let Some(acc) = tools_map.remove(id) {
                                        if let Some(name) = acc.name {
                                            let args_map = serde_json::from_str::<serde_json::Value>(&acc.arguments)
                                                .ok()
                                                .and_then(|v| v.as_object().cloned())
                                                .unwrap_or_default();
                                            function_calls.push(FunctionCall {
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
                                            super::shared::shared_model_logic::send_tool_ws_update(&ws_manager_trait, Some(inbox.clone()), last).await?;
                                        }
                                    }
                                }
                            }
                        }
                        // Function call lifecycle (OpenAI Responses API uses function_call instead of tool_call)
                        "response.function_call.created" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v.get("call_id").and_then(|s| s.as_str()).map(|s| s.to_string())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str()).map(|s| s.to_string())) {
                                    let name = v.get("name").and_then(|s| s.as_str()).map(|s| s.to_string())
                                        .or_else(|| v.get("function").and_then(|f| f.get("name")).and_then(|s| s.as_str()).map(|s| s.to_string()));
                                    eprintln!("DEBUG function_call.created: id={}, name={:?}", id, name);
                                    tools_map.entry(id).or_insert(ToolAccum { name, arguments: String::new(), call_type: Some("function".to_string()) });
                                }
                            }
                        }
                        "response.function_call.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v.get("call_id").and_then(|s| s.as_str())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str())) {
                                    let entry = tools_map.entry(id.to_string()).or_insert(ToolAccum { name: None, arguments: String::new(), call_type: Some("function".to_string()) });
                                    // name may arrive here too
                                    if entry.name.is_none() {
                                        entry.name = v.get("name").and_then(|s| s.as_str()).map(|s| s.to_string())
                                            .or_else(|| v.get("function").and_then(|f| f.get("name")).and_then(|s| s.as_str()).map(|s| s.to_string()));
                                    }
                                    // delta may be in {"delta":{"arguments":"..."}}
                                    if let Some(delta) = v.get("delta") {
                                        if let Some(args) = delta.get("arguments").and_then(|a| a.as_str()) {
                                            eprintln!("DEBUG function_call.delta: id={}, args_delta='{}'", id, args);
                                            entry.arguments.push_str(args);
                                        }
                                    }
                                }
                            }
                        }
                        "response.function_call_arguments.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                // For OpenAI Responses API, the ID is in item_id field
                                if let Some(id) = v.get("item_id").and_then(|s| s.as_str())
                                    .or_else(|| v.get("call_id").and_then(|s| s.as_str()))
                                    .or_else(|| v.get("id").and_then(|s| s.as_str())) {
                                    let entry = tools_map.entry(id.to_string()).or_insert(ToolAccum { name: None, arguments: String::new(), call_type: Some("function".to_string()) });
                                    // OpenAI Responses API puts the delta directly in the "delta" field
                                    if let Some(delta) = v.get("delta").and_then(|d| d.as_str()) {
                                        eprintln!("DEBUG function_call_arguments.delta: id={}, delta='{}'", id, delta);
                                        entry.arguments.push_str(delta);
                                    } else {
                                        eprintln!("DEBUG function_call_arguments.delta: id={}, no delta found in data: '{}'", id, data_buf.chars().take(200).collect::<String>());
                                    }
                                }
                            }
                        }
                        "response.function_call_arguments.done" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                // For OpenAI Responses API, the ID is in item_id field  
                                if let Some(id) = v.get("item_id").and_then(|s| s.as_str())
                                    .or_else(|| v.get("call_id").and_then(|s| s.as_str()))
                                    .or_else(|| v.get("id").and_then(|s| s.as_str())) {
                                    eprintln!("DEBUG function_call_arguments.done: id={}, tools_map contains: {:?}", id, tools_map.get(id));
                                    
                                    // Get the complete arguments from the event (fallback to accumulated)
                                    let final_arguments = v.get("arguments").and_then(|a| a.as_str())
                                        .map(|s| s.to_string())
                                        .or_else(|| tools_map.get(id).map(|acc| acc.arguments.clone()))
                                        .unwrap_or_default();
                                    
                                    if let Some(acc) = tools_map.remove(id) {
                                        eprintln!("DEBUG function_call_arguments.done: removed from tools_map, name={:?}, final_args='{}'", acc.name, final_arguments);
                                        if let Some(name) = acc.name {
                                            let args_map = serde_json::from_str::<serde_json::Value>(&final_arguments)
                                                .ok()
                                                .and_then(|v| v.as_object().cloned())
                                                .unwrap_or_default();
                                            function_calls.push(FunctionCall {
                                                name,
                                                arguments: args_map,
                                                tool_router_key: None,
                                                response: None,
                                                index: function_calls.len() as u64,
                                                id: Some(id.to_string()),
                                                call_type: acc.call_type.or(Some("function".to_string())),
                                            });
                                            eprintln!("DEBUG function_call_arguments.done: added to function_calls, total count now: {}", function_calls.len());
                                            
                                            // Send WebSocket update for this function call
                                            if let Some(ref inbox) = inbox_name {
                                                if let Some(last) = function_calls.last() {
                                                    let _ = super::shared::shared_model_logic::send_tool_ws_update(&ws_manager_trait, Some(inbox.clone()), last).await;
                                                }
                                            }
                                        } else {
                                            eprintln!("DEBUG function_call_arguments.done: WARNING - acc.name is None!");
                                        }
                                    } else {
                                        eprintln!("DEBUG function_call_arguments.done: WARNING - id '{}' not found in tools_map!", id);
                                    }
                                }
                            }
                        }
                        "response.function_call.done" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(id) = v.get("call_id").and_then(|s| s.as_str())
                                    .or_else(|| v.get("id").and_then(|s| s.as_str())) {
                                    eprintln!("DEBUG function_call.done: id={}, tools_map contains: {:?}", id, tools_map.get(id));
                                    if let Some(acc) = tools_map.remove(id) {
                                        eprintln!("DEBUG function_call.done: removed from tools_map, name={:?}, args='{}'", acc.name, acc.arguments);
                                        if let Some(name) = acc.name {
                                            let args_map = serde_json::from_str::<serde_json::Value>(&acc.arguments)
                                                .ok()
                                                .and_then(|v| v.as_object().cloned())
                                                .unwrap_or_default();
                                            function_calls.push(FunctionCall {
                                                name,
                                                arguments: args_map,
                                                tool_router_key: None,
                                                response: None,
                                                index: function_calls.len() as u64,
                                                id: Some(id.to_string()),
                                                call_type: acc.call_type.or(Some("function".to_string())),
                                            });
                                            eprintln!("DEBUG function_call.done: added to function_calls, total count now: {}", function_calls.len());
                                        } else {
                                            eprintln!("DEBUG function_call.done: WARNING - acc.name is None!");
                                        }
                                    } else {
                                        eprintln!("DEBUG function_call.done: WARNING - id '{}' not found in tools_map!", id);
                                    }
                                    if let Some(ref inbox) = inbox_name {
                                        if let Some(last) = function_calls.last() {
                                            super::shared::shared_model_logic::send_tool_ws_update(&ws_manager_trait, Some(inbox.clone()), last).await?;
                                        }
                                    }
                                }
                            }
                        }
                        // Completion end
                        "response.completed" => {
                            // finalize any remaining tool calls
                            let ids: Vec<String> = tools_map.keys().cloned().collect();
                            eprintln!("DEBUG response.completed: finalizing {} remaining tool calls", ids.len());
                            for id in ids {
                                if let Some(acc) = tools_map.remove(&id) {
                                    eprintln!("DEBUG response.completed: finalizing tool call id={}, name={:?}, args='{}'", id, acc.name, acc.arguments);
                                    if let Some(name) = acc.name {
                                        let args_map = serde_json::from_str::<serde_json::Value>(&acc.arguments)
                                            .ok()
                                            .and_then(|v| v.as_object().cloned())
                                            .unwrap_or_default();
                                        function_calls.push(FunctionCall {
                                            name,
                                            arguments: args_map,
                                            tool_router_key: None,
                                            response: None,
                                            index: function_calls.len() as u64,
                                            id: Some(id.to_string()),
                                            call_type: acc.call_type.or(Some("function".to_string())),
                                        });
                                        eprintln!("DEBUG response.completed: added to function_calls, total count now: {}", function_calls.len());
                                    }
                                }
                            }

                            // Send tool updates for any finalized function calls
                            if let Some(ref inbox) = inbox_name {
                                if let Some(last_function_call) = function_calls.last() {
                                    let _ = super::shared::shared_model_logic::send_tool_ws_update(&ws_manager_trait, Some(inbox.clone()), last_function_call).await;
                                }
                            }

                            if let Some(ref inbox) = inbox_name {
                                super::shared::shared_model_logic::send_ws_update(
                                    &ws_manager_trait,
                                    Some(inbox.clone()),
                                    "",
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
                                            let name = item.get("name").and_then(|n| n.as_str()).map(|s| s.to_string())
                                                .or_else(|| item.get("function_name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                                                .or_else(|| item.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string()));
                                            eprintln!("DEBUG output_item.added (function_call): id={}, name={:?}, full_item={}", id, name, serde_json::to_string(&item).unwrap_or_default());
                                            tools_map.entry(id.to_string()).or_insert(ToolAccum { 
                                                name, 
                                                arguments: String::new(), 
                                                call_type: Some("function".to_string()) 
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        "response.output_item.done" => {
                            // Check if this is a completed function call
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data_buf) {
                                if let Some(item) = v.get("item") {
                                    if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                                        if let Some(id) = item.get("id").and_then(|id| id.as_str()) {
                                            // Try to extract function name from the completed item
                                            let name = item.get("name").and_then(|n| n.as_str()).map(|s| s.to_string())
                                                .or_else(|| item.get("function_name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                                                .or_else(|| item.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string()));
                                            eprintln!("DEBUG output_item.done (function_call): id={}, name={:?}, full_item={}", id, name, serde_json::to_string(&item).unwrap_or_default());
                                            
                                            // Update the tools_map entry with the function name if we found it
                                            if let Some(name) = name {
                                                if let Some(entry) = tools_map.get_mut(id) {
                                                    if entry.name.is_none() {
                                                        entry.name = Some(name);
                                                        eprintln!("DEBUG output_item.done: updated function name for id {}", id);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            // Log unmatched events to see what we're missing
                            eprintln!("DEBUG Unmatched streaming event: '{}' with data: '{}'", ev, data_buf.chars().take(100).collect::<String>());
                        }
                    }
                }
            } else {
                break;
            }
        }
    }

    // Debug: log final accumulated data
    eprintln!("DEBUG Final streaming result - response_text: '{}', reasoning_text: '{}', function_calls count: {}", 
              response_text.chars().take(100).collect::<String>(), 
              reasoning_text.chars().take(100).collect::<String>(), 
              function_calls.len());
    if !function_calls.is_empty() {
        eprintln!("DEBUG Function calls: {:?}", function_calls);
    }

    Ok(LLMInferenceResponse::new(
        response_text,
        if reasoning_text.is_empty() { None } else { Some(reasoning_text) },
        json!({}),
        function_calls,
        Vec::new(),
        None,
    ))
}
fn add_options_to_payload_responses(payload: &mut serde_json::Value, config: Option<&JobConfig>, is_reasoning_model: bool) {
    // Helper to read env var or config value
    fn read_env_var<T: std::str::FromStr>(key: &str) -> Option<T> {
        std::env::var(key).ok().and_then(|val| val.parse::<T>().ok())
    }
    fn get_value<T: Clone + std::str::FromStr>(env_key: &str, config_value: Option<&T>) -> Option<T> {
        config_value.cloned().or_else(|| read_env_var::<T>(env_key))
    }

    if let Some(seed) = get_value("LLM_SEED", config.and_then(|c| c.seed.as_ref())) {
        payload["seed"] = serde_json::json!(seed);
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
            let effort = config.and_then(|c| c.reasoning_effort.clone()).unwrap_or_else(|| "medium".to_string());
            payload["reasoning"] = json!({"effort": effort, "summary": "detailed"});
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
                    // Carry over common sampling/behavior fields
                    "frequency_penalty" => payload["frequency_penalty"] = value.clone(),
                    "logit_bias" => payload["logit_bias"] = value.clone(),
                    "logprobs" => payload["logprobs"] = value.clone(),
                    "top_logprobs" => payload["top_logprobs"] = value.clone(),
                    "n" => payload["n"] = value.clone(),
                    "presence_penalty" => payload["presence_penalty"] = value.clone(),
                    "response_format" => payload["response_format"] = value.clone(),
                    "service_tier" => payload["service_tier"] = value.clone(),
                    "stop" => payload["stop"] = value.clone(),
                    "parallel_tool_calls" => payload["parallel_tool_calls"] = value.clone(),
                    // Skip sampling params on reasoning models
                    "temperature" if is_reasoning_model => (),
                    "top_p" if is_reasoning_model => (),
                    "top_k" if is_reasoning_model => (),
                    _ => (),
                };
            }
        }
    }
}


fn transform_input_messages_for_responses(messages_json: serde_json::Value) -> serde_json::Value {
    // Expected input: Array of chat-style messages: [{role, content, images?, tool_calls? ...}]
    // Output: Array of Responses messages: [{role, content: [ {type: 'input_text'|'input_image'|...} ] }]
    let mut out: Vec<serde_json::Value> = Vec::new();
    if let Some(arr) = messages_json.as_array() {
        for msg in arr {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            // Skip legacy 'function' role; transform 'tool' role to 'user' for Responses API
            if role == "function" { continue; }
            
            // Responses API only supports: 'assistant', 'system', 'developer', 'user'
            let normalized_role = if role == "tool" { "user" } else { role };

            let content = msg.get("content");
            if content.is_none() || content == Some(&serde_json::Value::Null) {
                // Invalid for Responses; skip
                continue;
            }

            let mut content_blocks: Vec<serde_json::Value> = Vec::new();
            match content.unwrap() {
                serde_json::Value::String(s) => {
                    let block_type = if normalized_role == "assistant" { "output_text" } else { "input_text" };
                    content_blocks.push(json!({"type": block_type, "text": s}));
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        if let Some(itype) = item.get("type").and_then(|t| t.as_str()) {
                            match itype {
                                "text" => {
                                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                        let block_type = if normalized_role == "assistant" { "output_text" } else { "input_text" };
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
                            let block_type = if role == "assistant" { "output_text" } else { "input_text" };
                            content_blocks.push(json!({"type": block_type, "text": text}));
                        }
                    }
                }
                _ => {
                    // Not a supported format; skip this message
                    continue;
                }
            }

            if content_blocks.is_empty() { continue; }

            let mut new_msg = serde_json::Map::new();
            new_msg.insert("role".into(), json!(normalized_role));
            new_msg.insert("content".into(), serde_json::Value::Array(content_blocks));

            out.push(serde_json::Value::Object(new_msg));

            // Handle tool_calls for assistant messages - convert to function_call items
            if role == "assistant" {
                if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
                    for tool_call in tool_calls {
                        if let (Some(id), Some(func)) = (
                            tool_call.get("id").and_then(|i| i.as_str()),
                            tool_call.get("function")
                        ) {
                            if let (Some(name), Some(args)) = (
                                func.get("name").and_then(|n| n.as_str()),
                                func.get("arguments").and_then(|a| a.as_str())
                            ) {
                                let function_call_item = json!({
                                    "type": "function_call",
                                    "call_id": id,
                                    "name": name,
                                    "arguments": args,
                                    "status": "completed"
                                });
                                out.push(function_call_item);
                            }
                        }
                    }
                }
            }
        }
    }
    serde_json::Value::Array(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

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
                            .or_else(|| function_obj.and_then(|f| f.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string()));
                        let id_opt = item.get("call_id").and_then(|id| id.as_str()).map(|s| s.to_string())
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
        assert_eq!(args.get("url").and_then(|v| v.as_str()), Some("https://www.youtube.com/watch?v=a10M_i42z7M"));
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
    fn test_transform_tool_role_to_user() {
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
        
        // Check second message (should be transformed from "tool" to "user")
        let tool_message = &result_array[1];
        assert_eq!(tool_message.get("role").unwrap(), "user"); // Should be "user", not "tool"
        
        // Verify tool_call_id is NOT preserved (not supported by Responses API)
        assert!(tool_message.get("tool_call_id").is_none());
        
        // Verify content is properly transformed
        let content = tool_message.get("content").unwrap().as_array().unwrap();
        assert_eq!(content.len(), 1);
        let text_block = &content[0];
        assert_eq!(text_block.get("type").unwrap(), "input_text"); // Should be "input_text" since role is now "user"
        assert_eq!(text_block.get("text").unwrap(), "Tool execution failed with error");
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
        
        // Should have: user message, assistant message, function_call item, tool response (as user)
        assert_eq!(result_array.len(), 4);
        
        // First: user message
        assert_eq!(result_array[0].get("role").and_then(|r| r.as_str()), Some("user"));
        
        // Second: assistant message
        assert_eq!(result_array[1].get("role").and_then(|r| r.as_str()), Some("assistant"));
        
        // Third: function_call item
        let function_call = &result_array[2];
        assert_eq!(function_call.get("type").and_then(|t| t.as_str()), Some("function_call"));
        assert_eq!(function_call.get("call_id").and_then(|id| id.as_str()), Some("call_123456"));
        assert_eq!(function_call.get("name").and_then(|n| n.as_str()), Some("youtube_transcript_extractor_2_0"));
        assert_eq!(function_call.get("status").and_then(|s| s.as_str()), Some("completed"));
        
        // Fourth: tool response transformed to user role
        assert_eq!(result_array[3].get("role").and_then(|r| r.as_str()), Some("user"));
        let tool_content = &result_array[3]["content"].as_array().unwrap()[0];
        assert_eq!(tool_content.get("type").and_then(|t| t.as_str()), Some("input_text"));
        assert_eq!(tool_content.get("text").and_then(|t| t.as_str()), Some("Transcript extracted successfully"));
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
        
        let extracted_id = event_with_both.get("call_id").and_then(|s| s.as_str()).map(|s| s.to_string())
            .or_else(|| event_with_both.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()));
        assert_eq!(extracted_id, Some("call_123456".to_string()));
        
        // Test fallback to id when call_id is missing
        let event_with_id_only = json!({
            "id": "regular_id_123",
            "name": "test_tool"
        });
        
        let extracted_id = event_with_id_only.get("call_id").and_then(|s| s.as_str()).map(|s| s.to_string())
            .or_else(|| event_with_id_only.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()));
        assert_eq!(extracted_id, Some("regular_id_123".to_string()));
        
        // Test no extraction when neither field is present
        let event_without_ids = json!({
            "name": "test_tool"
        });
        
        let extracted_id = event_without_ids.get("call_id").and_then(|s| s.as_str()).map(|s| s.to_string())
            .or_else(|| event_without_ids.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()));
        assert_eq!(extracted_id, None);
    }
}

