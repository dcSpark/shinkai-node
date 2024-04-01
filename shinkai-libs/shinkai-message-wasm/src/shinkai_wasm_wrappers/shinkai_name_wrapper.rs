use serde::{Deserialize, Serialize};
use serde_wasm_bindgen;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use wasm_bindgen::prelude::*;

use crate::shinkai_wasm_wrappers::shinkai_wasm_error::ShinkaiWasmError;

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShinkaiNameWrapper {
    inner: ShinkaiName,
}

#[wasm_bindgen]
impl ShinkaiNameWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(shinkai_name_js: &JsValue) -> Result<ShinkaiNameWrapper, JsValue> {
        let shinkai_name_str = shinkai_name_js
            .as_string()
            .ok_or_else(|| JsValue::from_str("Expected a string for shinkai_name_js"))?;
        let shinkai_name = ShinkaiName::new(shinkai_name_str)?;
        Ok(ShinkaiNameWrapper { inner: shinkai_name })
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_full_name(&self) -> JsValue {
        JsValue::from_str(&self.inner.full_name)
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_node_name_string(&self) -> JsValue {
        JsValue::from_str(&self.inner.node_name)
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_profile_name_string(&self) -> JsValue {
        match &self.inner.profile_name {
            Some(profile_name) => JsValue::from_str(profile_name),
            None => JsValue::NULL,
        }
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_subidentity_type(&self) -> JsValue {
        match &self.inner.subidentity_type {
            Some(subidentity_type) => JsValue::from_str(&subidentity_type.to_string()),
            None => JsValue::NULL,
        }
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_subidentity_name(&self) -> JsValue {
        match &self.inner.subidentity_name {
            Some(subidentity_name) => JsValue::from_str(subidentity_name),
            None => JsValue::NULL,
        }
    }

    #[wasm_bindgen]
    pub fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    #[wasm_bindgen]
    pub fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(json_str)
    }

    #[wasm_bindgen]
    pub fn extract_profile(&self) -> Result<ShinkaiNameWrapper, JsValue> {
        let profile = self.inner.extract_profile().map_err(|e| JsValue::from_str(&e))?;
        Ok(ShinkaiNameWrapper { inner: profile })
    }

    #[wasm_bindgen]
    pub fn extract_node(&self) -> ShinkaiNameWrapper {
        let node = self.inner.extract_node();
        ShinkaiNameWrapper { inner: node }
    }
}
