use serde::{Deserialize, Serialize};
use serde_wasm_bindgen;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::{
    schemas::inbox_name::InboxName,
    shinkai_message::shinkai_message_schemas::{JobCreationInfo, JobMessage},
};
use wasm_bindgen::prelude::*;

use crate::shinkai_wasm_wrappers::shinkai_wasm_error::ShinkaiWasmError;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::AssociatedUI;

#[wasm_bindgen]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JobScopeWrapper {
    inner: JobScope,
}

#[wasm_bindgen]
impl JobScopeWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(buckets_js: &JsValue, documents_js: &JsValue) -> Result<JobScopeWrapper, JsValue> {
        let job_scope = JobScope::new_default();
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
    inner: JobCreationInfo,
}

#[wasm_bindgen]
impl JobCreationWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(scope_js: &JsValue, is_hidden: bool, associated_ui_js: JsValue) -> Result<JobCreationWrapper, JsValue> {
        let scope: JobScope = serde_wasm_bindgen::from_value(scope_js.clone())?;
        let associated_ui: Option<AssociatedUI> = if associated_ui_js.is_null() || associated_ui_js.is_undefined() {
            None
        } else {
            Some(serde_wasm_bindgen::from_value(associated_ui_js)?)
        };
        let job_creation = JobCreationInfo {
            scope,
            is_hidden: Some(is_hidden),
            associated_ui,
        };
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
        let deserialized: JobCreationInfo = serde_json::from_str(s).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(JobCreationWrapper { inner: deserialized })
    }

    #[wasm_bindgen]
    pub fn from_jsvalue(js_value: &JsValue) -> Result<JobCreationWrapper, JsValue> {
        let deserialized: JobCreationInfo = serde_wasm_bindgen::from_value(js_value.clone())?;
        Ok(JobCreationWrapper { inner: deserialized })
    }

    #[wasm_bindgen(js_name = empty)]
    pub fn empty() -> Result<JobCreationWrapper, JsValue> {
        let job_scope = JobScope::new_default();
        Ok(JobCreationWrapper {
            inner: JobCreationInfo {
                scope: job_scope,
                is_hidden: Some(false),
                associated_ui: None,
            },
        })
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
    pub fn new(
        job_id_js: &JsValue,
        content_js: &JsValue,
        files_inbox: &JsValue,
        parent: &JsValue,
        workflow_code: &JsValue,
        workflow_name: &JsValue,
    ) -> Result<JobMessageWrapper, JsValue> {
        let job_id: String = serde_wasm_bindgen::from_value(job_id_js.clone())?;
        let content: String = serde_wasm_bindgen::from_value(content_js.clone())?;
        let files_inbox: String = serde_wasm_bindgen::from_value(files_inbox.clone())?;
        let parent: Option<String> = if parent.is_null() || parent.is_undefined() {
            None
        } else {
            Some(serde_wasm_bindgen::from_value(parent.clone())?)
        };
        let workflow_code: Option<String> = if workflow_code.is_null() || workflow_code.is_undefined() {
            None
        } else {
            Some(serde_wasm_bindgen::from_value(workflow_code.clone())?)
        };
        let workflow_name: Option<String> = if workflow_name.is_null() || workflow_name.is_undefined() {
            None
        } else {
            Some(serde_wasm_bindgen::from_value(workflow_name.clone())?)
        };
        let job_message = JobMessage {
            job_id,
            content,
            files_inbox,
            parent,
            workflow_code,
            workflow_name,
            sheet_job_data: None,
            callback: None,
        };
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
    pub fn from_strings(
        job_id: &str,
        content: &str,
        files_inbox: &str,
        parent: &str,
        workflow_code: Option<String>,
        workflow_name: Option<String>,
    ) -> JobMessageWrapper {
        let job_message = JobMessage {
            job_id: job_id.to_string(),
            content: content.to_string(),
            files_inbox: files_inbox.to_string(),
            parent: Some(parent.to_string()),
            workflow_code,
            workflow_name,
            sheet_job_data: None,
            callback: None,
        };
        JobMessageWrapper { inner: job_message }
    }
}
