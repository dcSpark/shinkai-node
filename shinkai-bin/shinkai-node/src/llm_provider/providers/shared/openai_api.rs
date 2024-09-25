use crate::llm_provider::error::LLMProviderError;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use base64::decode;
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::{self};
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCallResponse {
    pub response: String,
    pub function_call: FunctionCall,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    FunctionCall(FunctionCallResponse),
    Text(String),
    ImageUrl { url: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIApiMessage {
    pub role: String,
    pub content: Option<MessageContent>,
    pub function_call: Option<FunctionCall>,
}

impl Serialize for OpenAIApiMessage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_struct("OpenAIApiMessage", 3)?;
        map.serialize_field("role", &self.role)?;
        if let Some(content) = &self.content {
            map.serialize_field("content", content)?;
        }
        if let Some(function_call) = &self.function_call {
            map.serialize_field("function_call", function_call)?;
        }
        map.end()
    }
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}

fn get_image_type(base64_str: &str) -> Option<&'static str> {
    let decoded = decode(base64_str).ok()?;
    if decoded.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpeg")
    } else if decoded.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', b'\x1A', b'\n']) {
        Some("png")
    } else if decoded.starts_with(&[b'G', b'I', b'F', b'8']) {
        Some("gif")
    } else {
        None
    }
}

pub fn openai_prepare_messages(model: &LLMProviderInterface, prompt: Prompt) -> Result<PromptResult, LLMProviderError> {
    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    // Generate the messages and filter out images
    let chat_completion_messages = prompt.generate_openai_messages(
        Some(max_input_tokens),
        Some("tool".to_string()),
        &ModelCapabilitiesManager::num_tokens_from_llama3,
    )?;

    // Get a more accurate estimate of the number of used tokens
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&chat_completion_messages);
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
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .map(|mut message| {
                let images = message.get("images").cloned();
                let text = message.get("content").cloned();

                if let Some(serde_json::Value::Array(images_array)) = images {
                    let mut content = vec![];
                    if let Some(text) = text {
                        content.push(serde_json::json!({"type": "text", "text": text}));
                    }
                    for image in images_array {
                        if let serde_json::Value::String(image_str) = image {
                            if let Some(image_type) = get_image_type(&image_str) {
                                content.push(serde_json::json!({
                                    "type": "image_url",
                                    "image_url": {"url": format!("data:image/{};base64,{}", image_type, image_str)}
                                }));
                            }
                        }
                    }
                    message["content"] = serde_json::json!(content);
                    message.as_object_mut().unwrap().remove("images");
                }
                message
            })
            .collect(),
        _ => vec![],
    };

    // Flatten the tools array to extract functions directly
    let tools_vec = match tools_json {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .flat_map(|tool| {
                if let serde_json::Value::Object(mut map) = tool {
                    // TODO: functions is deprecated in favor of tools. Update it
                    map.remove("functions")
                        .and_then(|functions| {
                            if let serde_json::Value::Array(funcs) = functions {
                                Some(funcs)
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
    use serde_json::json;

    #[test]
    fn test_openai_api_message_with_function_call() {
        let json_str = r#"
        {
            "role": "assistant",
            "content": null,
            "function_call": {
                "name": "concat_strings",
                "arguments": {
                    "first_string": "hola",
                    "second_string": " chao"
                }
            }
        }
        "#;

        // Deserialize the JSON string to OpenAIApiMessage
        let message: OpenAIApiMessage = serde_json::from_str(json_str).expect("Failed to deserialize");

        // Check the deserialized values
        assert_eq!(message.role, "assistant");
        assert!(message.content.is_none());
        assert!(message.function_call.is_some());

        if let Some(function_call) = message.function_call.clone() {
            assert_eq!(function_call.name, "concat_strings");
            assert_eq!(
                function_call.arguments,
                json!({"first_string": "hola", "second_string": " chao"})
            );
        }

        // Serialize the OpenAIApiMessage back to JSON
        let serialized_json = serde_json::to_string(&message).expect("Failed to serialize");

        // Deserialize again to check round-trip consistency
        let deserialized_message: OpenAIApiMessage =
            serde_json::from_str(&serialized_json).expect("Failed to deserialize");

        // Check the deserialized values again
        assert_eq!(deserialized_message.role, "assistant");
        assert!(deserialized_message.content.is_none());
        assert!(deserialized_message.function_call.is_some());

        if let Some(function_call) = deserialized_message.function_call {
            assert_eq!(function_call.name, "concat_strings");
            assert_eq!(
                function_call.arguments,
                json!({"first_string": "hola", "second_string": " chao"})
            );
        }
    }

    #[test]
    fn test_openai_response_after_tool_usage_parsing() {
        let response_text = r#"
        {
            "id": "chatcmpl-9cQYyc4ENYwJ5ChU4WHtRv7uPRHbN",
            "object": "chat.completion",
            "created": 1718945600,
            "model": "gpt-4-1106-preview",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "The concatenated result of \"hola\" and \"chao\" is \"hola chao\"."
                    },
                    "logprobs": null,
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 156,
                "completion_tokens": 21,
                "total_tokens": 177
            },
            "system_fingerprint": null
        }
        "#;

        // Deserialize the JSON string to OpenAIResponse
        let response: OpenAIResponse = serde_json::from_str(response_text).expect("Failed to deserialize");

        // Check the deserialized values
        assert_eq!(response.id, "chatcmpl-9cQYyc4ENYwJ5ChU4WHtRv7uPRHbN");
        assert_eq!(response.object, "chat.completion");
        assert_eq!(response.created, 1718945600);
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.usage.prompt_tokens, 156);
        assert_eq!(response.usage.completion_tokens, 21);
        assert_eq!(response.usage.total_tokens, 177);

        let choice = &response.choices[0];
        assert_eq!(choice.index, 0);
        assert_eq!(choice.message.role, "assistant");
        if let Some(MessageContent::Text(content)) = &choice.message.content {
            assert_eq!(
                content,
                "The concatenated result of \"hola\" and \"chao\" is \"hola chao\"."
            );
        } else {
            panic!("Expected text content");
        }
    }
}
