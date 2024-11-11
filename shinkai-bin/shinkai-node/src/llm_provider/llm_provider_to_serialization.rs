use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;

use super::llm_provider::LLMProvider;

impl From<LLMProvider> for SerializedLLMProvider {
    fn from(agent: LLMProvider) -> Self {
        SerializedLLMProvider {
            id: agent.id,
            full_identity_name: agent.full_identity_name,
            external_url: agent.external_url,
            api_key: agent.api_key,
            model: agent.model,
        }
    }
}
