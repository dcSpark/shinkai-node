use crate::{agent::{execution::job_prompts::Prompt, providers::shared::{openai::openai_prepare_messages, togetherai::llama_prepare_messages}, error::AgentError}, db::ShinkaiDB};
use shinkai_message_primitives::schemas::{
    agents::serialized_agent::{AgentLLMInterface, SerializedAgent},
    shinkai_name::ShinkaiName,
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum AgentsCapabilitiesManagerError {
    GeneralError(String),
    NotImplemented(String),
}

impl std::fmt::Display for AgentsCapabilitiesManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AgentsCapabilitiesManagerError::GeneralError(err) => write!(f, "General error: {}", err),
            AgentsCapabilitiesManagerError::NotImplemented(model) => write!(f, "Model not implemented: {}", model),
        }
    }
}

impl From<AgentError> for AgentsCapabilitiesManagerError {
    fn from(error: AgentError) -> Self {
        AgentsCapabilitiesManagerError::GeneralError(error.to_string())
    }
}

impl std::error::Error for AgentsCapabilitiesManagerError {}

#[derive(Clone, Debug, PartialEq)]
pub struct PromptResult {
    pub value: PromptResultEnum,
    pub remaining_tokens: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PromptResultEnum {
    Text(String),
    Value(serde_json::Value),
}

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
                model_type if model_type.starts_with("togethercomputer/llama-2") => {
                    vec![AgentCapability::TextInference]
                }
                _ => vec![],
            },
            AgentLLMInterface::LocalLLM(_) => vec![],
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type.as_str() {
                "gpt" | "gpt4" | "gpt-4-1106-preview" => vec![AgentCapability::TextInference],
                "gpt-vision" | "gpt-4-vision-preview" => vec![AgentCapability::ImageAnalysis],
                "dall-e" => vec![AgentCapability::ImageGeneration],
                _ => vec![],
            },
            AgentLLMInterface::Ollama(ollama) => {
                if ollama.model_type.starts_with("llama-2") {
                    vec![AgentCapability::TextInference]
                } else if ollama.model_type.starts_with("mistral") {
                    vec![AgentCapability::TextInference]
                } else if ollama.model_type.starts_with("deepseek") {
                    vec![AgentCapability::TextInference]
                } else if ollama.model_type.starts_with("meditron") {
                    vec![AgentCapability::TextInference]
                } else if ollama.model_type.starts_with("starling-lm") {
                    vec![AgentCapability::TextInference]
                } else if ollama.model_type.starts_with("orca2") {
                    vec![AgentCapability::TextInference]
                } else if ollama.model_type.starts_with("yi") {
                    vec![AgentCapability::TextInference]
                } else if ollama.model_type.starts_with("yarn-mistral") {
                    vec![AgentCapability::TextInference]
                } else if ollama.model_type.starts_with("yarn-llama2") {
                    vec![AgentCapability::TextInference]
                } else {
                    vec![]
                }
            }
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
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type.as_str() {
                "gpt" | "gpt4" | "gpt-4-1106-preview" => AgentCost::Expensive,
                "gpt-vision" | "gpt-4-vision-preview" => AgentCost::GoodValue,
                "dall-e" => AgentCost::GoodValue,
                _ => AgentCost::Unknown,
            },
            AgentLLMInterface::Ollama(_) => AgentCost::Cheap,
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
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type.as_str() {
                "gpt" | "gpt4" | "gpt-4-1106-preview" => AgentPrivacy::RemoteGreedy,
                "gpt-vision" | "gpt-4-vision-preview" => AgentPrivacy::RemoteGreedy,
                "dall-e" => AgentPrivacy::RemoteGreedy,
                _ => AgentPrivacy::Unknown,
            },
            AgentLLMInterface::Ollama(_) => AgentPrivacy::Local,
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

    pub async fn route_prompt_with_model(
        prompt: Prompt,
        model: &AgentLLMInterface,
    ) -> Result<PromptResult, AgentsCapabilitiesManagerError> {
        match model {
            AgentLLMInterface::OpenAI(openai) => {
                if openai.model_type.starts_with("gpt-") {
                    let total_tokens = Self::get_max_tokens(model);
                    let tiktoken_messages = openai_prepare_messages(model, openai.clone().model_type, prompt, total_tokens)?;
                    Ok(tiktoken_messages)
                } else {
                    Err(AgentsCapabilitiesManagerError::NotImplemented(
                        openai.model_type.clone(),
                    ))
                }
            },
            AgentLLMInterface::GenericAPI(genericapi) => {
                if genericapi.model_type.starts_with("togethercomputer/llama-2") {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string = llama_prepare_messages(model, genericapi.clone().model_type, prompt, total_tokens)?;
                    Ok(messages_string)
                } else {
                    Err(AgentsCapabilitiesManagerError::NotImplemented(
                        genericapi.model_type.clone(),
                    ))
                }
            },
            AgentLLMInterface::LocalLLM(_) => {
                Err(AgentsCapabilitiesManagerError::NotImplemented("LocalLLM".to_string()))
            }
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => Err(AgentsCapabilitiesManagerError::NotImplemented(
                shinkai_backend.model_type.clone(),
            )),
            AgentLLMInterface::Ollama(ollama) => {
                if ollama.model_type.starts_with("mistral") {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string = llama_prepare_messages(model, ollama.clone().model_type, prompt, total_tokens)?;
                    Ok(messages_string)
                } else {
                    Err(AgentsCapabilitiesManagerError::NotImplemented(
                        ollama.model_type.clone(),
                    ))
                }
            },
        }
    }

    pub fn get_max_tokens(model: &AgentLLMInterface) -> usize {
        match model {
            AgentLLMInterface::OpenAI(openai) => {
                if openai.model_type == "gpt-4-1106-preview" || openai.model_type == "gpt-4-vision-preview" {
                    128_000
                } else {
                    let normalized_model = Self::normalize_model(&model.clone());
                    tiktoken_rs::model::get_context_size(normalized_model.as_str())
                }
            }
            AgentLLMInterface::GenericAPI(genericapi) => {
                // Fill in the appropriate logic for GenericAPI
                4096
            }
            AgentLLMInterface::LocalLLM(_) => {
                // Fill in the appropriate logic for LocalLLM
                0
            }
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => {
                if shinkai_backend.model_type == "gpt" {
                    128_000
                } else {
                    let normalized_model = Self::normalize_model(&model.clone());
                    tiktoken_rs::model::get_context_size(normalized_model.as_str())
                }
            }
            AgentLLMInterface::Ollama(_) => {
                // Fill in the appropriate logic for Ollama
                4096
            }
        }
    }

    pub fn get_max_output_tokens(model: &AgentLLMInterface) -> usize {
        match model {
            AgentLLMInterface::OpenAI(_) => {
                // Fill in the appropriate logic for OpenAI
                4096
            }
            AgentLLMInterface::GenericAPI(_) => {
                // Fill in the appropriate logic for GenericAPI
                4096
            }
            AgentLLMInterface::LocalLLM(_) => {
                // Fill in the appropriate logic for LocalLLM
                4096
            }
            AgentLLMInterface::ShinkaiBackend(_) => {
                // Fill in the appropriate logic for ShinkaiBackend
                4096
            }
            AgentLLMInterface::Ollama(_) => {
                // Fill in the appropriate logic for Ollama
                4096
            }
        }
    }

    // Note(Nico): this may be necessary bc some libraries are not caught up with the latest models e.g. tiktoken-rs
    pub fn normalize_model(model: &AgentLLMInterface) -> String {
        match model {
            AgentLLMInterface::OpenAI(openai) => {
                if openai.model_type.starts_with("gpt-4") {
                    "gpt-4-32k".to_string()
                } else if openai.model_type.starts_with("gpt-3.5") {
                    "gpt-3.5-turbo-16k".to_string()
                } else {
                    "gpt-4".to_string()
                }
            }
            AgentLLMInterface::GenericAPI(genericapi) => {
                // Fill in the appropriate logic for GenericAPI
                "".to_string()
            }
            AgentLLMInterface::LocalLLM(_) => {
                // Fill in the appropriate logic for LocalLLM
                "".to_string()
            }
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => {
                if shinkai_backend.model_type.starts_with("gpt") {
                    "gpt-4-32k".to_string()
                } else {
                    "gpt-4".to_string()
                }
            }
            AgentLLMInterface::Ollama(_) => {
                // Fill in the appropriate logic for Ollama
                "".to_string()
            }
        }
    }
}
