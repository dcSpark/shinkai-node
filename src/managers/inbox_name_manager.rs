use crate::{db::db_errors::ShinkaiMessageDBError, shinkai_message_proto::ShinkaiMessage, shinkai_message::shinkai_message_handler::ShinkaiMessageHandler};

pub struct InboxNameManager {
    inbox_name: String,
}

impl InboxNameManager {
    pub fn from_inbox_name(inbox_name: String) -> Self {
        InboxNameManager { inbox_name }
    }

    pub fn parse_parts(&self) -> Result<(String, String, String, String, bool), ShinkaiMessageDBError> {
        let parts: Vec<&str> = self.inbox_name.split("::").collect();

        if parts.len() != 3 {
            return Err(ShinkaiMessageDBError::InvalidInboxName);
        }

        let is_e2e = match parts[2].parse() {
            Ok(b) => b,
            Err(_) => return Err(ShinkaiMessageDBError::InvalidInboxName),
        };

        let (sender, sender_subidentity, recipient, recipient_subidentity) = self.parse_inbox_parts(parts[1])?;

        Ok((sender, sender_subidentity, recipient, recipient_subidentity, is_e2e))
    }

    fn parse_inbox_parts(&self, parts: &str) -> Result<(String, String, String, String), ShinkaiMessageDBError> {
        let identity_parts: Vec<&str> = parts.split("_").collect();

        if identity_parts.len() != 2 {
            return Err(ShinkaiMessageDBError::InvalidInboxName);
        }

        let sender_parts: Vec<&str> = identity_parts[0].split("|").collect();
        let recipient_parts: Vec<&str> = identity_parts[1].split("|").collect();

        if sender_parts.len() != 2 || recipient_parts.len() != 2 {
            return Err(ShinkaiMessageDBError::InvalidInboxName);
        }

        let sender = sender_parts[0].to_string();
        let sender_subidentity = sender_parts[1].to_string();
        let recipient = recipient_parts[0].to_string();
        let recipient_subidentity = recipient_parts[1].to_string();

        Ok((sender, sender_subidentity, recipient, recipient_subidentity))
    }

    pub fn is_e2e(&self) -> Result<bool, ShinkaiMessageDBError> {
        let (_, _, _, _, is_e2e) = self.parse_parts()?;
        Ok(is_e2e)
    }

