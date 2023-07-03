use serde::{Serialize, Deserialize};
use crate::shinkai_message_proto;
use shinkai_message_proto::{Body, ShinkaiMessage, ExternalMetadata};
use prost::Message;
use std::borrow::Cow;

#[derive(Serialize, Deserialize)]
pub struct FieldWrapper {
    name: String,
    field_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct MessageSchemaTypeWrapper {
    type_name: String,
    fields: Vec<FieldWrapper>,
}

#[derive(Serialize, Deserialize)]
pub struct TopicWrapper {
    topic_id: String,
    channel_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct InternalMetadataWrapper {
    message_schema_type: MessageSchemaTypeWrapper,
    topic: TopicWrapper,
    content: String,
}

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
}

#[derive(Serialize, Deserialize)]
pub struct ShinkaiMessageWrapper {
    body: BodyWrapper,
    external_metadata: ExternalMetadataWrapper,
    encryption: String,
}

impl From<&shinkai_message_proto::ShinkaiMessage> for ShinkaiMessageWrapper {
    fn from(msg: &shinkai_message_proto::ShinkaiMessage) -> Self {
        ShinkaiMessageWrapper {
            body: BodyWrapper {
                content: msg.body.as_ref().map_or(String::from(""), |b| b.content.clone()),
                internal_metadata: InternalMetadataWrapper {
                    message_schema_type: MessageSchemaTypeWrapper {
                        type_name: msg.body.as_ref().and_then(|b| b.internal_metadata.as_ref()).and_then(|im| im.message_schema_type.as_ref()).map_or(String::from(""), |mst| mst.type_name.clone()),
                        fields: msg.body.as_ref().and_then(|b| b.internal_metadata.as_ref()).and_then(|im| im.message_schema_type.as_ref()).map_or(vec![], |mst| mst.fields.iter().map(|f| FieldWrapper { 
                            name: f.name.clone(),
                            field_type: f.field_type.clone(),
                        }).collect::<Vec<_>>()),
                    },
                    topic: TopicWrapper {
                        topic_id: msg.body.as_ref().and_then(|b| b.internal_metadata.as_ref()).and_then(|im| im.topic.as_ref()).map_or(String::from(""), |t| t.topic_id.clone()),
                        channel_id: msg.body.as_ref().and_then(|b| b.internal_metadata.as_ref()).and_then(|im| im.topic.as_ref()).map_or(String::from(""), |t| t.channel_id.clone()),
                    },
                    content: msg.body.as_ref().and_then(|b| b.internal_metadata.as_ref()).map_or(String::from(""), |im| im.content.clone()),
                },
            },
            external_metadata: ExternalMetadataWrapper {
                sender: msg.external_metadata.as_ref().map_or(String::from(""), |em| em.sender.clone()),
                recipient: msg.external_metadata.as_ref().map_or(String::from(""), |em| em.recipient.clone()),
                scheduled_time: msg.external_metadata.as_ref().map_or(String::from(""), |em| em.scheduled_time.clone()),
                signature: msg.external_metadata.as_ref().map_or(String::from(""), |em| em.signature.clone()),
            },
            encryption: msg.encryption.clone(),
        }
    }
}

impl From<ShinkaiMessageWrapper> for ShinkaiMessage {
    fn from(wrapper: ShinkaiMessageWrapper) -> Self {
        ShinkaiMessage {
            body: Some(Body {
                content: wrapper.body.content,
                internal_metadata: Some(shinkai_message_proto::InternalMetadata {
                    message_schema_type: Some(shinkai_message_proto::MessageSchemaType {
                        type_name: wrapper.body.internal_metadata.message_schema_type.type_name,
                        fields: wrapper.body.internal_metadata.message_schema_type.fields.into_iter().map(|field| {
                            shinkai_message_proto::Field {
                                name: field.name,
                                field_type: field.field_type,
                            }
                        }).collect(),
                    }),
                    topic: Some(shinkai_message_proto::Topic {
                        topic_id: wrapper.body.internal_metadata.topic.topic_id,
                        channel_id: wrapper.body.internal_metadata.topic.channel_id,
                    }),
                    content: wrapper.body.internal_metadata.content,
                }),
            }),
            external_metadata: Some(ExternalMetadata {
                sender: wrapper.external_metadata.sender,
                recipient: wrapper.external_metadata.recipient,
                scheduled_time: wrapper.external_metadata.scheduled_time,
                signature: wrapper.external_metadata.signature,
            }),
            encryption: wrapper.encryption,
        }
    }
}
