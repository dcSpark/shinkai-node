use super::shinkai_message_schemas::MessageSchemaType;
use crate::shinkai_utils::encryption::EncryptionMethod;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ShinkaiMessage {
    pub body: MessageBody,
    pub external_metadata: ExternalMetadata,
    pub encryption: EncryptionMethod,
    pub version: ShinkaiVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ShinkaiBody {
    pub message_data: MessageData,
    pub internal_metadata: InternalMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct InternalMetadata {
    pub sender_subidentity: String,
    pub recipient_subidentity: String,
    pub inbox: String,
    pub signature: String,
    pub encryption: EncryptionMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_api_data: Option<NodeApiData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ExternalMetadata {
    pub sender: String,
    pub recipient: String,
    pub scheduled_time: String,
    pub signature: String,
    pub intra_sender: String,
    pub other: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct NodeApiData {
    pub parent_hash: String,
    pub node_message_hash: String,
    pub node_timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct EncryptedShinkaiBody {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct EncryptedShinkaiData {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct ShinkaiData {
    pub message_raw_content: String,
    pub message_content_schema: MessageSchemaType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum MessageBody {
    #[serde(rename = "encrypted")]
    Encrypted(EncryptedShinkaiBody),
    #[serde(rename = "unencrypted")]
    Unencrypted(ShinkaiBody),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum MessageData {
    #[serde(rename = "encrypted")]
    Encrypted(EncryptedShinkaiData),
    #[serde(rename = "unencrypted")]
    Unencrypted(ShinkaiData),
}

#[derive(Debug, Clone, PartialEq, ToSchema)]
pub enum ShinkaiVersion {
    V1_0,
    Unsupported,
}
