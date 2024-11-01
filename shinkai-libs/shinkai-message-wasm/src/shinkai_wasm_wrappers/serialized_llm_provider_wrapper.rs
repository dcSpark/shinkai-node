use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use shinkai_message_primitives::schemas::{
    llm_providers::serialized_llm_provider::{LLMProviderInterface, SerializedLLMProvider},
    shinkai_name::ShinkaiName,
};
use wasm_bindgen::prelude::*;

pub trait SerializedLLMProviderJsValueConversion {
    fn from_jsvalue(j: &JsValue) -> Result<Self, JsValue>
    where
        Self: Sized;
    fn to_jsvalue(&self) -> Result<JsValue, JsValue>;
    fn to_json_str(&self) -> Result<String, JsValue>;
    fn from_json_str(s: &str) -> Result<Self, JsValue>
    where
        Self: Sized;
    #[allow(clippy::too_many_arguments)]
    fn from_strings(
        id: String,
        full_identity_name: String,
        external_url: String,
        api_key: String,
        model: String,
    ) -> Result<Self, JsValue>
    where
        Self: Sized;
}

impl SerializedLLMProviderJsValueConversion for SerializedLLMProvider {
    fn from_jsvalue(j: &JsValue) -> Result<Self, JsValue> {
        from_value(j.clone()).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    fn to_jsvalue(&self) -> Result<JsValue, JsValue> {
        to_value(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    fn to_json_str(&self) -> Result<String, JsValue> {
        serde_json::to_string(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    fn from_json_str(s: &str) -> Result<Self, JsValue> {
        serde_json::from_str(s).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[allow(clippy::too_many_arguments)]
    fn from_strings(
        id: String,
        full_identity_name: String,
        external_url: String,
        api_key: String,
        model: String,
    ) -> Result<Self, JsValue> {
        // Convert the strings to the appropriate types
        let external_url = if external_url.is_empty() {
            None
        } else {
            Some(external_url)
        };
        let api_key = if api_key.is_empty() { None } else { Some(api_key) };
        let model = model
            .parse::<LLMProviderInterface>()
            .map_err(|_| JsValue::from_str("Invalid model"))?;

        Ok(SerializedLLMProvider {
            id,
            full_identity_name: ShinkaiName::new(full_identity_name)?,
            external_url,
            api_key,
            model,
        })
    }
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SerializedLLMProviderWrapper {
    inner: SerializedLLMProvider,
}

#[wasm_bindgen]
impl SerializedLLMProviderWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(serialized_agent_js: &JsValue) -> Result<SerializedLLMProviderWrapper, JsValue> {
        let serialized_agent = SerializedLLMProvider::from_jsvalue(serialized_agent_js)?;
        Ok(SerializedLLMProviderWrapper {
            inner: serialized_agent,
        })
    }

    #[wasm_bindgen(js_name = fromStrings)]
    #[allow(clippy::too_many_arguments)]
    pub fn from_strings(
        id: String,
        full_identity_name: String,
        external_url: String,
        api_key: String,
        model: String,
    ) -> Result<SerializedLLMProviderWrapper, JsValue> {
        let inner = SerializedLLMProvider::from_strings(
            id,
            full_identity_name,
            external_url,
            api_key,
            model,
        )?;
        Ok(SerializedLLMProviderWrapper { inner })
    }

    #[wasm_bindgen(method)]
    pub fn to_jsvalue(&self) -> Result<JsValue, JsValue> {
        self.inner.to_jsvalue()
    }

    #[wasm_bindgen(js_name = fromJsValue)]
    pub fn from_jsvalue(j: &JsValue) -> Result<SerializedLLMProviderWrapper, JsValue> {
        let inner = SerializedLLMProvider::from_jsvalue(j)?;
        Ok(SerializedLLMProviderWrapper { inner })
    }

    #[wasm_bindgen]
    pub fn to_json_str(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn from_json_str(s: &str) -> Result<SerializedLLMProviderWrapper, JsValue> {
        let inner: SerializedLLMProvider = serde_json::from_str(s).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(SerializedLLMProviderWrapper { inner })
    }

    #[wasm_bindgen(method, getter)]
    pub fn inner(&self) -> Result<JsValue, JsValue> {
        self.inner.to_jsvalue()
    }
}
