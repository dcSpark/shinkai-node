use crate::schemas::shinkai_name::ShinkaiName;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentLLMInterface {
    #[serde(rename = "openai")]
    OpenAI(OpenAI),
    #[serde(rename = "local-llm")]
    LocalLLM(LocalLLM),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LocalLLM {}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OpenAI {
    pub model_type: String,
}

impl FromStr for AgentLLMInterface {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("openai:") {
            let model_type = s.strip_prefix("openai:").unwrap_or("").to_string();
            Ok(AgentLLMInterface::OpenAI(OpenAI { model_type }))
        } else {
            // TODO: nothing else for now
            Err(())
        }
    }
}
