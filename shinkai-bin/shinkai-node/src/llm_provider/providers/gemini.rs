use std::sync::Arc;

use super::super::{error::LLMProviderError, execution::prompts::prompts::Prompt};
use super::shared::ollama::ollama_conversation_prepare_messages;
use super::shared::openai::openai_prepare_messages;
use super::LLMService;
use crate::llm_provider::execution::chains::inference_chain_trait::LLMInferenceResponse;
use crate::llm_provider::providers::shared::openai::MessageContent;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use crate::network::ws_manager::WSUpdateHandler;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{Gemini, LLMProviderInterface};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tokio::sync::Mutex;

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

                // curl https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key=$GOOGLE_API_KEY
                let url = format!("{}{}:generateContent?key={}", base_url, self.model_type, key); // :streamGenerateContent (?)

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
                    .ok_or_else(|| LLMProviderError::UnexpectedPromptResultVariant("Expected array of messages".to_string()))?
                    .iter()
                    .map(|msg| {
                        let role = match msg["role"].as_str() {
                            Some("system") => "user", // Gemini doesn't have a system role, so we'll use user
                            Some("assistant") => "model",
                            Some("user") | _ => "user",
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
                        "maxOutputTokens": 2048,
                        "stopSequences": []
                    },
                    "safetySettings": []
                });

                // Print payload as a pretty JSON string
                match serde_json::to_string_pretty(&payload) {
                    Ok(pretty_json) => eprintln!("Payload: {}", pretty_json),
                    Err(e) => eprintln!("Failed to serialize payload: {:?}", e),
                };

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

                let response_text = res.text().await?;
                eprintln!("Response Text: {:?}", response_text);
                let data_resp: Result<JsonValue, _> = serde_json::from_str(&response_text);
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Response Text: {:?}", response_text).as_str(),
                );

                match data_resp {
                    Ok(value) => {
                        if let Some(error) = value.get("error") {
                            let code = error.get("code").and_then(|c| c.as_str());
                            let formatted_error = if let (Some(code), Some(message)) =
                                (code, error.get("message").and_then(|m| m.as_str()))
                            {
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

                        let data: GeminiResponse =
                            serde_json::from_value(value).map_err(LLMProviderError::SerdeError)?;

                        let response_string: String = data
                            .candidates
                            .iter()
                            .filter_map(|candidate| candidate.content.parts.iter().next().map(|part| part.text.clone()))
                            .collect::<Vec<String>>()
                            .join(" ");

                        eprintln!("Response String: {:?}", response_string);
                        Ok(LLMInferenceResponse::new(response_string, json!({}), None))
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Failed to parse response: {:?}", e).as_str(),
                        );
                        Err(LLMProviderError::SerdeError(e))
                    }
                }
            } else {
                Err(LLMProviderError::ApiKeyNotSet)
            }
        } else {
            Err(LLMProviderError::UrlNotSet)
        }
    }
}
