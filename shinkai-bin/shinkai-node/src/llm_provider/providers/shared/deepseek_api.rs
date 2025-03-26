// This file is intentionally minimal since DeepSeek API is compatible with OpenAI API
// We reuse the OpenAI API implementation for message preparation and response handling

use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::providers::shared::openai_api;
use crate::managers::model_capabilities_manager::{PromptResult, PromptResultEnum};
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;
use uuid::Uuid;

// Re-export OpenAI API types for DeepSeek
pub use openai_api::{
    MessageContent, OpenAIResponse,
};

/// Prepare messages for DeepSeek API using the OpenAI format
/// DeepSeek API is compatible with OpenAI API, so we can reuse the OpenAI message preparation
pub fn deepseek_prepare_messages(
    model: &LLMProviderInterface,
    prompt: Prompt,
) -> Result<PromptResult, LLMProviderError> {
    let result = openai_api::openai_prepare_messages(model, prompt)?;
    let tools_json = result.functions.unwrap_or_else(Vec::new);
    let messages_json = result.messages.clone();

    // Adapt messages to DeepSeek format
    let messages_json = match messages_json {
        PromptResultEnum::Value(messages) => {
            let messages_array = messages.as_array().unwrap();
            let transformed_messages: Vec<serde_json::Value> = messages_array.iter().map(|message| {
                let mut new_message = message.clone();
                if message["role"] == "assistant" {
                    if let Some(content) = message.get("content") {
                        if let Some(content_array) = content.as_array() {
                            if let Some(first_content) = content_array.first() {
                                if let Some(text) = first_content.get("text") {
                                    new_message["content"] = text.clone();
                                }
                            }
                        }
                    }
                    if let Some(function_call) = message.get("function_call") {
                        new_message["tool_calls"] = serde_json::json!([{
                            "function": function_call,
                            "id": function_call["name"].clone(),
                            "type": "function"
                        }]);
                        if let Some(obj) = new_message.as_object_mut() {
                            obj.remove("function_call");
                        }
                    }
                }
                if message["role"] == "function" {
                    new_message["role"] = serde_json::Value::String("tool".to_string());
                    new_message["tool_call_id"] = new_message["name"].clone();
                }
                new_message
            }).collect();
            PromptResultEnum::Value(serde_json::Value::Array(transformed_messages))
        }
        _ => messages_json,
    };

    Ok(PromptResult {
        messages: messages_json,
        functions: Some(tools_json),
        remaining_output_tokens: result.remaining_output_tokens,
        tokens_used: result.tokens_used,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{DeepSeek, SerializedLLMProvider};
    use shinkai_message_primitives::schemas::subprompts::{SubPrompt, SubPromptType};

    #[test]
    fn test_deepseek_prepare_messages() {
        let mut prompt = Prompt::new();
        prompt.add_sub_prompts(vec![
            SubPrompt::Omni(
                SubPromptType::System,
                "You are a helpful assistant.".to_string(),
                vec![],
                98,
            ),
            SubPrompt::Omni(SubPromptType::User, "Hello!".to_string(), vec![], 97),
        ]);

        let model = LLMProviderInterface::DeepSeek(DeepSeek {
            model_type: "deepseek-chat".to_string(),
        });

        let result = deepseek_prepare_messages(&model, prompt).expect("Failed to prepare messages");

        // Verify that the messages are prepared correctly
        if let crate::managers::model_capabilities_manager::PromptResultEnum::Value(messages) = &result.messages {
            let messages_array = messages.as_array().unwrap();
            assert_eq!(messages_array.len(), 2);

            let system_message = &messages_array[0];
            let user_message = &messages_array[1];

            assert_eq!(system_message["role"], "system");
            assert_eq!(user_message["role"], "user");
        } else {
            panic!("Expected Value variant in PromptResultEnum");
        }
    }
}
