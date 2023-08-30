use wasm_bindgen::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShinkaiWasmError {
    #[error("Failed to parse MessageSchemaType: {0}")]
    MessageSchemaTypeParseError(String),
    #[error("Failed to serialize/deserialize with Serde JSON: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Failed to serialize/deserialize with Serde WASM: {0}")]
    SerdeWasmBindgenError(#[from] serde_wasm_bindgen::Error),
    #[error("JsValue was not a string")]
    JsValueNotString,
}

impl Into<JsValue> for ShinkaiWasmError {
    fn into(self) -> JsValue {
        // Convert the ShinkaiWasmError into a string and then into a JsValue
        JsValue::from_str(&self.to_string())
    }
}
