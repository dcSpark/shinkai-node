use crate::schemas::shinkai_name::ShinkaiName;
use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use utoipa::ToSchema;

// Agent has a few fields that are not serializable, so we need to create a struct that is serializable
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct SerializedLLMProvider {
    pub id: String,
    pub full_identity_name: ShinkaiName,
    pub external_url: Option<String>,
    pub api_key: Option<String>,
    pub model: LLMProviderInterface,
}

impl SerializedLLMProvider {
    pub fn get_provider_string(&self) -> String {
        match &self.model {
            LLMProviderInterface::OpenAI(_) => "openai",
            LLMProviderInterface::TogetherAI(_) => "togetherai",
            LLMProviderInterface::Ollama(_) => "ollama",
            LLMProviderInterface::ShinkaiBackend(_) => "shinkai-backend",
            LLMProviderInterface::LocalLLM(_) => "local-llm",
            LLMProviderInterface::Groq(_) => "groq",
            LLMProviderInterface::Gemini(_) => "gemini",
            LLMProviderInterface::Exo(_) => "exo",
            LLMProviderInterface::OpenRouter(_) => "openrouter",
        }
        .to_string()
    }

    pub fn baml_provider_string(&self) -> String {
        match &self.model {
            LLMProviderInterface::OpenAI(_) => "openai".to_string(),
            LLMProviderInterface::TogetherAI(_) => "openai-generic".to_string(),
            LLMProviderInterface::Ollama(_) => "ollama".to_string(),
            LLMProviderInterface::ShinkaiBackend(_) => "shinkai-backend".to_string(),
            LLMProviderInterface::LocalLLM(_) => "local-llm".to_string(),
            LLMProviderInterface::Groq(_) => "openai-generic".to_string(),
            LLMProviderInterface::Gemini(_) => "google-ai".to_string(),
            LLMProviderInterface::Exo(_) => "openai-generic".to_string(),
            LLMProviderInterface::OpenRouter(_) => "openai-generic".to_string(),
        }
    }

    pub fn get_model_string(&self) -> String {
        match &self.model {
            LLMProviderInterface::OpenAI(openai) => openai.model_type.clone(),
            LLMProviderInterface::TogetherAI(togetherai) => togetherai.model_type.clone(),
            LLMProviderInterface::Ollama(ollama) => ollama.model_type.clone(),
            LLMProviderInterface::ShinkaiBackend(shinkaibackend) => shinkaibackend.model_type.clone(),
            LLMProviderInterface::LocalLLM(_) => "local-llm".to_string(),
            LLMProviderInterface::Groq(groq) => groq.model_type.clone(),
            LLMProviderInterface::Gemini(gemini) => gemini.model_type.clone(),
            LLMProviderInterface::Exo(exo) => exo.model_type.clone(),
            LLMProviderInterface::OpenRouter(openrouter) => openrouter.model_type.clone(),
        }
    }

    pub fn mock_provider() -> Self {
        SerializedLLMProvider {
            id: "mock_agent".to_string(),
            full_identity_name: ShinkaiName::new("@@test.shinkai/main/agent/mock_agent".to_string()).unwrap(),
            external_url: Some("https://api.example.com".to_string()),
            api_key: Some("mockapikey".to_string()),
            model: LLMProviderInterface::OpenAI(OpenAI {
                model_type: "gpt-4o-mini".to_string(),
            }),
        }
    }

    pub fn baml_provider_base_url(&self) -> Option<String> {
        let mut base_url = self.external_url.clone().unwrap_or_default();

        // Conditionally append "/v1" based on the model type
        match &self.model {
            LLMProviderInterface::OpenAI(_) | LLMProviderInterface::Ollama(_) => {
                if !base_url.ends_with("/v1") {
                    base_url = format!("{}/v1", base_url.trim_end_matches('/'));
                }
                Some(base_url)
            }
            LLMProviderInterface::Gemini(_) => None,
            _ => Some(base_url),
        }
    }
}

