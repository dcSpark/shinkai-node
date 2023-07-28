use serde::{Deserialize, Serialize};

use crate::{shinkai_message_proto::{self, Body, InternalMetadata, ExternalMetadata}, schemas::message_schemas::{JobMessage, JobCreation, JobPreMessage, MessageSchemaType}};

#[derive(Serialize, Deserialize)]
pub struct BodyWrapper {
    pub content: String,
    pub parsed_content: ParsedContent,
    pub internal_metadata: InternalMetadataWrapper,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ParsedContent {
    JobCreation(JobCreation),
    JobMessage(JobMessage),
    PreMessage(JobPreMessage),
    PureText(String),
}

#[derive(Serialize, Deserialize)]
pub struct ExternalMetadataWrapper {
    pub sender: String,
    pub recipient: String,
    pub scheduled_time: String,
    pub signature: String,
    pub other: String,
}

#[derive(Serialize, Deserialize)]
pub struct ShinkaiMessageWrapper {
    pub body: BodyWrapper,
    pub external_metadata: ExternalMetadataWrapper,
    pub encryption: String,
}

#[derive(Serialize, Deserialize)]
pub struct InternalMetadataWrapper {
    pub sender_subidentity: String,
    pub recipient_subidentity: String,
    pub message_schema_type: String,
    pub inbox: String,
    pub encryption: String,
}

impl From<&shinkai_message_proto::ShinkaiMessage> for ShinkaiMessageWrapper {
    fn from(msg: &shinkai_message_proto::ShinkaiMessage) -> Self {
        let parsed_content = match msg.body.as_ref().and_then(|b| b.internal_metadata.as_ref()).and_then(|im| MessageSchemaType::from_str(&im.message_schema_type)).unwrap_or(MessageSchemaType::PureText) {
            MessageSchemaType::JobCreationSchema => ParsedContent::JobCreation(serde_json::from_str(&msg.body.as_ref().unwrap().content).unwrap()),
            MessageSchemaType::JobMessageSchema => ParsedContent::JobMessage(serde_json::from_str(&msg.body.as_ref().unwrap().content).unwrap()),
            MessageSchemaType::PreMessageSchema => ParsedContent::PreMessage(serde_json::from_str(&msg.body.as_ref().unwrap().content).unwrap()),
            MessageSchemaType::PureText => ParsedContent::PureText(msg.body.as_ref().unwrap().content.clone()),
        };

        ShinkaiMessageWrapper {
            body: BodyWrapper {
                content: msg.body.as_ref().map_or(String::from(""), |b| b.content.clone()),
                parsed_content,
                internal_metadata: msg.body.as_ref().and_then(|b| b.internal_metadata.as_ref()).map(|im| {
                    InternalMetadataWrapper {
                        sender_subidentity: im.sender_subidentity.clone(),
                        recipient_subidentity: im.recipient_subidentity.clone(),
                        message_schema_type: im.message_schema_type.clone(),
                        inbox: im.inbox.clone(),
                        encryption: im.encryption.clone(),
                    }
                }).unwrap_or(InternalMetadataWrapper {
                    sender_subidentity: String::from(""),
                    recipient_subidentity: String::from(""),
                    message_schema_type: String::from(""),
                    inbox: String::from(""),
                    encryption: String::from(""),
                }),
            },
            external_metadata: msg.external_metadata.as_ref().map(|em| {
                ExternalMetadataWrapper {
                    sender: em.sender.clone(),
                    recipient: em.recipient.clone(),
                    scheduled_time: em.scheduled_time.clone(),
                    signature: em.signature.clone(),
                    other: em.other.clone(),
                }
            }).unwrap_or(ExternalMetadataWrapper {
                sender: String::from(""),
                recipient: String::from(""),
                scheduled_time: String::from(""),
                signature: String::from(""),
                other: String::from(""),
            }),
            encryption: msg.encryption.clone(),
        }
    }
}

impl From<ShinkaiMessageWrapper> for shinkai_message_proto::ShinkaiMessage {
    fn from(wrapper: ShinkaiMessageWrapper) -> Self {
        shinkai_message_proto::ShinkaiMessage {
            body: Some(Body {
                content: wrapper.body.content,
                internal_metadata: Some(InternalMetadata {
                    sender_subidentity: wrapper.body.internal_metadata.sender_subidentity,
                    recipient_subidentity: wrapper.body.internal_metadata.recipient_subidentity,
                    message_schema_type: wrapper.body.internal_metadata.message_schema_type,
                    inbox: wrapper.body.internal_metadata.inbox,
                    encryption: wrapper.body.internal_metadata.encryption,
                }),
            }),
            external_metadata: Some(ExternalMetadata {
                sender: wrapper.external_metadata.sender,
                recipient: wrapper.external_metadata.recipient,
                scheduled_time: wrapper.external_metadata.scheduled_time,
                signature: wrapper.external_metadata.signature,
                other: wrapper.external_metadata.other,
            }),
            encryption: wrapper.encryption,
        }
    }
}
