use crate::{
    agent::{
        error::AgentError,
        execution::job_prompts::Prompt,
        providers::shared::{openai::openai_prepare_messages, togetherai::{llama_prepare_messages, llava_prepare_messages}},
    },
    db::ShinkaiDB,
};
use regex::Regex;
use shinkai_message_primitives::schemas::{
    agents::serialized_agent::{AgentLLMInterface, SerializedAgent},
    shinkai_name::ShinkaiName,
};
use std::{sync::Arc, fmt};
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum ModelCapabilitiesManagerError {
    GeneralError(String),
    NotImplemented(String),
}

impl std::fmt::Display for ModelCapabilitiesManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ModelCapabilitiesManagerError::GeneralError(err) => write!(f, "General error: {}", err),
            ModelCapabilitiesManagerError::NotImplemented(model) => write!(f, "Model not implemented: {}", model),
        }
    }
}

impl From<AgentError> for ModelCapabilitiesManagerError {
    fn from(error: AgentError) -> Self {
        ModelCapabilitiesManagerError::GeneralError(error.to_string())
    }
}

impl std::error::Error for ModelCapabilitiesManagerError {}

#[derive(Clone, Debug, PartialEq)]
pub struct PromptResult {
    pub value: PromptResultEnum,
    pub remaining_tokens: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Base64ImageString(pub String);

impl fmt::Display for Base64ImageString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PromptResultEnum {
    Text(String),
    ImageAnalysis(String, Base64ImageString),
    Value(serde_json::Value),
}

// Enum for capabilities
#[derive(Clone, Debug, PartialEq)]
pub enum ModelCapability {
    TextInference,
    ImageGeneration,
    ImageAnalysis,
}

// Enum for cost
#[derive(Clone, Debug, PartialEq)]
pub enum ModelCost {
    Unknown,
    Cheap,
    GoodValue,
    Expensive,
}

// Enum for privacy
#[derive(Clone, Debug, PartialEq)]
pub enum ModelPrivacy {
    Unknown,
    Local,
    RemotePrivate,
    RemoteGreedy,
}

// Struct for AgentsCapabilitiesManager
pub struct ModelCapabilitiesManager {
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub profile: ShinkaiName,
    pub agents: Vec<SerializedAgent>,
}

impl ModelCapabilitiesManager {
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
    pub fn get_capability(agent: &SerializedAgent) -> (Vec<ModelCapability>, ModelCost, ModelPrivacy) {
        let capabilities = Self::get_agent_capabilities(&agent.model);
        let cost = Self::get_agent_cost(&agent.model);
        let privacy = Self::get_agent_privacy(&agent.model);

        (capabilities, cost, privacy)
    }

