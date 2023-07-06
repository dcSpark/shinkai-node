// shinkai_message.rs

use crate::shinkai_message_proto::ShinkaiMessage;
use chrono::Utc;
use prost::Message;
use sha2::{Digest, Sha256};

pub struct ShinkaiMessageHandler;
pub type ProfileName = String;

impl ShinkaiMessageHandler {
    pub fn encode_message(message: ShinkaiMessage) -> Vec<u8> {
        let mut bytes = Vec::new();
        message.encode(&mut bytes).unwrap();
        bytes
    }

    pub fn decode_message(bytes: Vec<u8>) -> Result<ShinkaiMessage, prost::DecodeError> {
        ShinkaiMessage::decode(bytes.as_slice())
    }

    pub fn generate_time_now() -> String {
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%f").to_string();
        let scheduled_time = format!("{}{}", &timestamp[..17], &timestamp[17..20]);
        scheduled_time
    }

    pub fn calculate_hash(message: &ShinkaiMessage) -> String {
        let mut hasher = Sha256::new();

        hasher.update(format!("{:?}", message));
        let result = hasher.finalize();
        format!("{:x}", result)
    }
}

#[cfg(test)]
mod tests {
    use crate::shinkai_message::encryption::{
        unsafe_deterministic_encryption_keypair, EncryptionMethod,
    };
    use crate::shinkai_message::signatures::{
        unsafe_deterministic_signature_keypair, verify_signature,
    };
    use crate::shinkai_message::{
        shinkai_message_builder::ShinkaiMessageBuilder,
        shinkai_message_handler::ShinkaiMessageHandler,
    };
    use crate::shinkai_message_proto::{Field, ShinkaiMessage};

    fn build_message(encryption: EncryptionMethod) -> ShinkaiMessage {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let fields = vec![
            Field {
                name: "field1".to_string(),
                field_type: "type1".to_string(),
            },
            Field {
                name: "field2".to_string(),
                field_type: "type2".to_string(),
            },
        ];

        let message_result =
            ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk)
                .body("Hello World".to_string())
                .encryption(encryption)
                .message_schema_type("MyType".to_string(), fields)
                .topic("my_topic".to_string(), "my_channel".to_string())
                .internal_metadata_content("InternalContent".to_string())
                .external_metadata_with_schedule(
                    recipient,
                    sender,
                    "20230702T20533481345".to_string(),
                )
                .build();

        return message_result.unwrap();
    }

    #[test]
    fn test_encode_message() {
        let message = build_message(EncryptionMethod::None);
        let encoded_message = ShinkaiMessageHandler::encode_message(message);
        assert!(encoded_message.len() > 0);
    }

    #[test]
    fn test_encode_message_with_encryption() {
        let message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305);
        let encoded_message = ShinkaiMessageHandler::encode_message(message);
        assert!(encoded_message.len() > 0);
    }

    #[test]
    fn test_decode_message() {
        let message = build_message(EncryptionMethod::None);
        let encoded_message = ShinkaiMessageHandler::encode_message(message.clone());
        let decoded_message = ShinkaiMessageHandler::decode_message(encoded_message).unwrap();

        // Assert that the decoded message is the same as the original message
        let body = decoded_message.body.as_ref().unwrap();
        assert_eq!(body.content, "Hello World");

        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "InternalContent");
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .type_name,
            "MyType"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .name,
            "field1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .field_type,
            "type1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .name,
            "field2"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .field_type,
            "type2"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().topic_id,
            "my_topic"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().channel_id,
            "my_channel"
        );

        assert_eq!(decoded_message.encryption, "None");

        let (_, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let external_metadata = decoded_message.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.recipient, recipient);
        assert_eq!(external_metadata.scheduled_time, "20230702T20533481345");
        assert!(verify_signature(
            &my_identity_pk,
            &message,
            &external_metadata.signature
        )
        .unwrap())
    }

    #[test]
    fn test_decode_encrypted_message() {
        let message = build_message(EncryptionMethod::None);
        let encoded_message = ShinkaiMessageHandler::encode_message(message.clone());
        let decoded_message = ShinkaiMessageHandler::decode_message(encoded_message).unwrap();

        // Assert that the decoded message is the same as the original message
        let body = decoded_message.body.as_ref().unwrap();
        assert_eq!(body.content, "Hello World");

        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "InternalContent");
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .type_name,
            "MyType"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .name,
            "field1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .field_type,
            "type1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .name,
            "field2"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .field_type,
            "type2"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().topic_id,
            "my_topic"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().channel_id,
            "my_channel"
        );

        assert_eq!(decoded_message.encryption, "None");

        let (_, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string(); 

        let external_metadata = decoded_message.external_metadata.as_ref().unwrap();
        assert_eq!(
            external_metadata.sender,
            sender
        );
        assert_eq!(
            external_metadata.recipient,
            recipient
        );
        assert_eq!(external_metadata.scheduled_time, "20230702T20533481345");
        assert!(verify_signature(
            &my_identity_pk,
            &message,
            &external_metadata.signature
        )
        .unwrap()) 
    }
}
