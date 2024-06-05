use crate::{
    agent::{
        error::AgentError,
        execution::prompts::prompts::Prompt,
        providers::shared::{
            openai::openai_prepare_messages,
            shared_model_logic::{llama_prepare_messages, llava_prepare_messages},
        },
    },
    db::ShinkaiDB,
};
use shinkai_message_primitives::schemas::{
    agents::serialized_agent::{AgentLLMInterface, SerializedAgent},
    shinkai_name::ShinkaiName,
};
use std::{
    fmt,
    sync::{Arc, Weak},
};
use tiktoken_rs::ChatCompletionRequestMessage;

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
    pub db: Weak<ShinkaiDB>,
    pub profile: ShinkaiName,
    pub agents: Vec<SerializedAgent>,
}

impl ModelCapabilitiesManager {
    // Constructor
    pub async fn new(db: Weak<ShinkaiDB>, profile: ShinkaiName) -> Self {
        let db_arc = db.upgrade().unwrap();
        let agents = Self::get_agents(&db_arc, profile.clone()).await;
        Self { db, profile, agents }
    }

    // Function to get all agents from the database for a profile
    async fn get_agents(db: &Arc<ShinkaiDB>, profile: ShinkaiName) -> Vec<SerializedAgent> {
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
                "gpt-4o" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
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
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type().as_str() {
                "gpt" | "gpt4" | "gpt-4-1106-preview" | "PREMIUM_TEXT_INFERENCE" | "STANDARD_TEXT_INFERENCE" => {
                    vec![ModelCapability::TextInference]
                }
                "gpt-vision" | "gpt-4-vision-preview" | "PREMIUM_VISION_INFERENCE" => {
                    vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference]
                }
                "dall-e" => vec![ModelCapability::ImageGeneration],
                _ => vec![],
            },
            AgentLLMInterface::Ollama(ollama) => match ollama.model_type.as_str() {
                model_type if model_type.starts_with("llama-2") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("mistral") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("mixtral") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("deepseek") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("meditron") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("starling-lm") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("orca2") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("yi") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("aya") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("codestral") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("yarn-mistral") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("llama3") => vec![ModelCapability::TextInference],
                model_type if model_type.starts_with("llava") => {
                    vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
                }
                model_type if model_type.starts_with("bakllava") => {
                    vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
                }
                model_type if model_type.contains("minicpm_llama3") => {
                    vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
                }
                model_type if model_type.starts_with("yarn-llama2") => vec![ModelCapability::TextInference],
                _ => vec![],
            },
            AgentLLMInterface::Groq(groq) => {
                vec![ModelCapability::TextInference]
            }
        }
    }

    // Static method to get cost of an agent model
    pub fn get_agent_cost(model: &AgentLLMInterface) -> ModelCost {
        match model {
            AgentLLMInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-4o" => ModelCost::Cheap,
                "gpt-3.5-turbo-1106" => ModelCost::Cheap,
                "gpt-4-1106-preview" => ModelCost::GoodValue,
                "gpt-4-vision-preview" => ModelCost::GoodValue,
                "dall-e-3" => ModelCost::GoodValue,
                _ => ModelCost::Unknown,
            },
            AgentLLMInterface::GenericAPI(genericapi) => match genericapi.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => ModelCost::Cheap,
                "togethercomputer/llama3" => ModelCost::Cheap,
                "yorickvp/llava-13b" => ModelCost::Expensive,
                _ => ModelCost::Unknown,
            },
            AgentLLMInterface::LocalLLM(_) => ModelCost::Cheap,
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type().as_str() {
                "gpt4" | "gpt-4-1106-preview" | "PREMIUM_TEXT_INFERENCE" => ModelCost::Expensive,
                "gpt-vision" | "gpt-4-vision-preview" | "STANDARD_TEXT_INFERENCE" | "PREMIUM_VISION_INFERENCE" => {
                    ModelCost::GoodValue
                }
                "dall-e" => ModelCost::GoodValue,
                _ => ModelCost::Unknown,
            },
            AgentLLMInterface::Ollama(_) => ModelCost::Cheap,
            AgentLLMInterface::Groq(_) => ModelCost::Cheap,
        }
    }

    // Static method to get privacy of an agent model
    pub fn get_agent_privacy(model: &AgentLLMInterface) -> ModelPrivacy {
        match model {
            AgentLLMInterface::OpenAI(_) => ModelPrivacy::RemoteGreedy,
            AgentLLMInterface::GenericAPI(_) => ModelPrivacy::RemoteGreedy,
            AgentLLMInterface::LocalLLM(_) => ModelPrivacy::Local,
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type().as_str() {
                "PREMIUM_TEXT_INFERENCE" => ModelPrivacy::RemoteGreedy,
                "PREMIUM_VISION_INFERENCE" => ModelPrivacy::RemoteGreedy,
                "STANDARD_TEXT_INFERENCE" => ModelPrivacy::RemoteGreedy,
                _ => ModelPrivacy::Unknown,
            },
            AgentLLMInterface::Ollama(_) => ModelPrivacy::Local,
            AgentLLMInterface::Groq(_) => ModelPrivacy::RemoteGreedy,
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
                    let tiktoken_messages = openai_prepare_messages(model, prompt)?;
                    Ok(tiktoken_messages)
                } else {
                    Err(ModelCapabilitiesManagerError::NotImplemented(openai.model_type.clone()))
                }
            }
            AgentLLMInterface::GenericAPI(genericapi) => {
                if genericapi.model_type.starts_with("togethercomputer/llama-2")
                    || genericapi.model_type.starts_with("meta-llama/Llama-3")
                {
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
                shinkai_backend.model_type().clone(),
            )),
            AgentLLMInterface::Ollama(ollama) => {
                if ollama.model_type.starts_with("mistral")
                    || ollama.model_type.starts_with("llama2")
                    || ollama.model_type.starts_with("llama3")
                    || ollama.model_type.starts_with("wizardlm2")
                    || ollama.model_type.starts_with("starling-lm")
                    || ollama.model_type.starts_with("neural-chat")
                    || ollama.model_type.starts_with("vicuna")
                    || ollama.model_type.starts_with("mixtral")
                    || ollama.model_type.starts_with("falcon2")
                    || ollama.model_type.starts_with("dolphin-llama3")
                    || ollama.model_type.starts_with("command-r-plus")
                    || ollama.model_type.starts_with("wizardlm2")
                    || ollama.model_type.starts_with("phi3")
                    || ollama.model_type.starts_with("aya")
                    || ollama.model_type.starts_with("codestral")
                    || ollama
                        .model_type
                        .starts_with("adrienbrault/nous-hermes2theta-llama3-8b")
                    || ollama.model_type.contains("minicpm_llama3")
                {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string =
                        llama_prepare_messages(model, ollama.clone().model_type, prompt, total_tokens)?;
                    Ok(messages_string)
                } else if ollama.model_type.starts_with("llava")
                    || ollama.model_type.starts_with("bakllava")
                    || ollama.model_type.starts_with("llava-phi3")
                {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string =
                        llava_prepare_messages(model, ollama.clone().model_type.clone(), prompt, total_tokens)?;
                    Ok(messages_string)
                } else {
                    Err(ModelCapabilitiesManagerError::NotImplemented(ollama.model_type.clone()))
                }
            }
            AgentLLMInterface::Groq(groq) => {
                let total_tokens = Self::get_max_tokens(model);
                let messages_string = llama_prepare_messages(model, groq.clone().model_type, prompt, total_tokens)?;
                Ok(messages_string)
            }
        }
    }

    /// Returns the maximum number of tokens allowed for the given model.
    pub fn get_max_tokens(model: &AgentLLMInterface) -> usize {
        match model {
            AgentLLMInterface::OpenAI(openai) => {
                if openai.model_type == "gpt-4o"
                    || openai.model_type == "gpt-4-1106-preview"
                    || openai.model_type == "gpt-4-vision-preview"
                {
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
                } else if genericapi.model_type.starts_with("mistralai/Mistral-7B-Instruct-v0.2") {
                    16_000
                    //  32_000
                } else if genericapi.model_type.starts_with("meta-llama/Llama-3") {
                    8_000
                } else if genericapi.model_type.starts_with("mistralai/Mixtral-8x22B") {
                    65_000
                } else {
                    4096
                }
            }
            AgentLLMInterface::LocalLLM(_) => {
                // Fill in the appropriate logic for LocalLLM
                0
            }
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => {
                if shinkai_backend.model_type() == "PREMIUM_TEXT_INFERENCE"
                    || shinkai_backend.model_type() == "PREMIUM_VISION_INFERENCE"
                {
                    128_000
                } else if shinkai_backend.model_type() == "STANDARD_TEXT_INFERENCE" {
                    32_000
                } else {
                    let normalized_model = Self::normalize_model(&model.clone());
                    tiktoken_rs::model::get_context_size(normalized_model.as_str())
                }
            }
            AgentLLMInterface::Groq(groq) => {
                // Fill in the appropriate logic for GenericAPI
                if groq.model_type == "llama3-70b-8192" {
                    8_000
                } else {
                    4096
                }
            }
            AgentLLMInterface::Ollama(ollama) => {
                return match ollama.model_type.as_str() {
                    model_type if model_type.starts_with("mistral:7b-instruct-v0.2") => 32_000,
                    model_type if model_type.starts_with("mixtral:8x7b-instruct-v0.1") => 16_000,
                    model_type if model_type.starts_with("mixtral:8x22b") => 65_000,
                    model_type if model_type.starts_with("llama3-gradient") => {
                        eprintln!("llama3-gradient detected");
                        return 256_000;
                    }
                    model_type if model_type.starts_with("falcon2") => 8_000,
                    model_type if model_type.starts_with("llama3-chatqa") => 8_000,
                    model_type if model_type.starts_with("llava-phi3") => 4_000,
                    model_type if model_type.contains("minicpm_llama3") => 4_000,
                    model_type if model_type.starts_with("dolphin-llama3") => 8_000,
                    model_type if model_type.starts_with("command-r-plus") => 128_000,
                    model_type if model_type.starts_with("codestral") => 32_000,
                    model_type if model_type.starts_with("aya") => 32_000,
                    model_type if model_type.starts_with("wizardlm2") => 8_000,
                    model_type if model_type.starts_with("phi2") => 4_000,
                    model_type if model_type.starts_with("adrienbrault/nous-hermes2theta-llama3-8b") => 8_000,
                    model_type if model_type.starts_with("llama3") || model_type.starts_with("llava-llama3") => 8_000,
                    _ => 4096, // Default token count if no specific model type matches
                };
            }
        }
    }

    /// Returns the maximum number of input tokens allowed for the given model, leaving room for output tokens.
    pub fn get_max_input_tokens(model: &AgentLLMInterface) -> usize {
        let max_tokens = Self::get_max_tokens(model);
        let max_output_tokens = Self::get_max_output_tokens(model);
        if max_tokens > max_output_tokens {
            max_tokens - max_output_tokens
        } else {
            max_output_tokens
        }
    }

    pub fn get_max_output_tokens(model: &AgentLLMInterface) -> usize {
        match model {
            AgentLLMInterface::OpenAI(_) => {
                // Fill in the appropriate logic for OpenAI
                4096
            }
            AgentLLMInterface::GenericAPI(_) => {
                if Self::get_max_tokens(model) < 8500 {
                    2800
                } else {
                    4096
                }
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
            AgentLLMInterface::Groq(_) => {
                // Fill in the appropriate logic for Ollama
                4096
            }
        }
    }

    /// Returns the remaining number of output tokens allowed for the LLM to use
    pub fn get_remaining_output_tokens(model: &AgentLLMInterface, used_tokens: usize) -> usize {
        let max_tokens = Self::get_max_tokens(model);
        let mut remaining_output_tokens = max_tokens.saturating_sub(used_tokens);
        remaining_output_tokens = std::cmp::min(
            remaining_output_tokens,
            ModelCapabilitiesManager::get_max_output_tokens(&model.clone()),
        );
        remaining_output_tokens
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
            AgentLLMInterface::GenericAPI(_genericapi) => {
                // Fill in the appropriate logic for GenericAPI
                "".to_string()
            }
            AgentLLMInterface::LocalLLM(_) => {
                // Fill in the appropriate logic for LocalLLM
                "".to_string()
            }
            AgentLLMInterface::Groq(_) => {
                // Fill in the appropriate logic for LocalLLM
                "".to_string()
            }
            AgentLLMInterface::ShinkaiBackend(shinkai_backend) => {
                if shinkai_backend.model_type().starts_with("gpt") {
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

    pub fn generic_token_estimation(text: &str) -> usize {
        let average_token_size = 4;
        let buffer_percentage = 0.1;
        let char_count = text.chars().count();
        let estimated_tokens = (char_count as f64 / average_token_size as f64).ceil() as usize;
        

        (estimated_tokens as f64 * (1.0 - buffer_percentage)).floor() as usize
    }

    /// Counts the number of tokens from the list of messages
    pub fn num_tokens_from_messages(messages: &[ChatCompletionRequestMessage]) -> usize {
        let average_token_size = 4; // Average size of a token (in characters)
        let buffer_percentage = 0.15; // Buffer to account for tokenization variance

        let mut total_characters = 0;
        for message in messages {
            total_characters += message.role.chars().count() + 1; // +1 for a space or newline after the role
            if let Some(ref content) = message.content {
                total_characters += content.chars().count() + 1; // +1 for spaces or newlines between messages
            }
            if let Some(ref name) = message.name {
                total_characters += name.chars().count() + 1; // +1 for a space or newline after the name
            }
        }

        // Calculate estimated tokens without the buffer
        let estimated_tokens = (total_characters as f64 / average_token_size as f64).ceil() as usize;

        // Apply the buffer to estimate the total token count
        let buffered_token_count = ((estimated_tokens as f64) * (1.0 - buffer_percentage)).floor() as usize;

        (buffered_token_count as f64 * 2.6).floor() as usize
    }

    /// Counts the number of tokens from a single message string for llama3 model,
    /// where every three normal letters (a-zA-Z) allow an empty space to not be counted,
    /// and other symbols are counted as 1 token.
    /// This implementation avoids floating point arithmetic by scaling counts.
    pub fn count_tokens_from_message_llama3(message: &str) -> usize {
        let mut token_count = 0;
        let mut alphabetic_count = 0; // Total count of alphabetic characters
        let mut space_count = 0; // Total count of spaces

        // First pass: count alphabetic characters and spaces
        for c in message.chars() {
            if c.is_ascii_alphabetic() {
                alphabetic_count += 1;
            } else if c.is_whitespace() {
                space_count += 1;
            }
        }

        // Calculate how many spaces can be ignored
        let spaces_to_ignore = alphabetic_count / 3;

        // Determine the alphabetic token weight based on the number of alphabetic characters
        let alphabetic_token_weight = if alphabetic_count > 500 { 8 } else { 10 };

        // Second pass: count tokens, adjusting for spaces that can be ignored
        for c in message.chars() {
            if c.is_ascii_alphabetic() {
                token_count += alphabetic_token_weight; // Counting as 1/3, so add 1 to the scaled count
            } else if c.is_whitespace() {
                if spaces_to_ignore > 0 {
                    space_count -= 10; // Reduce the count of spaces to ignore by the scaling factor
                } else {
                    token_count += 30; // Count the space as a full token if not enough alphabetic characters
                }
            } else {
                token_count += 30; // Non-alphabetic characters count as a full token, add 3 to the scaled count
            }
        }

        (token_count / 30) + 1 // Divide the scaled count by 30 and floor the result, add 1 to account for any remainder
    }

    /// Counts the number of tokens from the list of messages for llama3 model,
    /// where every three normal letters (a-zA-Z) allow an empty space to not be counted,
    /// and other symbols are counted as 1 token.
    /// This implementation avoids floating point arithmetic by scaling counts.
    pub fn num_tokens_from_llama3(messages: &[ChatCompletionRequestMessage]) -> usize {
        let num: usize = messages
            .iter()
            .map(|message| {
                let role_prefix = match message.role.as_str() {
                    "user" => "User: ",
                    "sys" => "Sys: ",
                    "assistant" => "A: ",
                    _ => "",
                };
                let full_message = format!(
                    "{}{}\n",
                    role_prefix,
                    message.content.as_ref().unwrap_or(&"".to_string())
                );
                Self::count_tokens_from_message_llama3(&full_message)
            })
            .sum();

        (num as f32 * 1.04) as usize
    }
}

// TODO: add a tokenizer library only in the dev env and test that the estimations are always above it and in a specific margin (% wise)
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use tiktoken_rs::ChatCompletionRequestMessage;

    // Helper function to convert a vector of ChatCompletionRequestMessage to a single string
    fn messages_to_string(messages: &[ChatCompletionRequestMessage]) -> String {
        messages
            .iter()
            .map(|message| {
                format!(
                    "{}: {} ({})",
                    message.role,
                    message.content.as_ref().unwrap_or(&"".to_string()),
                    message.name.as_ref().unwrap_or(&"".to_string())
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    // #[test]
    fn test_num_tokens_from_messages_empty() {
        let messages: Vec<ChatCompletionRequestMessage> = vec![];
        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Converted messages: \"{}\"", messages_to_string(&messages));
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);
        // assert_eq!(num_tokens, 0);
        // assert_eq!(num_tokens_llama3, 1);
    }

    // #[test]
    fn test_num_tokens_from_messages_single_message() {
        let messages = vec![ChatCompletionRequestMessage {
            role: "user".to_string(),
            content: Some("Hello, how are you?".to_string()),
            name: Some("Alice".to_string()),
            function_call: None,
        }];
        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Converted messages: \"{}\"", messages_to_string(&messages));
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);
        // assert_eq!(num_tokens, 15);
        // assert_eq!(num_tokens_llama3, 10);
    }

    // #[test]
    fn test_num_tokens_from_messages_multiple_messages() {
        let messages = vec![
            ChatCompletionRequestMessage {
                role: "user".to_string(),
                content: Some("Hello".to_string()),
                name: Some("Alice".to_string()),
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "bot".to_string(),
                content: Some("Hi there!".to_string()),
                name: Some("Bob".to_string()),
                function_call: None,
            },
        ];
        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Converted messages: \"{}\"", messages_to_string(&messages));
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);
        // assert_eq!(num_tokens, 17);
        // assert_eq!(num_tokens_llama3, 9);
    }

    // #[test]
    fn test_num_tokens_from_messages_complex_content() {
        let messages = vec![ChatCompletionRequestMessage {
            role: "user".to_string(),
            content: Some("Hello, how are you doing today? I hope everything is fine.".to_string()),
            name: Some("Alice".to_string()),
            function_call: None,
        }];
        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Converted messages: \"{}\"", messages_to_string(&messages));
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);
        // assert_eq!(num_tokens, 35);
        // assert_eq!(num_tokens_llama3, 19);
    }

    // #[test]
    fn test_num_tokens_from_complex_scenario() {
        let messages = vec![
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user as much information as possible using paragraphs, blocks, and bulletpoint lists. Remember to only use single quotes (never double quotes) inside of strings that you respond with.".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("The user has asked: ".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "user".to_string(),
                content: Some("tell me about Minecraft".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("Use the content to directly answer the user's question with as much information as is available. If the user talks about `it` or `this`, they are referencing the content. Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and '\\n' separated paragraphs. Do not include further JSON inside of the `answer` field, unless the user requires it based on what they asked. Format answer so that it is easily readable with newlines after each 2 sentences and bullet point lists as needed:".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("Then respond using the following EBNF and absolutely nothing else: '{' 'answer' ':' string '}' ".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("```json".to_string()),
                name: None,
                function_call: None,
            },
        ];

        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Converted messages: \"{}\"", messages_to_string(&messages));
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);
    }

    #[test]
    // fn test_num_tokens_from_real_prompt_success_overestimate() {
    fn test_num_tokens_from_real_prompt() {
        let file_path = "../../files/for tests/token_estimation_test_prompt.txt";
        let content_result = fs::read_to_string(file_path);
        let content = match content_result {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Failed to read file: {:?}", e);
                return;
            }
        };

        // Alternatively generate the prompt using the struct and then into messages
        // let mut prompt = Prompt::new();
        // for text in content
        //     .chars()
        //     .collect::<Vec<char>>()
        //     .chunks(chunk_size)
        // {
        //     prompt.add_content(text.to_string(), SubPromptType::User, 100);
        // }
        // let result = openai_prepare_messages(&model, prompt)?;

        let chunk_size = 400;
        let messages: Vec<ChatCompletionRequestMessage> = content
            .chars()
            .collect::<Vec<char>>()
            .chunks(chunk_size)
            .map(|chunk| ChatCompletionRequestMessage {
                role: "user".to_string(),
                content: Some(chunk.iter().collect::<String>()),
                name: Some("Alice".to_string()),
                function_call: None,
            })
            .collect();
        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Converted messages: \"{}\"", messages_to_string(&messages));
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);

        // Check that the estimate is greater than the numbers below to ensure it over estimates and not under
        assert!(num_tokens_llama3 > 28000);
        assert!(num_tokens > 34000);
    }

    // #[test]
    fn test_num_tokens_from_poker_probability_explanation() {
        let messages = vec![
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("Calculating the probabilities of winning in Texas Hold'em is a complex task that involves combinatorics and an understanding of the game's rules. Here's a simplified version of how you might approach writing code to calculate these probabilities in Python. This example assumes you have a function that can evaluate the strength of a hand and that you are calculating the probability for a specific point in the game (e.g., after the flop).".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("```python\nimport itertools\nimport random\n\n# Assume hand_strength is a function that takes a list of cards and returns a score indicating the strength of the hand.\n\ndef calculate_probabilities(player_hand, community_cards, deck):\n    wins = 0\n    iterations = 10000  # Number of simulations to run\n    for _ in range(iterations):\n        # Shuffle the deck and deal out the rest of the community cards plus opponent's cards\n        random.shuffle(deck)\n        remaining_community = deck[:5-len(community_cards)]\n        opponent_hand = deck[5-len(community_cards):7-len(community_cards)]\n        final_community = community_cards + remaining_community\n\n        # Evaluate the hands\n        player_score = hand_strength(player_hand + final_community)\n        opponent_score = hand_strength(opponent_hand + final_community)\n\n        # Compare the hands to determine a win\n        if player_score > opponent_score:\n            wins += 1\n\n    # Calculate the probability\n    probability = wins / iterations\n    return probability\n\n# Example usage:\n# Assuming the deck is a list of all 52 cards, player_hand is the player's 2 cards, and community_cards are the cards on the board\n# deck = [...]\n# player_hand = [...]\n# community_cards = [...]\n# win_probability = calculate_probabilities(player_hand, community_cards, deck)\n# print('Winning Probability:', win_probability)\n```".to_string()),
                name: None,
                function_call: None,
            },
            ChatCompletionRequestMessage {
                role: "system".to_string(),
                content: Some("The function `calculate_probabilities` runs a Monte Carlo simulation to estimate the winning probability for a player's hand against a single opponent. Note that in a real poker game, you would need to evaluate against multiple opponents, deal with incomplete information, and dynamically adjust as the community cards are revealed. For a more accurate probability calculation, consider factors like the number of players, their ranges, and how the hand could develop with future community cards".to_string()),
                name: None,
                function_call: None,
            },
        ];

        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Converted messages: \"{}\"", messages_to_string(&messages));
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);
    }
}
