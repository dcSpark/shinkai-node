use shinkai_message_primitives::shinkai_message::{shinkai_message_schemas::{RegistrationCodeRequest, APIAddAgentRequest, APIReadUpToTimeRequest, APIGetMessagesFromInboxRequest, JobRecipient, JobPreMessage, JobToolCall, JobMessage, JobScope}, shinkai_message::ShinkaiMessage};
use wasm_bindgen::JsValue;

use super::{wasm_shinkai_message::SerdeWasmMethods, shinkai_wasm_error::ShinkaiWasmError};

impl SerdeWasmMethods for RegistrationCodeRequest {
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

impl SerdeWasmMethods for APIAddAgentRequest {
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

impl SerdeWasmMethods for APIReadUpToTimeRequest {
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

impl SerdeWasmMethods for APIGetMessagesFromInboxRequest {
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

impl SerdeWasmMethods for JobRecipient {
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

impl SerdeWasmMethods for JobPreMessage {
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

impl SerdeWasmMethods for JobToolCall {
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

impl SerdeWasmMethods for JobMessage {
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

impl SerdeWasmMethods for JobScope {
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