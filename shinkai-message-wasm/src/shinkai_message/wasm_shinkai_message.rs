use super::shinkai_message::{Body, ExternalMetadata, InternalMetadata, ShinkaiMessage};
use serde_json::json;
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

impl InternalMetadata {
    pub fn new(
        sender_subidentity: String,
        recipient_subidentity: String,
        message_schema_type: String,
        inbox: String,
        encryption: String,
    ) -> Self {
        InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            message_schema_type,
            inbox,
            encryption,
        }
    }

    pub fn to_jsvalue(&self) -> JsValue {
        let s = serde_json::to_string(self).unwrap();
        JsValue::from_str(&s)
    }

    pub fn from_jsvalue(j: &JsValue) -> Self {
        let s = j.as_string().unwrap();
        serde_json::from_str(&s).unwrap()
    }
}

impl ExternalMetadata {
    pub fn new(sender: String, recipient: String, scheduled_time: String, signature: String, other: String) -> Self {
        ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other,
        }
    }

    pub fn to_jsvalue(&self) -> JsValue {
        let s = serde_json::to_string(self).unwrap();
        JsValue::from_str(&s)
    }

    pub fn from_jsvalue(j: &JsValue) -> Self {
        let s = j.as_string().unwrap();
        serde_json::from_str(&s).unwrap()
    }
}

impl Body {
    pub fn new(content: String, internal_metadata: Option<InternalMetadata>) -> Self {
        Body {
            content,
            internal_metadata,
        }
    }

    pub fn to_jsvalue(&self) -> JsValue {
        let internal_metadata = match &self.internal_metadata {
            Some(v) => serde_json::to_string(v).unwrap(),
            None => String::from("null"),
        };

        let result = json!({
            "content": self.content,
            "internal_metadata": internal_metadata
        });

        to_value(&result).unwrap()
    }

    pub fn from_jsvalue(j: &JsValue) -> Self {
        let parsed: serde_json::Value = from_value(j.clone()).unwrap();
        let content = parsed["content"].as_str().unwrap().to_string();
        let internal_metadata_str = parsed["internal_metadata"].as_str().unwrap();

        let internal_metadata = if internal_metadata_str == "null" {
            None
        } else {
            let internal_metadata: InternalMetadata = serde_json::from_str(internal_metadata_str).unwrap();
            Some(internal_metadata)
        };

        Body {
            content,
            internal_metadata,
        }
    }
}

impl ShinkaiMessage {
    pub fn new(body: Option<Body>, external_metadata: Option<ExternalMetadata>, encryption: String) -> Self {
        ShinkaiMessage {
            body,
            external_metadata,
            encryption
        }
    }

    pub fn to_jsvalue(&self) -> JsValue {
        let body = match &self.body {
            Some(v) => serde_wasm_bindgen::from_value(v.to_jsvalue()).unwrap(),
            None => serde_json::Value::Null
        };

        let external_metadata = match &self.external_metadata {
            Some(v) => serde_wasm_bindgen::from_value(v.to_jsvalue()).unwrap(),
            None => serde_json::Value::Null
        };

        let result = json!({
            "body": body,
            "external_metadata": external_metadata,
            "encryption": self.encryption
        });

        to_value(&result).unwrap()
    }

    pub fn from_jsvalue(j: &JsValue) -> Self {
        let parsed: serde_json::Value = from_value(j.clone()).unwrap();
        let encryption = parsed["encryption"].as_str().unwrap().to_string();

        let body = match parsed["body"].is_null() {
            false => {
                let body_value: serde_json::Value = parsed["body"].clone();
                let body_jsvalue: JsValue = to_value(&body_value).unwrap();
                Some(Body::from_jsvalue(&body_jsvalue))
            },
            true => None
        };

        let external_metadata = match parsed["external_metadata"].is_null() {
            false => {
                let external_metadata_value: serde_json::Value = parsed["external_metadata"].clone();
                let external_metadata_jsvalue: JsValue = to_value(&external_metadata_value).unwrap();
                Some(ExternalMetadata::from_jsvalue(&external_metadata_jsvalue))
            },
            true => None
        };

        ShinkaiMessage {
            body,
            external_metadata,
            encryption
        }
    }
}
