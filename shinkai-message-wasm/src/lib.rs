use shinkai_message::{shinkai_message_builder::ShinkaiMessageBuilder, encryption::{EncryptionMethod, unsafe_deterministic_encryption_keypair}, signatures::unsafe_deterministic_signature_keypair};
use wasm_bindgen::prelude::*;
use prost::Message;
use crate::shinkai_message_proto::{ShinkaiMessage, Body, InternalMetadata, ExternalMetadata};
pub mod shinkai_message;

// Include the generated protobuf code
pub mod shinkai_message_proto {
    include!(concat!(env!("OUT_DIR"), "/shinkai_message_proto.rs"));
}

pub fn builder_test() -> Vec<u8> {
    let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let recipient = "@@other_node.shinkai".to_string();
    let sender = "@@my_node.shinkai".to_string();
    let scheduled_time = "20230702T20533481345".to_string();    

    let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk)
    .body("body content".to_string())
    .body_encryption(EncryptionMethod::None)
    .message_schema_type("schema type".to_string())
    .internal_metadata("".to_string(), "".to_string(), "".to_string(), EncryptionMethod::None)
    .external_metadata_with_schedule(recipient.clone(), sender.clone(), scheduled_time.clone())
    .build();

    let mut buf = vec![];
    message_result.unwrap().encode(&mut buf).unwrap();

    buf
}

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
    shinkai_message.encode(&mut buf).unwrap();

    buf
}

// Expose a function that parses a ShinkaiMessage from bytes
#[wasm_bindgen]
pub fn parse_message(data: &[u8]) -> String {
    let shinkai_message: ShinkaiMessage = Message::decode(data).unwrap();
    format!("{:?}", shinkai_message)
}