    // Static method to get capabilities of an agent model
    pub fn get_agent_capabilities(model: &AgentLLMInterface) -> Vec<ModelCapability> {
        match model {
            AgentLLMInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-3.5-turbo-1106" => vec![ModelCapability::TextInference],
                "gpt-4-1106-preview" => vec![ModelCapability::TextInference],
                "gpt-4-vision-preview" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "dall-e-3" => vec![ModelCapability::ImageGeneration],
                model_type if model_type.starts_with("gpt-") => vec![ModelCapability::TextInference],
                _ => vec![],
            },
            AgentLLMInterface::GenericAPI(genericapi) => match genericapi.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => vec![ModelCapability::TextInference],
                "yorickvp/llava-13b" => vec![ModelCapability::ImageAnalysis],
                model_type if model_type.starts_with("togethercomputer/llama-2") => {
                    vec![ModelCapability::TextInference]
                }
                _ => vec![],
            },
            AgentLLMInterface::LocalLLM(_) => vec![],
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type.as_str() {
                "gpt" | "gpt4" | "gpt-4-1106-preview" => vec![ModelCapability::TextInference],
                "gpt-vision" | "gpt-4-vision-preview" => vec![ModelCapability::ImageAnalysis],
                "dall-e" => vec![ModelCapability::ImageGeneration],
                _ => vec![],
            },
            AgentLLMInterface::Ollama(ollama) => {
                if ollama.model_type.starts_with("llama-2") {
                    vec![ModelCapability::TextInference]
                } else if ollama.model_type.starts_with("mistral") {
                    vec![ModelCapability::TextInference]
                } else if ollama.model_type.starts_with("mixtral") {
                    vec![ModelCapability::TextInference]
                } else if ollama.model_type.starts_with("deepseek") {
                    vec![ModelCapability::TextInference]
                } else if ollama.model_type.starts_with("meditron") {
                    vec![ModelCapability::TextInference]
                } else if ollama.model_type.starts_with("starling-lm") {
                    vec![ModelCapability::TextInference]
                } else if ollama.model_type.starts_with("orca2") {
                    vec![ModelCapability::TextInference]
                } else if ollama.model_type.starts_with("yi") {
                    vec![ModelCapability::TextInference]
                } else if ollama.model_type.starts_with("yarn-mistral") {
                    vec![ModelCapability::TextInference]
                } else if ollama.model_type.starts_with("llava") {
                    vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
                } else if ollama.model_type.starts_with("bakllava") {
                    vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
                } else if ollama.model_type.starts_with("yarn-llama2") {
                    vec![ModelCapability::TextInference]
                } else {
                    vec![]
                }
            }
        }
    }

    // Static method to get cost of an agent model
    pub fn get_agent_cost(model: &AgentLLMInterface) -> ModelCost {
        match model {
            AgentLLMInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-3.5-turbo-1106" => ModelCost::Cheap,
                "gpt-4-1106-preview" => ModelCost::GoodValue,
                "gpt-4-vision-preview" => ModelCost::GoodValue,
                "dall-e-3" => ModelCost::GoodValue,
                _ => ModelCost::Unknown,
            },
            AgentLLMInterface::GenericAPI(genericapi) => match genericapi.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => ModelCost::Cheap,
                "yorickvp/llava-13b" => ModelCost::Expensive,
                _ => ModelCost::Unknown,
            },
            AgentLLMInterface::LocalLLM(_) => ModelCost::Cheap,
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type.as_str() {
                "gpt" | "gpt4" | "gpt-4-1106-preview" => ModelCost::Expensive,
                "gpt-vision" | "gpt-4-vision-preview" => ModelCost::GoodValue,
                "dall-e" => ModelCost::GoodValue,
                _ => ModelCost::Unknown,
            },
            AgentLLMInterface::Ollama(_) => ModelCost::Cheap,
        }
    }

    // Static method to get privacy of an agent model
    pub fn get_agent_privacy(model: &AgentLLMInterface) -> ModelPrivacy {
        match model {
            AgentLLMInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-3.5-turbo-1106" => ModelPrivacy::RemoteGreedy,
                "gpt-4-1106-preview" => ModelPrivacy::RemoteGreedy,
                "gpt-4-vision-preview" => ModelPrivacy::RemoteGreedy,
                "dall-e-3" => ModelPrivacy::RemoteGreedy,
                _ => ModelPrivacy::Unknown,
            },
            AgentLLMInterface::GenericAPI(genericapi) => match genericapi.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => ModelPrivacy::RemoteGreedy,
                "yorickvp/llava-13b" => ModelPrivacy::RemoteGreedy,
                _ => ModelPrivacy::Unknown,
            },
            AgentLLMInterface::LocalLLM(_) => ModelPrivacy::Local,
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type.as_str() {
                "gpt" | "gpt4" | "gpt-4-1106-preview" => ModelPrivacy::RemoteGreedy,
                "gpt-vision" | "gpt-4-vision-preview" => ModelPrivacy::RemoteGreedy,
                "dall-e" => ModelPrivacy::RemoteGreedy,
                _ => ModelPrivacy::Unknown,
            },
            AgentLLMInterface::Ollama(_) => ModelPrivacy::Local,
        }
    }

    // Function to check capabilities
    pub async fn check_capabilities(&self) -> Vec<(Vec<ModelCapability>, ModelCost, ModelPrivacy)> {
        let agents = self.agents.clone();
        agents.into_iter().map(|agent| Self::get_capability(&agent)).collect()
    }

    // Function to check if a specific capability is available
    pub async fn has_capability(&self, capability: ModelCapability) -> bool {
        let capabilities = self.check_capabilities().await;
        capabilities.iter().any(|(caps, _, _)| caps.contains(&capability))
    }

    // Function to check if a specific cost is available
    pub async fn has_cost(&self, cost: ModelCost) -> bool {
        let capabilities = self.check_capabilities().await;
        capabilities.iter().any(|(_, c, _)| c == &cost)
    }

    // Function to check if a specific privacy is available
    pub async fn has_privacy(&self, privacy: ModelPrivacy) -> bool {
        let capabilities = self.check_capabilities().await;
        capabilities.iter().any(|(_, _, p)| p == &privacy)
    }

    pub async fn route_prompt_with_model(
        prompt: Prompt,
        model: &AgentLLMInterface,
    ) -> Result<PromptResult, ModelCapabilitiesManagerError> {
        match model {
            AgentLLMInterface::OpenAI(openai) => {
                if openai.model_type.starts_with("gpt-") {
                    let total_tokens = Self::get_max_tokens(model);
                    let tiktoken_messages =
                        openai_prepare_messages(model, openai.clone().model_type, prompt, total_tokens)?;
                    Ok(tiktoken_messages)
                } else {
                    Err(ModelCapabilitiesManagerError::NotImplemented(openai.model_type.clone()))
                }
            }
            AgentLLMInterface::GenericAPI(genericapi) => {
                if genericapi.model_type.starts_with("togethercomputer/llama-2") {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string =
                        llama_prepare_messages(model, genericapi.clone().model_type, prompt, total_tokens)?;
                    Ok(messages_string)
                } else {
                    Err(ModelCapabilitiesManagerError::NotImplemented(
                        genericapi.model_type.clone(),
                    ))
                }
            }
            AgentLLMInterface::LocalLLM(_) => {
                Err(ModelCapabilitiesManagerError::NotImplemented("LocalLLM".to_string()))
            }
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => Err(ModelCapabilitiesManagerError::NotImplemented(
                shinkai_backend.model_type.clone(),
            )),
            AgentLLMInterface::Ollama(ollama) => {
                if ollama.model_type.starts_with("mistral")
                    || ollama.model_type.starts_with("llama2")
                    || ollama.model_type.starts_with("starling-lm")
                    || ollama.model_type.starts_with("neural-chat")
                    || ollama.model_type.starts_with("vicuna")
                    || ollama.model_type.starts_with("mixtral")
                {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string =
                        llama_prepare_messages(model, ollama.clone().model_type, prompt, total_tokens)?;
                    Ok(messages_string)
                } else if ollama.model_type.starts_with("llava") || ollama.model_type.starts_with("bakllava") {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string =
                        llava_prepare_messages(model, ollama.clone().model_type.clone(), prompt, total_tokens)?;
                    Ok(messages_string)
                } else {
                    Err(ModelCapabilitiesManagerError::NotImplemented(ollama.model_type.clone()))
                }
            }
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
                if genericapi.model_type == "mistralai/Mixtral-8x7B-Instruct-v0.1" {
                    32_000
                } else {
                    4096
                }
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
            AgentLLMInterface::Ollama(ollama) => {
                // This searches for xxk in the name and it uses that if found, otherwise it uses 4096
                let re = Regex::new(r"(\d+)k").unwrap();
                match re.captures(&ollama.model_type) {
                    Some(caps) => caps.get(1).map_or(4096, |m| m.as_str().parse().unwrap_or(4096)),
                    None => 4096,
                }
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
