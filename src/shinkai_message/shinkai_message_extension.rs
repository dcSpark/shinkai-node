use crate::shinkai_message_proto;
use serde::{Deserialize, Serialize};
use shinkai_message_proto::{Body, ExternalMetadata, ShinkaiMessage, InternalMetadata};

#[derive(Serialize, Deserialize)]
pub struct BodyWrapper {
    content: String,
    internal_metadata: InternalMetadataWrapper,
}

#[derive(Serialize, Deserialize)]
pub struct ExternalMetadataWrapper {
    sender: String,
    recipient: String,
    scheduled_time: String,
    signature: String,
    other: String,
}

#[derive(Serialize, Deserialize)]
pub struct ShinkaiMessageWrapper {
    body: BodyWrapper,
    external_metadata: ExternalMetadataWrapper,
    encryption: String,
}

#[derive(Serialize, Deserialize)]
pub struct InternalMetadataWrapper {
    sender_subidentity: String,
    recipient_subidentity: String,
    message_schema_type: String,
    inbox: String,
    encryption: String,
}

impl From<&shinkai_message_proto::ShinkaiMessage> for ShinkaiMessageWrapper {
    fn from(msg: &shinkai_message_proto::ShinkaiMessage) -> Self {
        ShinkaiMessageWrapper {
            body: BodyWrapper {
                content: msg.body.as_ref().map_or(String::from(""), |b| b.content.clone()),
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
