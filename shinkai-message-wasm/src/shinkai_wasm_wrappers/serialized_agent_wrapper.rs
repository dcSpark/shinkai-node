use crate::schemas::agents::serialized_agent::{AgentAPIModel, OpenAI, SerializedAgent, SleepAPI};
use crate::schemas::shinkai_name::ShinkaiName;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SerializedAgentWrapper {
    inner: SerializedAgent,
}

#[wasm_bindgen]
impl SerializedAgentWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(serialized_agent_js: &JsValue) -> Result<SerializedAgentWrapper, JsValue> {
        let serialized_agent = SerializedAgent::from_jsvalue(serialized_agent_js)?;
        Ok(SerializedAgentWrapper {
            inner: serialized_agent,
        })
    }

    #[wasm_bindgen(js_name = fromStrings)]
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
    ) -> Result<SerializedAgentWrapper, JsValue> {
        let inner = SerializedAgent::from_strings(
            id,
            full_identity_name,
            perform_locally,
            external_url,
            api_key,
            model,
            toolkit_permissions,
            storage_bucket_permissions,
            allowed_message_senders,
        )?;
        Ok(SerializedAgentWrapper { inner })
    }

    #[wasm_bindgen(method)]
    pub fn to_jsvalue(&self) -> Result<JsValue, JsValue> {
        self.inner.to_jsvalue()
    }

    #[wasm_bindgen(js_name = fromJsValue)]
    pub fn from_jsvalue(j: &JsValue) -> Result<SerializedAgentWrapper, JsValue> {
        let inner = SerializedAgent::from_jsvalue(j)?;
        Ok(SerializedAgentWrapper { inner })
    }

    #[wasm_bindgen]
    pub fn to_json_str(&self) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[wasm_bindgen]
    pub fn from_json_str(s: &str) -> Result<SerializedAgentWrapper, JsValue> {
        let inner: SerializedAgent = serde_json::from_str(s).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(SerializedAgentWrapper { inner })
    }

    #[wasm_bindgen(method, getter)]
    pub fn inner(&self) -> Result<JsValue, JsValue> {
        self.inner.to_jsvalue()
    }
}
