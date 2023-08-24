use super::{shinkai_message::{ShinkaiMessage, MessageBody, MessageData}, shinkai_message_schemas::MessageSchemaType};


impl ShinkaiMessage {
    pub fn get_message_raw_content(&self) -> Option<String> {
        match &self.body {
            MessageBody::Unencrypted(body) => match &body.message_data {
                MessageData::Unencrypted(data) => Some(data.message_raw_content.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn get_message_content_schema(&self) -> Option<MessageSchemaType> {
        match &self.body {
            MessageBody::Unencrypted(body) => match &body.message_data {
                MessageData::Unencrypted(data) => Some(data.message_content_schema.clone()),
                _ => None,
            },
            _ => None,
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
}