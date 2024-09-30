use base64::decode;
use shinkai_message_primitives::schemas::{
    llm_providers::serialized_llm_provider::LLMProviderInterface, prompts::Prompt,
};

use crate::{
    llm_provider::error::LLMProviderError,
    managers::model_capabilities_manager::{ModelCapabilitiesManager, PromptResult, PromptResultEnum},
};

pub fn llama_prepare_messages(
    _model: &LLMProviderInterface,
    _model_type: String,
    prompt: Prompt,
    total_tokens: usize,
) -> Result<PromptResult, LLMProviderError> {
    let messages_string =
        prompt.generate_genericapi_messages(Some(total_tokens), &ModelCapabilitiesManager::num_tokens_from_llama3)?;

    let used_tokens = ModelCapabilitiesManager::count_tokens_from_message_llama3(&messages_string);

    Ok(PromptResult {
        messages: PromptResultEnum::Text(messages_string.clone()),
        functions: None,
        remaining_tokens: total_tokens - used_tokens,
    })
}

pub fn get_image_type(base64_str: &str) -> Option<&'static str> {
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
