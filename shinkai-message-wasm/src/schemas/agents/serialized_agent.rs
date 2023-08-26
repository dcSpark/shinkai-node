use serde::{Serialize, Deserialize};

use crate::schemas::shinkai_name::ShinkaiName;

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
pub enum AgentAPIModel {
    OpenAI(OpenAI),
    Sleep(SleepAPI),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OpenAI {
    pub model_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SleepAPI {}