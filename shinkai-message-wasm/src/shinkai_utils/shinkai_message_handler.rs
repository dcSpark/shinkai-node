// use std::io::Error;

// use chrono::Utc;
// use sha2::{Digest, Sha256};

// use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
// use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

// use crate::shinkai_message::{
//     shinkai_message::{MessageBody, MessageData, ShinkaiBody, ShinkaiMessage},
//     shinkai_message_error::ShinkaiMessageError,
//     shinkai_message_schemas::MessageSchemaType,
// };

// pub struct ShinkaiMessageHandler;
// pub type ProfileName = String;

// #[derive(Debug, PartialEq)]
// pub enum EncryptionStatus {
//     NotCurrentlyEncrypted,
//     BodyEncrypted,
//     ContentEncrypted,
// }

// impl ShinkaiMessageHandler {
//     pub fn encode_message_body(body: MessageBody) -> Vec<u8> {
//         bincode::serialize(&body).unwrap()
//     }

//     pub fn decode_message_body(encoded: Vec<u8>) -> ShinkaiBody {
//         bincode::deserialize(&encoded[..]).unwrap()
//     }

//     pub fn encode_shinkai_body(body: ShinkaiBody) -> Vec<u8> {
//         bincode::serialize(&body).unwrap()
//     }

//     pub fn decode_shinkai_body(encoded: Vec<u8>) -> ShinkaiBody {
//         bincode::deserialize(&encoded[..]).unwrap()
//     }

//     pub fn encode_message(message: ShinkaiMessage) -> Vec<u8> {
//         bincode::serialize(&message).unwrap()
//     }

//     pub fn decode_message(encoded: Vec<u8>) -> ShinkaiMessage {
//         bincode::deserialize(&encoded[..]).unwrap()
//     }

//     pub fn encode_body_result(body: ShinkaiBody) -> Result<Vec<u8>, bincode::Error> {
//         bincode::serialize(&body)
//     }

//     pub fn decode_body_result(encoded: Vec<u8>) -> Result<ShinkaiBody, bincode::Error> {
//         bincode::deserialize(&encoded[..])
//     }

//     pub fn encode_message_result(message: ShinkaiMessage) -> Result<Vec<u8>, bincode::Error> {
//         bincode::serialize(&message)
//     }

//     pub fn decode_message_result(encoded: Vec<u8>) -> Result<ShinkaiMessage, bincode::Error> {
//         bincode::deserialize(&encoded[..])
//     }

//     pub fn as_json_string(message: ShinkaiMessage) -> Result<String, Error> {
//         let message_json = serde_json::to_string_pretty(&message);
//         message_json.map_err(|e| Error::new(std::io::ErrorKind::Other, e))
//     }

//     pub fn generate_time_now() -> String {
//         let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S.%f").to_string();
//         let scheduled_time = format!("{}Z", &timestamp[..23]);
//         scheduled_time
//     }

//     pub fn calculate_hash(message: &ShinkaiMessage) -> String {
//         let mut hasher = Sha256::new();

//         hasher.update(format!("{:?}", message));
//         let result = hasher.finalize();
//         format!("{:x}", result)
//     }

//     pub fn calculate_body_hash_from_body(body: &MessageBody) -> String {
//         let mut hasher = Sha256::new();
//         hasher.update(format!("{:?}", body));
//         let result = hasher.finalize();
//         format!("{:x}", result)
//     }

//     pub fn calculate_body_hash(message: &ShinkaiMessage) -> String {
//         Self::calculate_body_hash_from_body(&message.body)
//     }

//     pub fn is_body_currently_encrypted(message: &ShinkaiMessage) -> bool {
//         matches!(message.body, MessageBody::Encrypted(_))
//     }

//     pub fn is_content_currently_encrypted(message: &ShinkaiMessage) -> bool {
//         match &message.body {
//             MessageBody::Encrypted(_) => true,
//             MessageBody::Unencrypted(body) => matches!(body.message_data, MessageData::Encrypted(_)),
//         }
//     }

//     pub fn get_encryption_status(message: ShinkaiMessage) -> EncryptionStatus {
//         if ShinkaiMessageHandler::is_body_currently_encrypted(&message) {
//             EncryptionStatus::BodyEncrypted
//         } else if ShinkaiMessageHandler::is_content_currently_encrypted(&message) {
//             EncryptionStatus::ContentEncrypted
//         } else {
//             EncryptionStatus::NotCurrentlyEncrypted
//         }
//     }

