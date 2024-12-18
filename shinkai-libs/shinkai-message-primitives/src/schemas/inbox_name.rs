use std::fmt;

use super::shinkai_name::{ShinkaiName, ShinkaiNameError};
use crate::shinkai_message::shinkai_message::{MessageBody, ShinkaiMessage};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, PartialEq)]
pub enum InboxNameError {
    ShinkaiNameError(ShinkaiNameError),
    InvalidFormat(String),
    InvalidOperation(String),
}

impl fmt::Display for InboxNameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            InboxNameError::ShinkaiNameError(ref err) => std::fmt::Display::fmt(err, f),
            InboxNameError::InvalidFormat(ref s) => write!(f, "Invalid inbox name format: {}", s),
            InboxNameError::InvalidOperation(ref s) => write!(f, "Invalid operation: {}", s),
        }
    }
}

impl From<ShinkaiNameError> for InboxNameError {
    fn from(error: ShinkaiNameError) -> Self {
        InboxNameError::ShinkaiNameError(error)
    }
}

impl std::error::Error for InboxNameError {}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InboxName {
    RegularInbox {
        value: String,
        is_e2e: bool,
        identities: Vec<ShinkaiName>,
    },
    JobInbox {
        value: String,
        unique_id: String,
        is_e2e: bool,
    },
}

impl InboxName {
    pub fn new(inbox_name: String) -> Result<Self, InboxNameError> {
        let inbox_name = inbox_name.to_lowercase();
        let parts: Vec<&str> = inbox_name.split("::").collect();
        if parts.len() < 3 || parts.len() > 101 {
            return Err(InboxNameError::InvalidFormat(inbox_name.clone()));
        }

        let is_e2e = parts
            .last()
            .unwrap()
            .parse::<bool>()
            .map_err(|_| InboxNameError::InvalidFormat(inbox_name.clone()))?;

        if parts[0] == "inbox" {
            let mut identities = Vec::new();
            for part in &parts[1..parts.len() - 1] {
                if !ShinkaiName::is_fully_valid(part.to_string()) {
                    return Err(InboxNameError::InvalidFormat(inbox_name.clone()));
                }
                match ShinkaiName::new(part.to_string()) {
                    Ok(name) => identities.push(name),
                    Err(_) => return Err(InboxNameError::InvalidFormat(inbox_name.clone())),
                }
            }

            Ok(InboxName::RegularInbox {
                value: inbox_name,
                is_e2e,
                identities,
            })
        } else if parts[0] == "job_inbox" {
            if is_e2e {
                return Err(InboxNameError::InvalidFormat(inbox_name.clone()));
            }
            let unique_id = parts[1].to_string();
            if unique_id.is_empty() {
                return Err(InboxNameError::InvalidFormat(inbox_name.clone()));
            }
            Ok(InboxName::JobInbox {
                value: inbox_name,
                unique_id,
                is_e2e,
            })
        } else {
            Err(InboxNameError::InvalidFormat(inbox_name.clone()))
        }
    }

    pub fn from_message(message: &ShinkaiMessage) -> Result<InboxName, InboxNameError> {
        match &message.body {
            MessageBody::Unencrypted(body) => {
                let inbox_name = body.internal_metadata.inbox.clone();
                InboxName::new(inbox_name)
            }
            _ => Err(InboxNameError::InvalidFormat("Expected Unencrypted MessageBody".into())),
        }
    }

    /// Returns the job ID if the InboxName is a JobInbox, otherwise returns None
    pub fn get_job_id(&self) -> Option<String> {
        match self {
            InboxName::JobInbox { unique_id, .. } => Some(unique_id.clone()),
            InboxName::RegularInbox { .. } => None,
        }
    }

    pub fn has_creation_access(&self, identity_name: ShinkaiName) -> Result<bool, InboxNameError> {
        if let InboxName::RegularInbox { identities, .. } = self {
            for identity in identities {
                if identity.contains(&identity_name) {
                    return Ok(true);
                }
            }
            Ok(false)
        } else {
            Err(InboxNameError::InvalidOperation(
                "has_creation_access is not applicable for JobInbox".to_string(),
            ))
        }
    }

    pub fn has_sender_creation_access(&self, message: ShinkaiMessage) -> Result<bool, InboxNameError> {
        match ShinkaiName::from_shinkai_message_using_sender_subidentity(&message) {
            Ok(shinkai_name) => self.has_creation_access(shinkai_name),
            Err(_) => Ok(false),
        }
    }