#[derive(Debug, Clone, PartialEq, ToSchema)]
pub enum LLMProviderInterface {
    OpenAI(OpenAI),
    TogetherAI(TogetherAI),
    Ollama(Ollama),
    ShinkaiBackend(ShinkaiBackend),
    LocalLLM(LocalLLM),
    Groq(Groq),
    Gemini(Gemini),
    Exo(Exo),
    OpenRouter(OpenRouter),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct LocalLLM {}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct Ollama {
    pub model_type: String,
}

impl Ollama {
    pub fn model_type(&self) -> String {
        self.model_type.to_string()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct Groq {
    pub model_type: String,
}

impl Groq {
    pub fn model_type(&self) -> String {
        self.model_type.to_string()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct Exo {
    pub model_type: String,
}

impl Exo {
    pub fn model_type(&self) -> String {
        self.model_type.to_string()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct Gemini {
    pub model_type: String,
}

impl Gemini {
    pub fn model_type(&self) -> String {
        self.model_type.to_string()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct ShinkaiBackend {
    pub model_type: String,
}

impl ShinkaiBackend {
    pub fn new(model_type: &str) -> Self {
        ShinkaiBackend {
            model_type: model_type.to_uppercase(),
        }
    }

    pub fn model_type(&self) -> String {
        self.model_type.to_uppercase()
    }

    pub fn set_model_type(&mut self, model_type: &str) {
        self.model_type = model_type.to_string();
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct OpenAI {
    pub model_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OpenRouter {
    pub model_type: String,
}

impl OpenRouter {
    pub fn model_type(&self) -> String {
        self.model_type.to_string()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TogetherAI {
    pub model_type: String,
}

impl FromStr for LLMProviderInterface {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_lowercase();
        if s.starts_with("openai:") {
            let model_type = s.strip_prefix("openai:").unwrap_or("").to_string();
            Ok(LLMProviderInterface::OpenAI(OpenAI { model_type }))
        } else if s.starts_with("togetherai:") {
            let model_type = s.strip_prefix("togetherai:").unwrap_or("").to_string();
            Ok(LLMProviderInterface::TogetherAI(TogetherAI { model_type }))
        } else if s.starts_with("ollama:") {
            let model_type = s.strip_prefix("ollama:").unwrap_or("").to_string();
            Ok(LLMProviderInterface::Ollama(Ollama { model_type }))
        } else if s.starts_with("shinkai-backend:") {
            let model_type = s.strip_prefix("shinkai-backend:").unwrap_or("").to_string();
            Ok(LLMProviderInterface::ShinkaiBackend(ShinkaiBackend { model_type }))
        } else if s.starts_with("groq:") {
            let model_type = s.strip_prefix("groq:").unwrap_or("").to_string();
            Ok(LLMProviderInterface::Groq(Groq { model_type }))
        } else if s.starts_with("gemini:") {
            let model_type = s.strip_prefix("gemini:").unwrap_or("").to_string();
            Ok(LLMProviderInterface::Gemini(Gemini { model_type }))
        } else if s.starts_with("exo:") {
            let model_type = s.strip_prefix("exo:").unwrap_or("").to_string();
            Ok(LLMProviderInterface::Exo(Exo { model_type }))
        } else if s.starts_with("openrouter:") {
            let model_type = s.strip_prefix("openrouter:").unwrap_or("").to_string();
            Ok(LLMProviderInterface::OpenRouter(OpenRouter { model_type }))
        } else {
            Err(())
        }
    }
}

impl Serialize for LLMProviderInterface {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            LLMProviderInterface::OpenAI(openai) => {
                let model_type = format!("openai:{}", openai.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::TogetherAI(togetherai) => {
                let model_type = format!("togetherai:{}", togetherai.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::Ollama(ollama) => {
                let model_type = format!("ollama:{}", ollama.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::ShinkaiBackend(shinkaibackend) => {
                let model_type = format!("shinkai-backend:{}", shinkaibackend.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::Groq(groq) => {
                let model_type = format!("groq:{}", groq.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::Gemini(gemini) => {
                let model_type = format!("gemini:{}", gemini.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::Exo(exo) => {
                let model_type = format!("exo:{}", exo.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::OpenRouter(openrouter) => {
                let model_type = format!("openrouter:{}", openrouter.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::LocalLLM(_) => serializer.serialize_str("local-llm"),
        }
    }
}

struct LLMProviderInterfaceVisitor;

impl<'de> Visitor<'de> for LLMProviderInterfaceVisitor {
    type Value = LLMProviderInterface;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string representing an LLMProviderInterface variant")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let parts: Vec<&str> = value.splitn(2, ':').collect();
        match parts[0] {
            "openai" => Ok(LLMProviderInterface::OpenAI(OpenAI {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "togetherai" => Ok(LLMProviderInterface::TogetherAI(TogetherAI {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "ollama" => Ok(LLMProviderInterface::Ollama(Ollama {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "shinkai-backend" => Ok(LLMProviderInterface::ShinkaiBackend(ShinkaiBackend {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "groq" => Ok(LLMProviderInterface::Groq(Groq {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "gemini" => Ok(LLMProviderInterface::Gemini(Gemini {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "exo" => Ok(LLMProviderInterface::Exo(Exo {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "openrouter" => Ok(LLMProviderInterface::OpenRouter(OpenRouter {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "local-llm" => Ok(LLMProviderInterface::LocalLLM(LocalLLM {})),
            _ => Err(de::Error::unknown_variant(
                value,
                &[
                    "openai",
                    "togetherai",
                    "ollama",
                    "shinkai-backend",
                    "local-llm",
                    "groq",
                    "exo",
                    "gemini",
                    "openrouter",
                ],
            )),
        }
    }
}

impl<'de> Deserialize<'de> for LLMProviderInterface {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(LLMProviderInterfaceVisitor)
    }
}
