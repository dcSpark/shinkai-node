mod schemas;
mod shinkai_message;
mod shinkai_utils;

use std::error::Error;

use crate::shinkai_message::shinkai_message::{InternalMetadata, ExternalMetadata, ShinkaiMessage, Body};

fn main() -> Result<(), Box<dyn Error>> {
    let internal_metadata = InternalMetadata {
        sender_subidentity: "sender_subidentity".into(),
        recipient_subidentity: "recipient_subidentity".into(),
        message_schema_type: "message_schema_type".into(),
        inbox: "inbox".into(),
        encryption: "encryption".into(),
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
        encryption: "encryption".into(),
    };

    println!("Shinkai message: {:?}", shinkai_message);

    Ok(())
}
