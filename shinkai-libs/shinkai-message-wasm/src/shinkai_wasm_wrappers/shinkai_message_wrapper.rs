use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shinkai_message_primitives::{shinkai_message::shinkai_message::{ShinkaiMessage, MessageBody, ShinkaiBody, ExternalMetadata}, shinkai_utils::encryption::EncryptionMethod};
use wasm_bindgen::prelude::*;
use crate::shinkai_wasm_wrappers::wasm_shinkai_message::SerdeWasmMethods;

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShinkaiMessageWrapper {
    inner: ShinkaiMessage,
}

#[wasm_bindgen]
impl ShinkaiMessageWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(shinkai_message_js: &JsValue) -> Result<ShinkaiMessageWrapper, JsValue> {
        match ShinkaiMessage::from_jsvalue(shinkai_message_js) {
            Ok(shinkai_message) => Ok(ShinkaiMessageWrapper { inner: shinkai_message }),
            Err(e) => Err(JsValue::from_str(&e.to_string())),
        }
    }

    #[wasm_bindgen(method, getter)]
    pub fn message_body(&self) -> Result<JsValue, JsValue> {
        match &self.inner.body {
            MessageBody::Unencrypted(body) => {
                body.to_jsvalue().map_err(|e| JsValue::from_str(&e.to_string()))
            },
            // Add other variants of MessageBody if needed
            _ => Err(JsValue::from_str("Unsupported MessageBody variant")),
        }
    }

    #[wasm_bindgen(method, setter)]
    pub fn set_message_body(&mut self, body: JsValue) -> Result<(), JsValue> {
        let body = ShinkaiBody::from_jsvalue(&body).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.inner.body = MessageBody::Unencrypted(body);
        Ok(())
    }

    #[wasm_bindgen(method, getter)]
    pub fn external_metadata(&self) -> Result<JsValue, JsValue> {
        self.inner
            .external_metadata
            .clone()
            .to_jsvalue()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(method, setter)]
    pub fn set_external_metadata(&mut self, external_metadata: JsValue) -> Result<(), JsValue> {
        let external_metadata =
            ExternalMetadata::from_jsvalue(&external_metadata).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.inner.external_metadata = external_metadata;
        Ok(())
    }

    #[wasm_bindgen(method, getter)]
    pub fn encryption(&self) -> String {
        self.inner.encryption.as_str().to_owned()
    }

    #[wasm_bindgen(method, setter)]
    pub fn set_encryption(&mut self, encryption: String) {
        self.inner.encryption = EncryptionMethod::from_str(&encryption);
    }

    #[wasm_bindgen(method)]
    pub fn to_jsvalue(&self) -> Result<JsValue, JsValue> {
        self.inner.to_jsvalue().map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen(js_name = fromJsValue)]
    pub fn from_jsvalue(j: &JsValue) -> Result<ShinkaiMessageWrapper, JsValue> {
        match ShinkaiMessage::from_jsvalue(j) {
            Ok(inner) => Ok(ShinkaiMessageWrapper { inner }),
            Err(e) => Err(JsValue::from_str(&e.to_string())),
        }
    }

    #[wasm_bindgen]
    pub fn to_json_str(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn from_json_str(s: &str) -> Result<ShinkaiMessageWrapper, JsValue> {
        let inner: ShinkaiMessage = serde_json::from_str(s).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(ShinkaiMessageWrapper { inner })
    }

    #[wasm_bindgen]
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();

        hasher.update(format!("{:?}", self.inner));
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    #[wasm_bindgen]
    pub fn generate_time_now() -> String {
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%f").to_string();
        let scheduled_time = format!("{}{}", &timestamp[..17], &timestamp[17..20]);
        scheduled_time
    }
}
