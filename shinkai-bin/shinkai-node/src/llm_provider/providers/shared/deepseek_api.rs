// This file is intentionally minimal since DeepSeek API is compatible with OpenAI API
// We reuse the OpenAI API implementation for message preparation and response handling

use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::providers::shared::openai_api;
use crate::managers::model_capabilities_manager::PromptResult;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::LLMProviderInterface;
use shinkai_message_primitives::schemas::prompts::Prompt;

// Re-export OpenAI API types for DeepSeek
pub use openai_api::{
    Choice, FunctionCall, FunctionCallResponse, MessageContent, OpenAIApiMessage, OpenAIResponse, ToolCall, Usage,
};

/// Prepare messages for DeepSeek API using the OpenAI format
/// DeepSeek API is compatible with OpenAI API, so we can reuse the OpenAI message preparation
pub fn deepseek_prepare_messages(model: &LLMProviderInterface, prompt: Prompt) -> Result<PromptResult, LLMProviderError> {
    openai_api::openai_prepare_messages(model, prompt)
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
