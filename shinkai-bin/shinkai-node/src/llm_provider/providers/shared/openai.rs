use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::prompts::prompts::Prompt;
use crate::managers::model_capabilities_manager::ModelCapabilitiesManager;
use crate::managers::model_capabilities_manager::PromptResult;
use crate::managers::model_capabilities_manager::PromptResultEnum;
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use serde_json::{self};
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;

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
    pub arguments: JsonValue,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "data")]
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

#[derive(Serialize)]
pub struct ApiPayload {
    model: String,
    messages: String,
    temperature: f64,
    max_tokens: usize,
}

pub fn openai_prepare_messages(model: &LLMProviderInterface, prompt: Prompt) -> Result<PromptResult, LLMProviderError> {
    let max_input_tokens = ModelCapabilitiesManager::get_max_input_tokens(model);

    // Generate the messages and filter out images
    let chat_completion_messages = prompt.generate_openai_messages(Some(max_input_tokens))?;
    let filtered_chat_completion_messages: Vec<_> = chat_completion_messages
        .clone()
        .into_iter()
        .filter(|message| message.name.as_deref() != Some("image"))
        .collect();

    // Get a more accurate estimate of the number of used tokens
    let used_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&filtered_chat_completion_messages);
    // Calculate the remaining output tokens available
    let remaining_output_tokens = ModelCapabilitiesManager::get_remaining_output_tokens(model, used_tokens);

    // Separate messages into those with a valid role and those without
    let (messages_with_role, tools): (Vec<_>, Vec<_>) = filtered_chat_completion_messages
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
}
