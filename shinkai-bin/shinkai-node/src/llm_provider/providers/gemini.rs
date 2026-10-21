use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::gemini_api::gemini_prepare_messages;
use super::shared::shared_model_logic::{save_image_file, send_tool_ws_update, send_ws_update};
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::{FunctionCall, LLMInferenceResponse};
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{Gemini, LLMProviderInterface};
use shinkai_message_primitives::schemas::prompts::Prompt;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;
use std::error::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct GeminiStreamingResponse {
    candidates: Vec<StreamingCandidate>,
}

#[derive(Debug, Deserialize)]
struct StreamingCandidate {
    content: StreamingContent,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
    #[serde(default)]
    function_call: Option<FunctionCallResponse>,
}

#[derive(Debug, Deserialize)]
struct StreamingContent {
    parts: Vec<StreamingPart>,
}

#[derive(Debug, Deserialize)]
struct StreamingPart {
    #[serde(default)]
    text: String,
    #[serde(rename = "functionCall")]
    function_call: Option<FunctionCallResponse>,
    #[serde(default)]
    thought: bool,
    #[serde(rename = "inlineData")]
    inline_data: Option<InlineData>,
}

#[derive(Debug, Deserialize)]
struct InlineData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct FunctionCallResponse {
    #[serde(default)]
    id: Option<String>,
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: UsageMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
struct Candidate {
    content: Content,
    #[serde(rename = "finishReason")]
    finish_reason: String,
    index: u32,
    #[serde(rename = "safetyRatings")]
    safety_ratings: Vec<SafetyRating>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Content {
    parts: Vec<Part>,
    role: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SafetyRating {
    category: String,
    probability: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: u32,
    #[serde(rename = "totalTokenCount")]
    total_token_count: u32,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorResponse {
    error: GeminiError,
}

#[derive(Debug, Deserialize)]
struct GeminiError {
    code: i32,
    message: String,
    status: String,
}

fn gemini_generation_config(
    model: &str,
    config: Option<&JobConfig>,
    remaining_output_tokens: usize,
) -> serde_json::Value {
    let mut generation_config = json!({
        "temperature": config.and_then(|c| c.temperature).unwrap_or(1.0),
        "maxOutputTokens": config.and_then(|c| c.max_tokens).unwrap_or(remaining_output_tokens as u64).min(remaining_output_tokens as u64)
    });
    if let Some(top_k) = config.and_then(|c| c.top_k) {
        generation_config["topK"] = json!(top_k);
    }
    if let Some(top_p) = config.and_then(|c| c.top_p) {
        generation_config["topP"] = json!(top_p);
    }
    let current_thinking = model.starts_with("gemini-3") || model.starts_with("gemini-4");
    if current_thinking || config.and_then(|c| c.thinking) == Some(true) {
        let effort = config.and_then(|c| c.reasoning_effort.as_deref());
        if current_thinking {
            let level = match effort {
                _ if model == "gemini-3.5-flash-lite" && config.and_then(|c| c.thinking) == Some(false) => "minimal",
                Some("minimal") if model == "gemini-3.5-flash-lite" => "minimal",
                Some("low") => "low",
                Some("medium") if model == "gemini-3.5-flash-lite" => "medium",
                Some("high" | "xhigh" | "max") => "high",
                _ if model == "gemini-3.5-flash-lite" => "minimal",
                _ if model.contains("pro") => "high",
                _ => "medium",
            };
            generation_config["thinkingConfig"] = json!({
                "thinkingLevel": level,
                "includeThoughts": config.and_then(|c| c.thinking).unwrap_or(false)
            });
        } else {
            let budget = match effort {
                Some("low") => 1024,
                Some("medium") => 8192,
                Some("high") => 24576,
                _ => -1,
            };
            generation_config["thinkingConfig"] = json!({
                "thinkingBudget": budget, "includeThoughts": true
            });
        }
    }
    generation_config
}

#[async_trait]
impl LLMService for Gemini {
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
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let base_url = if base_url.ends_with('/') {
                    base_url.to_string()
                } else {
                    format!("{}/", base_url)
                };

                let session_id = Uuid::new_v4().to_string();
                let url = format!("{}{}:streamGenerateContent?key={}", base_url, self.model_type, key);

                let result = gemini_prepare_messages(&model, prompt)?;
                let contents = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                let generation_config =
                    gemini_generation_config(&self.model_type, config.as_ref(), result.remaining_output_tokens);

                let mut payload = json!({
                    "generationConfig": generation_config,
                    "safety_settings": [
                        {
                            "category": "HARM_CATEGORY_DANGEROUS_CONTENT",
                            "threshold": "BLOCK_NONE"
                        },
                        {
                            "category": "HARM_CATEGORY_HARASSMENT",
                            "threshold": "BLOCK_NONE"
                        },
                        {
                            "category": "HARM_CATEGORY_HATE_SPEECH",
                            "threshold": "BLOCK_NONE"
                        },
                        {
                            "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT",
                            "threshold": "BLOCK_NONE"
                        }
                    ]
                });

                if let Some(payload_obj) = payload.as_object_mut() {
                    if let Some(contents_obj) = contents.as_object() {
                        for (key, value) in contents_obj {
                            payload_obj.insert(key.clone(), value.clone());
                        }
                    }
                }

                // Print payload as a pretty JSON string only if IS_TESTING is true
                if std::env::var("LOG_ALL").unwrap_or_default() == "true"
                    || std::env::var("LOG_ALL").unwrap_or_default() == "1"
                {
                    match serde_json::to_string_pretty(&payload) {
                        Ok(pretty_json) => eprintln!("cURL Payload: {}", pretty_json),
                        Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
                    };
                }

                let payload_log = payload.clone();
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

                let res = client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
                    .await?;
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Status: {:?}", res.status()).as_str(),
                );

                let mut stream = res.bytes_stream();
                let mut thinking_content = String::new();
                let mut regular_content = String::new();
                let mut generated_files = Vec::new();
                let mut buffer = String::new();
                let mut is_done = false;
                let mut finish_reason = None;
                let mut function_calls = Vec::new();
                let mut native_parts = Vec::new();
                let mut in_thinking = false;
                let mut thinking_started = false;

                while let Some(item) = stream.next().await {
                    match item {
                        Ok(chunk) => {
                            process_chunk(
                                &chunk,
                                &mut buffer,
                                &mut thinking_content,
                                &mut regular_content,
                                &mut generated_files,
                                &session_id,
                                &ws_manager_trait,
                                &inbox_name,
                                &db,
                                &mut is_done,
                                &mut finish_reason,
                                &mut function_calls,
                                &mut native_parts,
                                &mut in_thinking,
                                &mut thinking_started,
                            )
                            .await?;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                format!("Error while receiving chunk: {:?}, Error Source: {:?}", e, e.source())
                                    .as_str(),
                            );
                            return Err(LLMProviderError::NetworkError(e.to_string()));
                        }
                    }
                }

                Ok(LLMInferenceResponse::new(
                    regular_content,
                    if thinking_content.is_empty() {
                        None
                    } else {
                        Some(thinking_content)
                    },
                    json!({}),
                    function_calls,
                    generated_files,
                    None,
                ))
            } else {
                Err(LLMProviderError::ApiKeyNotSet)
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_chunk(
    chunk: &[u8],
    buffer: &mut String,
    thinking_content: &mut String,
    regular_content: &mut String,
    generated_files: &mut Vec<ShinkaiPath>,
    session_id: &str,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: &Option<InboxName>,
    db: &SqliteManager,
    is_done: &mut bool,
    finish_reason: &mut Option<String>,
    function_calls: &mut Vec<FunctionCall>,
    native_parts: &mut Vec<JsonValue>,
    in_thinking: &mut bool,
    thinking_started: &mut bool,
) -> Result<(), LLMProviderError> {
    let chunk_str = String::from_utf8_lossy(chunk);
    buffer.push_str(&chunk_str);

    // Remove leading comma or square bracket if they exist
    if buffer.starts_with(',') || buffer.starts_with('[') {
        buffer.remove(0);
    }

    // Remove trailing square bracket if it exists
    if buffer.ends_with(']') {
        buffer.pop();
        *is_done = function_calls.is_empty(); // Set is_done to true if buffer ends with ']'
    }

    // Add a trailing ']' to make it a valid JSON array
    let json_str = format!("[{}]", buffer);

    match serde_json::from_str::<Vec<JsonValue>>(&json_str) {
        Ok(array) => {
            for value in array {
                // First check if this is an error response
                if let Ok(error_response) = serde_json::from_value::<GeminiErrorResponse>(value.clone()) {
                    return Err(LLMProviderError::NetworkError(format!(
                        "Gemini API error ({}): {} - Status: {}",
                        error_response.error.code, error_response.error.message, error_response.error.status
                    )));
                }

                process_gemini_response(
                    value,
                    thinking_content,
                    regular_content,
                    generated_files,
                    session_id,
                    ws_manager_trait,
                    inbox_name,
                    db,
                    is_done,
                    finish_reason,
                    function_calls,
                    native_parts,
                    in_thinking,
                    thinking_started,
                )
                .await?;
            }
            buffer.clear();
        }
        Err(e) => {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                &format!("Failed to parse chunk as JSON array: {:?}.", e),
            );
        }
    }

    // Check if is_done is true and send a final message if necessary
    if *is_done {
        // If we're done and still in thinking mode, close the thinking tag
        if *in_thinking {
            *in_thinking = false;
        }

        let _ = send_ws_update(
            ws_manager_trait,
            inbox_name.clone(),
            session_id,
            regular_content.to_string(),
            false,
            *is_done,
            finish_reason.clone(),
        )
        .await;
    }

    Ok(())
}

async fn process_gemini_response(
    value: JsonValue,
    thinking_content: &mut String,
    regular_content: &mut String,
    generated_files: &mut Vec<ShinkaiPath>,
    session_id: &str,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: &Option<InboxName>,
    db: &SqliteManager,
    is_done: &bool,
    finish_reason: &mut Option<String>,
    function_calls: &mut Vec<FunctionCall>,
    native_parts: &mut Vec<JsonValue>,
    in_thinking: &mut bool,
    thinking_started: &mut bool,
) -> Result<(), LLMProviderError> {
    if let Some(parts) = value
        .pointer("/candidates/0/content/parts")
        .and_then(|parts| parts.as_array())
    {
        native_parts.extend(parts.iter().cloned());
    }
    if let Ok(response) = serde_json::from_value::<GeminiStreamingResponse>(value) {
        for candidate in &response.candidates {
            // Always update finish reason from candidate
            finish_reason.clone_from(&candidate.finish_reason);

            // Handle function calls at candidate level
            if let Some(function_call) = &candidate.function_call {
                process_function_call(function_call, ws_manager_trait, inbox_name, function_calls).await;
            }

            // Handle text content and function calls in parts
            for part in &candidate.content.parts {
                // Handle function calls in parts
                if let Some(function_call) = &part.function_call {
                    process_function_call(function_call, ws_manager_trait, inbox_name, function_calls).await;
                }

                // Handle text content
                if !part.text.is_empty() {
                    let mut text_to_add = String::new();

                    // Handle thinking logic
                    if part.thought {
                        // This is a thought
                        if !*thinking_started {
                            *thinking_started = true;
                            *in_thinking = true;
                        }
                        thinking_content.push_str(&part.text);
                    } else {
                        // This is a normal response
                        if *in_thinking {
                            *in_thinking = false;
                            let _ = send_ws_update(
                                ws_manager_trait,
                                inbox_name.clone(),
                                session_id,
                                "".to_string(),
                                false,
                                true,
                                None,
                            )
                            .await;
                        }
                        regular_content.push_str(&part.text);
                    }

                    text_to_add.push_str(&part.text);

                    let _ = send_ws_update(
                        ws_manager_trait,
                        inbox_name.clone(),
                        session_id,
                        text_to_add,
                        part.thought,
                        *is_done,
                        finish_reason.clone(),
                    )
                    .await;
                }

                // Handle inline data (images)
                if let Some(inline_data) = &part.inline_data {
                    match save_image_file(&inline_data.mime_type, &inline_data.data, inbox_name, session_id, db).await {
                        Ok(shinkai_path) => {
                            generated_files.push(shinkai_path);
                        }
                        Err(e) => eprintln!("Failed to save image file: {:?}", e),
                    }
                }
            }
        }
    }
    // Keep every native part, including empty signature parts, in its original
    // order. Each parallel call carries the same complete assistant turn.
    for call in function_calls {
        let metadata = call.provider_metadata.get_or_insert_with(|| json!({}));
        metadata["gemini_parts"] = json!(native_parts);
    }
    Ok(())
}

async fn process_function_call(
    function_call: &FunctionCallResponse,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: &Option<InboxName>,
    function_calls: &mut Vec<FunctionCall>,
) {
    let fc = FunctionCall {
        name: function_call.name.clone(),
        arguments: function_call.args.as_object().cloned().unwrap_or_default(),
        tool_router_key: function_call
            .args
            .get("tool_router_key")
            .and_then(|key| key.as_str().map(|s| s.to_string())),
        response: None,
        index: function_calls.len() as u64,
        id: function_call.id.clone(),
        provider_metadata: (!function_call.args.is_object())
            .then(|| json!({"function_argument_error": "Function arguments must be a JSON object"})),
        call_type: Some("function".to_string()),
    };
    function_calls.push(fc.clone());

    // Send WebSocket update for function call
    let _ = send_tool_ws_update(ws_manager_trait, inbox_name.clone(), &fc).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::EmbeddingModelType;
    use shinkai_embedding::model_type::OllamaTextEmbeddingsInference;
    use shinkai_sqlite::SqliteManager;
    use std::sync::Arc;
    use tempfile::NamedTempFile;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn gemini_invalid_arguments_cannot_execute_an_empty_schema_tool() {
        use crate::llm_provider::execution::chains::generic_chain::generic_inference_chain::execute_valid_function_call;
        use crate::managers::tool_router::ToolCallFunctionResponse;
        let db = setup_test_db().await;
        for args in [
            json!([]),
            json!("bad"),
            json!(3),
            json!(true),
            JsonValue::Null,
            json!({}),
        ] {
            let valid = args.is_object();
            let parts = json!([{
                "functionCall": {"name": "zero_arg_tool", "id": "native-call", "args": args},
                "thoughtSignature": "signed-native-part"
            }]);
            let mut calls = Vec::new();
            let mut native_parts = Vec::new();
            process_gemini_response(
                json!({"candidates": [{"content": {"role": "model", "parts": parts}}]}),
                &mut String::new(),
                &mut String::new(),
                &mut Vec::new(),
                "test",
                &None,
                &None,
                &db,
                &false,
                &mut None,
                &mut calls,
                &mut native_parts,
                &mut false,
                &mut false,
            )
            .await
            .unwrap();
            assert_eq!(calls.len(), 1);
            let call = &calls[0];
            assert_eq!(call.id.as_deref(), Some("native-call"));
            assert_eq!(call.name, "zero_arg_tool");
            let metadata = call.provider_metadata.as_ref().unwrap();
            assert_eq!(metadata["gemini_parts"], parts);
            assert_eq!(metadata.get("function_argument_error").is_some(), !valid);
            let mut executed = false;
            let execution_call = call.clone();
            let result = execute_valid_function_call(call, async {
                executed = true;
                Ok(ToolCallFunctionResponse {
                    function_call: execution_call,
                    response: "executed".into(),
                })
            })
            .await;
            assert_eq!(executed, valid);
            assert_eq!(result.is_ok(), valid);
        }
    }

    #[tokio::test]
    async fn gemini_signature_round_trip_keeps_parallel_native_parts() {
        use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptType};
        let parts = json!([
            {"text": "", "thought": true, "thoughtSignature": "signed-thought"},
            {"functionCall": {"name": "weather", "args": {"city": "Paris"}, "id": "call-1"}, "thoughtSignature": "signed-call"},
            {"functionCall": {"name": "weather", "args": {"city": "London"}, "id": "call-2"}}
        ]);
        let db = setup_test_db().await;
        let mut thinking = String::new();
        let mut regular = String::new();
        let mut files = Vec::new();
        let mut calls = Vec::new();
        let mut native_parts = Vec::new();
        let mut finish_reason = None;
        let mut in_thinking = false;
        let mut thinking_started = false;
        for chunk_parts in [json!([parts[0], parts[1]]), json!([parts[2]])] {
            process_gemini_response(
                json!({"candidates": [{"content": {"role": "model", "parts": chunk_parts}}]}),
                &mut thinking,
                &mut regular,
                &mut files,
                "test",
                &None,
                &None,
                &db,
                &false,
                &mut finish_reason,
                &mut calls,
                &mut native_parts,
                &mut in_thinking,
                &mut thinking_started,
            )
            .await
            .unwrap();
        }
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id.as_deref(), Some("call-1"));
        assert_eq!(calls[1].id.as_deref(), Some("call-2"));
        let metadata = json!({"gemini_parts": parts});
        assert!(calls
            .iter()
            .all(|call| call.provider_metadata.as_ref() == Some(&metadata)));
        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(vec![SubPrompt::Content(
            SubPromptType::User,
            "Get the weather".into(),
            100,
        )]);
        for call in calls {
            let mut function = serde_json::to_value(call).unwrap();
            // The saved assistant call has no tool result yet.
            function.as_object_mut().unwrap().remove("response");
            prompt.add_function_call(function.clone(), 100);
            prompt.add_function_call_response(
                json!({"function_call": function, "response": "{\"temperature\":20}"}),
                100,
            );
        }
        let model = LLMProviderInterface::Gemini(Gemini {
            model_type: "gemini-3.8-flash".into(),
        });
        let result = gemini_prepare_messages(&model, prompt).unwrap();
        let PromptResultEnum::Value(payload) = result.messages else {
            panic!("Expected JSON payload")
        };
        let contents = payload["contents"].as_array().unwrap();
        let model_turns: Vec<_> = contents.iter().filter(|c| c["role"] == "model").collect();
        assert_eq!(model_turns.len(), 1);
        assert_eq!(model_turns[0]["parts"], parts);
        let results = contents.last().unwrap()["parts"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["functionResponse"]["id"], "call-1");
        assert_eq!(results[1]["functionResponse"]["id"], "call-2");
        assert_eq!(results[0]["functionResponse"]["name"], "weather");
    }

