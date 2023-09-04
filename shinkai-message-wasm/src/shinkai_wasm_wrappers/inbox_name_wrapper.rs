use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::{schemas::inbox_name::InboxName, shinkai_message::shinkai_wasm_error::ShinkaiWasmError};

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InboxNameWrapper {
    inner: InboxName,
}

#[wasm_bindgen]
impl InboxNameWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(inbox_name_js: &JsValue) -> Result<InboxNameWrapper, JsValue> {
        let shinkai_name_str = inbox_name_js
            .as_string()
            .ok_or_else(|| JsValue::from_str("Expected a string for shinkai_name_js"))?;
        let shinkai_name = InboxName::new(shinkai_name_str)?;
        Ok(InboxNameWrapper { inner: shinkai_name })
    }

    #[wasm_bindgen(method, getter)]
    pub fn to_string(&self) -> Result<JsValue, JsValue> {
        let string_value = match &self.inner {
            InboxName::RegularInbox { value, .. } => value.clone(),
            InboxName::JobInbox { value, .. } => value.clone(),
        };
        Ok(JsValue::from_str(&string_value))
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_value(&self) -> JsValue {
        match &self.inner {
            InboxName::RegularInbox { value, .. } => JsValue::from_str(value),
            InboxName::JobInbox { value, .. } => JsValue::from_str(value),
        }
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_is_e2e(&self) -> bool {
        match &self.inner {
            InboxName::RegularInbox { is_e2e, .. } => *is_e2e,
            InboxName::JobInbox { is_e2e, .. } => *is_e2e,
        }
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_identities(&self) -> Result<JsValue, JsValue> {
        match &self.inner {
            InboxName::RegularInbox { identities, .. } => {
                let identities_str: Vec<String> = identities.iter().map(|i| i.to_string()).collect();
                serde_wasm_bindgen::to_value(&identities_str).map_err(JsValue::from)
            }
            InboxName::JobInbox { .. } => Ok(JsValue::NULL),
        }
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_unique_id(&self) -> JsValue {
        match &self.inner {
            InboxName::JobInbox { unique_id, .. } => JsValue::from_str(unique_id),
            InboxName::RegularInbox { .. } => JsValue::NULL,
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
    pub fn get_regular_inbox_name_from_params(
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
        is_e2e: bool,
    ) -> Result<InboxNameWrapper, JsValue> {
        let inbox_name = InboxName::get_regular_inbox_name_from_params(
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            is_e2e,
        )?;
        Ok(InboxNameWrapper { inner: inbox_name })
    }

    #[wasm_bindgen]
    pub fn get_job_inbox_name_from_params(unique_id: String) -> Result<InboxNameWrapper, JsValue> {
        let inbox_name = InboxName::get_job_inbox_name_from_params(unique_id)?;
        Ok(InboxNameWrapper { inner: inbox_name })
    }

    #[wasm_bindgen]
    pub fn get_inner(&self) -> JsValue {
        self.to_jsvalue().unwrap()
    }
}
