use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use tiktoken_rs::ChatCompletionRequestMessage;

use crate::{
    llm_provider::{error::LLMProviderError, execution::prompts::prompts::Prompt},
    managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResult, PromptResultEnum},
};

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
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&chat_completion_messages);
    // Calculate the remaining output tokens available
    let remaining_output_tokens = ModelCapabilitiesManager::get_remaining_output_tokens(model, used_tokens);

    // Converts the ChatCompletionMessages to OpenAIApiMessages
    let messages = from_chat_completion_messages(chat_completion_messages)?;

    let messages_json = serde_json::to_value(messages)?;
    Ok(PromptResult {
        value: PromptResultEnum::Value(messages_json),
        remaining_tokens: remaining_output_tokens,
    })
}

/// Converts ChatCompletionRequestMessages to OllamaMessage
fn from_chat_completion_messages(
    chat_completion_messages: Vec<ChatCompletionRequestMessage>,
) -> Result<Vec<OllamaMessage>, LLMProviderError> {
    let mut messages: Vec<OllamaMessage> = Vec::new();
    let mut iter = chat_completion_messages.into_iter().peekable();

    while let Some(message) = iter.next() {
        if let Some(content) = message.content {
            let mut images = None;

            if message.role == "user" {
                if let Some(next_message) = iter.peek() {
                    if next_message.role == "user" && next_message.name.as_deref() == Some("image") {
                        if let Some(image_content) = &next_message.content {
                            images = Some(vec![image_content.clone()]);
                            iter.next(); // Consume the next message
                        }
                    }
                }
            }

            messages.push(OllamaMessage {
                role: message.role,
                content,
                images,
            });
        }
    }

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiktoken_rs::ChatCompletionRequestMessage;

    #[test]
    fn test_from_chat_completion_messages() {
        let chat_completion_messages = vec![
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("You are a very helpful assistant that's very good at completing a task.".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "user".to_string(),
                content: Some("The current main task at hand is: `describe this`".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "user".to_string(),
                content: Some("iVBORw0KGgoAAAANSUhEUgAAAlgAAAJYCAYAAAC".to_string()),
                name: Some("image".to_string()),
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and separated paragraphs. Start your response with # Answer".to_string()),
                name: None,
                function_call: None,
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
                content: "Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and separated paragraphs. Start your response with # Answer".to_string(),
                images: None,
            },
        ];

        let result = from_chat_completion_messages(chat_completion_messages).unwrap();
        assert_eq!(result, expected_messages);
    }
}