    #[test]
    fn gemini_request_output_uses_prepared_budget_and_user_cap() {
        use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
        use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptType};
        for id in [
            "gemini-4-argon",
            "gemini-3.8-flash",
            "gemini-3.5-flash-lite",
            "gemini-1.5-flash",
        ] {
            let model = LLMProviderInterface::Gemini(Gemini { model_type: id.into() });
            let mut prompt = Prompt::new();
            prompt.add_sub_prompts(vec![SubPrompt::Content(SubPromptType::User, "Hello".into(), 100)]);
            let prepared = gemini_prepare_messages(&model, prompt).unwrap();
            let budget = prepared.remaining_output_tokens;
            assert_eq!(budget, ModelCapabilitiesManager::get_max_output_tokens(&model));
            let request = json!({"generationConfig": gemini_generation_config(id, None, budget)});
            assert_eq!(request["generationConfig"]["maxOutputTokens"], budget);
            if id != "gemini-1.5-flash" {
                assert_eq!(budget, 65_536);
            }
            let mut config = JobConfig::empty();
            config.max_tokens = Some(1024);
            assert_eq!(
                gemini_generation_config(id, Some(&config), budget)["maxOutputTokens"],
                1024
            );
            config.max_tokens = Some((budget + 1) as u64);
            assert_eq!(
                gemini_generation_config(id, Some(&config), budget)["maxOutputTokens"],
                budget
            );
            let remaining = ModelCapabilitiesManager::get_remaining_output_tokens(
                &model,
                ModelCapabilitiesManager::get_max_tokens(&model) - 512,
            );
            assert_eq!(remaining, 512);
            assert_eq!(
                gemini_generation_config(id, Some(&config), remaining)["maxOutputTokens"],
                512
            );
            assert_eq!(gemini_generation_config(id, None, remaining)["maxOutputTokens"], 512);
        }
    }

    #[test]
    fn test_current_gemini_thinking_levels() {
        let mut config = JobConfig::empty();
        config.thinking = Some(true);
        config.reasoning_effort = Some("high".into());
        for model in ["gemini-4-argon", "gemini-3.8-flash", "gemini-3.5-flash-lite"] {
            let value = gemini_generation_config(model, Some(&config), 65_536);
            assert_eq!(value["thinkingConfig"]["thinkingLevel"], "high");
            assert!(value["thinkingConfig"].get("thinkingBudget").is_none());
            assert!(value.get("topK").is_none());
        }
        let value = gemini_generation_config("gemini-2.5-flash", Some(&config), 65_536);
        assert_eq!(value["thinkingConfig"]["thinkingBudget"], 24576);
    }

    #[test]
    fn test_flash_lite_minimal_thinking_contract() {
        let model = "gemini-3.5-flash-lite";
        assert_eq!(
            gemini_generation_config(model, None, 65_536)["thinkingConfig"]["thinkingLevel"],
            "minimal"
        );
        let mut config = JobConfig::empty();
        config.thinking = Some(true);
        for effort in ["minimal", "low", "medium", "high"] {
            config.reasoning_effort = Some(effort.into());
            let value = gemini_generation_config(model, Some(&config), 65_536);
            assert_eq!(value["thinkingConfig"]["thinkingLevel"], effort);
        }
        config.thinking = Some(false);
        config.reasoning_effort = Some("high".into());
        let value = gemini_generation_config(model, Some(&config), 65_536);
        assert_eq!(value["thinkingConfig"]["thinkingLevel"], "minimal");
        assert_eq!(value["thinkingConfig"]["includeThoughts"], false);
        for other in ["gemini-3.8-flash", "gemini-4-argon"] {
            config.reasoning_effort = Some("minimal".into());
            assert_eq!(
                gemini_generation_config(other, Some(&config), 65_536)["thinkingConfig"]["thinkingLevel"],
                "medium"
            );
            assert_eq!(
                gemini_generation_config(other, None, 65_536)["thinkingConfig"]["thinkingLevel"],
                "medium"
            );
        }
    }

    async fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = std::path::PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type = EmbeddingModelType::default();

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_process_first_chunk() {
        let chunk = b"[{
            \"candidates\": [
                {
                    \"content\": {
                        \"parts\": [
                            {
                                \"text\": \"The\"
                            }
                        ],
                        \"role\": \"model\"
                    },
                    \"finishReason\": \"STOP\",
                    \"index\": 0
                }
            ],
            \"usageMetadata\": {
                \"promptTokenCount\": 41,
                \"candidatesTokenCount\": 1,
                \"totalTokenCount\": 42
            }
        }";

        let mut buffer = String::new();
        let mut thinking_content = String::new();
        let mut regular_content = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut function_calls = Vec::new();
        let mut native_parts = Vec::new();
        let mut in_thinking = false;
        let mut thinking_started = false;

        process_chunk(
            chunk,
            &mut buffer,
            &mut thinking_content,
            &mut regular_content,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
            &mut native_parts,
            &mut in_thinking,
            &mut thinking_started,
        )
        .await
        .unwrap();

        assert_eq!(regular_content, "The");
        assert!(!is_done);
        assert_eq!(finish_reason, Some("STOP".to_string()));
    }

    #[tokio::test]
    async fn test_process_second_chunk() {
        let chunk = b",
        {
            \"candidates\": [
                {
                    \"content\": {
                        \"parts\": [
                            {
                                \"text\": \" Roman Empire was a vast and powerful civilization that dominated much of Europe, North Africa\"
                            }
                        ],
                        \"role\": \"model\"
                    },
                    \"finishReason\": \"STOP\",
                    \"index\": 0,
                    \"safetyRatings\": [
                        {
                            \"category\": \"HARM_CATEGORY_SEXUALLY_EXPLICIT\",
                            \"probability\": \"NEGLIGIBLE\"
                        },
                        {
                            \"category\": \"HARM_CATEGORY_HATE_SPEECH\",
                            \"probability\": \"NEGLIGIBLE\"
                        },
                        {
                            \"category\": \"HARM_CATEGORY_HARASSMENT\",
                            \"probability\": \"NEGLIGIBLE\"
                        },
                        {
                            \"category\": \"HARM_CATEGORY_DANGEROUS_CONTENT\",
                            \"probability\": \"NEGLIGIBLE\"
                        }
                    ]
                }
            ],
            \"usageMetadata\": {
                \"promptTokenCount\": 41,
                \"candidatesTokenCount\": 17,
                \"totalTokenCount\": 58
            }
        }";

        let mut buffer = String::new();
        let mut thinking_content = String::new();
        let mut regular_content = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut function_calls = Vec::new();
        let mut native_parts = Vec::new();
        let mut in_thinking = false;
        let mut thinking_started = false;

        process_chunk(
            chunk,
            &mut buffer,
            &mut thinking_content,
            &mut regular_content,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
            &mut native_parts,
            &mut in_thinking,
            &mut thinking_started,
        )
        .await
        .unwrap();

        assert_eq!(
            regular_content,
            " Roman Empire was a vast and powerful civilization that dominated much of Europe, North Africa"
        );
        assert!(!is_done);
        assert_eq!(finish_reason, Some("STOP".to_string()));
    }

    #[tokio::test]
    async fn test_process_last_chunk() {
        let chunk = b",
        {
            \"candidates\": [
                {
                    \"content\": {
                        \"parts\": [
                            {
                                \"text\": \" in greater detail. \\n\"
                            }
                        ],
                        \"role\": \"model\"
                    },
                    \"finishReason\": \"STOP\",
                    \"index\": 0,
                    \"safetyRatings\": [
                        {
                            \"category\": \"HARM_CATEGORY_SEXUALLY_EXPLICIT\",
                            \"probability\": \"NEGLIGIBLE\"
                        },
                        {
                            \"category\": \"HARM_CATEGORY_HATE_SPEECH\",
                            \"probability\": \"NEGLIGIBLE\"
                        },
                        {
                            \"category\": \"HARM_CATEGORY_HARASSMENT\",
                            \"probability\": \"NEGLIGIBLE\"
                        },
                        {
                            \"category\": \"HARM_CATEGORY_DANGEROUS_CONTENT\",
                            \"probability\": \"NEGLIGIBLE\"
                        }
                    ]
                }
            ],
            \"usageMetadata\": {
                \"promptTokenCount\": 15,
                \"candidatesTokenCount\": 644,
                \"totalTokenCount\": 659
            }
        }]";

        let mut buffer = String::new();
        let mut thinking_content = String::new();
        let mut regular_content = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut function_calls = Vec::new();
        let mut native_parts = Vec::new();
        let mut in_thinking = false;
        let mut thinking_started = false;

        process_chunk(
            chunk,
            &mut buffer,
            &mut thinking_content,
            &mut regular_content,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
            &mut native_parts,
            &mut in_thinking,
            &mut thinking_started,
        )
        .await
        .unwrap();

        assert_eq!(regular_content, " in greater detail. \n");
        assert!(is_done);
        assert_eq!(finish_reason, Some("STOP".to_string()));
    }

    #[tokio::test]
    async fn test_process_function_call_in_parts() {
        // First chunk with the main response
        let chunk1 = br#"[{
            "candidates": [
            {
                "content": {
                "parts": [
                    {
                    "functionCall": {
                        "name": "duckduckgo_search",
                        "args": {
                            "message": "movies"
                        }
                    }
                    }
                ],
                "role": "model"
                },
                "finishReason": "STOP",
                "safetyRatings": [
                {
                    "category": "HARM_CATEGORY_HATE_SPEECH",
                    "probability": "NEGLIGIBLE"
                },
                {
                    "category": "HARM_CATEGORY_DANGEROUS_CONTENT",
                    "probability": "NEGLIGIBLE"
                },
                {
                    "category": "HARM_CATEGORY_HARASSMENT",
                    "probability": "NEGLIGIBLE"
                },
                {
                    "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT",
                    "probability": "NEGLIGIBLE"
                }
                ]
            }
            ],
            "usageMetadata": {
            "promptTokenCount": 193,
            "candidatesTokenCount": 7,
            "totalTokenCount": 200
            },
            "modelVer"#;

        // Second chunk with the version string continuation
        let chunk2 = br#"sion": "gemini-1.5-flash"
}"#;

        // Third chunk closing the array
        let chunk3 = br#"]"#;

        let mut buffer = String::new();
        let mut thinking_content = String::new();
        let mut regular_content = String::new();
        let mut generated_files = Vec::new();
        let mut function_calls = Vec::new();
        let mut native_parts = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut in_thinking = false;
        let mut thinking_started = false;

        // Process each chunk sequentially
        let chunks: Vec<&[u8]> = vec![chunk1, chunk2, chunk3];
        for chunk in chunks {
            process_chunk(
                chunk,
                &mut buffer,
                &mut thinking_content,
                &mut regular_content,
                &mut generated_files,
                session_id,
                &ws_manager_trait,
                &inbox_name,
                &db,
                &mut is_done,
                &mut finish_reason,
                &mut function_calls,
                &mut native_parts,
                &mut in_thinking,
                &mut thinking_started,
            )
            .await
            .unwrap();
        }

        assert_eq!(regular_content, "");
        assert_eq!(function_calls.len(), 1);
        let fc = &function_calls[0];
        assert_eq!(fc.name, "duckduckgo_search");
        assert_eq!(
            fc.arguments,
            serde_json::json!({
                "message": "movies"
            })
            .as_object()
            .unwrap()
            .clone()
        );
        assert_eq!(finish_reason, Some("STOP".to_string()));
        assert!(!is_done);
    }

    #[tokio::test]
    async fn test_process_thinking_mode() {
        // First chunk with thinking
        let chunk1 = br#"[{
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {
                                "text": "**Clarifying Limitations**\n\nI understand the need for the latest news, but I'm constrained by my training data. I can't access real-time information or browse the internet. My knowledge has a cutoff date, so \"latest news\" isn't possible in the way you might expect. I'm focusing on clarifying these limitations to avoid misunderstandings.\n\n\n",
                                "thought": true
                            }
                        ],
                        "role": "model"
                    },
                    "index": 0
                }
            ],
            "usageMetadata": {
                "promptTokenCount": 46,
                "totalTokenCount": 117,
                "thoughtsTokenCount": 71
            }
        }"#;

        // Second chunk with more thinking
        let chunk2 = br#",
        {
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {
                                "text": "**Highlighting Constraints**\n\nI'm working on explaining my limitations regarding \"the latest news.\" I'm unable to provide real-time updates as my knowledge base is fixed. To clarify, I can't browse the internet or access live news feeds. I'm focusing on explaining how users can find more current information elsewhere.\n\n\n",
                                "thought": true
                            }
                        ],
                        "role": "model"
                    },
                    "index": 0
                }
            ],
            "usageMetadata": {
                "promptTokenCount": 46,
                "totalTokenCount": 139,
                "thoughtsTokenCount": 93
            }
        }"#;

        // Third chunk with normal response (thinking should end here)
        let chunk3 = br#",
        {
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {
                                "text": "As an AI, I don't have real-time access to breaking news or the ability to browse the internet for the very latest updates. My knowledge is based on the data I was trained on, which has a cutoff date.\n\n"
                            }
                        ],
                        "role": "model"
                    },
                    "index": 0
                }
            ],
            "usageMetadata": {
                "promptTokenCount": 46,
                "candidatesTokenCount": 45,
                "totalTokenCount": 184,
                "thoughtsTokenCount": 93
            }
        }"#;

        // Fourth chunk with more normal response
        let chunk4 = br#",
        {
            "candidates": [
                {
                    "content": {
                        "parts": [
                            {
                                "text": "To get the most up-to-date news, I recommend checking reputable news sources like:\n\n*   **Major news organizations:** Reuters, Associated Press, BBC News"
                            }
                        ],
                        "role": "model"
                    },
                    "finishReason": "STOP",
                    "index": 0
                }
            ]
        }]"#;

        let mut buffer = String::new();
        let mut thinking_content = String::new();
        let mut regular_content = String::new();
        let mut generated_files = Vec::new();
        let mut function_calls = Vec::new();
        let mut native_parts = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut in_thinking = false;
        let mut thinking_started = false;

        // Process each chunk sequentially
        let chunks: Vec<&[u8]> = vec![chunk1, chunk2, chunk3, chunk4];
        for chunk in chunks {
            process_chunk(
                chunk,
                &mut buffer,
                &mut thinking_content,
                &mut regular_content,
                &mut generated_files,
                session_id,
                &ws_manager_trait,
                &inbox_name,
                &db,
                &mut is_done,
                &mut finish_reason,
                &mut function_calls,
                &mut native_parts,
                &mut in_thinking,
                &mut thinking_started,
            )
            .await
            .unwrap();
        }

        // Verify the response contains thinking and regular content
        assert!(thinking_content.contains("**Clarifying Limitations**"));
        assert!(thinking_content.contains("**Highlighting Constraints**"));
        assert!(regular_content.contains("As an AI, I don't have real-time access"));
        assert!(regular_content.contains("Major news organizations"));
        assert!(thinking_started);
        assert!(!in_thinking); // Should not be in thinking mode at the end
        assert_eq!(finish_reason, Some("STOP".to_string()));
        assert!(is_done);
    }

    #[tokio::test]
    async fn test_process_error_response() {
        let chunk = br#"[{
            "error": {
                "code": 503,
                "message": "The model is overloaded. Please try again later.",
                "status": "UNAVAILABLE"
            }
        }]"#;

        let mut buffer = String::new();
        let mut thinking_content = String::new();
        let mut regular_content = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut function_calls = Vec::new();
        let mut native_parts = Vec::new();
        let mut in_thinking = false;
        let mut thinking_started = false;

        let result = process_chunk(
            chunk,
            &mut buffer,
            &mut thinking_content,
            &mut regular_content,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
            &mut native_parts,
            &mut in_thinking,
            &mut thinking_started,
        )
        .await;

        // Verify that we got an error response
        assert!(result.is_err());
        if let Err(err) = result {
            match err {
                LLMProviderError::NetworkError(msg) => {
                    assert!(msg.contains("The model is overloaded"));
                    assert!(msg.contains("503"));
                }
                _ => panic!("Expected NetworkError variant"),
            }
        }
    }

    #[tokio::test]
    async fn test_process_image_chunk() {
        let chunk = b"[{
            \"candidates\": [
                {
                    \"content\": {
                        \"parts\": [
                            {
                                \"inlineData\": {
                                    \"mimeType\": \"image/png\",
                                    \"data\": \"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==\"
                                }
                            }
                        ],
                        \"role\": \"model\"
                    },
                    \"finishReason\": \"STOP\",
                    \"index\": 0
                }
            ],
            \"usageMetadata\": {
                \"promptTokenCount\": 57,
                \"candidatesTokenCount\": 1310,
                \"totalTokenCount\": 1367,
                \"promptTokensDetails\": [
                    {
                        \"modality\": \"TEXT\",
                        \"tokenCount\": 57
                    }
                ],
                \"candidatesTokensDetails\": [
                    {
                        \"modality\": \"IMAGE\",
                        \"tokenCount\": 1290
                    }
                ]
            }
        }";

        let mut buffer = String::new();
        let mut thinking_content = String::new();
        let mut regular_content = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let job_id = "jobid_18b7d629-751f-4b0d-8f14-2ffbeb521106";
        let inbox_name: Option<InboxName> =
            Some(InboxName::get_job_inbox_name_from_params(job_id.to_string()).unwrap());
        let db = setup_test_db().await;

        // Create the job in the database so save_and_process_file_with_jobid won't fail
        use shinkai_message_primitives::schemas::job_config::JobConfig;
        use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;

        let scope = MinimalJobScope::default();
        let config = JobConfig::empty();
        db.create_new_job(
            job_id.to_string(),
            "test_agent".to_string(),
            scope,
            false,
            None,
            Some(config),
        )
        .expect("Failed to create test job");
        let mut is_done = false;
        let mut finish_reason = None;
        let mut function_calls = Vec::new();
        let mut native_parts = Vec::new();
        let mut in_thinking = false;
        let mut thinking_started = false;

        process_chunk(
            chunk,
            &mut buffer,
            &mut thinking_content,
            &mut regular_content,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
            &mut native_parts,
            &mut in_thinking,
            &mut thinking_started,
        )
        .await
        .unwrap();

        // Verify that the image was successfully processed and saved
        assert_eq!(finish_reason, Some("STOP".to_string()));

        // Verify that the image file reference was added to generated_files
        let found = generated_files
            .iter()
            .any(|f| f.path.to_string_lossy().contains("generated_image_test_session_id_"));
        assert!(
            found,
            "Expected a generated file starting with 'generated_image_test_session_id_'"
        );
    }
}
