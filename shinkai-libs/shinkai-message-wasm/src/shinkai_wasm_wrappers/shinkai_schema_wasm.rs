use serde_wasm_bindgen::{from_value, to_value};
use shinkai_message_primitives::schemas::shinkai_subscription_req::{
    FolderSubscription, PaymentOption, SubscriptionPayment,
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIAddAgentRequest, APIConvertFilesAndSaveToFolder, APICreateShareableFolder, APIGetMessagesFromInboxRequest,
    APIReadUpToTimeRequest, APIVecFSRetrieveVectorResource, APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder,
    APIVecFsCreateItem, APIVecFsDeleteFolder, APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson,
    APIVecFsRetrieveVectorSearchSimplifiedJson, JobMessage, RegistrationCodeRequest, TopicSubscription, WSMessage,
    WSMessageResponse,
};
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use wasm_bindgen::JsValue;

use super::{shinkai_wasm_error::ShinkaiWasmError, wasm_shinkai_message::SerdeWasmMethods};

impl SerdeWasmMethods for RegistrationCodeRequest {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
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
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
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
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
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
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
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
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
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
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFsRetrievePathSimplifiedJson {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIConvertFilesAndSaveToFolder {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFSRetrieveVectorResource {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFsRetrieveVectorSearchSimplifiedJson {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFsCreateFolder {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFsDeleteFolder {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFsMoveFolder {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFsCopyFolder {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFsCreateItem {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFsMoveItem {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APIVecFsCopyItem {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for TopicSubscription {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for WSMessage {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for WSMessageResponse {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::to_value(&self)?)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        Ok(serde_wasm_bindgen::from_value(j.clone())?)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        let json_str = serde_json::to_string(self).map_err(ShinkaiWasmError::from)?;
        Ok(json_str)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        let obj = serde_json::from_str(j).map_err(ShinkaiWasmError::from)?;
        Ok(obj)
    }
}

impl SerdeWasmMethods for APICreateShareableFolder {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        to_value(&self).map_err(ShinkaiWasmError::from)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        from_value(j.clone()).map_err(ShinkaiWasmError::from)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        serde_json::to_string(&self).map_err(ShinkaiWasmError::from)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        serde_json::from_str(j).map_err(ShinkaiWasmError::from)
    }
}

impl SerdeWasmMethods for FolderSubscription {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        to_value(&self).map_err(ShinkaiWasmError::from)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        from_value(j.clone()).map_err(ShinkaiWasmError::from)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        serde_json::to_string(&self).map_err(ShinkaiWasmError::from)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        serde_json::from_str(j).map_err(ShinkaiWasmError::from)
    }
}

impl SerdeWasmMethods for PaymentOption {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        to_value(&self).map_err(ShinkaiWasmError::from)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        from_value(j.clone()).map_err(ShinkaiWasmError::from)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        serde_json::to_string(&self).map_err(ShinkaiWasmError::from)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        serde_json::from_str(j).map_err(ShinkaiWasmError::from)
    }
}

impl SerdeWasmMethods for SubscriptionPayment {
    fn to_jsvalue(&self) -> Result<JsValue, ShinkaiWasmError> {
        to_value(&self).map_err(ShinkaiWasmError::from)
    }

    fn from_jsvalue(j: &JsValue) -> Result<Self, ShinkaiWasmError> {
        from_value(j.clone()).map_err(ShinkaiWasmError::from)
    }

    fn to_json_str(&self) -> Result<String, ShinkaiWasmError> {
        serde_json::to_string(&self).map_err(ShinkaiWasmError::from)
    }

    fn from_json_str(j: &str) -> Result<Self, ShinkaiWasmError> {
        serde_json::from_str(j).map_err(ShinkaiWasmError::from)
    }
}