//     pub fn validate_message_schema(msg: &ShinkaiMessage, schema: MessageSchemaType) -> Result<(), ShinkaiMessageError> {
//         if let MessageBody::Unencrypted(body) = &msg.body {
//             if let MessageData::Unencrypted(data) = &body.message_data {
//                 if data.message_content_schema != schema {
//                     return Err(ShinkaiMessageError::InvalidMessageSchemaType(
//                         "Invalid message schema type".into(),
//                     ));
//                 }
//             } else {
//                 return Err(ShinkaiMessageError::InvalidMessageSchemaType(
//                     "Message data is encrypted".into(),
//                 ));
//             }
//         } else {
//             return Err(ShinkaiMessageError::MissingMessageBody("Missing message body".into()));
//         }
//         Ok(())
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::shinkai_message::shinkai_message_schemas::MessageSchemaType;
//     use crate::shinkai_utils::encryption::{unsafe_deterministic_encryption_keypair, EncryptionMethod};
//     use crate::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
//     use crate::shinkai_utils::signatures::{unsafe_deterministic_signature_keypair, verify_signature};

//     fn build_message(body_encryption: EncryptionMethod, content_encryption: EncryptionMethod) -> ShinkaiMessage {
//         let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
//         let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
//         let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

//         let recipient = "@@other_node.shinkai".to_string();
//         let sender = "@@my_node.shinkai".to_string();
//         let scheduled_time = "2023-07-02T20:53:34.813Z".to_string();

//         let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk)
//             .message_raw_content("Hello World".to_string())
//             .body_encryption(body_encryption)
//             .message_schema_type(MessageSchemaType::TextContent)
//             .internal_metadata("".to_string(), "".to_string(), content_encryption)
//             .external_metadata_with_schedule(recipient, sender, "2023-07-02T20:53:34.813Z".to_string())
//             .build();

//         return message_result.unwrap();
//     }

//     #[test]
//     fn test_is_body_currently_encrypted_encryption_none() {
//         let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
//         assert!(!ShinkaiMessageHandler::is_body_currently_encrypted(&message));
//     }

//     #[test]
//     fn test_is_body_currently_encrypted_encryption_set_with_internal_metadata() {
//         let message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305, EncryptionMethod::None);
//         assert!(ShinkaiMessageHandler::is_body_currently_encrypted(&message));
//     }

//     #[test]
//     fn test_encode_message() {
//         let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
//         let encoded_message = ShinkaiMessageHandler::encode_message(message);
//         assert!(encoded_message.len() > 0);
//     }

//     #[test]
//     fn test_encode_message_with_encryption() {
//         let message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305, EncryptionMethod::None);
//         let encoded_message = ShinkaiMessageHandler::encode_message(message);
//         assert!(encoded_message.len() > 0);
//     }

//     #[test]
//     fn test_is_content_currently_encrypted() {
//         // Test case when body encryption is set to EncryptionMethod::None
//         let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
//         assert!(!ShinkaiMessageHandler::is_content_currently_encrypted(&message));

//         // Test case when body encryption is set but internal_metadata.encryption is set to EncryptionMethod::None
//         let mut message = build_message(EncryptionMethod::None, EncryptionMethod::None);
//         assert!(!ShinkaiMessageHandler::is_content_currently_encrypted(&message));

//         // Test case when body encryption is set, internal_metadata.encryption is set and message_schema_type is None
//         let mut message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305, EncryptionMethod::None);
//         assert!(ShinkaiMessageHandler::is_content_currently_encrypted(&message));
//     }

//     #[test]
//     fn test_get_encryption_status() {
//         // Test case when body encryption is set to EncryptionMethod::None
//         let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
//         assert_eq!(
//             ShinkaiMessageHandler::get_encryption_status(message),
//             EncryptionStatus::NotCurrentlyEncrypted
//         );

//         // Test case when body encryption is not set but internal_metadata.encryption is set to encrypt
//         let message = build_message(EncryptionMethod::None, EncryptionMethod::DiffieHellmanChaChaPoly1305);
//         assert_eq!(
//             ShinkaiMessageHandler::get_encryption_status(message),
//             EncryptionStatus::ContentEncrypted
//         );

//         // Test case when body encryption is set but internal_metadata.encryption is set to EncryptionMethod::None
//         let message = build_message(EncryptionMethod::DiffieHellmanChaChaPoly1305, EncryptionMethod::None);
//         assert_eq!(
//             ShinkaiMessageHandler::get_encryption_status(message),
//             EncryptionStatus::BodyEncrypted
//         );

//         // Test case when body encryption is set, internal_metadata.encryption is set and message_schema_type is None
//         let mut message = build_message(EncryptionMethod::None, EncryptionMethod::None);

//         let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
//         let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

