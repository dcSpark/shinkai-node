use std::io::{Error, ErrorKind};

use chrono::Utc;
use sha2::{Digest, Sha256};

use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{shinkai_utils::{encryption::{EncryptionMethod, encrypt_body}, signatures::sign_message}, shinkai_message::{shinkai_message::{ShinkaiMessage, Body}}};

pub struct ShinkaiMessageHandler;
pub type ProfileName = String;

#[derive(Debug, PartialEq)]
pub enum EncryptionStatus {
    NotCurrentlyEncrypted,
    BodyEncrypted,
    ContentEncrypted,
}

impl ShinkaiMessageHandler {
    pub fn encode_body(body: Body) -> Vec<u8> {
        bincode::serialize(&body).unwrap()
    }

    pub fn decode_body(encoded: Vec<u8>) -> Body {
        bincode::deserialize(&encoded[..]).unwrap()
    }
    
    pub fn encode_message(message: ShinkaiMessage) -> Vec<u8> {
        bincode::serialize(&message).unwrap()
    }

    pub fn decode_message(encoded: Vec<u8>) -> ShinkaiMessage {
        bincode::deserialize(&encoded[..]).unwrap()
    }

    pub fn encode_body_result(body: Body) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(&body)
    }
    
    pub fn decode_body_result(encoded: Vec<u8>) -> Result<Body, bincode::Error> {
        bincode::deserialize(&encoded[..])
    }
    
    pub fn encode_message_result(message: ShinkaiMessage) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(&message)
    }
    
    pub fn decode_message_result(encoded: Vec<u8>) -> Result<ShinkaiMessage, bincode::Error> {
        bincode::deserialize(&encoded[..])
    }
    
    pub fn as_json_string(message: ShinkaiMessage) -> Result<String, Error> {
        let message_json = serde_json::to_string_pretty(&message);
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

    pub fn encrypt_body_if_needed(
        message: ShinkaiMessage,
        my_encryption_secret_key: EncryptionStaticKey,
        receiver_public_key: EncryptionPublicKey,
    ) -> ShinkaiMessage {
        // if the message is already encrypted, return it
        if ShinkaiMessageHandler::is_body_currently_encrypted(&message) {
            return message;
        }

        let mut msg_to_encrypt = message.clone();
        msg_to_encrypt.encryption = EncryptionMethod::DiffieHellmanChaChaPoly1305;

        let encrypted_body = encrypt_body(
            &ShinkaiMessageHandler::encode_body(msg_to_encrypt.body.unwrap()),
            &my_encryption_secret_key,
            &receiver_public_key,
            &EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str().to_string(),
        )
        .expect("Failed to encrypt body");

        let new_body = Body {
            content: encrypted_body,
            internal_metadata: None,
        };

        msg_to_encrypt.body = Some(new_body);
        msg_to_encrypt
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

    pub fn is_body_currently_encrypted(message: &ShinkaiMessage) -> bool {
        if message.encryption == EncryptionMethod::None {
            return false;
        }

        match &message.body {
            Some(body) => body.internal_metadata.is_none(),
            None => false,
        }
    }

    pub fn is_content_currently_encrypted(message: &ShinkaiMessage) -> bool {
        println!("is_content_currently_encrypted {:?}", message);
        if ShinkaiMessageHandler::is_body_currently_encrypted(&message.clone()) {
            return true;
        }

        if let Some(body) = message.clone().body {
            if let Some(internal_metadata) = body.internal_metadata {
                // let encryption_method_none = EncryptionMethod::None.as_str().to_string();
                println!("internal_metadata.encryption is empty: {:?}", internal_metadata.message_schema_type.is_empty());
                println!("internal_metadata.encryption: {:?}", internal_metadata.encryption);
                if internal_metadata.encryption != EncryptionMethod::None
                    && internal_metadata.message_schema_type.is_empty()
                {
                    println!("true");
                    return true;
                }
            }
        }
        false
    }

    pub fn get_encryption_status(message: ShinkaiMessage) -> EncryptionStatus {
        if ShinkaiMessageHandler::is_body_currently_encrypted(&message) {
            EncryptionStatus::BodyEncrypted
        } else if ShinkaiMessageHandler::is_content_currently_encrypted(&message) {
            EncryptionStatus::ContentEncrypted
        } else {
            EncryptionStatus::NotCurrentlyEncrypted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shinkai_message::shinkai_message_schemas::MessageSchemaType;
    use crate::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
    use crate::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
    use crate::shinkai_utils::signatures::{unsafe_deterministic_signature_keypair, verify_signature};

    fn build_message(body_encryption: EncryptionMethod, content_encryption: EncryptionMethod) -> ShinkaiMessage {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk)
            .body("Hello World".to_string())
            .body_encryption(body_encryption)
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata("".to_string(), "".to_string(), "".to_string(), content_encryption)
            .external_metadata_with_schedule(recipient, sender, "20230702T20533481345".to_string())
            .build();

        return message_result.unwrap();
    }

    #[test]
    fn test_is_body_currently_encrypted_encryption_none() {
        let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
        assert!(!ShinkaiMessageHandler::is_body_currently_encrypted(&message));
    }

    #[test]
    fn test_is_body_currently_encrypted_encryption_set_no_internal_metadata() {
        let mut message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305, EncryptionMethod::None);
        message.body.as_mut().unwrap().internal_metadata = None;
        assert!(ShinkaiMessageHandler::is_body_currently_encrypted(&message));
    }

    #[test]
    fn test_is_body_currently_encrypted_encryption_set_with_internal_metadata() {
        let message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305, EncryptionMethod::None);
        assert!(ShinkaiMessageHandler::is_body_currently_encrypted(&message));
    }

    #[test]
    fn test_encode_message() {
        let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
        let encoded_message = ShinkaiMessageHandler::encode_message(message);
        assert!(encoded_message.len() > 0);
    }

    #[test]
    fn test_encode_message_with_encryption() {
        let message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305, EncryptionMethod::None);
        let encoded_message = ShinkaiMessageHandler::encode_message(message);
        assert!(encoded_message.len() > 0);
    }

    #[test]
    fn test_is_content_currently_encrypted() {
        // Test case when body encryption is set to EncryptionMethod::None
        let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
        assert!(!ShinkaiMessageHandler::is_content_currently_encrypted(&message));

        // Test case when body encryption is set but internal_metadata.encryption is set to EncryptionMethod::None
        let mut message = build_message(EncryptionMethod::None, EncryptionMethod::None);
        assert!(!ShinkaiMessageHandler::is_content_currently_encrypted(&message));

        // Test case when body encryption is set, internal_metadata.encryption is set and message_schema_type is None
        let mut message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305, EncryptionMethod::None);
        assert!(ShinkaiMessageHandler::is_content_currently_encrypted(&message));
    }

    #[test]
    fn test_get_encryption_status() {
        // Test case when body encryption is set to EncryptionMethod::None
        let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
        assert_eq!(
            ShinkaiMessageHandler::get_encryption_status(message),
            EncryptionStatus::NotCurrentlyEncrypted
        );

        // Test case when body encryption is not set but internal_metadata.encryption is set to encrypt
        let message = build_message(EncryptionMethod::None, EncryptionMethod::DiffieHellmanChaChaPoly1305);
        assert_eq!(
            ShinkaiMessageHandler::get_encryption_status(message),
            EncryptionStatus::ContentEncrypted
        );

        // Test case when body encryption is set but internal_metadata.encryption is set to EncryptionMethod::None
        let message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305, EncryptionMethod::None);
        assert_eq!(
            ShinkaiMessageHandler::get_encryption_status(message),
            EncryptionStatus::BodyEncrypted
        );

        // Test case when body encryption is set, internal_metadata.encryption is set and message_schema_type is None
        let mut message = build_message(EncryptionMethod::None, EncryptionMethod::None);

        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        message = ShinkaiMessageHandler::encrypt_body_if_needed(message, my_encryption_sk, node2_encryption_pk);
        assert_eq!(
            ShinkaiMessageHandler::get_encryption_status(message),
            EncryptionStatus::BodyEncrypted
        );
    }

    #[test]
    fn test_encode_and_decode_body() {
        let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
        let body = message.body.unwrap();

        let encoded_body = ShinkaiMessageHandler::encode_body(body.clone());
        assert!(encoded_body.len() > 0);

        let decoded_body = ShinkaiMessageHandler::decode_body(encoded_body);

        // Assert that the decoded body is the same as the original body
        assert_eq!(decoded_body.content, body.content);
    }

    #[test]
    fn test_decode_message() {
        let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
        let encoded_message = ShinkaiMessageHandler::encode_message(message.clone());
        let decoded_message = ShinkaiMessageHandler::decode_message(encoded_message);

        // Assert that the decoded message is the same as the original message
        let body = decoded_message.body.as_ref().unwrap();
        assert_eq!(body.content, "Hello World");

        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.sender_subidentity, "");
        assert_eq!(internal_metadata.recipient_subidentity, "");
        assert_eq!(internal_metadata.inbox, "");
        assert_eq!(internal_metadata.message_schema_type, MessageSchemaType::TextContent);

        assert_eq!(decoded_message.encryption, EncryptionMethod::None);

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
        let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
        let encoded_message = ShinkaiMessageHandler::encode_message(message.clone());
        let decoded_message = ShinkaiMessageHandler::decode_message(encoded_message);

        // Assert that the decoded message is the same as the original message
        let body = decoded_message.body.as_ref().unwrap();
        assert_eq!(body.content, "Hello World");

        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.sender_subidentity, "");
        assert_eq!(internal_metadata.recipient_subidentity, "");
        assert_eq!(internal_metadata.inbox, "");
        assert_eq!(internal_metadata.message_schema_type, MessageSchemaType::TextContent);
        assert_eq!(decoded_message.encryption, EncryptionMethod::None);

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
