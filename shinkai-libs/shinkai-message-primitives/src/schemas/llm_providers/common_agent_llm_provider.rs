use serde::{Deserialize, Serialize};

use crate::schemas::shinkai_name::ShinkaiName;

use super::{agent::Agent, serialized_llm_provider::SerializedLLMProvider};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ProviderOrAgent {
    LLMProvider(SerializedLLMProvider),
    Agent(Agent),
}

impl ProviderOrAgent {
    pub fn get_id(&self) -> &str {
        match self {
            ProviderOrAgent::LLMProvider(provider) => &provider.id,
            ProviderOrAgent::Agent(agent) => &agent.agent_id,
        }
    }

    pub fn get_llm_provider_id(&self) -> &str {
        match self {
            ProviderOrAgent::LLMProvider(provider) => &provider.id,
            ProviderOrAgent::Agent(agent) => &agent.llm_provider_id,
        }
    }

    pub fn get_full_identity_name(&self) -> &ShinkaiName {
        match self {
            ProviderOrAgent::LLMProvider(provider) => &provider.full_identity_name,
            ProviderOrAgent::Agent(agent) => &agent.full_identity_name,
        }
    }
}