    pub fn is_valid_format(&self) -> bool {
        match self.parse_parts() {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub fn from_message(message: &ShinkaiMessage) -> Result<Self, ShinkaiMessageDBError> {
        let is_e2e = ShinkaiMessageHandler::is_content_currently_encrypted(message);
        let external_metadata = message
            .external_metadata
            .as_ref()
            .ok_or(ShinkaiMessageDBError::MissingExternalMetadata)?;
        let body = message.body.as_ref().ok_or(ShinkaiMessageDBError::MissingBody)?;
        let internal_metadata = body
            .internal_metadata
            .as_ref()
            .ok_or(ShinkaiMessageDBError::MissingInternalMetadata)?;

        let inbox_name = Self::get_inbox_name_from_params(
            is_e2e,
            external_metadata.sender.clone(),
            internal_metadata.sender_subidentity.clone(),
            external_metadata.recipient.clone(),
            internal_metadata.recipient_subidentity.clone(),
        );

        Ok(InboxNameManager { inbox_name })
    }

    pub fn get_inbox_name_from_message(message: &ShinkaiMessage) -> Result<String, ShinkaiMessageDBError> {
        // Check if message is encrypted
        let is_e2e = ShinkaiMessageHandler::is_content_currently_encrypted(message);

        // Check if all necessary fields are present in the message
        let external_metadata = message
            .external_metadata
            .as_ref()
            .ok_or(ShinkaiMessageDBError::MissingExternalMetadata)?;
        let body = message.body.as_ref().ok_or(ShinkaiMessageDBError::MissingBody)?;
        let internal_metadata = body
            .internal_metadata
            .as_ref()
            .ok_or(ShinkaiMessageDBError::MissingInternalMetadata)?;

        let sender = external_metadata.sender.clone();
        let sender_subidentity = internal_metadata.sender_subidentity.clone();
        let recipient = external_metadata.recipient.clone();
        let recipient_subidentity = internal_metadata.recipient_subidentity.clone();

        let inbox_name = Self::get_inbox_name_from_params(
            is_e2e,
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
        );

        Ok(inbox_name)
    }

    fn get_inbox_name_from_params(
        is_e2e: bool,
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
    ) -> String {
        let identity_separator = "|";
        let inbox_name_separator = "::";
    
        let sender_full = format!("{}{}{}", sender, identity_separator, sender_subidentity);
        let recipient_full = format!("{}{}{}", recipient, identity_separator, recipient_subidentity);
    
        let mut inbox_name_parts = vec![sender_full, recipient_full];
        inbox_name_parts.sort();
    
        let inbox_name = format!(
            "inbox{}{}{}{}{}{}",
            inbox_name_separator,
            inbox_name_parts[0],
            inbox_name_separator,
            inbox_name_parts[1],
            inbox_name_separator,
            is_e2e
        );
    
        inbox_name
    }    
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shinkai_message_proto::{ShinkaiMessage, ExternalMetadata, Body, InternalMetadata};

    // Test creation of InboxNameManager instance from an inbox name
    #[test]
    fn test_from_inbox_name() {
        let inbox_name = "inbox::alice|primary_bob|secondary::true".to_string();
        let manager = InboxNameManager::from_inbox_name(inbox_name.clone());

        assert_eq!(manager.inbox_name, inbox_name);
    }

    // Test parsing of the inbox name
    #[test]
    fn test_parse_parts() {
        let inbox_name = "inbox::alice|primary_bob|secondary::true".to_string();
        let manager = InboxNameManager::from_inbox_name(inbox_name);

        let result = manager.parse_parts().unwrap();

        assert_eq!(result, ("alice".to_string(), "primary".to_string(), "bob".to_string(), "secondary".to_string(), true));
    }

    // Test for incorrect inbox name format
    #[test]
    fn test_parse_parts_error() {
        let inbox_name = "incorrect::format".to_string();
        let manager = InboxNameManager::from_inbox_name(inbox_name);

        let result = manager.parse_parts();

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ShinkaiMessageDBError::InvalidInboxName);
    }

    // Test is_e2e method
    #[test]
    fn test_is_e2e() {
        let inbox_name = "inbox::alice|primary_bob|secondary::true".to_string();
        let manager = InboxNameManager::from_inbox_name(inbox_name);

        assert!(manager.is_e2e().unwrap());
    }

    // Test for correct inbox name format
    #[test]
    fn test_is_valid_format() {
        let inbox_name = "inbox::alice|primary_bob|secondary::true".to_string();
        let manager = InboxNameManager::from_inbox_name(inbox_name);

        assert!(manager.is_valid_format());
    }

    #[test]
    fn test_from_message() {
        let mock_message = ShinkaiMessage {
            body: Some(Body {
                content: "ACK".into(),
                internal_metadata: Some(InternalMetadata {
                    sender_subidentity: "".into(),
                    recipient_subidentity: "".into(),
                    message_schema_type: "".into(),
                    inbox: "".into(),
                    encryption: "None".into(),
                }),
            }),
            external_metadata: Some(ExternalMetadata {
                sender: "@@node2.shinkai".into(),
                recipient: "@@node1.shinkai".into(),
                scheduled_time: "20230714T19363326163".into(),
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ".into(),
                other: "".into(),
            }),
            encryption: "None".into(),
        };

        let manager = InboxNameManager::from_message(&mock_message).unwrap();

        // Assuming your implementation of get_inbox_name_from_params returns "inbox::@@node2.shinkai|@@node1.shinkai::false" for this example
        assert_eq!(manager.inbox_name, "inbox::@@node2.shinkai|@@node1.shinkai::false");
    }

    // Test getting inbox name from a ShinkaiMessage
    #[test]
    fn test_get_inbox_name_from_message() {
        let mock_message = ShinkaiMessage {
            body: Some(Body {
                content: "ACK".into(),
                internal_metadata: Some(InternalMetadata {
                    sender_subidentity: "subidentity2".into(),
                    recipient_subidentity: "subidentity".into(),
                    message_schema_type: "".into(),
                    inbox: "".into(),
                    encryption: "None".into(),
                }),
            }),
            external_metadata: Some(ExternalMetadata {
                sender: "@@node2.shinkai".into(),
                recipient: "@@node1.shinkai".into(),
                scheduled_time: "20230714T19363326163".into(),
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ".into(),
                other: "".into(),
            }),
            encryption: "None".into(),
        };

        let inbox_name = InboxNameManager::get_inbox_name_from_message(&mock_message).unwrap();

        assert_eq!(inbox_name, "inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::false");
    }

    #[test]
    fn test_get_inbox_name_from_message_without_subidentities() {
        let mock_message = ShinkaiMessage {
            body: Some(Body {
                content: "ACK".into(),
                internal_metadata: Some(InternalMetadata {
                    sender_subidentity: "".into(),
                    recipient_subidentity: "".into(),
                    message_schema_type: "".into(),
                    inbox: "".into(),
                    encryption: "None".into(),
                }),
            }),
            external_metadata: Some(ExternalMetadata {
                sender: "@@node2.shinkai".into(),
                recipient: "@@node1.shinkai".into(),
                scheduled_time: "20230714T19363326163".into(),
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ".into(),
                other: "".into(),
            }),
            encryption: "None".into(),
        };

        let inbox_name = InboxNameManager::get_inbox_name_from_message(&mock_message).unwrap();

        assert_eq!(inbox_name, "inbox::@@node1.shinkai|::@@node2.shinkai|::false");
    }
}
