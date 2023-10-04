use anyhow::Result;
use shinkai_message_primitives::{shinkai_message::shinkai_message::{ExternalMetadata, InternalMetadata, ShinkaiBody, MessageData, ShinkaiMessage, MessageBody, ShinkaiVersion}, shinkai_utils::encryption::EncryptionMethod};
use wasm_bindgen::prelude::*;

use super::shinkai_wasm_error::ShinkaiWasmError;

pub trait SerdeWasmMethods {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError>;

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError>
    where
        Self: Sized;

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError>;

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError>
    where
        Self: Sized;
}

pub trait InternalMetadataMethods {
    fn new(
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        encryption: String,
        signature: String,
    ) -> Result<Self, ShinkaiWasmError>
    where
        Self: Sized;
}

impl InternalMetadataMethods for InternalMetadata {
    fn new(
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        encryption: String,
        signature: String,
    ) -> Result<Self, ShinkaiWasmError> {
        let encryption = EncryptionMethod::from_str(&encryption);
        Ok(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            signature,
            inbox,
            encryption,
        })
    }
}

impl SerdeWasmMethods for InternalMetadata {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let internal_metadata = serde_json::from_str(j).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(internal_metadata)
    }
}

pub trait ExternalMetadataMethods {
    fn new(
        sender: String,
        recipient: String,
        scheduled_time: String,
        signature: String,
        other: String,
        intra_sender: String,
    ) -> Self
    where
        Self: Sized;
}


impl ExternalMetadataMethods for ExternalMetadata {
    fn new(sender: String, recipient: String, scheduled_time: String, signature: String, other: String, intra_sender: String) -> Self {
        ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other,
            intra_sender
        }
    }
}

impl SerdeWasmMethods for ExternalMetadata {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let external_metadata = serde_json::from_str(j).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(external_metadata)
    }
}

pub trait ShinkaiBodyMethods {
    fn new(message_data: MessageData, internal_metadata: InternalMetadata) -> Self
    where
        Self: Sized;
}

impl ShinkaiBodyMethods for ShinkaiBody {
    fn new(message_data: MessageData, internal_metadata: InternalMetadata) -> Self {
        ShinkaiBody {
            message_data,
            internal_metadata,
        }
    }
}

impl SerdeWasmMethods for ShinkaiBody {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let body = serde_json::from_str(j).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(body)
    }
}

pub trait ShinkaiMessageMethods {
    fn new(
        message_body: MessageBody,
        external_metadata: ExternalMetadata,
        encryption: EncryptionMethod,
        version: Option<ShinkaiVersion>,
    ) -> Self
    where
        Self: Sized;
}

impl ShinkaiMessageMethods for ShinkaiMessage {
    fn new(
        message_body: MessageBody,
        external_metadata: ExternalMetadata,
        encryption: EncryptionMethod,
        version: Option<ShinkaiVersion>,
    ) -> Self {
        ShinkaiMessage {
            body: message_body,
            external_metadata,
            encryption,
            version: version.unwrap_or(ShinkaiVersion::V1_0),
        }
    }
}

impl SerdeWasmMethods for ShinkaiMessage {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let shinkai_message = serde_json::from_str(j).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(shinkai_message)
    }
}