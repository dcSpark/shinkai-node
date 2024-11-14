use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::V2ChatMessage};

use super::{
    job_config::JobConfig,
    llm_providers::{
        agent::Agent,
        serialized_llm_provider::{LLMProviderInterface, SerializedLLMProvider},
    },
    shinkai_name::ShinkaiName,
};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
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

    pub fn from_agent(agent: Agent, serialized_llm_provider: SerializedLLMProvider) -> Self {
        Self {
            id: agent.agent_id,
            full_identity_name: agent.full_identity_name,
            model: serialized_llm_provider.model,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SmartInbox {
    pub inbox_id: String,
    pub custom_name: String,
    pub datetime_created: String,
    pub last_message: Option<ShinkaiMessage>,
    pub is_finished: bool,
    pub job_scope: Option<Value>,
    pub agent: Option<LLMProviderSubset>,
    pub job_config: Option<JobConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct V2SmartInbox {
    pub inbox_id: String,
    pub custom_name: String,
    pub datetime_created: String,
    pub last_message: Option<V2ChatMessage>,
    pub is_finished: bool,
    pub agent: Option<LLMProviderSubset>,
    pub job_scope: Option<Value>,
    pub job_config: Option<JobConfig>,
}
