use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;

use super::llm_provider::LLMProvider;

impl From<LLMProvider> for SerializedLLMProvider {
    fn from(agent: LLMProvider) -> Self {
        SerializedLLMProvider {
            id: agent.id,
            full_identity_name: agent.full_identity_name,
            perform_locally: agent.perform_locally,
            external_url: agent.external_url,
            api_key: agent.api_key,
            model: agent.model,
            toolkit_permissions: agent.toolkit_permissions,
            storage_bucket_permissions: agent.storage_bucket_permissions,
            allowed_message_senders: agent.allowed_message_senders,
        }
    }
}
