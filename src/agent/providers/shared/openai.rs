use std::collections::HashMap;


use crate::agent::error::AgentError;
use crate::agent::execution::job_prompts::Prompt;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use serde::de::Deserializer;


use serde::ser::{SerializeMap, SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};

use serde_json::{self};
use shinkai_message_primitives::schemas::agents::serialized_agent::AgentLLMInterface;


#[derive(Debug, Deserialize)]
pub struct OpenAIResponse {
    id: String,
    object: String,
    created: u64,
    pub choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: i32,
    pub message: OpenAIApiMessage,
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
pub struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}

#[derive(Serialize)]
pub struct ApiPayload {
    model: String,
    messages: String,
    temperature: f64,
    max_tokens: usize,
}

pub fn openai_prepare_messages(
    model: &AgentLLMInterface,
    _model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, AgentError> {
    let half_total_tokens = total_tokens.checked_div(2).ok_or(AgentError::UnexpectedPromptResult(
        "Failed dividing total_tokens.".to_string(),
    ))?;
    let tiktoken_messages = prompt.generate_openai_messages(Some(half_total_tokens))?;

    let filtered_tiktoken_messages: Vec<_> = tiktoken_messages
        .clone()
        .into_iter()
        .filter(|message| message.name.as_deref() != Some("image"))
        .collect();

    let used_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&filtered_tiktoken_messages)
        .map_err(AgentError::TokenizationError)?;
    let mut max_tokens = std::cmp::max(5, total_tokens.saturating_sub(used_tokens));
    max_tokens = std::cmp::min(
        max_tokens,
        ModelCapabilitiesManager::get_max_output_tokens(&model.clone()),
    );

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
    Ok(PromptResult {
        value: PromptResultEnum::Value(messages_json),
        remaining_tokens: max_tokens,
    })
}