//         // Use the encrypt function from the ShinkaiMessage struct
//         message = match ShinkaiMessage::encrypt_outer_layer(&message, &my_encryption_sk, &node2_encryption_pk) {
//             Ok(encrypted_message) => encrypted_message,
//             Err(_) => panic!("Encryption failed"),
//         };

//         assert_eq!(
//             ShinkaiMessageHandler::get_encryption_status(message),
//             EncryptionStatus::BodyEncrypted
//         );
//     }

//     #[test]
//     fn test_encode_and_decode_message_body() {
//         let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
//         let body = message.body;

//         let encoded_body = ShinkaiMessageHandler::encode_message_body(body.clone());
//         assert!(encoded_body.len() > 0);

//         let decoded_body = ShinkaiMessageHandler::decode_message_body(encoded_body);

//         // Assert that the decoded body is the same as the original body
//         if let MessageBody::Unencrypted(body_unencrypted) = body {
//             if let MessageData::Unencrypted(data) = body_unencrypted.message_data {
//                 if let MessageData::Unencrypted(decoded_data) = decoded_body.message_data {
//                     assert_eq!(decoded_data.message_raw_content, data.message_raw_content);
//                 }
//             }
//         }
//     }

//     #[test]
//     fn test_decode_message() {
//         let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
//         let encoded_message = ShinkaiMessageHandler::encode_message(message.clone());
//         let decoded_message = ShinkaiMessageHandler::decode_message(encoded_message);

//         // Assert that the decoded message is the same as the original message
//         if let MessageBody::Unencrypted(body_unencrypted) = decoded_message.body {
//             if let MessageData::Unencrypted(data) = body_unencrypted.message_data {
//                 assert_eq!(data.message_raw_content, "Hello World");
//                 assert_eq!(data.message_content_schema, MessageSchemaType::TextContent);
//             }

//             let internal_metadata = &body_unencrypted.internal_metadata;
//             assert_eq!(internal_metadata.sender_subidentity, "");
//             assert_eq!(internal_metadata.recipient_subidentity, "");
//             assert_eq!(
//                 internal_metadata.inbox,
//                 "inbox::@@my_node.shinkai::@@other_node.shinkai::false"
//             );
//         }

//         assert_eq!(decoded_message.encryption, EncryptionMethod::None);

//         let (_, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
//         let recipient = "@@other_node.shinkai".to_string();
//         let sender = "@@my_node.shinkai".to_string();
//         let scheduled_time = "2023-07-02T20:53:34.813Z".to_string();

//         let external_metadata = decoded_message.external_metadata;
//         assert_eq!(external_metadata.sender, sender);
//         assert_eq!(external_metadata.recipient, recipient);
//         assert_eq!(external_metadata.scheduled_time, "2023-07-02T20:53:34.813Z");
//         assert!(verify_signature(&my_identity_pk, &message,).unwrap())
//     }

//     #[test]
//     fn test_decode_encrypted_message() {
//         let message = build_message(EncryptionMethod::None, EncryptionMethod::None);
//         let encoded_message = ShinkaiMessageHandler::encode_message(message.clone());
//         let decoded_message = ShinkaiMessageHandler::decode_message(encoded_message);
    
//         // Assert that the decoded message is the same as the original message
//         if let MessageBody::Unencrypted(body_unencrypted) = decoded_message.body {
//             if let MessageData::Unencrypted(data) = body_unencrypted.message_data {
//                 assert_eq!(data.message_raw_content, "Hello World");
//                 assert_eq!(data.message_content_schema, MessageSchemaType::TextContent);
//             }
    
//             let internal_metadata = &body_unencrypted.internal_metadata;
//             assert_eq!(internal_metadata.sender_subidentity, "");
//             assert_eq!(internal_metadata.recipient_subidentity, "");
//             assert_eq!(
//                 internal_metadata.inbox,
//                 "inbox::@@my_node.shinkai::@@other_node.shinkai::false"
//             );
//         }
    
//         assert_eq!(decoded_message.encryption, EncryptionMethod::None);

//         let (_, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
//         let recipient = "@@other_node.shinkai".to_string();
//         let sender = "@@my_node.shinkai".to_string();
//         let scheduled_time = "2023-07-02T20:53:34.813Z".to_string();

//         let external_metadata = &decoded_message.external_metadata;
//         assert_eq!(external_metadata.sender, sender);
//         assert_eq!(external_metadata.recipient, recipient);
//         assert_eq!(external_metadata.scheduled_time, "2023-07-02T20:53:34.813Z");
//         assert!(verify_signature(&my_identity_pk, &message,).unwrap())
//     }
// }
