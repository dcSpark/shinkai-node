use serde_json;
use shinkai_message_primitives::{
    schemas::{llm_providers::serialized_llm_provider::LLMProviderInterface, prompts::Prompt, subprompts::SubPrompt},
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};

use crate::{llm_provider::error::LLMProviderError, managers::model_capabilities_manager::{
    Base64ImageString, ModelCapabilitiesManager, PromptResult, PromptResultEnum,
}};

pub fn llama_prepare_messages(
    _model: &LLMProviderInterface,
    _model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, LLMProviderError> {
    let messages_string = prompt.generate_genericapi_messages(Some(total_tokens), &ModelCapabilitiesManager::num_tokens_from_llama3)?;

    let used_tokens = ModelCapabilitiesManager::count_tokens_from_message_llama3(&messages_string);

    Ok(PromptResult {
        messages: PromptResultEnum::Text(messages_string.clone()),
        functions: None,
        remaining_tokens: total_tokens - used_tokens,
    })
}