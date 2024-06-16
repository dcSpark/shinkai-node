use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;

use super::llm_provider::LLMProvider;

impl From<LLMProvider> for SerializedAgent {
    fn from(agent: LLMProvider) -> Self {
        SerializedAgent {
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
