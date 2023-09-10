use serde::{Deserialize, Serialize};
use serde_wasm_bindgen;
use shinkai_message_primitives::{shinkai_message::shinkai_message_schemas::{JobScope, JobCreation, JobMessage}, schemas::inbox_name::InboxName};
use wasm_bindgen::prelude::*;

use crate::shinkai_wasm_wrappers::shinkai_wasm_error::ShinkaiWasmError;

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JobScopeWrapper {
    inner: JobScope,
}

#[wasm_bindgen]
impl JobScopeWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(buckets_js: &JsValue, documents_js: &JsValue) -> Result<JobScopeWrapper, JsValue> {
        let buckets: Vec<InboxName> = serde_wasm_bindgen::from_value(buckets_js.clone())?;
        let documents: Vec<String> = serde_wasm_bindgen::from_value(documents_js.clone())?;
        let job_scope = JobScope::new(Some(buckets), Some(documents));
        Ok(JobScopeWrapper { inner: job_scope })
    }

    #[wasm_bindgen]
    pub fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self.inner)?)
    }

    #[wasm_bindgen]
    pub fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(&self.inner).map_err(|e| ShinkaiWasmError::from(e))?;
        Ok(json_str)
    }
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JobCreationWrapper {
    inner: JobCreation,
}

#[wasm_bindgen]
impl JobCreationWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(scope_js: &JsValue) -> Result<JobCreationWrapper, JsValue> {
        let scope: JobScope = serde_wasm_bindgen::from_value(scope_js.clone())?;
        let job_creation = JobCreation { scope };
        Ok(JobCreationWrapper { inner: job_creation })
    }

    #[wasm_bindgen]
    pub fn to_jsvalue(&self) -> Result<JsValue, JsValue> {
        Ok(serde_wasm_bindgen::to_value(&self.inner)?)
    }

    #[wasm_bindgen]
    pub fn to_json_str(&self) -> Result<String, JsValue> {
        let json_str = serde_json::to_string(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(json_str)
    }

    #[wasm_bindgen(method, getter)]
    pub fn get_scope(&self) -> Result<JsValue, JsValue> {
        Ok(serde_wasm_bindgen::to_value(&self.inner.scope)?)
    }

    #[wasm_bindgen]
    pub fn from_json_str(s: &str) -> Result<JobCreationWrapper, JsValue> {
        let deserialized: JobCreation = serde_json::from_str(s).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(JobCreationWrapper { inner: deserialized })
    }

    #[wasm_bindgen]
    pub fn from_jsvalue(js_value: &JsValue) -> Result<JobCreationWrapper, JsValue> {
        let deserialized: JobCreation = serde_wasm_bindgen::from_value(js_value.clone())?;
        Ok(JobCreationWrapper { inner: deserialized })
    }

    #[wasm_bindgen(js_name = empty)]
    pub fn empty() -> Result<JobCreationWrapper, JsValue> {
        let buckets: Vec<InboxName> = Vec::new();
        let documents: Vec<String> = Vec::new();
        let job_scope = JobScope::new(Some(buckets), Some(documents));
        Ok(JobCreationWrapper { inner: JobCreation { scope: job_scope } })
    }
}

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JobMessageWrapper {
    inner: JobMessage,
}

#[wasm_bindgen]
impl JobMessageWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(job_id_js: &JsValue, content_js: &JsValue) -> Result<JobMessageWrapper, JsValue> {
        let job_id: String = serde_wasm_bindgen::from_value(job_id_js.clone())?;
        let content: String = serde_wasm_bindgen::from_value(content_js.clone())?;
        let job_message = JobMessage { job_id, content };
        Ok(JobMessageWrapper { inner: job_message })
    }

    #[wasm_bindgen]
    pub fn to_jsvalue(&self) -> Result<JsValue, JsValue> {
        Ok(serde_wasm_bindgen::to_value(&self.inner)?)
    }

    #[wasm_bindgen]
    pub fn to_json_str(&self) -> Result<String, JsValue> {
        let json_str = serde_json::to_string(&self.inner).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(json_str)
    }

    #[wasm_bindgen]
    pub fn from_json_str(s: &str) -> Result<JobMessageWrapper, JsValue> {
        let deserialized: JobMessage = serde_json::from_str(s).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(JobMessageWrapper { inner: deserialized })
    }

    #[wasm_bindgen]
    pub fn from_jsvalue(js_value: &JsValue) -> Result<JobMessageWrapper, JsValue> {
        let deserialized: JobMessage = serde_wasm_bindgen::from_value(js_value.clone())?;
        Ok(JobMessageWrapper { inner: deserialized })
    }

    #[wasm_bindgen(js_name = fromStrings)]
    pub fn from_strings(job_id: &str, content: &str) -> JobMessageWrapper {
        let job_message = JobMessage { job_id: job_id.to_string(), content: content.to_string() };
        JobMessageWrapper { inner: job_message }
    }
}