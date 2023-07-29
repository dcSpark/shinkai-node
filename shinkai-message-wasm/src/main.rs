mod schemas;
mod shinkai_message;
mod shinkai_utils;
pub mod shinkai_wasm_wrappers;

use std::error::Error;

use crate::{shinkai_message::{shinkai_message::{InternalMetadata, ExternalMetadata, ShinkaiMessage, Body}, shinkai_message_schemas::MessageSchemaType}, shinkai_utils::encryption::EncryptionMethod};

fn main() -> Result<(), Box<dyn Error>> {
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");

    let internal_metadata = InternalMetadata {
        sender_subidentity: "sender_subidentity".into(),
        recipient_subidentity: "recipient_subidentity".into(),
        message_schema_type: MessageSchemaType::TextContent,
        inbox: "inbox".into(),
        encryption: EncryptionMethod::None,
    };

    let external_metadata = ExternalMetadata {
        sender: "sender".into(),
        recipient: "recipient".into(),
        scheduled_time: "scheduled_time".into(),
        signature: "signature".into(),
        other: "other".into(),
    };

    let body = Body {
        content: "content".into(),
        internal_metadata: Some(internal_metadata),
    };

    let shinkai_message = ShinkaiMessage {
        body: Some(body),
        external_metadata: Some(external_metadata),
        encryption: EncryptionMethod::None,
    };

    println!("Shinkai message: {:?}", shinkai_message);

    Ok(())
}
