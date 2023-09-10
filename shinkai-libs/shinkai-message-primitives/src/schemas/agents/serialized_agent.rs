use crate::schemas::shinkai_name::ShinkaiName;
use serde::{Deserialize, Serialize};

// Agent has a few fields that are not serializable, so we need to create a struct that is serializable
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SerializedAgent {
    pub id: String,
    pub full_identity_name: ShinkaiName,
    pub perform_locally: bool,
    pub external_url: Option<String>,
    pub api_key: Option<String>,
    pub model: AgentAPIModel,
    pub toolkit_permissions: Vec<String>,
    pub storage_bucket_permissions: Vec<String>,
    pub allowed_message_senders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentAPIModel {
    #[serde(rename = "openai")]
    OpenAI(OpenAI),
    #[serde(rename = "sleep")]
    Sleep(SleepAPI),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OpenAI {
    pub model_type: String,
}

use std::str::FromStr;

impl FromStr for AgentAPIModel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("openai:") {
            let model_type = s.strip_prefix("openai:").unwrap_or("").to_string();
            Ok(AgentAPIModel::OpenAI(OpenAI { model_type }))
        } else {
            Ok(AgentAPIModel::Sleep(SleepAPI {}))
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SleepAPI {}