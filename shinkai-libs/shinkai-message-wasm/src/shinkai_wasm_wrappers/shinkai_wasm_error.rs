use wasm_bindgen::prelude::*;
use thiserror::Error;
use std::error::Error;
use std::fmt;

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
    #[error("{0}")]
    Other(String),
}

impl WasmErrorWrapper {
    pub fn new(error: ShinkaiWasmError) -> Self {
        WasmErrorWrapper(error)
    }
}

impl Into<JsValue> for ShinkaiWasmError {
    fn into(self) -> JsValue {
        // Convert the ShinkaiWasmError into a string and then into a JsValue
        JsValue::from_str(&self.to_string())
    }
}
#[derive(Debug)]
pub struct WasmErrorWrapper(pub ShinkaiWasmError);

impl fmt::Display for WasmErrorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for WasmErrorWrapper {}

impl From<ShinkaiWasmError> for WasmErrorWrapper {
    fn from(error: ShinkaiWasmError) -> Self {
        WasmErrorWrapper(error)
    }
}

impl From<WasmErrorWrapper> for JsValue {
    fn from(error: WasmErrorWrapper) -> Self {
        JsValue::from_str(&error.to_string())
    }
}

impl From<JsValue> for ShinkaiWasmError {
    fn from(error: JsValue) -> Self {
        ShinkaiWasmError::Other(error.as_string().unwrap_or_else(|| "Unknown error".to_string()))
    }
}