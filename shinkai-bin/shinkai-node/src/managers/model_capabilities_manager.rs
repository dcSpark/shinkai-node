use crate::llm_provider::{
    error::LLMProviderError,
    providers::shared::{openai_api::openai_prepare_messages, shared_model_logic::llama_prepare_messages},
};
use ai_model_catalog::{get_openrouter_model, providers::openrouter};
use shinkai_message_primitives::{
    schemas::{
        llm_message::LlmMessage,
        llm_providers::{
            common_agent_llm_provider::ProviderOrAgent,
            serialized_llm_provider::{LLMProviderInterface, SerializedLLMProvider},
        },
        prompts::Prompt,
        shinkai_name::ShinkaiName,
    },
    shinkai_utils::utils::count_tokens_from_message_llama3,
};
use shinkai_sqlite::SqliteManager;
use std::{
    fmt,
    sync::{Arc, Weak},
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
    AudioAnalysis,
    VideoAnalysis,
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
                "gpt-5" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-5-mini" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-5-nano" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-5-chat-latest" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-4o" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-4o-mini" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-4.1-nano" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-4.1-mini" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "gpt-4.1" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "o1-mini" => vec![ModelCapability::TextInference],
                "gpt-3.5-turbo-1106" => vec![ModelCapability::TextInference],
                "gpt-4-1106-preview" => vec![ModelCapability::TextInference],
                "gpt-4-vision-preview" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "4o-preview" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "4o-mini" => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
                "dall-e-3" => vec![ModelCapability::ImageGeneration],
                "o3-mini" => vec![ModelCapability::TextInference],
                "gpt-4o-audio-preview" => vec![ModelCapability::TextInference, ModelCapability::AudioAnalysis],
                model_type if model_type.starts_with("o3") => {
                    vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference]
                }
                model_type if model_type.starts_with("o4-mini") => {
                    vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference]
                }
                model_type if model_type.starts_with("gpt-3.5") => vec![ModelCapability::TextInference],
                _ => vec![ModelCapability::TextInference], // Default to text inference for all other OpenAI models
            },
            LLMProviderInterface::TogetherAI(togetherai) => match togetherai.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => vec![ModelCapability::TextInference],
                "yorickvp/llava-13b" => vec![ModelCapability::ImageAnalysis],
                model_type if model_type.starts_with("togethercomputer/llama-2") => {
                    vec![ModelCapability::TextInference]
                }
                _ => vec![],
            },
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => {
                match shinkai_backend.model_type().to_uppercase().as_str() {
                    "FREE_TEXT_INFERENCE" | "STANDARD_TEXT_INFERENCE" | "PREMIUM_TEXT_INFERENCE" => {
                        vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference]
                    }
                    "CODE_GENERATOR" | "CODE_GENERATOR_NO_FEEDBACK" => {
                        vec![ModelCapability::TextInference]
                    }
                    _ => vec![],
                }
            }
            LLMProviderInterface::Ollama(model) => Self::get_shared_capabilities(model.model_type().as_str()),
            LLMProviderInterface::Exo(model) => Self::get_shared_capabilities(model.model_type().as_str()),
            LLMProviderInterface::Groq(model) => Self::get_shared_capabilities(model.model_type().as_str()),
            LLMProviderInterface::Gemini(gemini) => Self::get_gemini_capabilities(gemini.model_type.as_str()),
            LLMProviderInterface::OpenRouter(model) => Self::get_shared_capabilities(model.model_type().as_str()),
            LLMProviderInterface::Grok(grok) => {
                if grok.model_type.starts_with("grok-2-vision") {
                    vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
                } else if grok.model_type.starts_with("grok-4") {
                    vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
                } else {
                    vec![ModelCapability::TextInference]
                }
            }
            LLMProviderInterface::Claude(_) => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
            LLMProviderInterface::DeepSeek(_) => vec![ModelCapability::TextInference],
            LLMProviderInterface::LocalRegex(_) => vec![ModelCapability::ImageAnalysis, ModelCapability::TextInference],
        }
    }

    fn get_shared_capabilities(model_type: &str) -> Vec<ModelCapability> {
        match model_type {
            model_type if model_type.starts_with("llama3") => vec![ModelCapability::TextInference],
            model_type if model_type.starts_with("mistral-small3.2") || model_type.starts_with("mistral-small3.1") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            model_type if model_type.starts_with("llama3.2-vision") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
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
            model_type if model_type.starts_with("qwen2.5vl") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            model_type if model_type.starts_with("devstral") => {
                vec![ModelCapability::TextInference]
            }
            model_type if model_type.starts_with("magistral") => {
                vec![ModelCapability::TextInference]
            }
            model_type if model_type.starts_with("gemma3n") => {
                vec![ModelCapability::TextInference]
            }
            model_type if model_type.starts_with("gemma3") => {
                if model_type.contains(":1b") {
                    vec![ModelCapability::TextInference]
                } else {
                    vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
                }
            }
            model_type if model_type.starts_with("regex") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            _ => vec![ModelCapability::TextInference],
        }
    }

    fn get_gemini_capabilities(model_type: &str) -> Vec<ModelCapability> {
        match model_type {
            // Gemini 2.5 models
            model_type if model_type.starts_with("gemini-2.5-flash-preview-tts") => {
                vec![ModelCapability::TextInference]
            }
            model_type if model_type.starts_with("gemini-2.5-pro-preview-tts") => {
                vec![ModelCapability::TextInference]
            }
            model_type if model_type.starts_with("gemini-2.5-flash-preview-native-audio") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::AudioAnalysis,
                    ModelCapability::VideoAnalysis,
                ]
            }
            model_type if model_type.starts_with("gemini-2.5-flash-exp-native-audio") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::AudioAnalysis,
                    ModelCapability::VideoAnalysis,
                ]
            }
            model_type if model_type.starts_with("gemini-2.5-flash-image-preview") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::ImageAnalysis,
                    ModelCapability::ImageGeneration,
                ]
            }
            model_type if model_type.starts_with("gemini-2.5-flash") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::ImageAnalysis,
                    ModelCapability::VideoAnalysis,
                    ModelCapability::AudioAnalysis,
                ]
            }
            model_type if model_type.starts_with("gemini-2.5-pro") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::ImageAnalysis,
                    ModelCapability::VideoAnalysis,
                    ModelCapability::AudioAnalysis,
                ]
            }
            // Gemini 2.0 models
            model_type if model_type.starts_with("gemini-2.0-flash-preview-image-generation") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::ImageAnalysis,
                    ModelCapability::ImageGeneration,
                    ModelCapability::VideoAnalysis,
                    ModelCapability::AudioAnalysis,
                ]
            }
            model_type if model_type.starts_with("gemini-2.0-flash-lite") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::ImageAnalysis,
                    ModelCapability::VideoAnalysis,
                    ModelCapability::AudioAnalysis,
                ]
            }
            model_type if model_type.starts_with("gemini-2.0-flash") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::ImageAnalysis,
                    ModelCapability::VideoAnalysis,
                    ModelCapability::AudioAnalysis,
                ]
            }
            // Gemini 1.5 models
            model_type if model_type.starts_with("gemini-1.5-flash-8b") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::ImageAnalysis,
                    ModelCapability::VideoAnalysis,
                    ModelCapability::AudioAnalysis,
                ]
            }
            model_type if model_type.starts_with("gemini-1.5-flash") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::ImageAnalysis,
                    ModelCapability::VideoAnalysis,
                    ModelCapability::AudioAnalysis,
                ]
            }
            model_type if model_type.starts_with("gemini-1.5-pro") => {
                vec![
                    ModelCapability::TextInference,
                    ModelCapability::ImageAnalysis,
                    ModelCapability::VideoAnalysis,
                    ModelCapability::AudioAnalysis,
                ]
            }
            // Legacy and other Gemini models
            model_type if model_type.starts_with("gemini-pro") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            model_type if model_type.starts_with("gemini-ultra") => {
                vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis]
            }
            _ => vec![ModelCapability::TextInference, ModelCapability::ImageAnalysis],
        }
    }

    fn get_gemini_cost(model_type: &str) -> ModelCost {
        match model_type {
            // Gemini 2.5 models (preview/experimental - more expensive due to restricted limits)
            model_type if model_type.starts_with("gemini-2.5-flash-preview") => ModelCost::GoodValue,
            model_type if model_type.starts_with("gemini-2.5-pro-preview") => ModelCost::Expensive,
            model_type if model_type.starts_with("gemini-2.5-flash-image-preview") => ModelCost::Expensive,
            // Gemini 2.0 models
            model_type if model_type.starts_with("gemini-2.0-flash-lite") => ModelCost::VeryCheap,
            model_type if model_type.starts_with("gemini-2.0-flash") => ModelCost::Cheap,
            // Gemini 1.5 models
            model_type if model_type.starts_with("gemini-1.5-flash-8b") => ModelCost::VeryCheap,
            model_type if model_type.starts_with("gemini-1.5-flash") => ModelCost::Cheap,
            model_type if model_type.starts_with("gemini-1.5-pro") => ModelCost::GoodValue,
            // Legacy Gemini models
            model_type if model_type.starts_with("gemini-pro") => ModelCost::Cheap,
            model_type if model_type.starts_with("gemini-ultra") => ModelCost::Expensive,
            _ => ModelCost::Cheap,
        }
    }

    fn get_gemini_max_tokens(model_type: &str) -> usize {
        match model_type {
            // Gemini 2.5 models
            model_type if model_type.starts_with("gemini-2.5-flash-preview-05-20") => 1_048_576,
            model_type if model_type.starts_with("gemini-2.5-flash-preview-native-audio-dialog") => 128_000,
            model_type if model_type.starts_with("gemini-2.5-flash-exp-native-audio-thinking-dialog") => 128_000,
            model_type if model_type.starts_with("gemini-2.5-flash-preview-tts") => 8_000,
            model_type if model_type.starts_with("gemini-2.5-flash-image-preview") => 32_768,
            model_type if model_type.starts_with("gemini-2.5-pro-preview-06-05") => 1_048_576,
            model_type if model_type.starts_with("gemini-2.5-pro-preview-tts") => 8_000,
            // Gemini 2.0 models
            model_type if model_type.starts_with("gemini-2.0-flash-preview-image-generation") => 32_000,
            model_type if model_type.starts_with("gemini-2.0-flash-lite") => 1_048_576,
            model_type if model_type.starts_with("gemini-2.0-flash") => 1_048_576,
            // Gemini 1.5 models
            model_type if model_type.starts_with("gemini-1.5-pro") => 2_097_152,
            model_type if model_type.starts_with("gemini-1.5-flash") => 1_048_576,
            // Legacy Gemini models
            model_type if model_type.starts_with("gemini-pro") => 30_720,
            model_type if model_type.starts_with("gemini-ultra") => 30_720,
            _ => 1_000_000, // Default for unknown Gemini models
        }
    }

    fn get_gemini_max_output_tokens(model_type: &str) -> usize {
        match model_type {
            // Gemini 2.5 models
            model_type if model_type.starts_with("gemini-2.5-flash-preview-05-20") => 65_536,
            model_type if model_type.starts_with("gemini-2.5-flash-preview-native-audio-dialog") => 8_000,
            model_type if model_type.starts_with("gemini-2.5-flash-exp-native-audio-thinking-dialog") => 8_000,
            model_type if model_type.starts_with("gemini-2.5-flash-preview-tts") => 16_000,
            model_type if model_type.starts_with("gemini-2.5-flash-image-preview") => 32_768,
            model_type if model_type.starts_with("gemini-2.5-pro-preview-06-05") => 65_536,
            model_type if model_type.starts_with("gemini-2.5-pro-preview-tts") => 16_000,
            // Gemini 2.0 models
            model_type if model_type.starts_with("gemini-2.0-flash-preview-image-generation") => 8_192,
            model_type if model_type.starts_with("gemini-2.0-flash-lite") => 8_192,
            model_type if model_type.starts_with("gemini-2.0-flash") => 8_192,
            // Gemini 1.5 models
            model_type if model_type.starts_with("gemini-1.5-pro") => 8_192,
            model_type if model_type.starts_with("gemini-1.5-flash") => 8_192,
            // Legacy Gemini models
            model_type if model_type.starts_with("gemini-pro") => 2_048,
            model_type if model_type.starts_with("gemini-ultra") => 2_048,
            _ => 8_192, // Default for unknown Gemini models
        }
    }

    fn gemini_has_tool_capabilities(model_type: &str) -> bool {
        match model_type {
            // Gemini 2.5 models - TTS models don't support function calling
            model_type if model_type.starts_with("gemini-2.5-flash-preview-tts") => false,
            model_type if model_type.starts_with("gemini-2.5-flash-image-preview") => false,
            model_type if model_type.starts_with("gemini-2.5-pro-preview-tts") => false,
            model_type if model_type.starts_with("gemini-2.5-flash-preview-05-20") => true,
            model_type if model_type.starts_with("gemini-2.5-flash-preview-native-audio") => true,
            model_type if model_type.starts_with("gemini-2.5-flash-exp-native-audio") => true,
            model_type if model_type.starts_with("gemini-2.5-pro-preview") => true,
            // Gemini 2.0 models - Image generation model doesn't support function calling
            model_type if model_type.starts_with("gemini-2.0-flash-preview-image-generation") => false,
            model_type if model_type.starts_with("gemini-2.0-flash-lite") => true,
            model_type if model_type.starts_with("gemini-2.0-flash") => true,
            // Gemini 1.5 models
            model_type if model_type.starts_with("gemini-1.5-pro") => true,
            model_type if model_type.starts_with("gemini-1.5-flash") => true,
            // Legacy Gemini models
            model_type if model_type.starts_with("gemini-pro") => true,
            model_type if model_type.starts_with("gemini-ultra") => true,
            _ => true, // Default for unknown Gemini models
        }
    }

    // Static method to get cost of an agent model
    pub fn get_llm_provider_cost(model: &LLMProviderInterface) -> ModelCost {
        match model {
            LLMProviderInterface::OpenAI(openai) => match openai.model_type.as_str() {
                "gpt-5" => ModelCost::GoodValue,
                "gpt-5-mini" => ModelCost::Cheap,
                "gpt-5-nano" => ModelCost::VeryCheap,
                "gpt-5-chat-latest" => ModelCost::GoodValue,
                "gpt-4o" => ModelCost::GoodValue,
                "gpt-3.5-turbo-1106" => ModelCost::VeryCheap,
                "gpt-4o-mini" => ModelCost::VeryCheap,
                "gpt-4-1106-preview" => ModelCost::GoodValue,
                "gpt-4-vision-preview" => ModelCost::GoodValue,
                "gpt-4.1" => ModelCost::GoodValue,
                "gpt-4.1-nano" => ModelCost::VeryCheap,
                "gpt-4.1-mini" => ModelCost::Cheap,
                "dall-e-3" => ModelCost::GoodValue,
                "o3-mini" => ModelCost::Cheap,
                "o4-mini" => ModelCost::Cheap,
                "o1-mini" => ModelCost::Cheap,
                model_type if model_type.starts_with("o3") => ModelCost::Expensive,
                _ => ModelCost::Unknown,
            },
            LLMProviderInterface::TogetherAI(togetherai) => match togetherai.model_type.as_str() {
                "togethercomputer/llama-2-70b-chat" => ModelCost::Cheap,
                "togethercomputer/llama3" => ModelCost::Cheap,
                "yorickvp/llava-13b" => ModelCost::Expensive,
                _ => ModelCost::Unknown,
            },
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type().as_str() {
                "STANDARD_TEXT_INFERENCE" | "PREMIUM_TEXT_INFERENCE" => ModelCost::GoodValue,
                "CODE_GENERATOR" | "CODE_GENERATOR_NO_FEEDBACK" => ModelCost::Expensive,
                "FREE_TEXT_INFERENCE" => ModelCost::VeryCheap,
                _ => ModelCost::Unknown,
            },
            LLMProviderInterface::Ollama(_) => ModelCost::Free,
            LLMProviderInterface::Groq(_) => ModelCost::VeryCheap,
            LLMProviderInterface::Gemini(gemini) => Self::get_gemini_cost(gemini.model_type.as_str()),
            LLMProviderInterface::Exo(_) => ModelCost::Cheap,
            LLMProviderInterface::OpenRouter(_) => ModelCost::Free,
            LLMProviderInterface::Grok(grok) => {
                if grok.model_type.starts_with("grok-4") {
                    ModelCost::GoodValue
                } else if grok.model_type.starts_with("grok-3-mini") {
                    ModelCost::VeryCheap
                } else if grok.model_type.starts_with("grok-3-fast") {
                    ModelCost::Expensive
                } else if grok.model_type.starts_with("grok-3") {
                    ModelCost::GoodValue
                } else if grok.model_type.starts_with("grok-2-vision") {
                    ModelCost::Cheap
                } else {
                    ModelCost::Unknown
                }
            }
            LLMProviderInterface::Claude(claude) => match claude.model_type.as_str() {
                "claude-opus-4-1-20250805" | "claude-opus-4-1" => ModelCost::Expensive,
                "claude-opus-4-20250514" | "claude-opus-4-0" => ModelCost::Expensive,
                "claude-sonnet-4-20250514" | "claude-sonnet-4-0" => ModelCost::Cheap,
                "claude-3-7-sonnet-20250219" | "claude-3-7-sonnet-latest" => ModelCost::Cheap,
                "claude-3-5-sonnet-20241022" | "claude-3-5-sonnet-latest" => ModelCost::Cheap,
                "claude-3-5-haiku-20241022" | "claude-3-5-haiku-latest" => ModelCost::VeryCheap,
                "claude-3-opus-20240229" | "claude-3-opus-latest" => ModelCost::Expensive,
                "claude-3-haiku-20240307" => ModelCost::VeryCheap,
                "claude-3-sonnet-20240229" => ModelCost::Cheap,
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
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type().as_str() {
                "FREE_TEXT_INFERENCE" => ModelPrivacy::RemoteGreedy,
                "STANDARD_TEXT_INFERENCE" => ModelPrivacy::RemoteGreedy,
                "PREMIUM_TEXT_INFERENCE" => ModelPrivacy::RemoteGreedy,
                "CODE_GENERATOR" => ModelPrivacy::RemoteGreedy,
                "CODE_GENERATOR_NO_FEEDBACK" => ModelPrivacy::RemoteGreedy,
                _ => ModelPrivacy::Unknown,
            },
            LLMProviderInterface::Ollama(_) => ModelPrivacy::Local,
            LLMProviderInterface::Groq(_) => ModelPrivacy::RemoteGreedy,
            LLMProviderInterface::Gemini(_) => ModelPrivacy::RemoteGreedy,
            LLMProviderInterface::Exo(_) => ModelPrivacy::Local,
            LLMProviderInterface::OpenRouter(_) => ModelPrivacy::Local,
            LLMProviderInterface::Grok(_) => ModelPrivacy::RemoteGreedy,
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
            LLMProviderInterface::Grok(grok) => {
                let total_tokens = Self::get_max_tokens(model);
                let messages_string = llama_prepare_messages(model, grok.clone().model_type, prompt, total_tokens)?;
                Ok(messages_string)
            }
            LLMProviderInterface::Claude(claude) => {
                let total_tokens = Self::get_max_tokens(model);
                let messages_string = llama_prepare_messages(model, claude.clone().model_type, prompt, total_tokens)?;
                Ok(messages_string)
            }
            LLMProviderInterface::DeepSeek(_) => {
                let tiktoken_messages = openai_prepare_messages(model, prompt)?;
                Ok(tiktoken_messages)
            }
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
                    || openai.model_type.starts_with("4o-mini")
                    || openai.model_type.starts_with("gpt-4-vision-preview")
                    || openai.model_type.starts_with("o1-mini")
                    || openai.model_type.starts_with("o1-preview")
                {
                    128_000
                } else if openai.model_type.starts_with("gpt-4.1") {
                    1_047_576
                } else if openai.model_type.starts_with("o3") || openai.model_type.starts_with("o4-mini") {
                    200_000
                } else if openai.model_type.starts_with("gpt-5") {
                    400_000
                } else if openai.model_type.starts_with("gpt-3.5") {
                    16_384
                } else {
                    200_000 // New default for OpenAI models
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
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => match shinkai_backend.model_type().as_str() {
                "FREE_TEXT_INFERENCE" => 400_000,     // gpt-5-mini
                "STANDARD_TEXT_INFERENCE" => 400_000, // gpt-5
                "PREMIUM_TEXT_INFERENCE" => 200_000,  // claude-sonnet-4
                "CODE_GENERATOR" => 128_000,
                "CODE_GENERATOR_NO_FEEDBACK" => 128_000,
                _ => 128_000,
            },
            LLMProviderInterface::Gemini(gemini) => Self::get_gemini_max_tokens(gemini.model_type.as_str()),
            LLMProviderInterface::Ollama(ollama) => Self::get_max_tokens_for_model_type(&ollama.model_type),
            LLMProviderInterface::Exo(exo) => Self::get_max_tokens_for_model_type(&exo.model_type),
            LLMProviderInterface::Groq(groq) => Self::get_max_tokens_for_model_type(&groq.model_type),
            LLMProviderInterface::Grok(grok) => {
                if grok.model_type.starts_with("grok-4") {
                    256_000
                } else if grok.model_type.starts_with("grok-3") {
                    131_072
                } else if grok.model_type.starts_with("grok-2") {
                    32_768
                } else {
                    131_072 // Default to grok-3 context window for unknown models
                }
            }
            LLMProviderInterface::OpenRouter(openrouter) => get_openrouter_model(&openrouter.model_type)
                .and_then(|m| m.context_length)
                .unwrap_or(4096) as usize,
            LLMProviderInterface::Claude(_) => 200_000, // All Claude models now have 200K context window
            LLMProviderInterface::DeepSeek(_) => 64_000,
            LLMProviderInterface::LocalRegex(_) => 128_000,
        }
    }

    fn get_max_tokens_for_model_type(model_type: &str) -> usize {
        match model_type {
            model_type if model_type.starts_with("gpt-oss") => 128_000,
            model_type if model_type.starts_with("mistral:7b-instruct-v0.2") => 32_000,
            model_type if model_type.starts_with("mistral-nemo") => 128_000,
            model_type if model_type.starts_with("mistral-small3.2") || model_type.starts_with("mistral-small3.1") => {
                128_000
            }
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
            model_type if model_type.starts_with("devstral") => 128_000,
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
            model_type if model_type.starts_with("qwen2.5vl") => 128_000,
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
            // Groq Production Models
            model_type if model_type.starts_with("groq/compound") => 131_072,
            model_type if model_type.starts_with("groq/compound-mini") => 131_072,
            model_type if model_type.starts_with("llama-3.1-8b-instant") => 131_072,
            model_type if model_type.starts_with("llama-3.3-70b-versatile") => 131_072,
            model_type if model_type.starts_with("meta-llama/llama-guard-4-12b") => 131_072,
            model_type if model_type.starts_with("openai/gpt-oss") => 128_000,
            // Groq Preview Models
            model_type if model_type.starts_with("meta-llama/llama-4-maverick-17b-128e-instruct") => 131_072,
            model_type if model_type.starts_with("meta-llama/llama-4-scout-17b-16e-instruct") => 131_072,
            model_type if model_type.starts_with("meta-llama/llama-prompt-guard-2-22m") => 512,
            model_type if model_type.starts_with("meta-llama/llama-prompt-guard-2-86m") => 512,
            model_type if model_type.starts_with("moonshotai/kimi-k2-instruct") => 131_072,
            model_type if model_type.starts_with("playai-tts") => 8_192,
            model_type if model_type.starts_with("playai-tts-arabic") => 8_192,
            model_type if model_type.starts_with("qwen/qwen3-32b") => 131_072,
            // Legacy Groq models (keeping for backward compatibility)
            model_type if model_type.starts_with("deepseek-r1-distill-llama-70b") => 131_072,
            model_type if model_type.starts_with("mistral-saba-24b") => 32_000,
            model_type if model_type.starts_with("qwen-qwq-32b") => 128_000,
            model_type if model_type.starts_with("allam-2-7b") => 4_096,
            model_type if model_type.starts_with("gemma2-9b-it") => 8_192,
            model_type if model_type.starts_with("llama3-70b-8192") => 8_192,
            model_type if model_type.starts_with("llama3-8b-8192") => 8_192,
            model_type if model_type.starts_with("llama-guard-3-8b") => 8_192,
            model_type if model_type.starts_with("mixtral-8x7b-32768") => 32_768,
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
            model_type if model_type.starts_with("gemma3n") => 32_000,
            model_type if model_type.starts_with("gemma3:1b") => 32_000,
            model_type if model_type.starts_with("gemma3:4b") => 128_000,
            model_type if model_type.starts_with("gemma3:12b") => 128_000,
            model_type if model_type.starts_with("gemma3:27b") => 128_000,
            model_type if model_type.starts_with("gemma3") => 128_000,
            model_type if model_type.starts_with("qwen3") => 32_000,
            model_type if model_type.starts_with("magistral") => 39_000,
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
                if openai.model_type.contains("4o-mini") || openai.model_type.starts_with("gpt-4o") {
                    16_384
                } else if openai.model_type.starts_with("o1-preview") || openai.model_type.starts_with("gpt-4.1") {
                    32768
                } else if openai.model_type.starts_with("o1-mini") {
                    65_536
                } else if openai.model_type.starts_with("o3") || openai.model_type.starts_with("o4-mini") {
                    100_000
                } else if openai.model_type.starts_with("gpt-5-chat-latest") {
                    16_384
                } else if openai.model_type.starts_with("gpt-5-mini") {
                    128_000
                } else if openai.model_type.starts_with("gpt-5-nano") {
                    128_000
                } else if openai.model_type.starts_with("gpt-5") {
                    128_000
                } else if openai.model_type.starts_with("gpt-3.5") {
                    4096
                } else {
                    32_000 // New default output tokens for OpenAI models
                }
            }
            LLMProviderInterface::TogetherAI(_) => {
                if Self::get_max_tokens(model) <= 8000 {
                    2800
                } else {
                    4096
                }
            }
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => {
                // Fill in the appropriate logic for ShinkaiBackend
                match shinkai_backend.model_type().as_str() {
                    "FREE_TEXT_INFERENCE" | "STANDARD_TEXT_INFERENCE" => 128000,
                    "PREMIUM_TEXT_INFERENCE" => 64000,
                    "CODE_GENERATOR" | "CODE_GENERATOR_NO_FEEDBACK" => 16384,
                    _ => 16384,
                }
            }
            LLMProviderInterface::Ollama(_) => {
                // Fill in the appropriate logic for Ollama
                if Self::get_max_tokens(model) <= 8000 {
                    2800
                } else {
                    4096
                }
            }
            LLMProviderInterface::Groq(groq) => {
                // Groq model-specific max output tokens based on official documentation
                match groq.model_type.as_str() {
                    model_type if model_type.starts_with("groq/compound") => 8_192,
                    model_type if model_type.starts_with("groq/compound-mini") => 8_192,
                    model_type if model_type.starts_with("llama-3.1-8b-instant") => 8_192,
                    model_type if model_type.starts_with("llama-3.3-70b-versatile") => 32_768,
                    model_type if model_type.starts_with("meta-llama/llama-guard-4-12b") => 128,
                    model_type if model_type.starts_with("openai/gpt-oss") => 65_536,
                    model_type if model_type.starts_with("meta-llama/llama-4-maverick-17b-128e-instruct") => 8_192,
                    model_type if model_type.starts_with("meta-llama/llama-4-scout-17b-16e-instruct") => 8_192,
                    model_type if model_type.starts_with("meta-llama/llama-prompt-guard-2-22m") => 512,
                    model_type if model_type.starts_with("meta-llama/llama-prompt-guard-2-86m") => 512,
                    model_type if model_type.starts_with("moonshotai/kimi-k2-instruct") => 16_384,
                    model_type if model_type.starts_with("playai-tts") => 8_192,
                    model_type if model_type.starts_with("playai-tts-arabic") => 8_192,
                    model_type if model_type.starts_with("qwen/qwen3-32b") => 40_960,
                    model_type if model_type.starts_with("deepseek-r1-distill-llama-70b") => 131_072,
                    _ => 4096,
                }
            }
            LLMProviderInterface::Grok(grok) => {
                if grok.model_type.starts_with("grok-4") {
                    128_000
                } else if grok.model_type.starts_with("grok-3") {
                    65_536
                } else if grok.model_type.starts_with("grok-2") {
                    16_384
                } else {
                    4096
                }
            }
            LLMProviderInterface::Exo(_) => 4096,
            LLMProviderInterface::Gemini(gemini) => Self::get_gemini_max_output_tokens(gemini.model_type.as_str()),
            LLMProviderInterface::OpenRouter(openrouter) => {
                let model = get_openrouter_model(&openrouter.model_type);
                let max_tokens = model.and_then(|m| m.top_provider.as_ref().and_then(|p| p.max_completion_tokens));
                max_tokens.unwrap_or(model.and_then(|m| m.context_length).unwrap_or(4096)) as usize
            }
            LLMProviderInterface::Claude(claude) => {
                if claude.model_type.starts_with("claude-opus-4") {
                    32_000
                } else if claude.model_type.starts_with("claude-sonnet-4") {
                    64_000
                } else if claude.model_type.starts_with("claude-3-7-sonnet") {
                    64_000
                } else if claude.model_type.starts_with("claude-3-5-sonnet") {
                    8192
                } else if claude.model_type.starts_with("claude-3-5-haiku") {
                    8192
                } else if claude.model_type.starts_with("claude-3-opus") {
                    4096
                } else if claude.model_type.starts_with("claude-3-haiku") {
                    4096
                } else {
                    8192 // Default for other Claude models
                }
            }
            LLMProviderInterface::DeepSeek(_) => 8192,
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
            LLMProviderInterface::OpenAI(openai) => {
                // o1-mini and gpt-5-chat specifically does not support function calling
                if openai.model_type.starts_with("o1-mini") {
                    false
                } else if openai.model_type.starts_with("gpt-5-chat-latest") {
                    false
                } else {
                    true
                }
            }
            LLMProviderInterface::Ollama(model) => {
                // For Ollama, check model type and respect the passed stream parameter
                model.model_type.starts_with("llama3.1")
                    || model.model_type.starts_with("llama3.2")
                    || model.model_type.starts_with("llama-3.1")
                    || model.model_type.starts_with("llama-3.2")
                    || model.model_type.starts_with("mistral-nemo")
                    || model.model_type.starts_with("mistral-small3.2")
                    || model.model_type.starts_with("mistral-small3.1")
                    || model.model_type.starts_with("mistral-small")
                    || model.model_type.starts_with("mistral-large")
                    || model.model_type.starts_with("mistral-pixtral")
                    || model.model_type.starts_with("qwen2.5-coder")
                    || model.model_type.starts_with("qwen2.5vl")
                    || model.model_type.starts_with("qwq")
                    || model.model_type.starts_with("qwen3")
                    || model.model_type.starts_with("devstral")
                    || model.model_type.starts_with("deepseek-r1:14b")
                    || model.model_type.starts_with("deepseek-r1:8b")
                    || model.model_type.starts_with("deepseek-r1:70b")
                    || model.model_type.starts_with("deepseek-v3")
                    || model.model_type.starts_with("command-r7b")
                    || model.model_type.starts_with("mistral-small")
                    || model.model_type.starts_with("magistral")
                    || model.model_type.starts_with("gpt-oss")
            }
            LLMProviderInterface::Groq(model) => {
                // Legacy and deprecated models
                model.model_type.starts_with("gemma2-9b-it")
                    || model.model_type.starts_with("llama3-70b-8192")
                    || model.model_type.starts_with("llama3-8b-8192")
                    || model.model_type.starts_with("qwen-qwq-32b")
                    || model.model_type.starts_with("magistral")
                    || model.model_type.starts_with("mixtral-8x7b-32768")
                    || model.model_type.starts_with("llama-3.3-70b-specdec")
                    || model.model_type.starts_with("llama-3.2-1b-preview")
                    || model.model_type.starts_with("llama-3.2-3b-preview")
                    || model.model_type.starts_with("llama-3.2-11b-vision-preview")
                    || model.model_type.starts_with("llama-3.2-90b-vision-preview")
                    || model.model_type.starts_with("llama-3.2")
                    || model.model_type.starts_with("llama3.2")
                    || model.model_type.starts_with("llama-3.1")
                    || model.model_type.starts_with("llama3.1")
                    || model.model_type.starts_with("qwen-2.5-coder-32b")
                    || model.model_type.starts_with("qwen-2.5-32b")
                    || model.model_type.starts_with("deepseek-r1-distill-qwen-32b")
                    || model.model_type.starts_with("deepseek-r1-distill-llama-70b")                    
                    // List from https://console.groq.com/docs/tool-use (sept 11, 2025)
                    || model.model_type.starts_with("openai/gpt-oss")
                    || model.model_type.starts_with("qwen/qwen3-32b")
                    || model.model_type.starts_with("moonshotai/kimi-k2-instruct")
                    || model.model_type.starts_with("meta-llama/llama-4-scout-17b-16e-instruct")
                    || model.model_type.starts_with("meta-llama/llama-4-maverick-17b-128e-instruct")
                    || model.model_type.starts_with("llama-3.3-70b-versatile")
                    || model.model_type.starts_with("llama-3.1-8b-instant")
            }
            LLMProviderInterface::OpenRouter(model) => {
                model.model_type.starts_with("llama-3.2")
                    || model.model_type.starts_with("llama3.2")
                    || model.model_type.starts_with("llama-3.1")
                    || model.model_type.starts_with("llama3.1")
                    || model.model_type.starts_with("mistral-nemo")
                    || model.model_type.starts_with("mistral-small3.2")
                    || model.model_type.starts_with("mistral-small3.1")
                    || model.model_type.starts_with("mistral-small")
                    || model.model_type.starts_with("mistral-large")
                    || model.model_type.starts_with("mistral-pixtral")
                    || model.model_type.starts_with("magistral")
                    || model.model_type.starts_with("gpt-oss")
            }
            LLMProviderInterface::Claude(_) => true, // All Claude models support tool calling
            LLMProviderInterface::ShinkaiBackend(_) => true,
            LLMProviderInterface::Gemini(model) => Self::gemini_has_tool_capabilities(model.model_type.as_str()),
            LLMProviderInterface::DeepSeek(_) => true,
            LLMProviderInterface::Grok(_) => true, // All Grok models support tool calling
            _ => false,
        }
    }

    /// Returns whether the given model has reasoning capabilities
    pub fn has_reasoning_capabilities(model: &LLMProviderInterface) -> bool {
        match model {
            LLMProviderInterface::OpenAI(openai) => {
                openai.model_type.starts_with("o1")
                    || openai.model_type.starts_with("o3")
                    || openai.model_type.starts_with("o4")
                    || (openai.model_type.starts_with("gpt-5") && openai.model_type != "gpt-5-chat-latest")
            }
            LLMProviderInterface::Ollama(ollama) => {
                ollama.model_type.starts_with("deepseek-r1")
                    || ollama.model_type.starts_with("magistral")
                    || ollama.model_type.starts_with("gpt-oss")
                    || ollama.model_type.starts_with("qwen3")
            }
            LLMProviderInterface::Groq(groq) => {
                groq.model_type.starts_with("deepseek-r1-distill-llama-70b")
                    || groq.model_type.starts_with("qwen/qwen3-32b")
                    || groq.model_type.starts_with("openai/gpt-oss")
            }
            LLMProviderInterface::DeepSeek(deepseek) => deepseek.model_type.starts_with("deepseek-reasoner"),
            LLMProviderInterface::Claude(claude) => {
                claude.model_type.starts_with("claude-opus-4-1")
                    || claude.model_type.starts_with("claude-opus-4")
                    || claude.model_type.starts_with("claude-sonnet-4")
                    || claude.model_type.starts_with("claude-3-7-sonnet")
            }
            LLMProviderInterface::Gemini(gemini) => {
                gemini.model_type == "gemini-2.5-flash-preview-05-20"
                    || gemini.model_type == "gemini-2.5-flash-lite-preview-06-17"
                    || gemini.model_type == "gemini-2.5-flash-lite"
                    || gemini.model_type == "gemini-2.5-flash"
                    || gemini.model_type == "gemini-2.5-pro"
                    || gemini.model_type == "gemini-2.0-flash-exp"
            }
            LLMProviderInterface::ShinkaiBackend(shinkai_backend) => {
                shinkai_backend.model_type().starts_with("FREE_TEXT_INFERENCE")
                    || shinkai_backend.model_type().starts_with("STANDARD_TEXT_INFERENCE")
                    || shinkai_backend.model_type().starts_with("PREMIUM_TEXT_INFERENCE")
            }
            _ => false,
        }
    }
}
