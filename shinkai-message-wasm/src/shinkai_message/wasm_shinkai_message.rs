use crate::shinkai_utils::encryption::EncryptionMethod;

use super::{
    shinkai_message::{ShinkaiBody, ExternalMetadata, InternalMetadata, ShinkaiMessage, ShinkaiVersion, MessageData, MessageBody},
    shinkai_message_schemas::MessageSchemaType,
};
use anyhow::Result;
use thiserror::Error;
use wasm_bindgen::prelude::*;

#[derive(Error, Debug)]
pub enum ShinkaiMessageWasmError {
    #[error("Failed to parse MessageSchemaType: {0}")]
    MessageSchemaTypeParseError(String),
    #[error("Failed to serialize/deserialize with Serde JSON: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Failed to serialize/deserialize with Serde WASM: {0}")]
    SerdeWasmBindgenError(#[from] serde_wasm_bindgen::Error),
    #[error("JsValue was not a string")]
    JsValueNotString,
}

impl InternalMetadata {
    pub fn new(
        sender_subidentity: String,
        recipient_subidentity: String,
        // message_schema_type: String,
        inbox: String,
        encryption: String,
        signature: String,
    ) -> Result<Self, ShinkaiMessageWasmError> {
        // let message_schema_type = MessageSchemaType::from_str(&message_schema_type)
            // .ok_or(ShinkaiMessageWasmError::MessageSchemaTypeParseError(message_schema_type.clone()))?;
        let encryption = EncryptionMethod::from_str(&encryption);

        Ok(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            // message_schema_type,
            signature,
            inbox,
            encryption,
        })
    }

    pub fn to_jsvalue(&self) -> Result<JsValue, ShinkaiMessageWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    pub fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiMessageWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    pub fn to_json_str(&self) -> Result<String, ShinkaiMessageWasmError> {
        let json_str = serde_json::to_string(self).map_err(|e| ShinkaiMessageWasmError::from(e))?;
        Ok(json_str)
    }

    pub fn from_json_str(j: &str) -> Result<Self, ShinkaiMessageWasmError> {
        let internal_metadata = serde_json::from_str(j).map_err(|e| ShinkaiMessageWasmError::from(e))?;
        Ok(internal_metadata)
    }
}

impl ExternalMetadata {
    pub fn new(sender: String, recipient: String, scheduled_time: String, signature: String, other: String) -> Self {
        ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other,
        }
    }

    pub fn to_jsvalue(&self) -> Result<JsValue, ShinkaiMessageWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    pub fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiMessageWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    pub fn to_json_str(&self) -> Result<String, ShinkaiMessageWasmError> {
        let json_str = serde_json::to_string(self).map_err(|e| ShinkaiMessageWasmError::from(e))?;
        Ok(json_str)
    }

    pub fn from_json_str(j: &str) -> Result<Self, ShinkaiMessageWasmError> {
        let external_metadata = serde_json::from_str(j).map_err(|e| ShinkaiMessageWasmError::from(e))?;
        Ok(external_metadata)
    }
}

impl ShinkaiBody {
    pub fn new(message_data: MessageData, internal_metadata: InternalMetadata) -> Self {
        ShinkaiBody {
            message_data,
            internal_metadata,
        }
    }

    pub fn to_jsvalue(&self) -> Result<JsValue, ShinkaiMessageWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    pub fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiMessageWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    pub fn to_json_str(&self) -> Result<String, ShinkaiMessageWasmError> {
        let json_str = serde_json::to_string(self).map_err(|e| ShinkaiMessageWasmError::from(e))?;
        Ok(json_str)
    }

    pub fn from_json_str(j: &str) -> Result<Self, ShinkaiMessageWasmError> {
        let body = serde_json::from_str(j).map_err(|e| ShinkaiMessageWasmError::from(e))?;
        Ok(body)
    }
}

impl ShinkaiMessage {
    pub fn new(message_body: MessageBody, external_metadata: ExternalMetadata, encryption: EncryptionMethod, version: Option<ShinkaiVersion>) -> Self {
        ShinkaiMessage {
            body: message_body,
            external_metadata,
            encryption,
            version: version.unwrap_or(ShinkaiVersion::V1_0),
        }
    }

    pub fn from_json_str(j: &str) -> Result<Self, ShinkaiMessageWasmError> {
        let shinkai_message = serde_json::from_str(j).map_err(|e| ShinkaiMessageWasmError::from(e))?;
        Ok(shinkai_message)
    }

    pub fn to_jsvalue(&self) -> Result<JsValue, JsValue> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    pub fn to_json_str(&self) -> Result<String, ShinkaiMessageWasmError> {
        let json_str = serde_json::to_string(self).map_err(|e| ShinkaiMessageWasmError::from(e))?;
        Ok(json_str)
    }

    pub fn from_jsvalue(j: &JsValue) -> Result<ShinkaiMessage, JsValue> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }
}
