use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;

use crate::{
    llm_provider::{error::LLMProviderError, execution::prompts::prompts::Prompt},
    managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResult, PromptResultEnum},
};

use super::llm_message::LlmMessage;

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaAPIResponse {
    pub model: String,
    pub created_at: String,
    pub response: Value,
    pub done: bool,
    pub total_duration: i64,
    pub load_duration: i64,
    pub prompt_eval_count: i32,
    pub prompt_eval_duration: i64,
    pub eval_count: i32,
    pub eval_duration: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OllamaAPIStreamingResponse {
    pub model: String,
    pub created_at: String,
    pub message: OllamaMessage,
    pub done: bool,
    pub done_reason: Option<String>,
    pub total_duration: Option<i64>,
    pub load_duration: Option<i64>,
    pub prompt_eval_count: Option<i32>,
    pub prompt_eval_duration: Option<i64>,
    pub eval_count: Option<i32>,
    pub eval_duration: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

pub fn ollama_conversation_prepare_messages(
    model: &LLMProviderInterface,
    prompt: Prompt,
) -> Result<PromptResult, LLMProviderError> {
    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    // Generate the messages and filter out images
    let chat_completion_messages = prompt.generate_openai_messages(Some(max_input_tokens))?;

    // Get a more accurate estimate of the number of used tokens
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_llama3(&chat_completion_messages);
    // Calculate the remaining output tokens available
    let remaining_output_tokens = ModelCapabilitiesManager::get_remaining_output_tokens(model, used_tokens);

    // Converts the ChatCompletionMessages to OpenAIApiMessages
    let messages = from_chat_completion_messages(chat_completion_messages)?;

    let messages_json = serde_json::to_value(messages)?;
    Ok(PromptResult {
        messages: PromptResultEnum::Value(messages_json),
        functions: None,
        remaining_tokens: remaining_output_tokens,
    })
}

/// Converts LlmMessage to OllamaMessage
fn from_chat_completion_messages(
    chat_completion_messages: Vec<LlmMessage>,
) -> Result<Vec<OllamaMessage>, LLMProviderError> {
    let mut messages: Vec<OllamaMessage> = Vec::new();
    let mut iter = chat_completion_messages.into_iter().peekable();

    while let Some(message) = iter.next() {
        if let Some(content) = message.content {
            let mut images = None;

            if message.role.clone().unwrap_or_default() == "user" {
                if let Some(next_message) = iter.peek() {
                    if next_message.role.clone().unwrap_or_default() == "user"
                        && next_message.name.as_deref() == Some("image")
                    {
                        if let Some(image_content) = &next_message.content {
                            images = Some(vec![image_content.clone()]);
                            iter.next(); // Consume the next message
                        }
                    }
                }
            }

            messages.push(OllamaMessage {
                role: message.role.unwrap_or_default(),
                content,
                images,
            });
        }
    }

    Ok(messages)
}

pub fn ollama_conversation_prepare_messages_with_tooling(
    model: &LLMProviderInterface,
    prompt: Prompt,
) -> Result<PromptResult, LLMProviderError> {
    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    // Generate the messages and filter out images
    let chat_completion_messages = prompt.generate_openai_messages(Some(max_input_tokens))?;

    // Get a more accurate estimate of the number of used tokens
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_llama3(&chat_completion_messages);
    // Calculate the remaining output tokens available
    let remaining_output_tokens = ModelCapabilitiesManager::get_remaining_output_tokens(model, used_tokens);

    // Separate messages into those with a valid role and those without
    let (messages_with_role, tools): (Vec<_>, Vec<_>) = chat_completion_messages
        .into_iter()
        .partition(|message| message.role.is_some());

    // Convert both sets of messages to serde Value
    let messages_json = serde_json::to_value(messages_with_role)?;
    let tools_json = serde_json::to_value(tools)?;

    // Convert messages_json and tools_json to Vec<serde_json::Value>
    let messages_vec = match messages_json {
        serde_json::Value::Array(arr) => arr,
        _ => vec![],
    };

    // Flatten the tools array to extract functions directly
    let tools_vec = match tools_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .flat_map(|tool| {
                if let serde_json::Value::Object(mut map) = tool {
                    map.remove("functions")
                        .and_then(|functions| {
                            if let serde_json::Value::Array(funcs) = functions {
                                Some(
                                    funcs
                                        .into_iter()
                                        .map(|func| {
                                            serde_json::json!({
                                                "type": "function",
                                                "function": func
                                            })
                                        })
                                        .collect(),
                                )
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default()
                } else {
                    vec![]
                }
            })
            .collect(),
        _ => vec![],
    };

    Ok(PromptResult {
        messages: PromptResultEnum::Value(serde_json::Value::Array(messages_vec)),
        functions: Some(tools_vec),
        remaining_tokens: remaining_output_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm_provider::providers::shared::llm_message::LlmMessage;

    #[test]
    fn test_from_llm_messages() {
        let llm_messages = vec![
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("You are a very helpful assistant that's very good at completing a task.".to_string()),
                name: None,
                function_call: None,
                functions: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("The current main task at hand is: `describe this`".to_string()),
                name: None,
                function_call: None,
                functions: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("iVBORw0KGgoAAAANSUhEUgAAAlgAAAJYCAYAAAC".to_string()),
                name: Some("image".to_string()),
                function_call: None,
                functions: None,
            },
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and separated paragraphs.".to_string()),
                name: None,
                function_call: None,
                functions: None,
            },
        ];

        let expected_messages = vec![
            OllamaMessage {
                role: "system".to_string(),
                content: "You are a very helpful assistant that's very good at completing a task.".to_string(),
                images: None,
            },
            OllamaMessage {
                role: "user".to_string(),
                content: "The current main task at hand is: `describe this`".to_string(),
                images: Some(vec!["iVBORw0KGgoAAAANSUhEUgAAAlgAAAJYCAYAAAC".to_string()]),
            },
            OllamaMessage {
                role: "system".to_string(),
                content: "Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and separated paragraphs.".to_string(),
                images: None,
            },
        ];

        let result = from_chat_completion_messages(llm_messages).unwrap();
        assert_eq!(result, expected_messages);
    }
}