    pub fn get_regular_inbox_name_from_params(
        sender: String,
        sender_subidentity: String,
        recipient: String,
        recipient_subidentity: String,
        is_e2e: bool,
    ) -> Result<InboxName, InboxNameError> {
        let inbox_name_separator = "::";

        let sender_full = if sender_subidentity.is_empty() {
            sender
        } else {
            format!("{}/{}", sender, sender_subidentity)
        };

        let recipient_full = if recipient_subidentity.is_empty() {
            recipient
        } else {
            format!("{}/{}", recipient, recipient_subidentity)
        };

        let sender_name = ShinkaiName::new(sender_full.clone())
            .map_err(|_| ShinkaiNameError::InvalidNameFormat(sender_full.to_string()))?;
        let recipient_name = ShinkaiName::new(recipient_full.clone())
            .map_err(|_| ShinkaiNameError::InvalidNameFormat(recipient_full.to_string()))?;

        let mut inbox_name_parts = [sender_name.to_string(), recipient_name.to_string()];
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
        InboxName::new(inbox_name)
    }

    pub fn get_job_inbox_name_from_params(unique_id: String) -> Result<InboxName, InboxNameError> {
        let inbox_name_separator = "::";
        let inbox_name = format!(
            "job_inbox{}{}{}false",
            inbox_name_separator, unique_id, inbox_name_separator
        );
        InboxName::new(inbox_name)
    }

