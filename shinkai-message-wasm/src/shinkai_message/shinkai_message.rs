use serde::{Deserialize, Serialize};
use crate::shinkai_utils::encryption::EncryptionMethod;
use super::shinkai_message_schemas::MessageSchemaType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiMessage {
    pub body: MessageBody,
    pub external_metadata: ExternalMetadata,
    pub encryption: EncryptionMethod,
    pub version: ShinkaiVersion
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShinkaiBody {
    pub message_data: MessageData,
    pub internal_metadata: InternalMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalMetadata {
    pub sender_subidentity: String,
    pub recipient_subidentity: String,
    pub inbox: String,
    pub signature: String,
    pub encryption: EncryptionMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalMetadata {
    pub sender: String,
    pub recipient: String,
    pub scheduled_time: String,
    pub signature: String,
    pub other: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedShinkaiBody {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EncryptedShinkaiData {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShinkaiData {
    pub message_raw_content: String,
    pub message_content_schema: MessageSchemaType,
}

#[derive(Debug, Clone)]
pub enum MessageBody {
    Encrypted(EncryptedShinkaiBody),
    Unencrypted(ShinkaiBody),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageData {
    Encrypted(EncryptedShinkaiData),
    Unencrypted(ShinkaiData),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShinkaiVersion {
    V1_0,
    Unsupported,
}
