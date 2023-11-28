use crate::db::ShinkaiDB;
use shinkai_message_primitives::schemas::{
    agents::serialized_agent::{AgentLLMInterface, SerializedAgent},
    shinkai_name::ShinkaiName,
};
use std::sync::Arc;
use tokio::sync::Mutex;

// Enum for capabilities
#[derive(Clone, Debug, PartialEq)]
pub enum AgentCapability {
    TextInference,
    ImageGeneration,
    ImageAnalysis,
}

// Enum for cost
#[derive(Clone, Debug, PartialEq)]
pub enum AgentCost {
    Unknown,
    Cheap,
    GoodValue,
    Expensive,
}

// Enum for privacy
#[derive(Clone, Debug, PartialEq)]
pub enum AgentPrivacy {
    Unknown,
    Local,
    RemotePrivate,
    RemoteGreedy,
}

// Struct for AgentsCapabilitiesManager
pub struct AgentsCapabilitiesManager {
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub profile: ShinkaiName,
    pub agents: Vec<SerializedAgent>,
}

impl AgentsCapabilitiesManager {
    // Constructor
    pub async fn new(db: Arc<Mutex<ShinkaiDB>>, profile: ShinkaiName) -> Self {
        let agents = Self::get_agents(&db, profile.clone()).await;
        Self { db, profile, agents }
    }

    // Function to get all agents from the database for a profile
    async fn get_agents(db: &Arc<Mutex<ShinkaiDB>>, profile: ShinkaiName) -> Vec<SerializedAgent> {
        let db = db.lock().await;
        db.get_agents_for_profile(profile).unwrap()
    }

    // Static method to get capability of an agent
    pub fn get_capability(agent: &SerializedAgent) -> (Vec<AgentCapability>, AgentCost, AgentPrivacy) {
        let capabilities = Self::get_agent_capabilities(&agent.model);
        let cost = Self::get_agent_cost(&agent.model);
        let privacy = Self::get_agent_privacy(&agent.model);

        (capabilities, cost, privacy)
    }

    // Static method to get capabilities of an agent model
    pub fn get_agent_capabilities(model: &AgentLLMInterface) -> Vec<AgentCapability> {
        match model {
            AgentLLMInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-3.5-turbo-1106" => vec![AgentCapability::TextInference],
                "gpt-4-1106-preview" => vec![AgentCapability::TextInference],
                "gpt-4-vision-preview" => vec![AgentCapability::ImageAnalysis, AgentCapability::TextInference],
                "dall-e-3" => vec![AgentCapability::ImageGeneration],
                model_type if model_type.starts_with("gpt-") => vec![AgentCapability::TextInference],
                _ => vec![],
            },
            AgentLLMInterface::GenericAPI(genericapi) => match genericapi.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => vec![AgentCapability::TextInference],
                "yorickvp/llava-13b" => vec![AgentCapability::ImageAnalysis],
                model_type if model_type.starts_with("togethercomputer/llama-2") => vec![AgentCapability::TextInference],
                _ => vec![],
            },
            AgentLLMInterface::LocalLLM(_) => vec![],
        }
    }

    // Static method to get cost of an agent model
    pub fn get_agent_cost(model: &AgentLLMInterface) -> AgentCost {
        match model {
            AgentLLMInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-3.5-turbo-1106" => AgentCost::Cheap,
                "gpt-4-1106-preview" => AgentCost::GoodValue,
                "gpt-4-vision-preview" => AgentCost::GoodValue,
                "dall-e-3" => AgentCost::GoodValue,
                _ => AgentCost::Unknown,
            },
            AgentLLMInterface::GenericAPI(genericapi) => match genericapi.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => AgentCost::Cheap,
                "yorickvp/llava-13b" => AgentCost::Expensive,
                _ => AgentCost::Unknown,
            },
            AgentLLMInterface::LocalLLM(_) => AgentCost::Cheap,
        }
    }

    // Static method to get privacy of an agent model
    pub fn get_agent_privacy(model: &AgentLLMInterface) -> AgentPrivacy {
        match model {
            AgentLLMInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-3.5-turbo-1106" => AgentPrivacy::RemoteGreedy,
                "gpt-4-1106-preview" => AgentPrivacy::RemoteGreedy,
                "gpt-4-vision-preview" => AgentPrivacy::RemoteGreedy,
                "dall-e-3" => AgentPrivacy::RemoteGreedy,
                _ => AgentPrivacy::Unknown,
            },
            AgentLLMInterface::GenericAPI(genericapi) => match genericapi.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => AgentPrivacy::RemoteGreedy,
                "yorickvp/llava-13b" => AgentPrivacy::RemoteGreedy,
                _ => AgentPrivacy::Unknown,
            },
            AgentLLMInterface::LocalLLM(_) => AgentPrivacy::Local,
        }
    }

    // Function to check capabilities
    pub async fn check_capabilities(&self) -> Vec<(Vec<AgentCapability>, AgentCost, AgentPrivacy)> {
        let agents = self.agents.clone();
        agents.into_iter().map(|agent| Self::get_capability(&agent)).collect()
    }

    // Function to check if a specific capability is available
    pub async fn has_capability(&self, capability: AgentCapability) -> bool {
        let capabilities = self.check_capabilities().await;
        capabilities.iter().any(|(caps, _, _)| caps.contains(&capability))
    }

    // Function to check if a specific cost is available
    pub async fn has_cost(&self, cost: AgentCost) -> bool {
        let capabilities = self.check_capabilities().await;
        capabilities.iter().any(|(_, c, _)| c == &cost)
    }

    // Function to check if a specific privacy is available
    pub async fn has_privacy(&self, privacy: AgentPrivacy) -> bool {
        let capabilities = self.check_capabilities().await;
        capabilities.iter().any(|(_, _, p)| p == &privacy)
    }
}
