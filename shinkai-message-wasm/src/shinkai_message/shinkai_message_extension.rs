use serde::{Deserialize, Serialize};

use crate::shinkai_message::shinkai_message::ShinkaiVersion;

use super::{
    shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
    shinkai_message_error::ShinkaiMessageError,
    shinkai_message_schemas::MessageSchemaType,
};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum EncryptionStatus {
    NotCurrentlyEncrypted,
    BodyEncrypted,
    ContentEncrypted,
}

impl ShinkaiMessage {
    pub fn get_message_content(&self) -> Result<String, ShinkaiMessageError> {
        match &self.body {
            MessageBody::Unencrypted(body) => match &body.message_data {
                MessageData::Unencrypted(data) => Ok(data.message_raw_content.clone()),
                _ => Err(ShinkaiMessageError::InvalidMessageSchemaType(
                    "Message data is encrypted".into(),
                )),
            },
            _ => Err(ShinkaiMessageError::MissingMessageBody("Missing message body".into())),
        }
    }

    pub fn get_message_content_schema(&self) -> Result<MessageSchemaType, ShinkaiMessageError> {
        match &self.body {
            MessageBody::Unencrypted(body) => match &body.message_data {
                MessageData::Unencrypted(data) => Ok(data.message_content_schema.clone()),
                _ => Err(ShinkaiMessageError::InvalidMessageSchemaType(
                    "Message data is encrypted".into(),
                )),
            },
            _ => Err(ShinkaiMessageError::MissingMessageBody("Missing message body".into())),
        }
    }

    pub fn get_sender_subidentity(&self) -> Option<String> {
        match &self.body {
            MessageBody::Unencrypted(body) => Some(body.internal_metadata.sender_subidentity.clone()),
            _ => None,
        }
    }

    pub fn get_recipient_subidentity(&self) -> Option<String> {
        match &self.body {
            MessageBody::Unencrypted(body) => Some(body.internal_metadata.recipient_subidentity.clone()),
            _ => None,
        }
    }

    pub fn is_body_currently_encrypted(&self) -> bool {
        matches!(self.body, MessageBody::Encrypted(_))
    }

    pub fn is_content_currently_encrypted(&self) -> bool {
        match &self.body {
            MessageBody::Encrypted(_) => true,
            MessageBody::Unencrypted(body) => matches!(body.message_data, MessageData::Encrypted(_)),
        }
    }

    pub fn get_encryption_status(self) -> EncryptionStatus {
        if self.is_body_currently_encrypted() {
            EncryptionStatus::BodyEncrypted
        } else if self.is_content_currently_encrypted() {
            EncryptionStatus::ContentEncrypted
        } else {
            EncryptionStatus::NotCurrentlyEncrypted
        }
    }

    pub fn encode_message(&self) -> Result<Vec<u8>, ShinkaiMessageError> {
        bincode::serialize(&self).map_err(|err| ShinkaiMessageError::from(err))
    }

    pub fn decode_message_result(encoded: Vec<u8>) -> Result<Self, ShinkaiMessageError> {
        let result: Result<ShinkaiMessage, _> = bincode::deserialize(&encoded[..]);
        match &result {
            Err(err) => {
                eprintln!("Failed to decode entire message: {:?}, error: {}", encoded, err);

                // Try to deserialize each part individually
                if let Err(e) = bincode::deserialize::<MessageBody>(&encoded[..]) {
                    eprintln!("Failed to decode MessageBody: {:?}, error: {}", encoded, e);
                }
                if let Err(e) = bincode::deserialize::<MessageData>(&encoded[..]) {
                    eprintln!("Failed to decode MessageData: {:?}, error: {}", encoded, e);
                }
                if let Err(e) = bincode::deserialize::<ShinkaiVersion>(&encoded[..]) {
                    eprintln!("Failed to decode ShinkaiVersion: {:?}, error: {}", encoded, e);
                }
            }
            _ => {}
        }
        result.map_err(|err| ShinkaiMessageError::from(err))
    }

    pub fn validate_message_schema(&self, schema: MessageSchemaType) -> Result<(), ShinkaiMessageError> {
        if let MessageBody::Unencrypted(body) = &self.body {
            if let MessageData::Unencrypted(data) = &body.message_data {
                if data.message_content_schema != schema {
                    return Err(ShinkaiMessageError::InvalidMessageSchemaType(
                        "Invalid message schema type".into(),
                    ));
                }
            } else {
                return Err(ShinkaiMessageError::InvalidMessageSchemaType(
                    "Message data is encrypted".into(),
                ));
            }
        } else {
            return Err(ShinkaiMessageError::MissingMessageBody("Missing message body".into()));
        }
        Ok(())
    }
}
