use crate::{
    db::db_errors::ShinkaiMessageDBError, shinkai_message::shinkai_message_handler::ShinkaiMessageHandler,
    shinkai_message_proto::ShinkaiMessage,
};

pub struct InboxNameManager {
    inbox_name: String,
}

#[derive(PartialEq, Debug)]
pub struct InboxNameParts {
    pub identity_a: String,
    pub identity_a_subidentity: String,
    pub identity_b: String,
    pub identity_b_subidentity: String,
    pub is_e2e: bool,
}

impl InboxNameManager {
    pub fn from_inbox_name(inbox_name: String) -> Self {
        InboxNameManager { inbox_name }
    }

    pub fn parse_parts(&self) -> Result<InboxNameParts, ShinkaiMessageDBError> {
        let parts: Vec<&str> = self.inbox_name.split("::").collect();
        if parts.len() != 4 {
            return Err(ShinkaiMessageDBError::InvalidInboxName);
        }

        let is_e2e = match parts[3].parse::<bool>() {
            Ok(b) => b,
            Err(_) => return Err(ShinkaiMessageDBError::InvalidInboxName),
        };

        let sender_parts: Vec<&str> = parts[1].split("|").collect();
        let recipient_parts: Vec<&str> = parts[2].split("|").collect();

        if sender_parts.len() != 2 || recipient_parts.len() != 2 {
            return Err(ShinkaiMessageDBError::InvalidInboxName);
        }

        let sender = sender_parts[0].to_string();
        let sender_subidentity = sender_parts[1].to_string();
        let recipient = recipient_parts[0].to_string();
        let recipient_subidentity = recipient_parts[1].to_string();

        // Return the results as an instance of InboxNameParts
        Ok(InboxNameParts {
            identity_a: sender,
            identity_a_subidentity: sender_subidentity,
            identity_b: recipient,
            identity_b_subidentity: recipient_subidentity,
            is_e2e
        })
    }

    pub fn is_e2e(&self) -> Result<bool, ShinkaiMessageDBError> {
        let parts = self.parse_parts()?;
        Ok(parts.is_e2e)
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

        let inbox_name =
            Self::get_inbox_name_from_params(is_e2e, sender, sender_subidentity, recipient, recipient_subidentity);

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
    use crate::shinkai_message_proto::{Body, ExternalMetadata, InternalMetadata, ShinkaiMessage};

    // Test creation of InboxNameManager instance from an inbox name
    #[test]
    fn test_from_inbox_name() {
        let inbox_name = "inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::true".to_string();
        let manager = InboxNameManager::from_inbox_name(inbox_name.clone());

        assert_eq!(manager.inbox_name, inbox_name);
    }

    // Test parsing of the inbox name
    #[test]
    fn test_parse_parts() {
        let inbox_name = "inbox::alice|primary::bob|secondary::true".to_string();
        let manager = InboxNameManager::from_inbox_name(inbox_name);

        let result = manager.parse_parts().unwrap();

        assert_eq!(
            result,
            InboxNameParts {
                identity_a: "alice".to_string(),
                identity_a_subidentity: "primary".to_string(),
                identity_b: "bob".to_string(),
                identity_b_subidentity: "secondary".to_string(),
                is_e2e: true,
            }
        );
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
        let inbox_name = "inbox::alice|primary::bob|secondary::true".to_string();
        let manager = InboxNameManager::from_inbox_name(inbox_name);

        assert!(manager.is_e2e().unwrap());
    }

    // Test for correct inbox name format
    #[test]
    fn test_is_valid_format() {
        let inbox_name = "inbox::alice|primary::bob|secondary::true".to_string();
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
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ"
                    .into(),
                other: "".into(),
            }),
            encryption: "None".into(),
        };

        let manager = InboxNameManager::from_message(&mock_message).unwrap();

        // Assuming your implementation of get_inbox_name_from_params returns "inbox::@@node2.shinkai|@@node1.shinkai::false" for this example
        println!("manager.inbox_name: {}", manager.inbox_name);
        assert_eq!(manager.inbox_name, "inbox::@@node1.shinkai|::@@node2.shinkai|::false");
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
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ"
                    .into(),
                other: "".into(),
            }),
            encryption: "None".into(),
        };

        let inbox_name = InboxNameManager::get_inbox_name_from_message(&mock_message).unwrap();

        assert_eq!(
            inbox_name,
            "inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::false"
        );
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
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ"
                    .into(),
                other: "".into(),
            }),
            encryption: "None".into(),
        };

        let inbox_name = InboxNameManager::get_inbox_name_from_message(&mock_message).unwrap();

        assert_eq!(inbox_name, "inbox::@@node1.shinkai|::@@node2.shinkai|::false");
    }

    #[test]
    fn test_full_loop_inbox_name_from_message() {
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
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ"
                    .into(),
                other: "".into(),
            }),
            encryption: "None".into(),
        };

        let inbox_name = InboxNameManager::get_inbox_name_from_message(&mock_message).unwrap();

        assert_eq!(
            inbox_name,
            "inbox::@@node1.shinkai|subidentity::@@node2.shinkai|subidentity2::false"
        );

        // parse parts from InboxNameManager
        let parsed_parts = InboxNameManager::from_inbox_name(inbox_name).parse_parts().unwrap();

        assert_eq!(parsed_parts.identity_a, "@@node1.shinkai");
        assert_eq!(parsed_parts.identity_a_subidentity, "subidentity");
        assert_eq!(parsed_parts.identity_b, "@@node2.shinkai");
        assert_eq!(parsed_parts.identity_b_subidentity, "subidentity2");
        assert_eq!(parsed_parts.is_e2e, false);
    }
}
