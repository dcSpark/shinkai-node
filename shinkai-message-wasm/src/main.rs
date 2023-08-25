mod schemas;
mod shinkai_message;
mod shinkai_utils;
pub mod shinkai_wasm_wrappers;

use std::error::Error;

use crate::{shinkai_message::{shinkai_message::{InternalMetadata, ExternalMetadata, ShinkaiMessage, ShinkaiVersion, MessageData, MessageBody, ShinkaiData, ShinkaiBody}, shinkai_message_schemas::MessageSchemaType}, shinkai_utils::encryption::EncryptionMethod};

fn main() -> Result<(), Box<dyn Error>> {
    console_log::init_with_level(log::Level::Debug).expect("error initializing log");

    let internal_metadata = InternalMetadata {
        sender_subidentity: "sender_subidentity".into(),
        recipient_subidentity: "recipient_subidentity".into(),
        signature: "signature".into(),
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

    let data = MessageData::Unencrypted(ShinkaiData {
        message_raw_content: "content".into(),
        message_content_schema: MessageSchemaType::TextContent,
    });

    let body = MessageBody::Unencrypted(ShinkaiBody {
        message_data: data,
        internal_metadata: internal_metadata,
    });
    

    let shinkai_message = ShinkaiMessage {
        body,
        external_metadata: external_metadata,
        encryption: EncryptionMethod::None,
        version: ShinkaiVersion::V1_0,
    };

    println!("Shinkai message: {:?}", shinkai_message);

    Ok(())
}
