// shinkai_message.rs

use std::io::Error;

use crate::shinkai_message_proto::{Body, ShinkaiMessage, ExternalMetadata};
use chrono::Utc;
use prost::Message;
use sha2::{Digest, Sha256};

use super::{shinkai_message_extension::ShinkaiMessageWrapper, signatures::sign_message};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};

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

    pub fn as_json_string(message: ShinkaiMessage) -> Result<String, Error> {
        let message_wrapper = ShinkaiMessageWrapper::from(&message);
        let message_json = serde_json::to_string_pretty(&message_wrapper);
        message_json.map_err(|e| Error::new(std::io::ErrorKind::Other, e))
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

    pub fn encode_body(body: Body) -> Vec<u8> {
        let mut bytes = Vec::new();
        body.encode(&mut bytes).unwrap();
        bytes
    }

    pub fn decode_body(bytes: Vec<u8>) -> Result<Body, prost::DecodeError> {
        Body::decode(bytes.as_slice())
    }

    pub fn re_sign_message(message: ShinkaiMessage, signature_sk: SignatureStaticKey) -> ShinkaiMessage {
        // make sure to not include the current signature in the hash
        let mut message = message.clone();

        if let Some(external_metadata) = &mut message.external_metadata {
            external_metadata.signature = String::from("");
        }

        let signature = sign_message(&signature_sk, message.clone());
        if let Some(external_metadata) = &mut message.external_metadata {
            external_metadata.signature = signature;
        }
        message
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
                .body_encryption(encryption)
                .message_schema_type("MyType".to_string(), fields)
                .internal_metadata("".to_string(), "".to_string(), "".to_string(), EncryptionMethod::None)
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
    fn test_encode_and_decode_body() {
        let message = build_message(EncryptionMethod::None);
        let body = message.body.unwrap();

        let encoded_body = ShinkaiMessageHandler::encode_body(body.clone());
        assert!(encoded_body.len() > 0);

        let decoded_body = ShinkaiMessageHandler::decode_body(encoded_body).unwrap();

        // Assert that the decoded body is the same as the original body
        assert_eq!(decoded_body.content, body.content);
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
        assert_eq!(internal_metadata.sender_subidentity, "");
        assert_eq!(internal_metadata.recipient_subidentity, "");
        assert_eq!(internal_metadata.inbox, "");
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

        assert_eq!(decoded_message.encryption, "None");

        let (_, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let external_metadata = decoded_message.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.recipient, recipient);
        assert_eq!(external_metadata.scheduled_time, "20230702T20533481345");
        assert!(verify_signature(&my_identity_pk, &message,).unwrap())
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
        assert_eq!(internal_metadata.sender_subidentity, "");
        assert_eq!(internal_metadata.recipient_subidentity, "");
        assert_eq!(internal_metadata.inbox, "");
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

        assert_eq!(decoded_message.encryption, "None");

        let (_, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let external_metadata = decoded_message.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.recipient, recipient);
        assert_eq!(external_metadata.scheduled_time, "20230702T20533481345");
        assert!(verify_signature(&my_identity_pk, &message,).unwrap())
    }
}
