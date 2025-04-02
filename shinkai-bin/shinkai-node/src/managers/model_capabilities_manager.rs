use crate::llm_provider::{
    error::LLMProviderError, providers::shared::{openai_api::openai_prepare_messages, shared_model_logic::llama_prepare_messages}
};
use shinkai_message_primitives::{
    schemas::{
        llm_message::LlmMessage, llm_providers::{
            common_agent_llm_provider::ProviderOrAgent, serialized_llm_provider::{LLMProviderInterface, SerializedLLMProvider}
        }, prompts::Prompt, shinkai_name::ShinkaiName
    }, shinkai_utils::utils::count_tokens_from_message_llama3
};
use shinkai_sqlite::SqliteManager;
use std::{
    fmt, sync::{Arc, Weak}
};

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

impl From<LLMProviderError> for ModelCapabilitiesManagerError {
    fn from(error: LLMProviderError) -> Self {
        ModelCapabilitiesManagerError::GeneralError(error.to_string())
    }
}

impl std::error::Error for ModelCapabilitiesManagerError {}

#[derive(Clone, Debug, PartialEq)]
pub struct PromptResult {
    pub messages: PromptResultEnum,
    pub functions: Option<Vec<serde_json::Value>>,
    pub remaining_output_tokens: usize,
    pub tokens_used: usize,
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
    Free,
    VeryCheap,
    Cheap,
    GoodValue,
    Expensive,
}

// Enum for privacy
#[derive(Clone, Debug, PartialEq)]
pub enum ModelPrivacy {
    Unknown,
    Local,
    RemoteGreedy,
}

// Struct for ModelCapabilitiesManager
pub struct ModelCapabilitiesManager {
    pub db: Weak<SqliteManager>,
    pub profile: ShinkaiName,
    pub llm_providers: Vec<SerializedLLMProvider>,
}

impl ModelCapabilitiesManager {
    // Constructor
    pub async fn new(db: Weak<SqliteManager>, profile: ShinkaiName) -> Self {
        let db_arc = db.upgrade().unwrap();
        let llm_providers = Self::get_llm_providers(&db_arc, profile.clone()).await;
        Self {
            db,
            profile,
            llm_providers,
        }
    }

    // Function to get all llm providers from the database for a profile
    async fn get_llm_providers(db: &Arc<SqliteManager>, profile: ShinkaiName) -> Vec<SerializedLLMProvider> {
        db.get_llm_providers_for_profile(profile).unwrap()
    }

    // Static method to get capability of an agent
    pub fn get_capability(agent: &SerializedLLMProvider) -> (Vec<ModelCapability>, ModelCost, ModelPrivacy) {
        let capabilities = Self::get_llm_provider_capabilities(&agent.model);
        let cost = Self::get_llm_provider_cost(&agent.model);
        let privacy = Self::get_llm_provider_privacy(&agent.model);

        (capabilities, cost, privacy)
    }

