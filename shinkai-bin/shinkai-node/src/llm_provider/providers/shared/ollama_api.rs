use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_message_primitives::schemas::{
    llm_message::LlmMessage, llm_providers::serialized_llm_provider::LLMProviderInterface, prompts::Prompt,
};

use crate::{
    llm_provider::error::LLMProviderError,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ToolCall {
    pub function: Function,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Function {
    pub name: String,
    pub arguments: Option<serde_json::Map<String, serde_json::Value>>,
}

pub fn ollama_prepare_messages(model: &LLMProviderInterface, prompt: Prompt) -> Result<PromptResult, LLMProviderError> {
    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    // Generate the messages and filter out images
    let chat_completion_messages = prompt.generate_llm_messages(
        Some(max_input_tokens),
        Some("tool".to_string()),
        &ModelCapabilitiesManager::num_tokens_from_llama3,
    )?;

    // Check if the prompt is not empty but messages are empty
    if !prompt.sub_prompts.is_empty() && chat_completion_messages.is_empty() {
        // Calculate the number of tokens used by the prompt
        let (_, used_tokens) = prompt.generate_chat_completion_messages(
            Some("tool".to_string()),
            &ModelCapabilitiesManager::num_tokens_from_llama3,
        );
        return Err(LLMProviderError::MessageTooLargeForLLM {
            max_tokens: max_input_tokens,
            used_tokens,
        });
    }

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
        remaining_output_tokens,
        tokens_used: used_tokens,
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
            let mut images = message.images.clone();

            if message.role.clone().unwrap_or_default() == "user" {
                if let Some(next_message) = iter.peek() {
                    if next_message.role.clone().unwrap_or_default() == "user" {
                        if let Some(next_images) = &next_message.images {
                            if images.is_none() {
                                images = Some(next_images.clone());
                            } else {
                                images.as_mut().unwrap().extend(next_images.clone());
                            }
                            iter.next(); // Consume the next message
                        }
                    }
                }
            }

            messages.push(OllamaMessage {
                role: message.role.unwrap_or_default(),
                content,
                images,
                tool_calls: None,
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
    let chat_completion_messages = prompt.generate_llm_messages(
        Some(max_input_tokens),
        Some("tool".to_string()),
        &ModelCapabilitiesManager::num_tokens_from_llama3,
    )?;

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
        remaining_output_tokens,
        tokens_used: used_tokens,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use shinkai_message_primitives::schemas::{
        llm_providers::serialized_llm_provider::SerializedLLMProvider,
        subprompts::{SubPrompt, SubPromptAssetType, SubPromptType},
    };

    use super::*;

    #[test]
    fn test_ollama_from_llm_messages() {
        let sub_prompts = vec![
            SubPrompt::Omni(
                SubPromptType::System,
                "You are Neko the cat respond like one".to_string(),
                vec![],
                98,
            ),
            SubPrompt::Omni(SubPromptType::User, "Hello".to_string(), vec![], 97),
            SubPrompt::Omni(
                SubPromptType::Assistant,
                "Great to meet you. What would you like to know?".to_string(),
                vec![],
                97,
            ),
            SubPrompt::Omni(
                SubPromptType::User,
                "I have two dogs in my house. How many paws are in my house?".to_string(),
                vec![(
                    SubPromptAssetType::Image,
                    "iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAMAAABrrFhUAAAABGdBTUEAALGPC/xhBQAAAAFzUkdCAK7OHOkAAAD5UExURQAAAACl7QCl7ACm7ACl7ACl7ACm7QCm7QCm7QCm7ACm7ACm7QCl7QCl6wCl7ACl7ACl7ACl7QCl6wCl7QCl7ACm7ACm7ACl7QCm7ACl7QCl6wCm7QCm7QCl7ACl7QCm7QCl7QCl7QCm7ACm7QCl6wCl7ACl7QCl7ACm7ACm7ACl7QCl7ACl7QCm7QCm7ACm7ACl7ACl7QCl6wCm7QCm6wCm7QCm7QCm7QCl7QCl7ACm7QCl7ACm7QCl7QCl7ACk6wCl7QCl7ACm7ACl7QCm7ACl7QCl7ACm7QCl7ACm7ACm7ACm7QCl7ACl7ACm7QCl7QCk7ACm7ACm7ahktTwAAABSdFJOUwDoJJubI+v7+vaN9fswD50JzFrLCCbo+esOCcvUWZvM6S9Z+YzP0cyc0VrriQ6MCCX0JIoK7J5Z9p6ZDi8PiCPr6NMl1CTRJSbn+p2cWiSgD4gNsVXUAAACIElEQVR42u3X11KVMRiG0Wx2+femN+kgiL1Ls4MKKhZQc/8X4xmnye+BM3yznjt4VyaTSUqSJEmSJElX/Z7/ObfS5Gtes3Kz+/D9P8y/MTPKYXr167zl/LXxYQ7V8OlBm/1jGzlcZ3v1+98t5YB1pqrPv5NDtj1Wef83ctC+LFYBjOewPa56/4ZxAXYvKwBe58BtlfdPjyIDTKwWAeZz6DaLAPdjA/SKAPdiAywUAQ5jAwyKAE1sgMkiQA4eAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA8D8Amtj7J4sAg9gAj4oAc7EB1osAvdgA3SLARWyAW0WA6VHk/ROrRYA0ExngR3l/ej6Mu3/5tAIg3Y4L8Kdmf5r9FHX/g2dVAKn/Meb+pX6qbOdbxP2dqVRd/068/XdPUosWj5djzf++P5va9fnNizjzR0dfU/uevOyuf7j2v+NmsNDbfJskSZIkSZKu+gtLvn0aIyUzCwAAAABJRU5ErkJggg==".to_string(),
                    "image.png".to_string(),
                )],
                100,
            ),
        ];

        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(sub_prompts);

        // Use the mock provider
        let model = SerializedLLMProvider::mock_provider().model;

        // Call the openai_prepare_messages function
        let result = ollama_prepare_messages(&model, prompt).expect("Failed to prepare messages");

        // Define the expected messages and functions
        let expected_messages = json!([
            {
                "role": "system",
                "content": "You are Neko the cat respond like one",
                "images": []
            },
            {
                "role": "user",
                "content": "Hello",
                "images": []
            },
            {
                "role": "assistant",
                "content": "Great to meet you. What would you like to know?",
                "images": []
            },
            {
                "role": "user",
                "content": "I have two dogs in my house. How many paws are in my house?",
                "images": ["iVBORw0KGgoAAAANSUhEUgAAAQAAAAEACAMAAABrrFhUAAAABGdBTUEAALGPC/xhBQAAAAFzUkdCAK7OHOkAAAD5UExURQAAAACl7QCl7ACm7ACl7ACl7ACm7QCm7QCm7QCm7ACm7ACm7QCl7QCl6wCl7ACl7ACl7ACl7QCl6wCl7QCl7ACm7ACm7ACl7QCm7ACl7QCl6wCm7QCm7QCl7ACl7QCm7QCl7QCl7QCm7ACm7QCl6wCl7ACl7QCl7ACm7ACm7ACl7QCl7ACl7QCm7QCm7ACm7ACl7ACl7QCl6wCm7QCm6wCm7QCm7QCm7QCl7QCl7ACm7QCl7ACm7QCl7QCl7ACk6wCl7QCl7ACm7ACl7QCm7ACl7QCl7ACm7QCl7ACm7ACm7ACm7QCl7ACl7ACm7QCl7QCk7ACm7ACm7ahktTwAAABSdFJOUwDoJJubI+v7+vaN9fswD50JzFrLCCbo+esOCcvUWZvM6S9Z+YzP0cyc0VrriQ6MCCX0JIoK7J5Z9p6ZDi8PiCPr6NMl1CTRJSbn+p2cWiSgD4gNsVXUAAACIElEQVR42u3X11KVMRiG0Wx2+femN+kgiL1Ls4MKKhZQc/8X4xmnye+BM3yznjt4VyaTSUqSJEmSJElX/Z7/ObfS5Gtes3Kz+/D9P8y/MTPKYXr167zl/LXxYQ7V8OlBm/1jGzlcZ3v1+98t5YB1pqrPv5NDtj1Wef83ctC+LFYBjOewPa56/4ZxAXYvKwBe58BtlfdPjyIDTKwWAeZz6DaLAPdjA/SKAPdiAywUAQ5jAwyKAE1sgMkiQA4eAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA8D8Amtj7J4sAg9gAj4oAc7EB1osAvdgA3SLARWyAW0WA6VHk/ROrRYA0ExngR3l/ej6Mu3/5tAIg3Y4L8Kdmf5r9FHX/g2dVAKn/Meb+pX6qbOdbxP2dqVRd/068/XdPUosWj5djzf++P5va9fnNizjzR0dfU/uevOyuf7j2v+NmsNDbfJskSZIkSZKu+gtLvn0aIyUzCwAAAABJRU5ErkJggg=="]
            }
        ]);

        // Assert the results
        assert_eq!(result.messages, PromptResultEnum::Value(expected_messages));
        assert!(result.remaining_output_tokens > 0);
    }

    #[test]
    fn test_from_llm_messages() {
        let llm_messages = vec![
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("You are a very helpful assistant that's very good at completing a task.".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("The current main task at hand is: `describe this`".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: Some(vec!["iVBORw0KGgoAAAANSUhEUgAAAlgAAAJYCAYAAAC".to_string()]),
            },
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and separated paragraphs.".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
        ];

        let expected_messages = vec![
            OllamaMessage {
                role: "system".to_string(),
                content: "You are a very helpful assistant that's very good at completing a task.".to_string(),
                images: None,
                tool_calls: None,
            },
            OllamaMessage {
                role: "user".to_string(),
                content: "The current main task at hand is: `describe this`".to_string(),
                images: Some(vec!["iVBORw0KGgoAAAANSUhEUgAAAlgAAAJYCAYAAAC".to_string()]),
                tool_calls: None,
            },
            OllamaMessage {
                role: "system".to_string(),
                content: "Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and separated paragraphs.".to_string(),
                images: None,
                tool_calls: None,
            },
        ];

        let result = from_chat_completion_messages(llm_messages).unwrap();
        assert_eq!(result, expected_messages);
    }
}
