use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_message_primitives::{schemas::{llm_providers::serialized_llm_provider::{LLMProviderInterface, SerializedLLMProvider}, shinkai_name::ShinkaiName}, shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::V2ChatMessage}};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LLMProviderSubset {
    pub id: String,
    pub full_identity_name: ShinkaiName,
    pub model: LLMProviderInterface,
}

impl LLMProviderSubset {
    pub fn from_serialized_llm_provider(serialized_llm_provider: SerializedLLMProvider) -> Self {
        Self {
            id: serialized_llm_provider.id,
            full_identity_name: serialized_llm_provider.full_identity_name,
            model: serialized_llm_provider.model,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SmartInbox {
    pub inbox_id: String,
    pub custom_name: String,
    pub datetime_created: String,
    pub last_message: Option<ShinkaiMessage>,
    pub is_finished: bool,
    pub job_scope: Option<Value>,
    pub agent: Option<LLMProviderSubset>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct V2SmartInbox {
    pub inbox_id: String,
    pub custom_name: String,
    pub datetime_created: String,
    pub last_message: Option<V2ChatMessage>,
    pub is_finished: bool,
    pub job_scope: Option<Value>,
    pub agent: Option<LLMProviderSubset>,
}
