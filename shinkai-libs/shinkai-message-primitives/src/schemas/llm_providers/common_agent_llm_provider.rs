use super::{agent::Agent, serialized_llm_provider::SerializedLLMProvider};

#[derive(Debug, Clone, PartialEq)]
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
}
