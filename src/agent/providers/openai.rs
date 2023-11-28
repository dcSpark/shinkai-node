use std::collections::HashMap;
use std::fmt;

use super::super::{error::AgentError, execution::job_prompts::Prompt};
use super::LLMProvider;
use async_trait::async_trait;
use reqwest::Client;
use serde::de::Deserializer;
use serde::de::Error;
use serde::de::{MapAccess, Visitor};
use serde::ser::{SerializeMap, SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use serde_json::{self, Map};
use shinkai_message_primitives::schemas::agents::serialized_agent::OpenAI;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use tiktoken_rs::model::get_context_size;
use tiktoken_rs::num_tokens_from_messages;

#[derive(Debug, Deserialize)]
pub struct Response {
    id: String,
    object: String,
    created: u64,
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct Choice {
    index: i32,
    message: OpenAIApiMessage,
}

#[derive(Debug, Clone)]
pub enum MessageContent {
    Text(String),
    ImageUrl { url: String },
}

impl Serialize for MessageContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            MessageContent::Text(text) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "text")?;
                map.serialize_entry("text", text)?;
                map.end()
            }
            MessageContent::ImageUrl { url } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "image_url")?;
                let url_map: HashMap<String, &String> = [("url".to_string(), url)].iter().cloned().collect();
                map.serialize_entry("image_url", &url_map)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for MessageContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Note: very ugly patch
        let s: String = Deserialize::deserialize(deserializer)?;
        Ok(MessageContent::Text(s))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIApiMessage {
    pub role: String,
    pub content: MessageContent,
}

impl Serialize for OpenAIApiMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_struct("OpenAIApiMessage", 2)?;
        map.serialize_field("role", &self.role)?;
        map.serialize_field("content", &[&self.content])?;
        map.end()
    }
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}

#[derive(Serialize)]
struct ApiPayload {
    model: String,
    messages: String, // Maybe it'd be better to have Vec<Message> here?
    temperature: f64,
    max_tokens: usize,
}

#[async_trait]
impl LLMProvider for OpenAI {
    async fn call_api(
        &self,
        client: &Client,
        url: Option<&String>,
        api_key: Option<&String>,
        prompt: Prompt,
    ) -> Result<JsonValue, AgentError> {
        if let Some(base_url) = url {
            if let Some(key) = api_key {
                let url = format!("{}{}", base_url, "/v1/chat/completions");

                let total_tokens = Self::get_max_tokens(self.model_type.as_str());
                let tiktoken_messages = prompt.generate_openai_messages(Some(total_tokens / 2))?;

                let filtered_tiktoken_messages: Vec<_> = tiktoken_messages
                    .clone()
                    .into_iter()
                    .filter(|message| message.name.as_deref() != Some("image"))
                    .collect();

                let used_tokens = num_tokens_from_messages(
                    Self::normalize_model(&self.model_type.clone()).as_str(),
                    &filtered_tiktoken_messages,
                )?;
                let mut max_tokens = std::cmp::max(5, total_tokens - used_tokens);
                max_tokens = std::cmp::min(max_tokens, Self::get_max_output_tokens(self.model_type.as_str()));

                let mut messages: Vec<OpenAIApiMessage> = tiktoken_messages
                    .into_iter()
                    .filter_map(|message| {
                        if let Some(content) = message.content {
                            let message_content = match &message.name {
                                Some(name) if name == "image" => MessageContent::ImageUrl { url: content },
                                _ => MessageContent::Text(content),
                            };

                            Some(OpenAIApiMessage {
                                role: message.role,
                                content: message_content,
                            })
                        } else {
                            eprintln!(
                                "Warning: Message with role '{}' has no content. Ignoring.",
                                message.role
                            );
                            None
                        }
                    })
                    .collect();

                if let Some(last_message) = messages.last_mut() {
                    match &mut last_message.content {
                        MessageContent::Text(text) => {
                            if !text.ends_with(" ```") {
                                text.push_str(" ```json");
                            }
                        }
                        _ => {}
                    }
                }
                let messages_json = serde_json::to_value(&messages)?;

                let mut payload = json!({
                    "model": self.model_type,
                    "messages": messages_json,
                    "temperature": 0.7,
                    "max_tokens": max_tokens,
                });

                // Openai doesn't support json_object response format for vision models. wut?
                if !self.model_type.contains("vision") {
                    payload["response_format"] = json!({ "type": "json_object" });
                }

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Body: {:?}", payload).as_str(),
                );

                let res = client
                    .post(url)
                    .bearer_auth(key)
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
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Call API Response Text: {:?}", response_text).as_str(),
                );

                let data_resp: Result<JsonValue, _> = serde_json::from_str(&response_text);

                match data_resp {
                    Ok(value) => {
                        if self.model_type.contains("vision") {
                            let data: Response = serde_json::from_value(value).map_err(AgentError::SerdeError)?;
                            let response_string: String = data
                                .choices
                                .iter()
                                .filter_map(|choice| match &choice.message.content {
                                    MessageContent::Text(text) => {
                                        // Unescape the JSON string
                                        let cleaned_json_str = text.replace("\\\"", "\"").replace("\\n", "\n");
                                        Some(cleaned_json_str)
                                    },
                                    MessageContent::ImageUrl { .. } => None,
                                })
                                .collect::<Vec<String>>()
                                .join(" ");
                            Self::extract_first_json_object(&response_string)
                        } else {
                            let data: Response = serde_json::from_value(value).map_err(AgentError::SerdeError)?;
                            let response_string: String = data
                                .choices
                                .iter()
                                .filter_map(|choice| match &choice.message.content {
                                    MessageContent::Text(text) => Some(text.clone()),
                                    MessageContent::ImageUrl { .. } => None,
                                })
                                .collect::<Vec<String>>()
                                .join(" ");
                            Self::extract_first_json_object(&response_string)
                        }
                    }
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            format!("Failed to parse response: {:?}", e).as_str(),
                        );
                        Err(AgentError::SerdeError(e))
                    }
                }
            } else {
                Err(AgentError::ApiKeyNotSet)
            }
        } else {
            Err(AgentError::UrlNotSet)
        }
    }

    /// Returns the maximum number of tokens supported based on
    /// the provided model string
    fn get_max_tokens(s: &str) -> usize {
        // Custom added, since not supported by Tiktoken atm
        if s == "gpt-4-1106-preview" {
            128_000
        } else {
            let normalized_model = Self::normalize_model(s);
            get_context_size(normalized_model.as_str())
        }
    }

    /// Returns a maximum number of output tokens
    fn get_max_output_tokens(s: &str) -> usize {
        4096
    }

    /// Normalizes the model string to one that is supported by Tiktoken crate
    fn normalize_model(s: &str) -> String {
        if s.to_string().starts_with("gpt-4") {
            "gpt-4-32k".to_string()
        } else if s.to_string().starts_with("gpt-3.5") {
            "gpt-3.5-turbo-16k".to_string()
        } else {
            "gpt-4".to_string()
        }
    }
}
