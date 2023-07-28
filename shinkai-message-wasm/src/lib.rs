use schemas::shinkai_message::{InternalMetadata, ExternalMetadata, Body, ShinkaiMessage};
use shinkai_message::{shinkai_message_builder::ShinkaiMessageBuilder, encryption::{EncryptionMethod, unsafe_deterministic_encryption_keypair}, signatures::unsafe_deterministic_signature_keypair};
use wasm_bindgen::prelude::*;

pub mod shinkai_message;
pub mod schemas;

// TODO: this needs to use shinkai message builder or something
// Expose a function that creates a new ShinkaiMessage
#[wasm_bindgen]
pub fn create_message() -> Vec<u8> {
    let internal_metadata = InternalMetadata {
        sender_subidentity: "sender_subidentity".to_string(),
        recipient_subidentity: "recipient_subidentity".to_string(),
        message_schema_type: "message_schema_type".to_string(),
        inbox: "inbox".to_string(),
        encryption: "encryption".to_string(),
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
        encryption: "encryption".to_string(),
    };

    let mut buf = vec![];
    // shinkai_message.encode(&mut buf).unwrap();

    buf
}

// Expose a function that parses a ShinkaiMessage from bytes
// #[wasm_bindgen]
// pub fn parse_message(data: &[u8]) -> String {
    // let shinkai_message: ShinkaiMessage = Message::decode(data).unwrap();
    // format!("{:?}", shinkai_message)
// }