    // Static method to get capabilities of an agent model
    pub fn get_llm_provider_capabilities(model: &LLMProviderInterface) -> Vec<ModelCapability> {
        match model {
            LLMProviderInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-4o" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-4o-mini" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-3.5-turbo-1106" => vec![ModelCapability::TextInference],
                "gpt-4-1106-preview" => vec![ModelCapability::TextInference],
                "gpt-4-vision-preview" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "4o-preview" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "4o-mini" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "dall-e-3" => vec![ModelCapability::ImageGeneration],
                model_type if model_type.starts_with("gpt-") => vec![ModelCapability::TextInference],
                _ => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
            },
            LLMProviderInterface::TogetherAI(togetherai) => match togetherai.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => vec![ModelCapability::TextInference],
                "yorickvp/llava-13b" => vec![ModelCapability::ImageAnalysis],
                model_type if model_type.starts_with("togethercomputer/llama-2") => {
                    vec![ModelCapability::TextInference]
                }
                _ => vec![],
            },
            LLMProviderInterface::LocalLLM(_) => vec![],
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type().as_str() {
                "gpt" | "gpt4" | "gpt-4-1106-preview" | "PREMIUM_TEXT_INFERENCE" | "STANDARD_TEXT_INFERENCE" => {
                    vec![ModelCapability::TextInference]
                }
                "gpt-vision"
                | "gpt-4-vision-preview"
                | "gp4o"
                | "gpt-4o"
                | "PREMIUM_VISION_INFERENCE"
                | "gpt-4o-mini" => {
                    vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference]
                }
                "dall-e" => vec![ModelCapability::ImageGeneration],
                _ => vec![],
            },
            LLMProviderInterface::Ollama(model) => Self::get_shared_capabilities(model.model_type().as_str()),
            LLMProviderInterface::Exo(model) => Self::get_shared_capabilities(model.model_type().as_str()),
            LLMProviderInterface::Groq(model) => Self::get_shared_capabilities(model.model_type().as_str()),
            LLMProviderInterface::Gemini(_) => vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis],
            LLMProviderInterface::OpenRouter(model) => Self::get_shared_capabilities(model.model_type().as_str()),
            LLMProviderInterface::Claude(_) => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
            LLMProviderInterface::DeepSeek(_) => vec![ModelCapability::TextInference],
            LLMProviderInterface::LocalRegex(_) => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
        }
    }

    fn get_shared_capabilities(model_type: &str) -> Vec<ModelCapability> {
        match model_type {
            model_type if model_type.starts_with("llama3") => vec![ModelCapability::TextInference],
            model_type if model_type.starts_with("llava") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            model_type if model_type.starts_with("bakllava") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            model_type if model_type.starts_with("moondream") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            model_type if model_type.contains("minicpm-v") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            model_type if model_type.starts_with("regex") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            _ => vec![ModelCapability::TextInference],
        }
    }

    // Static method to get cost of an agent model
    pub fn get_llm_provider_cost(model: &LLMProviderInterface) -> ModelCost {
        match model {
            LLMProviderInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-4o" => ModelCost::Cheap,
                "gpt-3.5-turbo-1106" => ModelCost::VeryCheap,
                "gpt-4o-mini" => ModelCost::VeryCheap,
                "gpt-4-1106-preview" => ModelCost::GoodValue,
                "gpt-4-vision-preview" => ModelCost::GoodValue,
                "dall-e-3" => ModelCost::GoodValue,
                _ => ModelCost::Unknown,
            },
            LLMProviderInterface::TogetherAI(togetherai) => match togetherai.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => ModelCost::Cheap,
                "togethercomputer/llama3" => ModelCost::Cheap,
                "yorickvp/llava-13b" => ModelCost::Expensive,
                _ => ModelCost::Unknown,
            },
            LLMProviderInterface::LocalLLM(_) => ModelCost::Cheap,
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type().as_str() {
                "gpt4" | "gpt-4-1106-preview" | "PREMIUM_TEXT_INFERENCE" => ModelCost::Expensive,
                "gpt-vision" | "gpt-4-vision-preview" | "STANDARD_TEXT_INFERENCE" | "PREMIUM_VISION_INFERENCE" => {
                    ModelCost::GoodValue
                }
                "dall-e" => ModelCost::GoodValue,
                _ => ModelCost::Unknown,
            },
            LLMProviderInterface::Ollama(_) => ModelCost::Free,
            LLMProviderInterface::Groq(_) => ModelCost::VeryCheap,
            LLMProviderInterface::Gemini(_) => ModelCost::Cheap,
            LLMProviderInterface::Exo(_) => ModelCost::Cheap,
            LLMProviderInterface::OpenRouter(_) => ModelCost::Free,
            LLMProviderInterface::Claude(claude) => match claude.model_type.as_str() {
                "claude-3-5-sonnet-20241022" | "claude-3-5-sonnet-latest" => ModelCost::Cheap,
                "claude-3-opus-20240229" | "claude-3-opus-latest" => ModelCost::GoodValue,
                "claude-3-sonnet-20240229" => ModelCost::Cheap,
                "claude-3-haiku-20240307" => ModelCost::VeryCheap,
                _ => ModelCost::Unknown,
            },
            LLMProviderInterface::DeepSeek(deepseek) => match deepseek.model_type.as_str() {
                "deepseek-chat" => ModelCost::Cheap,
                "deepseek-reasoner" => ModelCost::GoodValue,
                _ => ModelCost::Unknown,
            },
            LLMProviderInterface::LocalRegex(_) => ModelCost::Free,
        }
    }

    // Static method to get privacy of an llm provider model
    pub fn get_llm_provider_privacy(model: &LLMProviderInterface) -> ModelPrivacy {
        match model {
            LLMProviderInterface::OpenAI(_) => ModelPrivacy::RemoteGreedy,
            LLMProviderInterface::TogetherAI(_) => ModelPrivacy::RemoteGreedy,
            LLMProviderInterface::LocalLLM(_) => ModelPrivacy::Local,
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type().as_str() {
                "PREMIUM_TEXT_INFERENCE" => ModelPrivacy::RemoteGreedy,
                "PREMIUM_VISION_INFERENCE" => ModelPrivacy::RemoteGreedy,
                "STANDARD_TEXT_INFERENCE" => ModelPrivacy::RemoteGreedy,
                _ => ModelPrivacy::Unknown,
            },
            LLMProviderInterface::Ollama(_) => ModelPrivacy::Local,
            LLMProviderInterface::Groq(_) => ModelPrivacy::RemoteGreedy,
            LLMProviderInterface::Gemini(_) => ModelPrivacy::RemoteGreedy,
            LLMProviderInterface::Exo(_) => ModelPrivacy::Local,
            LLMProviderInterface::OpenRouter(_) => ModelPrivacy::Local,
            LLMProviderInterface::Claude(_) => ModelPrivacy::RemoteGreedy,
            LLMProviderInterface::DeepSeek(_) => ModelPrivacy::RemoteGreedy,
            LLMProviderInterface::LocalRegex(_) => ModelPrivacy::Local,
        }
    }

    // Function to check capabilities
    pub async fn check_capabilities(&self) -> Vec<(Vec<ModelCapability>, ModelCost, ModelPrivacy)> {
        let llm_providers = self.llm_providers.clone();
        llm_providers
            .into_iter()
            .map(|llm_provider| Self::get_capability(&llm_provider))
            .collect()
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
        model: &LLMProviderInterface,
    ) -> Result<PromptResult, ModelCapabilitiesManagerError> {
        match model {
            LLMProviderInterface::OpenAI(openai) => {
                if openai.model_type.starts_with("gpt-") {
                    let tiktoken_messages = openai_prepare_messages(model, prompt)?;
                    Ok(tiktoken_messages)
                } else {
                    Err(ModelCapabilitiesManagerError::NotImplemented(openai.model_type.clone()))
                }
            }
            LLMProviderInterface::TogetherAI(togetherai) => {
                if togetherai.model_type.starts_with("togethercomputer/llama-2")
                    || togetherai.model_type.starts_with("meta-llama/Llama-3")
                {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string =
                        llama_prepare_messages(model, togetherai.clone().model_type, prompt, total_tokens)?;
                    Ok(messages_string)
                } else {
                    Err(ModelCapabilitiesManagerError::NotImplemented(
                        togetherai.model_type.clone(),
                    ))
                }
            }
            LLMProviderInterface::LocalLLM(_) => {
                Err(ModelCapabilitiesManagerError::NotImplemented("LocalLLM".to_string()))
            }
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => Err(
                ModelCapabilitiesManagerError::NotImplemented(shinkai_backend.model_type().clone()),
            ),
            LLMProviderInterface::Ollama(ollama) => {
                if Self::get_shared_capabilities(ollama.model_type().as_str()).is_empty() {
                    Err(ModelCapabilitiesManagerError::NotImplemented(ollama.model_type.clone()))
                } else {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string =
                        llama_prepare_messages(model, ollama.clone().model_type, prompt, total_tokens)?;
                    Ok(messages_string)
                }
            }
            LLMProviderInterface::OpenRouter(openrouter) => {
                if Self::get_shared_capabilities(openrouter.model_type.as_str()).is_empty() {
                    Err(ModelCapabilitiesManagerError::NotImplemented(
                        openrouter.model_type.clone(),
                    ))
                } else {
                    let total_tokens = Self::get_max_tokens(model);
                    let messages_string =
                        llama_prepare_messages(model, openrouter.clone().model_type, prompt, total_tokens)?;
                    Ok(messages_string)
                }
            }
            LLMProviderInterface::Groq(groq) => {
                let total_tokens = Self::get_max_tokens(model);
                let messages_string = llama_prepare_messages(model, groq.clone().model_type, prompt, total_tokens)?;
                Ok(messages_string)
            }
            LLMProviderInterface::Gemini(gemini) => {
                let total_tokens = Self::get_max_tokens(model);
                let messages_string = llama_prepare_messages(model, gemini.clone().model_type, prompt, total_tokens)?;
                Ok(messages_string)
            }
            LLMProviderInterface::Exo(exo) => {
                let total_tokens = Self::get_max_tokens(model);
                let messages_string = llama_prepare_messages(model, exo.clone().model_type, prompt, total_tokens)?;
                Ok(messages_string)
            }
            LLMProviderInterface::Claude(claude) => {
                let total_tokens = Self::get_max_tokens(model);
                let messages_string = llama_prepare_messages(model, claude.clone().model_type, prompt, total_tokens)?;
                Ok(messages_string)
            },
            LLMProviderInterface::DeepSeek(_) => {
                let tiktoken_messages = openai_prepare_messages(model, prompt)?;
                Ok(tiktoken_messages)
            },
            LLMProviderInterface::LocalRegex(local_regex) => {
                let total_tokens = Self::get_max_tokens(model);
                let messages_string =
                    llama_prepare_messages(model, local_regex.clone().model_type, prompt, total_tokens)?;
                Ok(messages_string)
            }
        }
    }

    /// Returns the maximum number of tokens allowed for the given model.
    pub fn get_max_tokens(model: &LLMProviderInterface) -> usize {
        match model {
            LLMProviderInterface::OpenAI(openai) => {
                if openai.model_type.starts_with("gpt-4o")
                    || openai.model_type.starts_with("gpt-4-1106-preview")
                    || openai.model_type.starts_with("gpt-4o-mini")
                    || openai.model_type.starts_with("gpt-4-vision-preview")
                    || openai.model_type.starts_with("o1-mini")
                    || openai.model_type.starts_with("o1-preview")
                {
                    128_000
                } else {
                    32_000
                }
            }
            LLMProviderInterface::TogetherAI(togetherai) => {
                // Fill in the appropriate logic for GenericAPI
                if togetherai.model_type == "mistralai/Mixtral-8x7B-Instruct-v0.1" {
                    32_000
                } else if togetherai.model_type.starts_with("mistralai/Mistral-7B-Instruct-v0.2") {
                    16_000
                    //  32_000
                } else if togetherai.model_type.starts_with("meta-llama/Llama-3") {
                    8_000
                } else if togetherai.model_type.starts_with("mistralai/Mixtral-8x22B") {
                    65_000
                } else {
                    4096
                }
            }
            LLMProviderInterface::LocalLLM(_) => {
                // Fill in the appropriate logic for LocalLLM
                0
            }
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => {
                if shinkai_backend.model_type() == "PREMIUM_TEXT_INFERENCE"
                    || shinkai_backend.model_type() == "PREMIUM_VISION_INFERENCE"
                {
                    128_000
                } else {
                    128_000
                }
            }
            LLMProviderInterface::Gemini(_) => 1_000_000,
            LLMProviderInterface::Ollama(ollama) => Self::get_max_tokens_for_model_type(&ollama.model_type),
            LLMProviderInterface::Exo(exo) => Self::get_max_tokens_for_model_type(&exo.model_type),
            LLMProviderInterface::Groq(groq) => {
                std::cmp::min(Self::get_max_tokens_for_model_type(&groq.model_type), 7000)
            }
            LLMProviderInterface::OpenRouter(openrouter) => Self::get_max_tokens_for_model_type(&openrouter.model_type),
            LLMProviderInterface::Claude(_) => 200_000,
            LLMProviderInterface::DeepSeek(_) => 64_000,
            LLMProviderInterface::LocalRegex(_) => 128_000,
        }
    }

    fn get_max_tokens_for_model_type(model_type: &str) -> usize {
        match model_type {
            model_type if model_type.starts_with("mistral:7b-instruct-v0.2") => 32_000,
            model_type if model_type.starts_with("mistral-nemo") => 128_000,
            model_type if model_type.starts_with("mistral-small") => 128_000,
            model_type if model_type.starts_with("mistral-large") => 128_000,
            model_type if model_type.starts_with("mixtral:8x7b-instruct-v0.1") => 16_000,
            model_type if model_type.starts_with("mixtral:8x22b") => 65_000,
            model_type if model_type.starts_with("llama3-gradient") => 256_000,
            model_type if model_type.starts_with("falcon2") => 8_000,
            model_type if model_type.starts_with("llama3-chatqa") => 8_000,
            model_type if model_type.starts_with("llava-phi3") => 4_000,
            model_type if model_type.starts_with("phi4") => 16_000,
            model_type if model_type.contains("minicpm-v") => 8_000,
            model_type if model_type.starts_with("dolphin-llama3") => 8_000,
            model_type if model_type.starts_with("command-r-plus") => 128_000,
            model_type if model_type.starts_with("codestral") => 32_000,
            model_type if model_type.starts_with("gemma2") => 8_000,
            model_type if model_type.starts_with("qwen2:0.5b") => 32_000,
            model_type if model_type.starts_with("qwen2:1.5b") => 32_000,
            model_type if model_type.starts_with("qwen2:7b") => 128_000,
            model_type if model_type.starts_with("qwen2:72b") => 128_000,
            model_type if model_type.starts_with("qwen2.5:72b") => 128_000,
            model_type if model_type.starts_with("qwen2.5:0.5b") => 32_000,
            model_type if model_type.starts_with("qwen2.5:1.5b") => 32_000,
            model_type if model_type.starts_with("qwen2.5:3b") => 32_000,
            model_type if model_type.starts_with("qwen2.5:7b") => 128_000,
            model_type if model_type.starts_with("qwen2.5:14b") => 128_000,
            model_type if model_type.starts_with("qwen2.5:32b") => 128_000,
            model_type if model_type.starts_with("qwen2.5:72b") => 128_000,
            model_type if model_type.starts_with("qwen2.5-coder") => 128_000,
            model_type if model_type.starts_with("aya") => 32_000,
            model_type if model_type.starts_with("wizardlm2") => 8_000,
            model_type if model_type.starts_with("phi2") => 4_000,
            model_type if model_type.starts_with("adrienbrault/nous-hermes2theta-llama3-8b") => 8_000,
            model_type if model_type.starts_with("llama-3.2") => 128_000,
            model_type if model_type.starts_with("llama3.3") => 128_000,
            model_type if model_type.starts_with("llama3.4") => 128_000,
            model_type if model_type.starts_with("llama-3.1") => 128_000,
            model_type if model_type.starts_with("llama3.1") => 128_000,
            model_type if model_type.starts_with("llama3") || model_type.starts_with("llava-llama3") => 8_000,
            model_type if model_type.starts_with("claude") => 200_000,
            model_type if model_type.starts_with("llama-3.3-70b-versatile") => 128_000,
            model_type if model_type.starts_with("llama-3.1-8b-instant") => 128_000,
            model_type if model_type.starts_with("llama-guard-3-8b") => 8_192,
            model_type if model_type.starts_with("llama3-70b-8192") => 8_192,
            model_type if model_type.starts_with("llama3-8b-8192") => 8_192,
            model_type if model_type.starts_with("mixtral-8x7b-32768") => 32_768,
            model_type if model_type.starts_with("gemma2-9b-it") => 8_192,
            model_type if model_type.starts_with("llama-3.3-70b-specdec") => 8_192,
            model_type if model_type.starts_with("llama-3.2-1b-preview") => 128_000,
            model_type if model_type.starts_with("llama-3.2-3b-preview") => 128_000,
            model_type if model_type.starts_with("llama-3.2-11b-vision-preview") => 128_000,
            model_type if model_type.starts_with("llama-3.2-90b-vision-preview") => 128_000,
            model_type if model_type.starts_with("llama-3.2") => 128_000,
            model_type if model_type.starts_with("llama3.3") => 128_000,
            model_type if model_type.starts_with("llama3.4") => 128_000,
            model_type if model_type.starts_with("llama-3.1") => 128_000,
            model_type if model_type.starts_with("llama3.1") => 128_000,
            model_type if model_type.starts_with("deepseek-r1:14b") => 128_000,
            model_type if model_type.starts_with("deepseek-r1:8b") => 128_000,
            model_type if model_type.starts_with("deepseek-r1:70b") => 128_000,
            model_type if model_type.starts_with("deepseek-v3") => 128_000,
            model_type if model_type.starts_with("command-r7b") => 128_000,
            model_type if model_type.starts_with("mistral-small") => 128_000,
            model_type if model_type.starts_with("qwq") => 32_000,
            model_type if model_type.starts_with("gemma3:1b") => 32_000,
            model_type if model_type.starts_with("gemma3:4b") => 128_000,
            model_type if model_type.starts_with("gemma3:12b") => 128_000,
            model_type if model_type.starts_with("gemma3:27b") => 128_000,
            model_type if model_type.starts_with("gemma3") => 128_000,
            _ => 4096, // Default token count if no specific model type matches
        }
    }

    /// Returns the maximum number of input tokens allowed for the given model,
    /// leaving room for output tokens.
    pub fn get_max_input_tokens(model: &LLMProviderInterface) -> usize {
        let max_tokens = Self::get_max_tokens(model);
        let max_output_tokens = Self::get_max_output_tokens(model) / 2;
        if max_tokens > max_output_tokens {
            max_tokens - max_output_tokens
        } else {
            max_output_tokens
        }
    }

    pub fn get_max_output_tokens(model: &LLMProviderInterface) -> usize {
        match model {
            LLMProviderInterface::OpenAI(openai) => {
                if openai.model_type.starts_with("o1-preview") || openai.model_type.starts_with("o1-mini") {
                    32768
                } else {
                    16384
                }
            }
            LLMProviderInterface::TogetherAI(_) => {
                if Self::get_max_tokens(model) <= 8000 {
                    2800
                } else {
                    4096
                }
            }
            LLMProviderInterface::LocalLLM(_) => {
                // Fill in the appropriate logic for LocalLLM
                4096
            }
            LLMProviderInterface::ShinkaiBackend(_) => {
                // Fill in the appropriate logic for ShinkaiBackend
                4096
            }
            LLMProviderInterface::Ollama(_) => {
                // Fill in the appropriate logic for Ollama
                if Self::get_max_tokens(model) <= 8000 {
                    2800
                } else {
                    4096
                }
            }
            LLMProviderInterface::Groq(_) => {
                // Fill in the appropriate logic for Ollama
                4096
            }
            LLMProviderInterface::Exo(_) => 4096,
            LLMProviderInterface::Gemini(_) => {
                // Fill in the appropriate logic for Ollama
                8192
            }
            LLMProviderInterface::OpenRouter(_) => {
                // Fill in the appropriate logic for OpenRouter
                if Self::get_max_tokens(model) <= 8000 {
                    2800
                } else {
                    4096
                }
            }
            LLMProviderInterface::Claude(claude) => {
                if claude.model_type.starts_with("claude-3-5-sonnet") {
                    8192
                } else {
                    4096
                }
            },
            LLMProviderInterface::DeepSeek(deepseek) => 8192,
            LLMProviderInterface::LocalRegex(_) => 128_000,
        }
    }

    /// Returns the remaining number of output tokens allowed for the LLM to use
    pub fn get_remaining_output_tokens(model: &LLMProviderInterface, used_tokens: usize) -> usize {
        let max_tokens = Self::get_max_tokens(model);
        let mut remaining_output_tokens = max_tokens.saturating_sub(used_tokens);
        remaining_output_tokens = std::cmp::min(
            remaining_output_tokens,
            ModelCapabilitiesManager::get_max_output_tokens(&model.clone()),
        );
        remaining_output_tokens
    }

    /// Counts the number of tokens from the list of messages
    pub fn num_tokens_from_messages(messages: &[LlmMessage]) -> usize {
        let average_token_size = 4; // Average size of a token (in characters)
        let buffer_percentage = 0.15; // Buffer to account for tokenization variance

        let mut total_characters = 0;
        for message in messages {
            total_characters += message.role.clone().unwrap_or_default().chars().count() + 1; // +1 for a space or newline after the role
            if let Some(ref content) = message.content {
                total_characters += content.chars().count() + 1; // +1 for spaces or newlines between messages
            }
            if let Some(ref name) = message.name {
                total_characters += name.chars().count() + 1; // +1 for a space
                                                              // or newline
                                                              // after the name
            }
        }

        // Calculate estimated tokens without the buffer
        let estimated_tokens = (total_characters as f64 / average_token_size as f64).ceil() as usize;

        // Apply the buffer to estimate the total token count
        let buffered_token_count = ((estimated_tokens as f64) * (1.0 - buffer_percentage)).floor() as usize;

        (buffered_token_count as f64 * 2.6).floor() as usize
    }

    /// Counts the number of tokens from the list of messages for llama3 model,
    /// where every three normal letters (a-zA-Z) allow an empty space to not be
    /// counted, and other symbols are counted as 1 token.
    /// This implementation avoids floating point arithmetic by scaling counts.
    pub fn num_tokens_from_llama3(messages: &[LlmMessage]) -> usize {
        let num: usize = messages
            .iter()
            .map(|message| {
                let role_prefix = match message.role.as_deref().unwrap_or("") {
                    "user" => "User: ",
                    "sys" => "System: ",
                    "assistant" => "Assistant: ",
                    _ => "",
                };
                let full_message = format!(
                    "{}{}\n",
                    role_prefix,
                    message.content.as_ref().unwrap_or(&"".to_string())
                );
                count_tokens_from_message_llama3(&full_message)
            })
            .sum();

        (num as f32 * 1.04) as usize
    }

    /// Returns whether the given model supports tool/function calling
    /// capabilities
    pub async fn has_tool_capabilities_for_provider_or_agent(
        provider_or_agent: ProviderOrAgent,
        db: Arc<SqliteManager>,
        stream: Option<bool>,
    ) -> bool {
        match provider_or_agent {
            ProviderOrAgent::LLMProvider(serialized_llm_provider) => {
                ModelCapabilitiesManager::has_tool_capabilities(&serialized_llm_provider.model, stream)
            }
            ProviderOrAgent::Agent(agent) => {
                let llm_id = &agent.llm_provider_id;
                if let Some(llm_provider) = db.get_llm_provider(llm_id, &agent.full_identity_name).ok() {
                    if let Some(model) = llm_provider {
                        ModelCapabilitiesManager::has_tool_capabilities(&model.model, stream)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }

    /// Returns whether the given model supports tool/function calling
    /// capabilities
    pub fn has_tool_capabilities(model: &LLMProviderInterface, _stream: Option<bool>) -> bool {
        eprintln!("has tool capabilities model: {:?}", model);
        match model {
            LLMProviderInterface::OpenAI(_) => true,
            LLMProviderInterface::Ollama(model) => {
                // For Ollama, check model type and respect the passed stream parameter
                model.model_type.starts_with("llama3.1")
                    || model.model_type.starts_with("llama3.2")
                    || model.model_type.starts_with("llama-3.1")
                    || model.model_type.starts_with("llama-3.2")
                    || model.model_type.starts_with("mistral-nemo")
                    || model.model_type.starts_with("mistral-small")
                    || model.model_type.starts_with("mistral-large")
                    || model.model_type.starts_with("mistral-pixtral")
                    || model.model_type.starts_with("qwen2.5-coder")
                    || model.model_type.starts_with("qwq")
                    || model.model_type.starts_with("gemma3")
                    || model.model_type.starts_with("deepseek-r1:14b")
                    || model.model_type.starts_with("deepseek-r1:8b")
                    || model.model_type.starts_with("deepseek-r1:70b")
                    || model.model_type.starts_with("deepseek-v3")
                    || model.model_type.starts_with("command-r7b")
                    || model.model_type.starts_with("mistral-small")
            }
            LLMProviderInterface::Groq(model) => {
                model.model_type.starts_with("llama-3.3-70b-versatile")
                    || model.model_type.starts_with("llama-3.1-8b-instant")
                    || model.model_type.starts_with("llama-guard-3-8b")
                    || model.model_type.starts_with("llama3-70b-8192")
                    || model.model_type.starts_with("llama3-8b-8192")
                    || model.model_type.starts_with("mixtral-8x7b-32768")
                    || model.model_type.starts_with("gemma2-9b-it")
                    || model.model_type.starts_with("llama-3.3-70b-specdec")
                    || model.model_type.starts_with("llama-3.2-1b-preview")
                    || model.model_type.starts_with("llama-3.2-3b-preview")
                    || model.model_type.starts_with("llama-3.2-11b-vision-preview")
                    || model.model_type.starts_with("llama-3.2-90b-vision-preview")
                    || model.model_type.starts_with("llama-3.2")
                    || model.model_type.starts_with("llama3.2")
                    || model.model_type.starts_with("llama-3.1")
                    || model.model_type.starts_with("llama3.1")
            }
            LLMProviderInterface::OpenRouter(model) => {
                model.model_type.starts_with("llama-3.2")
                    || model.model_type.starts_with("llama3.2")
                    || model.model_type.starts_with("llama-3.1")
                    || model.model_type.starts_with("llama3.1")
                    || model.model_type.starts_with("mistral-nemo")
                    || model.model_type.starts_with("mistral-small")
                    || model.model_type.starts_with("mistral-large")
                    || model.model_type.starts_with("mistral-pixtral")
            }
            LLMProviderInterface::Claude(_) => true,
            LLMProviderInterface::ShinkaiBackend(_) => true,
            LLMProviderInterface::Gemini(model) => {
                model.model_type.starts_with("gemini-pro")
                    || model.model_type.starts_with("gemini-pro-vision")
                    || model.model_type.starts_with("gemini-ultra")
                    || model.model_type.starts_with("gemini-ultra-vision")
                    || model.model_type.starts_with("gemini-1.5")
                    || model.model_type.starts_with("gemini-2.0")
            },
            LLMProviderInterface::DeepSeek(_) => true,
            _ => false,
        }
    }

    /// Returns whether the given model has reasoning capabilities
    pub fn has_reasoning_capabilities(model: &LLMProviderInterface) -> bool {
        match model {
            LLMProviderInterface::OpenAI(openai) => {
                openai.model_type.starts_with("o1")
                    || openai.model_type.starts_with("o2")
                    || openai.model_type.starts_with("o3")
                    || openai.model_type.starts_with("o4")
                    || openai.model_type.starts_with("o5")
            }
            LLMProviderInterface::Ollama(ollama) => {
                ollama.model_type.starts_with("deepseek-r1") || ollama.model_type.starts_with("qwq")
            },
            LLMProviderInterface::DeepSeek(deepseek) => {
                deepseek.model_type.starts_with("deepseek-reasoner")
            },
            _ => false,
        }
    }
}

// TODO: add a tokenizer library only in the dev env and test that the
// estimations are always above it and in a specific margin (% wise)
#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    // Helper function to convert a vector of ChatCompletionRequestMessage to a
    // single string
    fn messages_to_string(messages: &[LlmMessage]) -> String {
        messages
            .iter()
            .map(|message| {
                format!(
                    "{}: {} ({})",
                    message.clone().role.unwrap_or_default(),
                    message.content.as_ref().unwrap_or(&"".to_string()),
                    message.name.as_ref().unwrap_or(&"".to_string())
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    // #[test]
    fn test_num_tokens_from_messages_empty() {
        let messages: Vec<LlmMessage> = vec![];
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
        let messages = vec![LlmMessage {
            role: Some("user".to_string()),
            content: Some("Hello, how are you?".to_string()),
            name: Some("Alice".to_string()),
            function_call: None,
            functions: None,
            images: None,
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
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("Hello".to_string()),
                name: Some("Alice".to_string()),
                function_call: None,
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("bot".to_string()),
                content: Some("Hi there!".to_string()),
                name: Some("Bob".to_string()),
                function_call: None,
                functions: None,
                images: None,
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
        let messages = vec![LlmMessage {
            role: Some("user".to_string()),
            content: Some("Hello, how are you doing today? I hope everything is fine.".to_string()),
            name: Some("Alice".to_string()),
            function_call: None,
            functions: None,
            images: None,
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
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("You are an advanced assistant who only has access to the provided content and your own knowledge to answer any question the user provides. Do not ask for further context or information in your answer to the user, but simply tell the user as much information as possible using paragraphs, blocks, and bulletpoint lists. Remember to only use single quotes (never double quotes) inside of strings that you respond with.".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("user".to_string()),
                content: Some("tell me about Minecraft".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("Use the content to directly answer the user's question with as much information as is available. If the user talks about `it` or `this`, they are referencing the content. Make the answer very readable and easy to understand formatted using markdown bulletpoint lists and '\\n' separated paragraphs. Do not include further JSON inside of the `answer` field, unless the user requires it based on what they asked. Format answer so that it is easily readable with newlines after each 2 sentences and bullet point lists as needed:".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("Then respond using the following EBNF and absolutely nothing else: '{' 'answer' ':' string '}' ".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("```json".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
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
        let messages: Vec<LlmMessage> = content
            .chars()
            .collect::<Vec<char>>()
            .chunks(chunk_size)
            .map(|chunk| LlmMessage {
                role: Some("user".to_string()),
                content: Some(chunk.iter().collect::<String>()),
                name: Some("Alice".to_string()),
                function_call: None,
                functions: None,
                images: None,
            })
            .collect();
        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Converted messages: \"{}\"", messages_to_string(&messages));
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);

        // Check that the estimate is greater than the numbers below to ensure it over
        // estimates and not under
        assert!(num_tokens_llama3 > 28000);
        assert!(num_tokens > 34000);
    }

    // #[test]
    fn test_num_tokens_from_poker_probability_explanation() {
        let messages = vec![
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("Calculating the probabilities of winning in Texas Hold'em is a complex task that involves combinatorics and an understanding of the game's rules. Here's a simplified version of how you might approach writing code to calculate these probabilities in Python. This example assumes you have a function that can evaluate the strength of a hand and that you are calculating the probability for a specific point in the game (e.g., after the flop).".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("```python\nimport itertools\nimport random\n\n# Assume hand_strength is a function that takes a list of cards and returns a score indicating the strength of the hand.\n\ndef calculate_probabilities(player_hand, community_cards, deck):\n    wins = 0\n    iterations = 10000  # Number of simulations to run\n    for _ in range(iterations):\n        # Shuffle the deck and deal out the rest of the community cards plus opponent's cards\n        random.shuffle(deck)\n        remaining_community = deck[:5-len(community_cards)]\n        opponent_hand = deck[5-len(community_cards):7-len(community_cards)]\n        final_community = community_cards + remaining_community\n\n        # Evaluate the hands\n        player_score = hand_strength(player_hand + final_community)\n        opponent_score = hand_strength(opponent_hand + final_community)\n\n        # Compare the hands to determine a win\n        if player_score > opponent_score:\n            wins += 1\n\n    # Calculate the probability\n    probability = wins / iterations\n    return probability\n\n# Example usage:\n# Assuming the deck is a list of all 52 cards, player_hand is the player's 2 cards, and community_cards are the cards on the board\n# deck = [...]\n# player_hand = [...]\n# community_cards = [...]\n# win_probability = calculate_probabilities(player_hand, community_cards, deck)\n# print('Winning Probability:', win_probability)\n```".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
            LlmMessage {
                role: Some("system".to_string()),
                content: Some("The function `calculate_probabilities` runs a Monte Carlo simulation to estimate the winning probability for a player's hand against a single opponent. Note that in a real poker game, you would need to evaluate against multiple opponents, deal with incomplete information, and dynamically adjust as the community cards are revealed. For a more accurate probability calculation, consider factors like the number of players, their ranges, and how the hand could develop with future community cards".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            },
        ];

        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Converted messages: \"{}\"", messages_to_string(&messages));
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);
    }

    // #[test]
    fn test_num_tokens_for_summarize_prompt() {
        let messages = vec![LlmMessage {
            role: Some("user".to_string()),
            content: Some(
                "Summarize this: CARING FOR PLANTS Thinking Like A Plant One way to come up with \
                good research questions and to care for your plants well is to try thinking about \
                what it’s like to be a plant. Plants are sessile: they can determine which direction \
                to grow in but are otherwise stuck in place. Even though plants can’t move around, \
                they must still survive the challenges of weather, air, soil, amount and quality of \
                water, neighboring plants, diseases, and many kinds of herbivores, both above ground \
                and below. I wish it would rain! Above ground, most tissues help the plant collect \
                light for photosynthesis. Stems, woody trunks, and branches lift some plants above \
                their neighbors to place their leaves in the best and brightest sunlight. Other plants \
                have leaves that are adapted to use lower amounts of sunlight. Leaves, and sometimes \
                stems, contain pores called stomata that let carbon dioxide into the plant for \
                photosynthesis and let extra oxygen out. Stomata also allow water vapor out of a plant, \
                and when it is hot, dry, or windy, plants can close their stomata to limit water loss. \
                A plant’s underground world affects its survival as well. Most land plants take up nearly \
                all of their water from the soil. Plant roots also take up inorganic nutrients such as \
                nitrogen, potassium, phosphorous, and about a dozen micronutrients, elements that plants \
                need in very small doses for survival and growth. Some plants can grow in soil containing \
                a lot of salt, very high or very low pH, or large amounts of metals toxic to other organisms. \
                Most plants, however, are sensitive to these and other factors – their roots may take up \
                whatever gets put into the soil, directly or by the flow of contaminated groundwater. \
                Thought Exercise: Based on the above information, what environmental factors do you think \
                can help a plant grow well? What factors might prevent it from growing well? Does it depend \
                on the species of the plant? Many organisms affect plants in ways that can be helpful or \
                harmful. Plants with showy flowers usually depend on pollinators to make seeds. Most plants \
                can form a symbiosis with fungi, providing carbon in exchange for mineral nutrients like \
                phosphorus. Legumes, like beans and peas, form symbioses with bacteria that can directly \
                provide nitrogen to the plant. Earthworms can aerate dense soil as they move, making it easier \
                for roots to grow through it. Earthworms also help break down organic matter to release soil \
                nutrients, making them easier for plants to take up. In contrast, herbivores and parasites may \
                attack and eat plants above or belowground. Plants also get diseases, although the bacteria and \
                viruses that cause plant infections are very different from those that cause illness in humans. \
                To help ward off these dangers, plants have several types of defenses. Sometimes these defenses, \
                which include poisons and sharp thorns can be harmful to humans. You can learn more about these \
                defenses and how to limit your risk of being harmed by the plants you work with in Investigating \
                Plants Safely. Thought experiment: What are some ways we can limit the presence of organisms that \
                damage or sicken plants? If we use these methods, will we affect the organisms that are helpful to \
                plants? What Do Plants Need to Survive? overwatering them! Plants can be drowned, just like animals, \
                if their roots are underwater and cut off from oxygen too long. Plants Need Water: The main type of \
                care plants need to grow well is regular watering. In nature, plants depend on rainfall for their \
                water, so the local climate affects which plants will be able to survive and reproduce in the wild. \
                In farm fields, gardens, and indoors, humans can provide plants with water. Even so, the leading \
                cause of death in houseplants is actually What Do Plants Need to Survive? More is Not Always Better! \
                are growing a plant indoors that depends on an insect or animal for pollination, you will have to \
                pollinate it by hand. If you will be growing plants through a full life cycle, it is important to \
                know how they reproduce. You may find it difficult to get seeds from plants that mainly reproduce \
                asexually, but it might be much easier to get tubers, stolons, or plantlets. If your plant is \
                wind-pollinated, placing a fan near the flowers may be enough to cause pollination and set seed \
                indoors. If you grow both roots and shoots at new nodes. Some plants produce underground storage \
                structures, like garlic bulbs or potato tubers, which may later grow roots and shoots. entirely \
                without seeds. They may not flower at all, but instead produce plantlets along leaf edges that \
                can break off and root in the place where they fall. Plants like strawberries can reproduce through \
                stolons, stems that grow along the ground occasionally Growing Your Own Seeds: Plants survive from \
                generation to generation by both sexual and asexual reproduction. Some flowers can be wind \
                pollinated, while others need insects or birds to transfer pollen and fertilize the ovules. \
                Fertilized ovules develop into seeds, which can be dispersed in an incredible number of ways. In \
                contrast, some plants reproduce asexually – partly or Thought Exercise: Why do you think that plants \
                need more nitrogen, phosphorus, and potassium compared to most other nutrients? What micronutrients \
                do you think plants might need? fertilizers now available can act as the basis for a wide range of \
                experiments! artificial fertilizers, so they can help support beneficial soil fungi and bacteria. \
                Chemical fertilizers can release nutrient levels that become toxic to these organisms. Common \
                examples of organic fertilizers include compost, fish fertilizer, and bone meal. Compost and fish \
                fertilizer tend to be high in nitrogen, while bone meal is high in phosphorus. The large variety of \
                Organic fertilizers also add nutrients to the soil, but the quantities of these nutrients are often \
                not as precisely known. They can still be described in N- P-K ratios, however. Whether stated in the \
                product or not, most contain a variety of micronutrients, because organic fertilizers come from \
                biological sources. They have the advantage of containing more carbon than most experiments lasting \
                a few days to a few weeks, though, an “all purpose” fertilizer or one with a 10-10-10 formulation will \
                usually be fine. The package should have instructions telling how much to use for your plants and how \
                often. micronutrients, but it can take a keen eye to figure out where on the package to learn which \
                ones and how much of each are present. Special, fine- tuned fertilizer formulations for flowers, \
                vegetables, and even African violets are now available for purchase. Some brands have one type of \
                fertilizer for a plant’s growth phase and another type for flowering and fruiting. For Chemical \
                fertilizers may provide only the major plant nutrients of nitrogen, phosphorus, and potassium. As \
                shorthand, bags of fertilizer are often sold with “N-P-K” values listed on the front. A listing of \
                5-1-1, for example, means that five units of nitrogen and one each of phosphorus and potassium are \
                found in a given amount of fertilizer. Some fertilizers also contain weeks or months. Time- release \
                fertilizers are useful for experiments lasting more than six weeks, while shorter experiments can \
                use liquid or soluble fertilizers. Many kinds of fertilizer can be used for indoor and outdoor \
                gardening. The two basic categories are chemical and organic fertilizers. Chemical fertilizers will \
                provide very precise quantities of known nutrients to the soil. They may come in a liquid form, a \
                solid that you dissolve in water, or as sticks or pellets that you place into the soil to release \
                nutrients over several plant has more nutrients available than it needs! Very short experiments, \
                like those that will take one hour to three days of class, will not need any fertilizer. Little growth \
                occurs over that amount of time. On the other hand, experiments lasting a week or longer will benefit \
                from a fertilizer if it is not already mixed into the soil. Roots Take Up Nutrients: Nutrients support \
                plant growth and development, and they can be provided in a fertilizer. Whether or not you need to use \
                fertilizer depends on several factors. Some potting soils already include fertilizer. You will not need \
                to add any extra fertilizer to grow plants in this type of soil unless you plan to carry out an experiment \
                on what happens when a unexplored territory. The best solution is to transplant the plant to a larger pot. \
                If you decide to transplant your plant, remember to be gentle – you want to avoid removing any roots. After \
                you are done transplanting, water the plant in its new, larger container right away. You should limit other \
                environmental changes for a few days to help prevent transplant shock. Finally, for experiments that will \
                take longer than two months, you should think about whether the plants may become rootbound in their pots. \
                Plant roots continue growing as long as a living plant is not dormant. For fast- growing or large plants, \
                roots can begin to grow in circles around the pot edges or even peek out of the holes on the bottom, \
                searching for routes to one or more drainage holes in the bottom. These let out any extra water from rain or \
                hand-watering. Next, since roots respire, it is important that oxygen be able to reach them. Mixing some \
                light, fluffy material like vermiculite or perlite with the darker organic matter makes potting soil more \
                porous. These lighter materials can act as pools of root-sustaining oxygen even when the soil is soaking wet. \
                To prevent roots from drowning, it is also important to use pots that have between each watering. helpful. \
                Soil is a bit hydrophobic until it absorbs a little water, so putting dry soil into a pot and adding water \
                from the top may leave dry spots below the surface. Adding just enough water to the soil so that it has a \
                spongy, cake-like consistency will make the soil just wet enough to easily absorb liquid during top-watering, \
                even if the top layer of soil dries completely One of the easiest ways to grow a plant indoors is to put some \
                potting soil into a container and then bury the plant roots (or a seed) in the soil. This sounds simple, but \
                a few small tricks can help support the plant or seed early on, easing transplant shock or allowing a seed to \
                sprout more quickly. First, mixing some water into the soil before placing it in a pot can be new places \
                underground, discovering new sources of water and nutrients. If you test different types of rooting materials \
                to see what sorts of things roots can grow through, you may be surprised at what plants can survive on! bottom \
                of a pond, while others may creep into the crevices in bare rock! If you have ever looked closely, you may have \
                seen plants rooted in discarded cloth or in the tiny “rocks” of a softball field. While roots hold a plant in \
                place, they also explore new territory as they grow. Even when a plant does not appear to be growing above \
                ground, plant roots may be growing into Plant Roots Need to Explore: Plants can root themselves in a surprising \
                variety of materials. Crop plants can usually grow in sandy or clay-filled soils, even if they grow best in \
                loamy soils. Some wild species grow roots in the muck at the Electricity flows to the light when the timer is \
                in the light period, but it is cut off during the dark period. (<10 h of light). If you use only natural \
                lighting, the time of year that you carry out an experiment can affect whether or not a plant blooms. \
                Artificial lighting can be used to change the photoperiod if you add a timer. Most light timers can be plugged \
                into an outlet, then set to turn on and off at specific times. The artificial light source is then plugged into \
                the timer. Plants also adjust their growth and time their flowering based on the time of day and time of year. \
                They can sense the number of hours of light in each day – the photoperiod. Providing the same species of plant \
                with two different photoperiods can give very different results. For example, some plants require long days (>14 \
                h of light) to flower, while others need short days preferences. You might want to test different types of \
                lighting to see if any differences are big enough to affect your favorite species. Thought Exercise: Which \
                colors of light do you think might cause the best growth in your favorite type of plant? Do you think growth is \
                affected directly by light color, or indirectly through another process? How might you test your ideas? varies \
                in cost, energy efficiency, bulb life, heat produced, and spectrum. Regular home or office incandescent or \
                fluorescent bulbs have different spectra from light bulbs made for plant growth, so they may not be as effective \
                for growing plants. Plants can also vary strongly in their lighting The color spectrum of artificial lighting \
                can influence how well a plant grows indoors. The spectrum used to measure PPFD runs from 400 – 700 nm, from \
                near-UV to far-red, because plants only use this range of the spectrum. Artificial plant lighting comes in \
                several varieties, including full-spectrum fluorescent bulbs, metal halide bulbs, and colored LED lighting. \
                Each type artificial lighting, such as 200 µmol/m2/s. Houseplants are often adapted to very low light intensity, \
                but they may grow better with some extra light. Full sunlight in summer at noon has a PPFD of about 2000 µmol \
                photons/m2/s, varying based on how far from the equator it is measured. Plants that grow in full sun, such as \
                crop plants, benefit from high PPFD and need artificial lighting to grow their best indoors. We need only about \
                20-50 µmol/m2/s to see well, so even shade-loving plants benefit from lower-intensity Plants Need Light: Light \
                is critical for plants to make sugars through photosynthesis. Plants also use light to judge the time of day and \
                year. For these reasons, lighting can influence plant growth, dormancy, and flowering. Light intensity, usually \
                measured as photosynthetic photon flux density (PPFD), is a measure of how much incoming light is available for \
                photosynthesis. Water from a kitchen or lab faucet is clean enough for the majority of plants. • Potted plants \
                should be given enough water so that the soil is fully soaked and some water runs out the bottom of the pot. \
                Empty out any extra water from the dish beneath the pot after watering. • The plant should usually not be watered \
                again until the top inch of soil is dry to the touch. It wasn’t long before I noticed the leaves had begun to \
                wilt and no new flowers were budding. If the leaves are wilting, I thought, the geranium must need more water! \
                I added another watering just before bedtime. Unfortunately, the extra water didn’t help. By the end of the month \
                the plant was dead! A few tips can help you keep your plants healthy as you water them: “OK,” I said, “I’ll start \
                right away.” I immediately watered the plant. The first thing I did the next morning was water it again, and I \
                re-watered it that night after supper. For the next several weeks I continued this tender loving care, watering \
                the geranium every morning and every night. I gave my mother a potted geranium last Mother’s Day. It had lots of \
                big red flowers standing tall above dozens of thick, velvety leaves. It was beautiful, and she really liked it. \
                “This is such a lovely gift,” she said. “Why don’t you take care of it for me?” More is Not Always Better! The Next \
                Best Thing to Nature…is Nature? Botanists have long used small paintbrushes to move pollen from one flower to \
                another. This kind of tool is important for controlled crosses, a breeding method in which both parents are \
                chosen to produce seed. More recently, some botanists began to try a more “natural” tool for hand-pollinating \
                flowers. The bodies of dead honeybees can be made into bee sticks by attaching them to small handles, such as \
                toothpicks. Bee sticks give better results than paintbrushes. In fact, they work so well that astronauts have used \
                them to pollinate plants during experiments in space! Different Species Have Different Tastes: different \
                environments and have very different watering needs. Looking up basic information on You now have some general \
                guidelines on how to care for plants. Always keep in mind, though, that different species have evolved differently \
                and thrive in environments that may be very different from each other. Caring for a cactus the way that you would \
                care for a tomato plant will most likely kill the cactus, and the reverse is also true! Tomatoes and cacti are \
                adapted to cultivation of or, for wild plants, the habitat of the plant you plan to work with can be a great way to \
                fine-tune the guidelines described here. Environmental Treatments for Plant Experiments should a treatment mixture \
                be to mimic beach sand or seawater? How do you measure how salty a liquid is? Far from shore, most plants cannot \
                tolerate much sodium in the soil. Even high levels of plant nutrients like potassium can “burn” plant leaves. You \
                may want to carry out a salt tolerance experiment on seaside or inland plants, but figuring out how to treat your \
                plants is tricky. Should you mix table salt with the potting soil? Spray salt water on the leaves? How salty \
                salinity both on their leaves and in the soil in which they have rooted. Some plants can even use the ocean as a \
                primary water source! Changing Soil Salinity: If your family has ever spent time at the ocean shore, you may have \
                noticed that your car was covered with a fine layer of salt when you returned. Ocean spray can pass through the air \
                as a fine mist and deposit salts in the surrounding environment as the water evaporates. If plants are to survive \
                near the ocean, they must be able to deal with this high Thought Exercise: Your class may not have a light meter \
                available. What other ways could you measure or estimate the amount of light in your treatments? Your class might \
                have a light meter you can use to measure the intensity of the light that reaches the plants. This is the best way \
                of knowing exactly how much shade you created if you don’t have additional information about your filters or if you \
                use cloth to limit the light intensity. method, it is important to keep the cloth away from the light source so that \
                there is no risk of it catching on fire from hot light bulbs or a rare electrical spark. compared to the “full sun” \
                treatment. Colored filters are also available if you wish to test the effect of different light wavelengths on plant \
                growth. You could also use white, gray, or black cotton cloth to reduce the amount of light that reaches the plant. \
                Folding over the cloth to form layers will more strongly limit the light intensity the plant gets. With this \
                Adjustable-intensity plant lighting does exist, but it is usually costly. Less expensive alternatives might be useful. \
                For example, filters for theater lighting could be easily placed between the light source and your plants. These \
                filters are carefully designed to allow a certain percentage of light to pass through, so you will know precisely how \
                much shade you have created Instead of growing some plants under LED lights and others under regular classroom \
                lighting, a better experiment would grow all plants under the same type of LED lights, but change the intensity of \
                one set of lights. Creating Shade for Plants: Reducing the light intensity that plants receive is simpler than \
                creating total darkness. This would be an indoor way to test the difference between a sunny climate and a cloudy one, \
                or compare an open prairie to the forest floor. In a well-designed experiment, the light source for all plants should \
                ideally be the same even if the intensity differs. turning out the classroom lights before opening a closet or closed \
                box to check on the plants would also limit their light exposure. experiment on extremely short photoperiods instead \
                of darkness! Therefore, you need to think about how to limit disruptions to your darkness treatment, and to your \
                controls, during such an experiment. Giving all of the plants a little bit extra water at the start may let you wait \
                an extra day or two before exposing the plants to light for the next watering. With permission, How long plants are \
                exposed to light also matters. Placing your plants in the closet will minimize the amount of light they can use, \
                especially if nobody opens the closet door during the entire experiment. If the closet is opened twice each class \
                period to find and store classroom materials, though, the plants will be regularly exposed to light. You may end up \
                carrying out an If you wish to darken only some parts of a plant, wrapping individual leaves or stems in aluminum foil \
                may be one useful approach. However, allowing gas exchange in these leaves is still important. Making small “bags” \
                of black plastic around the plant parts you wish to darken may allow better air flow while still effectively blocking \
                out light. plant inside a thick-walled cardboard box and use electrical tape to block all gaps, holes, and weak or \
                broken edges. Scientists use multiple layers of aluminum foil to wrap a clear container or drape several folds of \
                black velvet cloth over a frame to block light from an experiment. To create total darkness, you cannot simply put \
                your plants in a closet in a room where lights are used, because light will often shine underneath the door. Even this \
                tiny amount of light can be enough to signal to plants the direction they should grow to carry out more photosynthesis, \
                or to tell a seedling it has broken above the soil surface. Instead, you could place your this reason, it is important \
                to think about how dark your experimental treatment needs to be. This is based on the question you are testing. If you \
                need to make sure that absolutely no light reaches the plant, the way you create darkness will be different than what \
                you would do to treat a plant with minimal light. How Dark is Dark? Since photosynthesis is the main way a plant \
                produces food, one factor you might want to test is removing the energy for photosynthesis – light. Because light is \
                so critical to their survival, plants can be extremely sensitive to even very short durations or very low intensities \
                of light. Young seedlings can be as light-sensitive as photographic film! For also have trouble figuring out ways to \
                set up the experimental treatment you want to test. In this section, several ideas are presented for creating certain \
                environmental treatments and telling how they compare to what plants must survive in the wild. You may already have \
                ideas about an environmental condition you wish to test. If it’s a condition such as the amount of water or kind of \
                fertilizer, you now have some information about how a control treatment might be set up differently from an \
                experimental treatment. However, a few plant species might respond to environmental factors that have not yet been \
                mentioned. You may Environmental Treatments for Plant Experiments Irrigation Can Make Agriculture Salty! condition \
                in a biologically meaningful way, helpful resources may include the website for your state agricultural extension \
                office, gardening books and websites, or textbooks on biology, botany, and horticulture. Other Environmental \
                Treatments: Plants can be sensitive to a surprising array of environmental conditions. Temperature and humidity \
                can be relatively easy to change, for example. Such factors might be more likely to affect specialized species, \
                such as alpine or desert plants. If you are looking for ideas for a condition or species to test, or how you can \
                change an environmental lower soil pH can be made using iron sulfate or peat moss, although these will be more \
                useful in an experiment lasting over six weeks. About 300 g/m2 of iron sulfate will reduce the pH by about 0.5 \
                units, as will 60 g/m2 of peat moss. 100 grams per square meter of soil, or g/m2) is often used to increase \
                both soil pH and potassium content. Pulverized agricultural lime, which consists of calcium and magnesium \
                carbonate, is another common choice; 170 g/m2 will raise the pH approximately 0.5 units. A rain! Nitric or \
                sulfuric acids are better choices if you wish to test the effect of acid rain on plants indoors. Alternatively, \
                soil amendments can be added to potting soil to change its pH. Just as hydrochloric acid is not the most realistic \
                choice for lowering pH, few gardeners would add potassium hydroxide pellets to raise the pH of an acidic soil. \
                Instead, wood ash (up to Theoretically, the pH of a solution can be changed for an experiment by adding any acid \
                or a base to the normal watering solution. Some acids and bases are better to use than others, though. Adding a \
                few drops of hydrochloric acid and monitoring the result with a pH meter will allow you to precisely lower the \
                pH of a watering solution to 4.0, but this is not the cause of acid take up other elements. Acid soils make \
                toxic aluminum ions more available to plants, while alkaline soils make it difficult for roots to take up enough \
                iron. An unfavorable pH can therefore limit or provide an excess of some nutrients to plants. between 6.5 and 7.0, \
                while radishes prefer a soil pH between 6.0-6.5. Blueberries, which often grow wild in bogs, prefer even more \
                acidic soils and thrive in a soil pH of 4.0-5.0. Other plants, such as lilacs, prefer alkaline soils of pH 7.0-8.0. \
                Soil pH affects plants directly through the concentration of protons in the soil, but it can also change how easy \
                it is for roots to Tap water is healthy for most plants, and plants have survived for millennia on rainwater. Based \
                on this, do you think that a soil pH between 5.6 and 7.0 is well suited for plant growth? This is true for many \
                crops and garden plants. As for other environmental conditions, though, each species has a specific pH range \
                preference. For example, asparagus grows best with a soil pH water has a pH of about 7.0, while clean rainwater \
                contains some dissolved carbon dioxide and has a pH near 5.6. Acid rain contains dissolved sulfuric acid and \
                nitric acid from human and natural sources, so it has an even lower pH – 4.0 is typical, although some \
                thunderstorms in urban parts of the East Coast have had a pH as low as 2.0. Changing Soil pH: To measure how \
                acidic or basic a solution is, scientists use pH. A pH of 7 is considered to be neutral. A lower pH, between 0 \
                and 7, indicates greater acidity. A higher pH, up to 14, indicates greater alkalinity (basicity). Each one-unit \
                decrease of pH indicates the presence of ten times as many hydrogen ions or protons (H+) in a given volume of \
                solution. Pure houseplants’ or garden plants’ needs. In contrast, sodium is a plant micronutrient, so high \
                concentrations are difficult for plants to tolerate. To test the effects of salinity, scientists usually water \
                plants with different concentrations of salt water. Sodium chloride can be added to a regular fertilizer \
                solution, for example. While the fertilizer itself will also increase the solution’s conductivity, the nitrogen, \
                phosphorus, and potassium levels in the fertilizer are designed to be compatible with most water in the lab has \
                a conductivity of about 6-8 µS/m. Many garden stores sell handheld conductivity meters, which can often measure \
                pH as well. Measuring conductivity directly is useful if you wish to test the effects of salinity but expect \
                evaporation during an experiment lasting several days or weeks. most tap water has a salinity of 5-50 mS/m, \
                and distilled, deionized Salinity can be quantified as parts per thousand (‰), or the number of grams of salt \
                per kilogram of water. Today, scientists usually describe salinity in terms of a liquid solution’s ability to \
                conduct electricity, which is measured in Siemens per meter (S/m). Ocean water, which is salty enough to kill \
                most inland plants, has a salinity of 5 S/m, or 35‰. In contrast, high salt tolerance. Much of the land used \
                for agriculture is irrigated to ensure that crops have a consistent supply of water as they grow. In very dry \
                climates, some of this water quickly evaporates, leaving behind trace amounts of salt in the soil. Over many \
                seasons, regular irrigation can lead to soil salinization. As a result, some plant breeders are now trying to \
                develop crop varieties with Irrigation Can Make Agriculture Salty! Additional Resources GLOBAL%7Cplant&utm_medium=NLC&utm_source=NSNS&utm_content=Plant \
                Rooted in Experience: The Sensory World of Plants, by Daniel Chamovitz. This New Scientist article series shows \
                that plants can sense changes in their environment using abilities similar to human sight, touch, smell, taste, \
                and hearing. A subscription is required to access the full articles. http://www.newscientist.com/special/plant-senses?cmpid=NLC%7CNSNS%7C2012-2708- \
                Prevent Transplant Shock, by Kathy LaLiberte for Gardener’s Journal. This page gives written tips and photos on \
                preventing and identifying root-bound plants and transplant shock. http://blog.gardeners.com/2010/05/prevent-transplant-shock/ Pollinating, by the \
                Wisconsin FastPlants® Program. This page gives written and visual instructions on making bee sticks and using \
                them for pollinating rapid-cycling Brassica flowers. http://www.fastplants.org/how_to_grow/pollinating.php Plants, by Nelson Thornes. This website \
                contains several menus outlining basic information about plant anatomy and function. The site is no longer \
                maintained, but it is a good introduction to plants. http://www.nelsonthornes.com/secondary/science/scinet/scinet/plants/gloss/content.htm \
                Modifying Soil pH, by Laura Ducklow and Daniel Peterson. This University of Minnesota website describes ways that \
                gardeners can increase or decrease soil pH. http://www.sustland.umn.edu/implement/soil_ph.html How Do Different \
                Color Filters Affect Plant Growth? By UCSB ScienceLine. Three different scientists answer the question of how \
                light color influences plants. http://scienceline.ucsb.edu/getkey.php?key=3155 Web Pages: Arizona Master Gardener \
                Manual, by the University of Arizona College of Agriculture’s Cooperative Extension. This online book is a good \
                example of a state-specific resource for learning about what plants need to thrive. http://ag.arizona.edu/pubs/garden/mg/ The Basics About Light, by \
                PhilipsHorticulture. This video describes the role of light in plant growth. A comparison of light detection by \
                human eyes and light absorption by plants begins a little bit past the halfway point. http://www.youtube.com/watch?v=eaSIq9c14YE How to Re-Pot a \
                Plant, by HowdiniGuru. Learn the basics of how to transplant a houseplant! The same ideas can be transferred to \
                much smaller plants and pots. http://www.youtube.com/watch?v=67r-RFN0nho How to Grow Plants from Seed: Prepare \
                Soil for Seed Planting, by ExpertVillage. This brief video, part of a broader series on growing seeds, shows the \
                proper moisture level for potting soil. http://www.youtube.com/watch?v=fH_3aeyTnh0 Videos and Visual Resources: \
                Bee Sticks, by Kristin Malock. A short, funny video that shows elementary school students using bee sticks to \
                pollinate their FastPlants®. http://vimeo.com/40114205 Books and Articles: Abram, D., and K. Abram. 1991. Growing \
                Plants from Seed. New York, New York: Lyons Press. 224 pp. Lee, D.W. and E. von Wettberg. 2010. Using bottles to \
                study shade responses of seedlings and other plants. Plant Science Bulletin 56(1): 23-30. plants. Plant Science \
                Bulletin 56(1): 23-30. Portland, Oregon: Timber Press. 236 pp.".to_string()),
                name: None,
                function_call: None,
                functions: None,
                images: None,
            }];

        let num_tokens = ModelCapabilitiesManager::num_tokens_from_messages(&messages);
        let num_tokens_llama3 = ModelCapabilitiesManager::num_tokens_from_llama3(&messages);
        println!("Number of tokens calculated: {}", num_tokens);
        println!("Number of tokens calculated for llama3: {}", num_tokens_llama3);
        assert!(num_tokens > 13000);
        assert!(num_tokens_llama3 > 13000);
    }
}
