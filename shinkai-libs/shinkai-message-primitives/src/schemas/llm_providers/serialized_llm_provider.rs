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
    pub name: Option<String>,
    pub description: Option<String>,
    pub full_identity_name: ShinkaiName,
    pub external_url: Option<String>,
    pub api_key: Option<String>,
    pub model: LLMProviderInterface,
}

impl SerializedLLMProvider {
    pub fn get_provider_string(&self) -> String {
        match &self.model {
            LLMProviderInterface::OpenAI(_) => "openai",
            LLMProviderInterface::OpenAILegacy(_) => "openai-legacy",
            LLMProviderInterface::TogetherAI(_) => "togetherai",
            LLMProviderInterface::Ollama(_) => "ollama",
            LLMProviderInterface::ShinkaiBackend(_) => "shinkai-backend",
            LLMProviderInterface::Groq(_) => "groq",
            LLMProviderInterface::Grok(_) => "grok",
            LLMProviderInterface::Gemini(_) => "gemini",
            LLMProviderInterface::Exo(_) => "exo",
            LLMProviderInterface::OpenRouter(_) => "openrouter",
            LLMProviderInterface::Claude(_) => "claude",
            LLMProviderInterface::DeepSeek(_) => "deepseek",
            LLMProviderInterface::LocalRegex(_) => "local-regex",
        }
        .to_string()
    }

    pub fn baml_provider_string(&self) -> String {
        match &self.model {
            LLMProviderInterface::OpenAI(_) => "openai".to_string(),
            LLMProviderInterface::OpenAILegacy(_) => "openai".to_string(),
            LLMProviderInterface::TogetherAI(_) => "openai-generic".to_string(),
            LLMProviderInterface::Ollama(_) => "ollama".to_string(),
            LLMProviderInterface::ShinkaiBackend(_) => "shinkai-backend".to_string(),
            LLMProviderInterface::Groq(_) => "openai-generic".to_string(),
            LLMProviderInterface::Grok(_) => "openai-generic".to_string(),
            LLMProviderInterface::Gemini(_) => "google-ai".to_string(),
            LLMProviderInterface::Exo(_) => "openai-generic".to_string(),
            LLMProviderInterface::OpenRouter(_) => "openai-generic".to_string(),
            LLMProviderInterface::Claude(_) => "claude".to_string(),
            LLMProviderInterface::DeepSeek(_) => "openai-generic".to_string(),
            LLMProviderInterface::LocalRegex(_) => "local-regex".to_string(),
        }
    }

    pub fn get_model_string(&self) -> String {
        match &self.model {
            LLMProviderInterface::OpenAI(openai) => openai.model_type.clone(),
            LLMProviderInterface::OpenAILegacy(openailegacy) => openailegacy.model_type.clone(),
            LLMProviderInterface::TogetherAI(togetherai) => togetherai.model_type.clone(),
            LLMProviderInterface::Ollama(ollama) => ollama.model_type.clone(),
            LLMProviderInterface::ShinkaiBackend(shinkaibackend) => shinkaibackend.model_type.clone(),
            LLMProviderInterface::Groq(groq) => groq.model_type.clone(),
            LLMProviderInterface::Grok(grok) => grok.model_type.clone(),
            LLMProviderInterface::Gemini(gemini) => gemini.model_type.clone(),
            LLMProviderInterface::Exo(exo) => exo.model_type.clone(),
            LLMProviderInterface::OpenRouter(openrouter) => openrouter.model_type.clone(),
            LLMProviderInterface::Claude(claude) => claude.model_type.clone(),
            LLMProviderInterface::DeepSeek(deepseek) => deepseek.model_type.clone(),
            LLMProviderInterface::LocalRegex(local_regex) => local_regex.model_type.clone(),
        }
    }

    pub fn mock_provider() -> Self {
        SerializedLLMProvider {
            id: "mock_agent".to_string(),
            name: Some("Mock Agent".to_string()),
            description: Some("A mock agent for testing.".to_string()),
            full_identity_name: ShinkaiName::new("@@test.shinkai/main/agent/mock_agent".to_string()).unwrap(),
            external_url: Some("https://api.example.com".to_string()),
            api_key: Some("mockapikey".to_string()),
            model: LLMProviderInterface::OpenAI(OpenAI {
                model_type: "gpt-4o-mini".to_string(),
            }),
        }
    }

