use crate::schemas::shinkai_name::ShinkaiName;
use serde::{Deserialize, Serialize, Serializer};
use std::str::FromStr;
use serde::de::{self, Deserializer, Visitor};
use std::fmt;

// Agent has a few fields that are not serializable, so we need to create a struct that is serializable
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SerializedAgent {
    pub id: String,
    pub full_identity_name: ShinkaiName,
    pub perform_locally: bool,
    pub external_url: Option<String>,
    pub api_key: Option<String>,
    pub model: AgentLLMInterface,
    pub toolkit_permissions: Vec<String>,
    pub storage_bucket_permissions: Vec<String>,
    pub allowed_message_senders: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentLLMInterface {
    OpenAI(OpenAI),
    GenericAPI(GenericAPI),
    Ollama(Ollama),
    ShinkaiBackend(ShinkaiBackend),
    LocalLLM(LocalLLM),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LocalLLM {}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Ollama {
    pub model_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ShinkaiBackend {
    pub model_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OpenAI {
    pub model_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct GenericAPI {
    pub model_type: String,
}

impl FromStr for AgentLLMInterface {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("openai:") {
            let model_type = s.strip_prefix("openai:").unwrap_or("").to_string();
            Ok(AgentLLMInterface::OpenAI(OpenAI { model_type }))
        } else if s.starts_with("genericapi:") {
            let model_type = s.strip_prefix("genericapi:").unwrap_or("").to_string();
            Ok(AgentLLMInterface::GenericAPI(GenericAPI { model_type }))
        } else if s.starts_with("ollama:") {
            let model_type = s.strip_prefix("ollama:").unwrap_or("").to_string();
            Ok(AgentLLMInterface::Ollama(Ollama { model_type }))
        } else if s.starts_with("shinkai-backend:") {
            let model_type = s.strip_prefix("shinkai-backend:").unwrap_or("").to_string();
            Ok(AgentLLMInterface::ShinkaiBackend(ShinkaiBackend { model_type }))
        } else {
            Err(())
        }
    }
}

impl Serialize for AgentLLMInterface {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            AgentLLMInterface::OpenAI(openai) => {
                let model_type = format!("openai:{}", openai.model_type);
                serializer.serialize_str(&model_type)
            }
            AgentLLMInterface::GenericAPI(genericapi) => {
                let model_type = format!("genericapi:{}", genericapi.model_type);
                serializer.serialize_str(&model_type)
            }
            AgentLLMInterface::Ollama(ollama) => {
                let model_type = format!("ollama:{}", ollama.model_type);
                serializer.serialize_str(&model_type)
            }
            AgentLLMInterface::ShinkaiBackend(shinkaibackend) => {
                let model_type = format!("shinkai-backend:{}", shinkaibackend.model_type);
                serializer.serialize_str(&model_type)
            }
            AgentLLMInterface::LocalLLM(_) => serializer.serialize_str("local-llm"),
        }
    }
}

struct AgentLLMInterfaceVisitor;

impl<'de> Visitor<'de> for AgentLLMInterfaceVisitor {
    type Value = AgentLLMInterface;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string representing an AgentLLMInterface variant")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let parts: Vec<&str> = value.splitn(2, ':').collect();
        match parts[0] {
            "openai" => Ok(AgentLLMInterface::OpenAI(OpenAI {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "genericapi" => Ok(AgentLLMInterface::GenericAPI(GenericAPI {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "ollama" => Ok(AgentLLMInterface::Ollama(Ollama {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "shinkai-backend" => Ok(AgentLLMInterface::ShinkaiBackend(ShinkaiBackend {
                model_type: parts.get(1).unwrap_or(&"").to_string(),
            })),
            "local-llm" => Ok(AgentLLMInterface::LocalLLM(LocalLLM {})),
            _ => Err(de::Error::unknown_variant(
                value,
                &["openai", "genericapi", "ollama", "shinkai-backend", "local-llm"],
            )),
        }
    }
}

impl<'de> Deserialize<'de> for AgentLLMInterface {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(AgentLLMInterfaceVisitor)
    }
}
