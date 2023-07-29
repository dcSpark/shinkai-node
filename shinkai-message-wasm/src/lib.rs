use shinkai_message::{shinkai_message::{InternalMetadata, ExternalMetadata, Body, ShinkaiMessage}, shinkai_message_schemas::MessageSchemaType};
use shinkai_utils::encryption::EncryptionMethod;
use wasm_bindgen::prelude::*;

pub mod shinkai_message;
pub mod schemas;
pub mod shinkai_wasm_wrappers;
pub mod shinkai_utils;

pub use crate::shinkai_wasm_wrappers::shinkai_message_wrapper::ShinkaiMessageWrapper;
pub use crate::shinkai_wasm_wrappers::shinkai_message_builder_wrapper::ShinkaiMessageBuilderWrapper;

// TODO: this needs to use shinkai message builder or something
// Expose a function that creates a new ShinkaiMessage
#[wasm_bindgen]
pub fn create_message() -> Vec<u8> {
    let internal_metadata = InternalMetadata {
        sender_subidentity: "sender_subidentity".to_string(),
        recipient_subidentity: "recipient_subidentity".to_string(),
        message_schema_type: MessageSchemaType::TextContent,
        inbox: "inbox".to_string(),
        encryption: EncryptionMethod::None,
    };

    let external_metadata = ExternalMetadata {
        sender: "sender".to_string(),
        recipient: "recipient".to_string(),
        scheduled_time: "scheduled_time".to_string(),
        signature: "signature".to_string(),
        other: "other".to_string(),
    };

    let body = Body {
        content: "content".to_string(),
        internal_metadata: Some(internal_metadata),
    };

    let shinkai_message = ShinkaiMessage {
        body: Some(body),
        external_metadata: Some(external_metadata),
        encryption: EncryptionMethod::None,
    };

    let mut buf = vec![];
    // shinkai_message.encode(&mut buf).unwrap();

    buf
}