    pub fn mock_provider_with_reasoning() -> Self {
        SerializedLLMProvider {
            id: "mock_agent_reasoning".to_string(),
            name: Some("Mock Agent Reasoning".to_string()),
            description: Some("A mock agent with reasoning for testing.".to_string()),
            full_identity_name: ShinkaiName::new("@@test.shinkai/main/agent/mock_agent_reasoning".to_string()).unwrap(),
            external_url: Some("https://api.example.com".to_string()),
            api_key: Some("mockapikey".to_string()),
            model: LLMProviderInterface::OpenAI(OpenAI {
                model_type: "o1-mini".to_string(),
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
    OpenAILegacy(OpenAILegacy),
    TogetherAI(TogetherAI),
    Ollama(Ollama),
    ShinkaiBackend(ShinkaiBackend),
    Groq(Groq),
    Grok(Grok),
    Gemini(Gemini),
    Exo(Exo),
    OpenRouter(OpenRouter),
    Claude(Claude),
    DeepSeek(DeepSeek),
    LocalRegex(LocalRegex),
}

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
pub struct Grok {
    pub model_type: String,
}

impl Grok {
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct OpenAILegacy {
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct Claude {
    pub model_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct DeepSeek {
    pub model_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct LocalRegex {
    pub model_type: String,
}

impl LocalRegex {
    pub fn model_type(&self) -> String {
        self.model_type.to_string()
    }
}

impl FromStr for LLMProviderInterface {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Provider names are case-insensitive; model IDs are opaque and can be case-sensitive.
        let (provider, model_type) = s.split_once(':').ok_or(())?;
        let model_type = model_type.to_string();
        match provider.to_ascii_lowercase().as_str() {
            "openai" => Ok(LLMProviderInterface::OpenAI(OpenAI { model_type })),
            "openai-legacy" => Ok(LLMProviderInterface::OpenAILegacy(OpenAILegacy { model_type })),
            "togetherai" => Ok(LLMProviderInterface::TogetherAI(TogetherAI { model_type })),
            "ollama" => Ok(LLMProviderInterface::Ollama(Ollama { model_type })),
            "shinkai-backend" => Ok(LLMProviderInterface::ShinkaiBackend(ShinkaiBackend { model_type })),
            "groq" => Ok(LLMProviderInterface::Groq(Groq { model_type })),
            "grok" => Ok(LLMProviderInterface::Grok(Grok { model_type })),
            "gemini" => Ok(LLMProviderInterface::Gemini(Gemini { model_type })),
            "exo" => Ok(LLMProviderInterface::Exo(Exo { model_type })),
            "openrouter" => Ok(LLMProviderInterface::OpenRouter(OpenRouter { model_type })),
            "claude" => Ok(LLMProviderInterface::Claude(Claude { model_type })),
            "deepseek" => Ok(LLMProviderInterface::DeepSeek(DeepSeek { model_type })),
            "local-regex" => Ok(LLMProviderInterface::LocalRegex(LocalRegex { model_type })),
            _ => Err(()),
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
            LLMProviderInterface::OpenAILegacy(openai) => {
                let model_type = format!("openai-legacy:{}", openai.model_type);
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
            LLMProviderInterface::Grok(grok) => {
                let model_type = format!("grok:{}", grok.model_type);
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
            LLMProviderInterface::Claude(claude) => {
                let model_type = format!("claude:{}", claude.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::DeepSeek(deepseek) => {
                let model_type = format!("deepseek:{}", deepseek.model_type);
                serializer.serialize_str(&model_type)
            }
            LLMProviderInterface::LocalRegex(local_regex) => {
                let model_type = format!("local-regex:{}", local_regex.model_type);
                serializer.serialize_str(&model_type)
            }
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
        match parts[0].to_ascii_lowercase().as_str() {
            "openai" => Ok(LLMProviderInterface::OpenAI(OpenAI {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "openai-legacy" => Ok(LLMProviderInterface::OpenAILegacy(OpenAILegacy {
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
            "grok" => Ok(LLMProviderInterface::Grok(Grok {
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
            "claude" => Ok(LLMProviderInterface::Claude(Claude {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "deepseek" => Ok(LLMProviderInterface::DeepSeek(DeepSeek {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "local-regex" => Ok(LLMProviderInterface::LocalRegex(LocalRegex {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            _ => Err(de::Error::unknown_variant(
                value,
                &[
                    "openai",
                    "openai-legacy",
                    "togetherai",
                    "ollama",
                    "shinkai-backend",
                    "local-llm",
                    "groq",
                    "grok",
                    "exo",
                    "gemini",
                    "openrouter",
                    "claude",
                    "deepseek",
                    "local-regex",
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

#[cfg(test)]
mod provider_identifier_tests {
    use super::*;

    #[test]
    fn provider_prefix_is_case_insensitive_and_model_id_is_preserved() {
        // Include colons in the model ID: split only the provider separator.
        let model = "Owner/CaseSensitive-Model:Quant-4B";
        for prefix in [
            "OpenAI",
            "OPENAI-LEGACY",
            "ToGeThErAi",
            "Ollama",
            "SHINKAI-BACKEND",
            "Groq",
            "Grok",
            "Gemini",
            "EXO",
            "OpenRouter",
            "Claude",
            "DeepSeek",
            "LOCAL-REGEX",
        ] {
            let input = format!("{prefix}:{model}");
            let parsed = LLMProviderInterface::from_str(&input).unwrap();
            let serialized = serde_json::to_value(&parsed).unwrap();
            assert_eq!(
                serialized.as_str().unwrap(),
                format!("{}:{model}", prefix.to_ascii_lowercase())
            );
            assert_eq!(
                serde_json::from_value::<LLMProviderInterface>(serde_json::json!(input)).unwrap(),
                parsed
            );
            assert_eq!(
                serde_json::from_value::<LLMProviderInterface>(serialized).unwrap(),
                parsed
            );
        }
        assert!(LLMProviderInterface::from_str("unknown:Model").is_err());
        assert!(LLMProviderInterface::from_str("togetherai").is_err());
    }
}
