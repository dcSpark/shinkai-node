use std::sync::Arc;

use super::super::error::LLMProviderError;
use super::shared::gemini_api::gemini_prepare_messages;
use super::shared::shared_model_logic::save_image_file;
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
use shinkai_message_primitives::schemas::ws_types::{
    ToolMetadata, ToolStatus, ToolStatusType, WSMessageType, WSMetadata, WSUpdateHandler, WidgetMetadata
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
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

                let mut generation_config = json!({
                    "temperature": config.as_ref().and_then(|c| c.temperature).unwrap_or(0.9),
                    "topK": config.as_ref().and_then(|c| c.top_k).unwrap_or(1),
                    "topP": config.as_ref().and_then(|c| c.top_p).unwrap_or(1.0),
                    "maxOutputTokens": config.as_ref().and_then(|c| c.max_tokens).unwrap_or(8192)
                });

                // Add thinkingConfig if thinking is enabled
                if let Some(true) = config.as_ref().and_then(|c| c.thinking) {
                    let thinking_budget = match config.as_ref().and_then(|c| c.reasoning_effort.as_deref()) {
                        Some("low") => 1024,
                        Some("medium") => 8192,
                        Some("high") => 24576,
                        _ => -1, // Default unlimited budget
                    };

                    generation_config["thinkingConfig"] = json!({
                        "thinkingBudget": thinking_budget,
                        "includeThoughts": true
                    });
                }

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
                let mut response_text = String::new();
                let mut generated_files = Vec::new();
                let mut buffer = String::new();
                let mut is_done = false;
                let mut finish_reason = None;
                let mut function_calls = Vec::new();
                let mut in_thinking = false;
                let mut thinking_started = false;

                while let Some(item) = stream.next().await {
                    match item {
                        Ok(chunk) => {
                            process_chunk(
                                &chunk,
                                &mut buffer,
                                &mut response_text,
                                &mut generated_files,
                                &session_id,
                                &ws_manager_trait,
                                &inbox_name,
                                &db,
                                &mut is_done,
                                &mut finish_reason,
                                &mut function_calls,
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
                    response_text,
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
    response_text: &mut String,
    generated_files: &mut Vec<ShinkaiPath>,
    session_id: &str,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: &Option<InboxName>,
    db: &SqliteManager,
    is_done: &mut bool,
    finish_reason: &mut Option<String>,
    function_calls: &mut Vec<FunctionCall>,
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
                    response_text,
                    generated_files,
                    session_id,
                    ws_manager_trait,
                    inbox_name,
                    db,
                    is_done,
                    finish_reason,
                    function_calls,
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
            response_text.push_str("</think>");
            *in_thinking = false;
        }
        
        if let Some(ref manager) = ws_manager_trait {
            if let Some(ref inbox_name) = inbox_name {
                let m = manager.lock().await;
                let inbox_name_string = inbox_name.to_string();

                let metadata = WSMetadata {
                    id: Some(session_id.to_string()),
                    is_done: *is_done,
                    done_reason: finish_reason.clone(),
                    total_duration: None,
                    eval_count: None,
                };

                let ws_message_type = WSMessageType::Metadata(metadata);

                let _ = m
                    .queue_message(
                        WSTopic::Inbox,
                        inbox_name_string.clone(),
                        response_text.to_string(),
                        ws_message_type,
                        true,
                    )
                    .await;
            }
        }
    }

    Ok(())
}

async fn process_gemini_response(
    value: JsonValue,
    response_text: &mut String,
    generated_files: &mut Vec<ShinkaiPath>,
    session_id: &str,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: &Option<InboxName>,
    db: &SqliteManager,
    is_done: &bool,
    finish_reason: &mut Option<String>,
    function_calls: &mut Vec<FunctionCall>,
    in_thinking: &mut bool,
    thinking_started: &mut bool,
) -> Result<(), LLMProviderError> {
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
                            // First thought - prepend <think>
                            text_to_add.push_str("<think>");
                            *thinking_started = true;
                            *in_thinking = true;
                        }
                        text_to_add.push_str(&part.text);
                    } else {
                        // This is a normal response
                        if *in_thinking {
                            // End thinking mode - append </think>
                            text_to_add.push_str("</think>");
                            *in_thinking = false;
                        }
                        text_to_add.push_str(&part.text);
                    }
                    
                    response_text.push_str(&text_to_add);

                    if let Some(ref manager) = ws_manager_trait {
                        if let Some(ref inbox_name) = inbox_name {
                            let m = manager.lock().await;
                            let inbox_name_string = inbox_name.to_string();

                            let metadata = WSMetadata {
                                id: Some(session_id.to_string()),
                                is_done: *is_done,
                                done_reason: finish_reason.clone(),
                                total_duration: None,
                                eval_count: None,
                            };

                            let ws_message_type = WSMessageType::Metadata(metadata);

                            let _ = m
                                .queue_message(
                                    WSTopic::Inbox,
                                    inbox_name_string.clone(),
                                    text_to_add,
                                    ws_message_type,
                                    true,
                                )
                                .await;
                        }
                    }
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
        id: None,
        call_type: Some("function".to_string()),
    };
    function_calls.push(fc.clone());

    // Send WebSocket update for function call
    if let Some(ref manager) = ws_manager_trait {
        if let Some(ref inbox_name) = inbox_name {
            let m = manager.lock().await;
            let inbox_name_string = inbox_name.to_string();

            let tool_metadata = ToolMetadata {
                tool_name: fc.name.clone(),
                tool_router_key: fc.tool_router_key.clone(),
                args: serde_json::to_value(&fc.arguments)
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
                result: None,
                status: ToolStatus {
                    type_: ToolStatusType::Running,
                    reason: None,
                },
                index: fc.index,
            };

            let ws_message_type = WSMessageType::Widget(WidgetMetadata::ToolRequest(tool_metadata));

            let _ = m
                .queue_message(
                    WSTopic::Inbox,
                    inbox_name_string,
                    serde_json::to_string(&fc).unwrap_or_else(|_| "{}".to_string()),
                    ws_message_type,
                    true,
                )
                .await;
        }
    }
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

    async fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = std::path::PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type = EmbeddingModelType::OllamaTextEmbeddingsInference(
            OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM
        );

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
        let mut response_text = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut function_calls = Vec::new();
        let mut in_thinking = false;
        let mut thinking_started = false;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
            &mut in_thinking,
            &mut thinking_started,
        )
        .await
        .unwrap();

        assert_eq!(response_text, "The");
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
        let mut response_text = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut function_calls = Vec::new();
        let mut in_thinking = false;
        let mut thinking_started = false;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
            &mut in_thinking,
            &mut thinking_started,
        )
        .await
        .unwrap();

        assert_eq!(
            response_text,
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
        let mut response_text = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut function_calls = Vec::new();
        let mut in_thinking = false;
        let mut thinking_started = false;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
            &mut in_thinking,
            &mut thinking_started,
        )
        .await
        .unwrap();

        assert_eq!(response_text, " in greater detail. \n");
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
        let mut response_text = String::new();
        let mut generated_files = Vec::new();
        let mut function_calls = Vec::new();
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
                &mut response_text,
                &mut generated_files,
                session_id,
                &ws_manager_trait,
                &inbox_name,
                &db,
                &mut is_done,
                &mut finish_reason,
                &mut function_calls,
                &mut in_thinking,
                &mut thinking_started,
            )
            .await
            .unwrap();
        }

        assert_eq!(response_text, "");
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
        let mut response_text = String::new();
        let mut generated_files = Vec::new();
        let mut function_calls = Vec::new();
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
                &mut response_text,
                &mut generated_files,
                session_id,
                &ws_manager_trait,
                &inbox_name,
                &db,
                &mut is_done,
                &mut finish_reason,
                &mut function_calls,
                &mut in_thinking,
                &mut thinking_started,
            )
            .await
            .unwrap();
        }

        // Verify the response contains properly formatted thinking tags
        assert!(response_text.starts_with("<think>"));
        assert!(response_text.contains("**Clarifying Limitations**"));
        assert!(response_text.contains("**Highlighting Constraints**"));
        assert!(response_text.contains("</think>As an AI, I don't have real-time access"));
        assert!(response_text.contains("Major news organizations"));
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
        let mut response_text = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let db = setup_test_db().await;
        let mut is_done = false;
        let mut finish_reason = None;
        let mut function_calls = Vec::new();
        let mut in_thinking = false;
        let mut thinking_started = false;

        let result = process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
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
        let mut response_text = String::new();
        let mut generated_files = Vec::new();
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let job_id = "jobid_18b7d629-751f-4b0d-8f14-2ffbeb521106";
        let inbox_name: Option<InboxName> = Some(InboxName::get_job_inbox_name_from_params(job_id.to_string()).unwrap());
        let db = setup_test_db().await;
        
        // Create the job in the database so save_and_process_file_with_jobid won't fail
        use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
        use shinkai_message_primitives::schemas::job_config::JobConfig;
        
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
        let mut in_thinking = false;
        let mut thinking_started = false;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            &mut generated_files,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &db,
            &mut is_done,
            &mut finish_reason,
            &mut function_calls,
            &mut in_thinking,
            &mut thinking_started,
        )
        .await
        .unwrap();

        // Verify that the image was successfully processed and saved
        assert_eq!(finish_reason, Some("STOP".to_string()));
        
        // Verify that the image file reference was added to response_text  
        assert!(response_text.contains("generated_image_test_session_id_"));
    }
}
