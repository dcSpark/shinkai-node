use crate::schemas::shinkai_name::ShinkaiName;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::JsValue;

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

impl SerializedAgent {
    pub fn from_jsvalue(j: &JsValue) -> Result<SerializedAgent, JsValue> {
        from_value(j.clone()).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn to_jsvalue(&self) -> Result<JsValue, JsValue> {
        to_value(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn to_json_str(&self) -> Result<String, JsValue> {
        serde_json::to_string(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn from_json_str(s: &str) -> Result<Self, JsValue> {
        serde_json::from_str(s).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn from_strings(
        id: String,
        full_identity_name: String,
        perform_locally: String,
        external_url: String,
        api_key: String,
        model: String,
        toolkit_permissions: String,
        storage_bucket_permissions: String,
        allowed_message_senders: String,
    ) -> Result<Self, JsValue> {
        // Convert the strings to the appropriate types
        let perform_locally = perform_locally
            .parse::<bool>()
            .map_err(|_| JsValue::from_str("Invalid perform_locally"))?;
        let external_url = if external_url.is_empty() {
            None
        } else {
            Some(external_url)
        };
        let api_key = if api_key.is_empty() { None } else { Some(api_key) };
        let model = model
            .parse::<AgentAPIModel>()
            .map_err(|_| JsValue::from_str("Invalid model"))?;
        let toolkit_permissions = toolkit_permissions.split(',').map(|s| s.to_string()).collect();
        let storage_bucket_permissions = storage_bucket_permissions.split(',').map(|s| s.to_string()).collect();
        let allowed_message_senders = allowed_message_senders.split(',').map(|s| s.to_string()).collect();

        Ok(SerializedAgent {
            id,
            full_identity_name: ShinkaiName::new(full_identity_name)?,
            perform_locally,
            external_url,
            api_key,
            model,
            toolkit_permissions,
            storage_bucket_permissions,
            allowed_message_senders,
        })
    }
}
