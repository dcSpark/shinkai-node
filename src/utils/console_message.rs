// // message_creation.rs

// use super::{args::Args, keys::NodeKeys};
// use crate::{
//     shinkai_message::{
//         encryption::{string_to_encryption_public_key, EncryptionMethod},
//         shinkai_message_builder::ShinkaiMessageBuilder,
//         shinkai_message_extension::ShinkaiMessageWrapper,
//         signatures::clone_signature_secret_key,
//     },
//     shinkai_message_proto::{Field, ShinkaiMessage},
// };
// use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
// use serde_json;
// use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

// pub fn create_message(
//     args: &Args,
//     node_keys: &NodeKeys,
//     global_identity_name: &str,
//     identity_secret_key_string: &str,
// ) {
//     let node2_encryption_pk_str = args
//         .clone()
//         .receiver
//         .expect("receiver_encryption_pk argument is required for create_message");
//     let recipient = args
//         .recipient
//         .expect("recipient argument is required for create_message");
//     let other = args.other.unwrap_or("".to_string());
//     let node2_encryption_pk =
//         string_to_encryption_public_key(node2_encryption_pk_str.as_str()).unwrap();

//     println!("Creating message for recipient: {}", recipient);
//     println!("identity_secret_key: {}", identity_secret_key_string);
//     println!("receiver_encryption_pk: {}", node2_encryption_pk_str);

//     if let Some(code) = args.code_registration.clone() {
//         let message = code_registration(
//             node_keys,
//             node2_encryption_pk,
//             code,
//             global_identity_name,
//             recipient,
//         );
//         print_message_json(&message);
//     } else if args.create_message {
//         let message = create_plain_message(
//             node_keys,
//             node2_encryption_pk,
//             recipient,
//             other,
//             global_identity_name,
//         );
//         print_message_json(&message);
//     }
// }

// fn code_registration(
//     node_keys: &NodeKeys,
//     node2_encryption_pk: EncryptionPublicKey,
//     code: String,
//     global_identity_name: &str,
//     recipient: String,
// ) -> ShinkaiMessage {
//     let message = ShinkaiMessageBuilder::code_registration(
//         node_keys.encryption_secret_key.clone(),
//         clone_signature_secret_key(&node_keys.identity_secret_key),
//         node2_encryption_pk,
//         code.to_string(),
//         global_identity_name.to_string().clone(),
//         recipient.to_string(),
//     )
//     .expect("Failed to create message with code registration");

//     println!(
//         "Message's signature: {}",
//         message.clone().external_metadata.unwrap().signature
//     );
//     message
// }

// fn create_plain_message(
//     node_keys: &NodeKeys,
//     node2_encryption_pk: EncryptionPublicKey,
//     recipient: String,
//     other: String,
//     global_identity_name: &str,
// ) -> ShinkaiMessage {
//     let fields = vec![Field {
//         name: "field1".to_string(),
//         field_type: "type1".to_string(),
//     }];

//     let message = ShinkaiMessageBuilder::new(
//         node_keys.encryption_secret_key.clone(),
//         clone_signature_secret_key(&node_keys.identity_secret_key),
//         node2_encryption_pk,
//     )
//     .body("body content".to_string())
//     .encryption(EncryptionMethod::None)
//     .message_schema_type("schema type".to_string(), fields)
//     .internal_metadata("".to_string(), "".to_string(), "".to_string())
//     .external_metadata_with_other(
//         recipient.to_string(),
//         global_identity_name.to_string().clone(),
//         other.to_string(),
//     )
//     .build()
//     .expect("Failed to create message");

//     println!(
//         "Message's signature: {}",
//         message.clone().external_metadata.unwrap().signature
//     );
//     message
// }

// fn print_message_json(message: &ShinkaiMessage) {
//     // Parse the message to JSON and print to stdout
//     let message_wrapper = ShinkaiMessageWrapper::from(message);

//     // Serialize the wrapper into JSON and print to stdout
//     let message_json = serde_json::to_string_pretty(&message_wrapper);

//     match message_json {
//         Ok(json) => println!("{}", json),
//         Err(e) => println!("Error creating JSON: {}", e),
//     }
// }
