mod schemas;
mod shinkai_message;
mod shinkai_utils;

use std::error::Error;

use crate::{shinkai_message::{shinkai_message::{InternalMetadata, ExternalMetadata, ShinkaiMessage, ShinkaiVersion, MessageData, MessageBody, ShinkaiData, ShinkaiBody}, shinkai_message_schemas::MessageSchemaType}, shinkai_utils::encryption::EncryptionMethod};

fn main() -> Result<(), Box<dyn Error>> {
    Ok(())
}
