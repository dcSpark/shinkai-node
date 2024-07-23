use std::sync::Arc;

use super::super::{error::LLMProviderError, execution::prompts::prompts::Prompt};
use super::shared::openai::openai_prepare_messages;
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use crate::network::ws_manager::{WSMetadata, WSUpdateHandler};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{Gemini, LLMProviderInterface};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::error::Error;
use tokio::sync::Mutex;
use uuid::Uuid;
#[derive(Debug, Deserialize)]
struct GeminiStreamingResponse {
    candidates: Vec<StreamingCandidate>,
    // #[serde(rename = "usageMetadata")]
    // usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct StreamingCandidate {
    content: StreamingContent,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamingContent {
    parts: Vec<StreamingPart>,
    // role: String,
}

#[derive(Debug, Deserialize)]
struct StreamingPart {
    text: String,
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

                let result = openai_prepare_messages(&model, prompt)?;
                let messages = match result.messages {
                    PromptResultEnum::Value(v) => v,
                    _ => {
                        return Err(LLMProviderError::UnexpectedPromptResultVariant(
                            "Expected Value variant in PromptResultEnum".to_string(),
                        ))
                    }
                };

                // Convert OpenAI-style messages to Gemini format
                let contents: Vec<serde_json::Value> = messages
                    .as_array()
                    .ok_or_else(|| {
                        LLMProviderError::UnexpectedPromptResultVariant("Expected array of messages".to_string())
                    })?
                    .iter()
                    .map(|msg| {
                        let role = match msg["role"].as_str() {
                            Some("system") => "user", // Gemini doesn't have a system role, so we'll use user
                            Some("assistant") => "model",
                            Some("user") => "user",
                            _ => "user",
                        };
                        json!({
                            "role": role,
                            "parts": [{
                                "text": msg["content"]
                            }]
                        })
                    })
                    .collect();

                let payload = json!({
                    "contents": contents,
                    "generationConfig": {
                        "temperature": 0.9,
                        "topK": 1,
                        "topP": 1,
                        "maxOutputTokens": 2048
                    },
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

                // Print payload as a pretty JSON string only if IS_TESTING is true
                if std::env::var("IS_TESTING").unwrap_or_default() == "true" {
                    match serde_json::to_string_pretty(&payload) {
                        Ok(pretty_json) => eprintln!("Payload: {}", pretty_json),
                        Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
                    };
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
                let mut buffer = String::new();
                let mut is_done = false;
                let mut finish_reason = None;

                while let Some(item) = stream.next().await {
                    match item {
                        Ok(chunk) => {
                            process_chunk(
                                &chunk,
                                &mut buffer,
                                &mut response_text,
                                &session_id,
                                &ws_manager_trait,
                                &inbox_name,
                                &mut is_done,
                                &mut finish_reason,
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
                Ok(LLMInferenceResponse::new(response_text, json!({}), None))
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
    session_id: &str,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: &Option<InboxName>,
    is_done: &mut bool,
    finish_reason: &mut Option<String>,
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
        *is_done = true; // Set is_done to true if buffer ends with ']'
    }

    // Add a trailing ']' to make it a valid JSON array
    let json_str = format!("[{}]", buffer);

    match serde_json::from_str::<Vec<JsonValue>>(&json_str) {
        Ok(array) => {
            for value in array {
                process_gemini_response(
                    value,
                    response_text,
                    session_id,
                    ws_manager_trait,
                    inbox_name,
                    is_done,
                    finish_reason,
                )
                .await?;
            }
            buffer.clear();
        }
        Err(e) => {
            eprintln!("Failed to parse JSON array: {:?}", e);
        }
    }

    // Check if is_done is true and send a final message if necessary
    if *is_done {
        if let Some(ref manager) = ws_manager_trait {
            if let Some(ref inbox_name) = inbox_name {
                let m = manager.lock().await;
                let inbox_name_string = inbox_name.to_string();

                let metadata = WSMetadata {
                    id: Some(session_id.to_string()),
                    is_done: *is_done,
                    done_reason: None,
                    total_duration: None,
                    eval_count: None,
                };

                let _ = m
                    .queue_message(
                        WSTopic::Inbox,
                        inbox_name_string.clone(),
                        response_text.to_string(),
                        Some(metadata),
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
    session_id: &str,
    ws_manager_trait: &Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    inbox_name: &Option<InboxName>,
    is_done: &mut bool,
    finish_reason: &mut Option<String>,
) -> Result<(), LLMProviderError> {
    if let Ok(response) = serde_json::from_value::<GeminiStreamingResponse>(value) {
        for candidate in &response.candidates {
            for part in &candidate.content.parts {
                let content = &part.text;
                response_text.push_str(content);
                finish_reason.clone_from(&candidate.finish_reason);

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

                        let _ = m
                            .queue_message(
                                WSTopic::Inbox,
                                inbox_name_string.clone(),
                                content.to_string(),
                                Some(metadata),
                                true,
                            )
                            .await;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

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
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let mut is_done = false;
        let mut finish_reason = None;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &mut is_done,
            &mut finish_reason,
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
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let mut is_done = false;
        let mut finish_reason = None;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &mut is_done,
            &mut finish_reason,
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
        let session_id = "test_session_id";
        let ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>> = None;
        let inbox_name: Option<InboxName> = None;
        let mut is_done = false;
        let mut finish_reason = None;

        process_chunk(
            chunk,
            &mut buffer,
            &mut response_text,
            session_id,
            &ws_manager_trait,
            &inbox_name,
            &mut is_done,
            &mut finish_reason,
        )
        .await
        .unwrap();

        assert_eq!(response_text, " in greater detail. \n");
        assert!(is_done);
        assert_eq!(finish_reason, Some("STOP".to_string()));
    }
}
