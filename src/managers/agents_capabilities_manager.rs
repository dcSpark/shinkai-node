use crate::db::ShinkaiDB;
use shinkai_message_primitives::schemas::{
    agents::serialized_agent::{AgentLLMInterface, SerializedAgent},
    shinkai_name::ShinkaiName,
};
use std::sync::Arc;
use tokio::sync::Mutex;

// Enum for capabilities
#[derive(Clone, Debug, PartialEq)]
pub enum Capability {
    TextInference,
    ImageGeneration,
    ImageAnalysis,
}

// Struct for AgentsCapabilitiesManager
pub struct AgentsCapabilitiesManager {
    db: Arc<Mutex<ShinkaiDB>>,
    profile: ShinkaiName,
    agents: Vec<SerializedAgent>,
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

    // Function to check if a specific capability is available
    pub async fn is_capability_available(&self, capability: Capability) -> bool {
        let capabilities = self.check_capabilities().await;
        capabilities.contains(&capability)
    }

    // Static method to get capability of an agent
    pub fn get_capability(agent: &SerializedAgent) -> Vec<Capability> {
        match &agent.model {
            AgentLLMInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-3.5-turbo-1106" => vec![Capability::TextInference, Capability::ImageGeneration],
                "model2" => vec![Capability::ImageGeneration],
                _ => vec![Capability::ImageAnalysis],
            },
            AgentLLMInterface::GenericAPI(genericapi) => match genericapi.model_type.as_str() {
                "gpt-3.5-turbo-1106" => vec![Capability::TextInference, Capability::ImageGeneration],
                "model2" => vec![Capability::ImageGeneration],
                _ => vec![Capability::ImageAnalysis],
            },
            AgentLLMInterface::LocalLLM(_) => vec![Capability::ImageAnalysis],
        }
    }

    // Function to check capabilities
    pub async fn check_capabilities(&self) -> Vec<Capability> {
        let agents = self.agents.clone();
        agents
            .into_iter()
            .flat_map(|agent| Self::get_capability(&agent))
            .collect()
    }

    // Function to check if a specific capability is available
    pub async fn has_capability(&self, capability: Capability) -> bool {
        let capabilities = self.check_capabilities().await;
        capabilities.contains(&capability)
    }
}
