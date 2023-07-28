use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InternalMetadata {
    pub sender_subidentity: String,
    pub recipient_subidentity: String,
    pub message_schema_type: String,
    pub inbox: String,
    pub encryption: String,
    // TODO: add sub_signature
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalMetadata {
    pub sender: String,
    pub recipient: String,
    pub scheduled_time: String,
    pub signature: String,
    pub other: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub content: String,
    pub internal_metadata: Option<InternalMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShinkaiMessage {
    pub body: Option<Body>,
    pub external_metadata: Option<ExternalMetadata>,
    pub encryption: String,
}
