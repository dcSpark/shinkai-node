use crate::shinkai_message::shinkai_message::{ShinkaiMessage, Body, ExternalMetadata};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct ShinkaiMessageWrapper {
    inner: ShinkaiMessage,
}

#[wasm_bindgen]
impl ShinkaiMessageWrapper {
    #[wasm_bindgen(constructor)]
    pub fn new(body: JsValue, external_metadata: JsValue, encryption: String) -> ShinkaiMessageWrapper {
        let body = Body::from_jsvalue(&body);
        let external_metadata = ExternalMetadata::from_jsvalue(&external_metadata);
        let shinkai_message = ShinkaiMessage::new(Some(body), Some(external_metadata), encryption);
        
        ShinkaiMessageWrapper {
            inner: shinkai_message,
        }
    }

    #[wasm_bindgen(method, getter)]
    pub fn body(&self) -> JsValue {
        self.inner.body.clone().unwrap().to_jsvalue()
    }

    #[wasm_bindgen(method, setter)]
    pub fn set_body(&mut self, body: JsValue) {
        self.inner.body = Some(Body::from_jsvalue(&body));
    }

    #[wasm_bindgen(method, getter)]
    pub fn external_metadata(&self) -> JsValue {
        self.inner.external_metadata.clone().unwrap().to_jsvalue()
    }

    #[wasm_bindgen(method, setter)]
    pub fn set_external_metadata(&mut self, external_metadata: JsValue) {
        self.inner.external_metadata = Some(ExternalMetadata::from_jsvalue(&external_metadata));
    }

    #[wasm_bindgen(method, getter)]
    pub fn encryption(&self) -> String {
        self.inner.encryption.clone()
    }

    #[wasm_bindgen(method, setter)]
    pub fn set_encryption(&mut self, encryption: String) {
        self.inner.encryption = encryption;
    }

    #[wasm_bindgen(method)]
    pub fn to_jsvalue(&self) -> JsValue {
        self.inner.to_jsvalue()
    }

    #[wasm_bindgen(js_name = fromJsValue)]
    pub fn from_jsvalue(j: &JsValue) -> ShinkaiMessageWrapper {
        let inner = ShinkaiMessage::from_jsvalue(j);
        ShinkaiMessageWrapper { inner }
    }
}