    /// Returns the first half of the blake3 hash of the inbox name's value
    pub fn hash_value_first_half(&self) -> String {
        let value = match self {
            InboxName::RegularInbox { value, .. } => value,
            InboxName::JobInbox { value, .. } => value,
        };
        let full_hash = blake3::hash(value.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    /// Returns the value field of the inbox no matter if it's a regular or job inbox
    pub fn get_value(&self) -> String {
        let value = match self {
            InboxName::RegularInbox { value, .. } => value,
            InboxName::JobInbox { value, .. } => value,
        };
        value.clone()
    }
}

impl fmt::Display for InboxName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_value())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        shinkai_message::{
            shinkai_message::{
                ExternalMetadata, InternalMetadata, MessageBody, MessageData, NodeApiData, ShinkaiBody, ShinkaiData,
                ShinkaiVersion,
            },
            shinkai_message_schemas::MessageSchemaType,
        },
        shinkai_utils::encryption::EncryptionMethod,
    };

    use super::*;

    // Test new inbox name
    #[test]
    fn valid_inbox_names() {
        let valid_names = vec![
            "inbox::@@node.shinkai::true",
            "inbox::@@node1.shinkai/subidentity::false",
            "inbox::@@node.arb-sep-shinkai/subidentity::true",
            "inbox::@@alice.shinkai/profileName/agent/myChatGPTAgent::true",
            "inbox::@@alice.shinkai/profileName/device/myPhone::true",
            "inbox::@@node1.shinkai/subidentity::@@node2.shinkai/subidentity2::false",
            "inbox::@@node1.shinkai/subidentity::@@node2.shinkai/subidentity::@@node3.shinkai/subidentity2::false",
        ];

        for name in valid_names {
            let result = InboxName::new(name.to_string());
            assert!(result.is_ok(), "Expected valid inbox name {}", name);
        }
    }

    #[test]
    fn invalid_inbox_names() {
        let invalid_names = vec![
            "@@node1.shinkai::false",
            "inbox::@@node1.shinkai::falsee",
            "@@node1.shinkai",
            "inbox::@@node1.shinkai",
            "inbox::node1::false",
            "inbox::node1.shinkai::false",
            "inbox::@@node1::false",
            "inbox::@@node1.shinkai//subidentity::@@node2.shinkai::false",
            "inbox::@@node1/subidentity::false",
        ];

        for name in &invalid_names {
            let result = InboxName::new(name.to_string());
            assert!(
                result.is_err(),
                "Expected invalid inbox name, but got a valid one for: {}",
                name
            );
        }
    }

    #[test]
    fn valid_job_inbox_names() {
        let valid_names = vec![
            "job_inbox::unique_id_1::false",
            "job_inbox::unique_id_2::false",
            "job_inbox::job_1234::false",
            // add other valid examples here...
        ];

        for name in valid_names {
            let result = InboxName::new(name.to_string());
            assert!(result.is_ok(), "Expected valid job inbox name {}", name);
        }
    }

    #[test]
    fn invalid_job_inbox_names() {
        let invalid_names = vec![
            "job_inbox::false",
            "job_inbox::unique_id_1::true",
            "jobinbox::unique_id_2::false",
            "job_inbox::::false",
            "inbox::unique_id_1::false",
        ];

        for name in &invalid_names {
            let result = InboxName::new(name.to_string());
            assert!(
                result.is_err(),
                "Expected invalid job inbox name, but got a valid one for: {}",
                name
            );
        }
    }

    // Test creation of InboxNameManager instance from an inbox name
    #[test]
    fn test_from_inbox_name() {
        let inbox_name = "inbox::@@node1.shinkai/subidentity::@@node2.shinkai/subidentity2::true".to_string();
        let manager = InboxName::new(inbox_name.clone()).unwrap();

        match &manager {
            InboxName::RegularInbox { value, .. } => assert_eq!(value, &inbox_name),
            _ => panic!("Expected RegularInbox variant"),
        }
    }

    #[test]
    fn test_from_message() {
        let mock_message = ShinkaiMessage {
            body: MessageBody::Unencrypted(ShinkaiBody {
                message_data: MessageData::Unencrypted(ShinkaiData {
                    message_raw_content: "ACK".into(),
                    message_content_schema: MessageSchemaType::TextContent,
                }),
                internal_metadata: InternalMetadata {
                    sender_subidentity: "".into(),
                    recipient_subidentity: "".into(),
                    inbox: "inbox::@@node1.shinkai/subidentity::@@node2.shinkai/subidentity2::true".into(),
                    signature: "".into(),
                    encryption: EncryptionMethod::None,
                    node_api_data: None,
                },
            }),
            external_metadata: ExternalMetadata {
                sender: "@@node2.shinkai".into(),
                recipient: "@@node1.shinkai".into(),
                scheduled_time: "20230714T19363326163".into(),
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ"
                    .into(),
                other: "".into(),
                intra_sender: "".into(),
            },
            encryption: EncryptionMethod::None,
            version: ShinkaiVersion::V1_0,
        };

        let manager = InboxName::from_message(&mock_message).unwrap();
        match &manager {
            InboxName::RegularInbox { value, .. } => assert_eq!(
                value,
                "inbox::@@node1.shinkai/subidentity::@@node2.shinkai/subidentity2::true"
            ),
            _ => panic!("Expected RegularInbox variant"),
        }
    }

    #[test]
    fn test_from_message_invalid() {
        let mock_message = ShinkaiMessage {
            body: MessageBody::Unencrypted(ShinkaiBody {
                message_data: MessageData::Unencrypted(ShinkaiData {
                    message_raw_content: "ACK".into(),
                    message_content_schema: MessageSchemaType::TextContent,
                }),
                internal_metadata: InternalMetadata {
                    sender_subidentity: "".into(),
                    recipient_subidentity: "".into(),
                    inbox: "1nb0x::@@node1.shinkai/subidentity::@@node2.shinkai/subidentity2::truee".into(),
                    signature: "".into(),
                    encryption: EncryptionMethod::None,
                    node_api_data: None,
                },
            }),
            external_metadata: ExternalMetadata {
                sender: "@@node2.shinkai".into(),
                recipient: "@@node1.shinkai".into(),
                scheduled_time: "20230714T19363326163".into(),
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ"
                    .into(),
                other: "".into(),
                intra_sender: "".into(),
            },
            encryption: EncryptionMethod::None,
            version: ShinkaiVersion::V1_0,
        };

        let result = InboxName::from_message(&mock_message);
        assert!(result.is_err(), "Expected invalid conversion");
    }

    #[test]
    fn test_get_inbox_name_from_params_valid() {
        let sender = "@@sender.shinkai".to_string();
        let sender_subidentity = "subidentity".to_string();
        let recipient = "@@recipient.shinkai".to_string();
        let recipient_subidentity = "subidentity2".to_string();
        let is_e2e = true;

        let result = InboxName::get_regular_inbox_name_from_params(
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            is_e2e,
        );

        assert!(result.is_ok(), "Expected valid conversion");
    }

    #[test]
    fn test_get_inbox_name_from_reparable_params() {
        let sender = "sender.shinkai".to_string();
        let sender_subidentity = "subidentity".to_string();
        let recipient = "@@recipient".to_string();
        let recipient_subidentity = "subidentity2".to_string();
        let is_e2e = true;

        let result = InboxName::get_regular_inbox_name_from_params(
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            is_e2e,
        );

        assert!(result.is_ok(), "Expected valid conversion");
    }

    #[test]
    fn test_get_inbox_name_from_params_invalid() {
        let sender = "invald.sender".to_string(); // Invalid sender
        let sender_subidentity = "subidentity//1".to_string();
        let recipient = "@@@recipient.shinkai".to_string();
        let recipient_subidentity = "subidentity2".to_string();
        let is_e2e = true;

        let result = InboxName::get_regular_inbox_name_from_params(
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            is_e2e,
        );

        assert!(result.is_err(), "Expected invalid conversion");
    }

    #[test]
    fn test_has_creation_access() {
        let manager = InboxName::new(
            "inbox::@@node1.shinkai/subidentity::@@node2.shinkai::@@node3.shinkai/subidentity3::true".to_string(),
        )
        .unwrap();

        let identity_name = ShinkaiName::new("@@node1.shinkai/subidentity".to_string()).unwrap();
        let identity_name_2 = ShinkaiName::new("@@node2.shinkai/subidentity".to_string()).unwrap();

        match manager.has_creation_access(identity_name) {
            Ok(access) => assert!(access, "Expected identity to have creation access"),
            Err(err) => panic!("Unexpected error: {:?}", err),
        }

        match manager.has_creation_access(identity_name_2) {
            Ok(access) => assert!(access, "Expected identity to have creation access"),
            Err(err) => panic!("Unexpected error: {:?}", err),
        }
    }

    #[test]
    fn test_has_sender_creation_access() {
        let mock_message = ShinkaiMessage {
            body: MessageBody::Unencrypted(ShinkaiBody {
                message_data: MessageData::Unencrypted(ShinkaiData {
                    message_raw_content: "ACK".into(),
                    message_content_schema: MessageSchemaType::TextContent,
                }),
                internal_metadata: InternalMetadata {
                    sender_subidentity: "subidentity2".into(),
                    recipient_subidentity: "".into(),
                    inbox: "inbox::@@node1.shinkai::@@node2.shinkai/subidentity2::true".into(),
                    signature: "".into(),
                    encryption: EncryptionMethod::None,
                    node_api_data: Some(NodeApiData {
                        parent_hash: "".into(),
                        node_message_hash: "node_message_hash".into(),
                        node_timestamp: "20230714T19363326163".into(),
                    }),
                },
            }),
            external_metadata: ExternalMetadata {
                sender: "@@node2.shinkai".into(),
                recipient: "@@node1.shinkai".into(),
                scheduled_time: "20230714T19363326163".into(),
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ"
                    .into(),
                other: "".into(),
                intra_sender: "".into(),
            },
            encryption: EncryptionMethod::None,
            version: ShinkaiVersion::V1_0,
        };

        let manager = InboxName::from_message(&mock_message).unwrap();
        match manager.has_sender_creation_access(mock_message) {
            Ok(access) => assert!(access, "Expected sender to have creation access"),
            Err(err) => panic!("Unexpected error: {:?}", err),
        }
    }

    #[test]
    fn test_sender_does_not_have_creation_access() {
        let mock_message = ShinkaiMessage {
            body: MessageBody::Unencrypted(ShinkaiBody {
                message_data: MessageData::Unencrypted(ShinkaiData {
                    message_raw_content: "ACK".into(),
                    message_content_schema: MessageSchemaType::TextContent,
                }),
                internal_metadata: InternalMetadata {
                    sender_subidentity: "subidentity3".into(),
                    recipient_subidentity: "".into(),
                    inbox: "inbox::@@node1.shinkai::@@node2.shinkai::true".into(),
                    signature: "".into(),
                    encryption: EncryptionMethod::None,
                    node_api_data: Some(NodeApiData {
                        parent_hash: "parent_hash".into(),
                        node_message_hash: "node_message_hash".into(),
                        node_timestamp: "20230714T19363326163".into(),
                    }),
                },
            }),
            external_metadata: ExternalMetadata {
                sender: "@@node3.shinkai".into(),
                recipient: "@@node1.shinkai".into(),
                scheduled_time: "20230714T19363326163".into(),
                signature: "3PLx2vZV8kccEEbwPepPQYv2D5zaiSFJXy3JtK57fLuKyh7TBJmcwqMkuCnzLgzAxoatAyKnUSf41smqijpiPBFJ"
                    .into(),
                other: "".into(),
                intra_sender: "".into(),
            },
            encryption: EncryptionMethod::None,
            version: ShinkaiVersion::V1_0,
        };

        let manager = InboxName::from_message(&mock_message).unwrap();

        match manager.has_sender_creation_access(mock_message) {
            Ok(access) => assert!(!access, "Expected sender to not have creation access"),
            Err(err) => panic!("Unexpected error: {:?}", err),
        }
    }
}
