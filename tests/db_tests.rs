use async_channel::{bounded, Receiver, Sender};
use shinkai_node::db::ShinkaiMessageDB;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::{Node, SubIdentityManager, Subidentity};
use shinkai_node::shinkai_message::encryption::{
    decrypt_body_message, decrypt_content_message, encryption_public_key_to_string, encryption_secret_key_to_string,
    hash_encryption_public_key, unsafe_deterministic_encryption_keypair, EncryptionMethod,
};
use shinkai_node::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_node::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use shinkai_node::shinkai_message::signatures::{
    clone_signature_secret_key, sign_message, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_node::shinkai_message::utils::hash_string;
use shinkai_node::shinkai_message_proto::ShinkaiMessage;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

fn generate_message_with_text(
    content: String,
    my_encryption_secret_key: EncryptionStaticKey,
    my_signature_secret_key: SignatureStaticKey,
    receiver_public_key: EncryptionPublicKey,
    recipient_subidentity_name: String,
    origin_destination_identity_name: String,
) -> ShinkaiMessage {
    let message = ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .body(content.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type("MyType".to_string())
        .internal_metadata(
            "".to_string(),
            recipient_subidentity_name.clone().to_string(),
            "".to_string(),
            EncryptionMethod::None,
        )
        .external_metadata_with_schedule(
            origin_destination_identity_name.clone().to_string(),
            origin_destination_identity_name.clone().to_string(),
            "20230702T20533481345".to_string(),
        )
        .build()
        .unwrap();
    message
}

#[test]
fn db_inbox() {
    setup();

    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let (node1_subidentity_sk, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (node1_subencryption_sk, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

    let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));

    let message = generate_message_with_text(
        "Hello World".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
    );

    let mut shinkai_db = ShinkaiMessageDB::new(&node1_db_path).unwrap();
    let _ = shinkai_db.insert_message(&message.clone());

    let last_messages_all = shinkai_db.get_last_messages_from_all(10).unwrap();
    assert_eq!(last_messages_all.len(), 1);
    assert_eq!(
        last_messages_all[0].clone().body.unwrap().content,
        "Hello World".to_string()
    );

    let inbox_name = ShinkaiMessageHandler::get_inbox_name(&message.clone()).unwrap();
    assert_eq!(
        inbox_name,
        "inbox_@@node1.shinkai@@node1.shinkaimain_profile_node1_false".to_string()
    );

    println!("Inbox name: {}", inbox_name);
    let last_messages_inbox = shinkai_db
        .get_last_messages_from_inbox(inbox_name.to_string(), 10)
        .unwrap();
    assert_eq!(last_messages_inbox.len(), 1);
    assert_eq!(
        last_messages_inbox[0].clone().body.unwrap().content,
        "Hello World".to_string()
    );

    // Get last unread messages
    let last_unread = shinkai_db
        .get_last_unread_messages_from_inbox(inbox_name.clone().to_string(), 10, None)
        .unwrap();
    assert_eq!(last_unread.len(), 1);
    assert_eq!(last_unread[0].clone().body.unwrap().content, "Hello World".to_string());

    let message2 = generate_message_with_text(
        "Hello World 2".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
    );
    let message3 = generate_message_with_text(
        "Hello World 3".to_string(),
        node1_encryption_sk,
        node1_identity_sk,
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
    );
    match shinkai_db.insert_message(&message2.clone()) {
        Ok(_) => println!("message2 inserted successfully"),
        Err(e) => println!("Failed to insert message2: {}", e),
    }

    match shinkai_db.insert_message(&message3.clone()) {
        Ok(_) => println!("message3 inserted successfully"),
        Err(e) => println!("Failed to insert message3: {}", e),
    }

    let last_messages_inbox = shinkai_db
        .get_last_messages_from_inbox(inbox_name.clone().to_string(), 2)
        .unwrap();
    println!("Last messages inbox: {:?}", last_messages_inbox);
    assert_eq!(last_messages_inbox.len(), 2);

    let last_unread_messages_inbox = shinkai_db
        .get_last_unread_messages_from_inbox(inbox_name.clone().to_string(), 2, None)
        .unwrap();
    assert_eq!(last_unread_messages_inbox.len(), 2);
    assert_eq!(
        last_unread_messages_inbox[0].clone().body.unwrap().content,
        "Hello World 3".to_string()
    );
    assert_eq!(
        last_unread_messages_inbox[1].clone().body.unwrap().content,
        "Hello World 2".to_string()
    );
    println!("Last unread messages inbox: {:?}", last_unread_messages_inbox);

    let offset = ShinkaiMessageHandler::get_message_offset_db_key(&last_unread_messages_inbox[1].clone()).unwrap();
    let last_unread_messages_inbox_page2 = shinkai_db
        .get_last_unread_messages_from_inbox(inbox_name.clone().to_string(), 3, Some(offset))
        .unwrap();
    assert_eq!(last_unread_messages_inbox_page2.len(), 1);
    assert_eq!(
        last_unread_messages_inbox_page2[0].clone().body.unwrap().content,
        "Hello World".to_string()
    );

    // Mark as read up to a certain time
    shinkai_db
        .mark_as_read_up_to(inbox_name.clone().to_string(), "20230703T00000000000".to_string())
        .unwrap();

    let last_messages_inbox = shinkai_db
        .get_last_unread_messages_from_inbox(inbox_name.clone().to_string(), 2, None)
        .unwrap();
    assert_eq!(last_messages_inbox.len(), 0);

    // Test permissions
    // shinkai_db
    //     .add_permission(&node1_identity_name, "device_perms", "device1", Permission::Admin)
    //     .unwrap();
    // assert!(shinkai_db
    //     .has_permission(&node1_identity_name, "device_perms", "device1", Permission::Admin)
    //     .unwrap());
    // shinkai_db
    //     .remove_permission(&node1_identity_name, "device_perms", "device1")
    //     .unwrap();
    // assert!(!shinkai_db
    //     .has_permission(&node1_identity_name, "device_perms", "device1", Permission::Admin)
    //     .unwrap());
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use shinkai_node::{shinkai_message_proto::ShinkaiMessage, shinkai_message::{encryption::unsafe_deterministic_private_key, shinkai_message_builder::ShinkaiMessageBuilder}};
//     use prost::Message;
//     use rocksdb::{ColumnFamilyDescriptor, Error, Options, DB};
//     use std::{convert::TryInto, collections::HashMap};
//     // use tempfile::Builder;

//     fn get_test_db_path() -> String {
//         let temp_dir = Builder::new()
//             .prefix("test_db")
//             .rand_bytes(5)
//             .tempdir()
//             .unwrap();
//         temp_dir.into_path().to_str().unwrap().to_string()
//     }

//     fn get_test_message() -> ShinkaiMessage {
//         let (secret_key, public_key) = unsafe_deterministic_private_key(0);

//         // Replace this with actual field data
//         let fields = HashMap::new();

//         // Build the ShinkaiMessage
//         ShinkaiMessageBuilder::new(&secret_key, &public_key)
//             .body("body content".to_string())
//             .encryption("no_encryption".to_string())
//             .message_schema_type("schema type".to_string(), &fields)
//             .topic("topic_id".to_string(), "channel_id".to_string())
//             .internal_metadata_content("internal metadata content".to_string())
//             .external_metadata(&public_key)
//             .build()
//             .unwrap()
//     }

//     #[test]
//     fn test_insert_get() {
//         let db_path = get_test_db_path();
//         let db = ShinkaiMessageDB::new(&db_path).unwrap();
//         let message = get_test_message();

//         // Insert the message in AllMessages topic
//         let key = ShinkaiMessageHandler::calculate_hash(&message);
//         db.insert(key.clone(), &message, Topic::AllMessages).unwrap();

//         // Retrieve the message and validate it
//         let retrieved_message = db.get(key, Topic::AllMessages).unwrap().unwrap();
//         assert_eq!(message, retrieved_message);
//     }

//     #[test]
//     fn test_insert_message() {
//         let db_path = get_test_db_path();
//         let db = ShinkaiMessageDB::new(&db_path).unwrap();
//         let message = get_test_message();

//         // Insert the message
//         db.insert_message(&message).unwrap();

//         // Retrieve the message from AllMessages and validate it
//         let all_messages_key = ShinkaiMessageHandler::calculate_hash(&message);
//         let retrieved_message = db.get(all_messages_key, Topic::AllMessages).unwrap().unwrap();
//         assert_eq!(message, retrieved_message);

//         // Retrieve the pointer from AllMessagesTimeKeyed and validate it
//         let time_keyed_key = if message.scheduled_time.is_empty() {
//             ShinkaiMessageHandler::generate_time_now()
//         } else {
//             message.scheduled_time.clone()
//         };
//         let retrieved_key = db.get(time_keyed_key, Topic::AllMessagesTimeKeyed).unwrap().unwrap();
//         assert_eq!(all_messages_key, retrieved_key);
//     }

//     #[test]
//     fn test_schedule_message() {
//         let db_path = get_test_db_path();
//         let db = ShinkaiMessageDB::new(&db_path).unwrap();
//         let message = get_test_message();

//         // Schedule the message
//         db.schedule_message(&message).unwrap();

//         // Retrieve the scheduled message and validate it
//         let scheduled_key = if message.scheduled_time.is_empty() {
//             ShinkaiMessageHandler::generate_time_now()
//         } else {
//             message.scheduled_time.clone()
//         };
//         let retrieved_message = db.get(scheduled_key, Topic::ScheduledMessage).unwrap().unwrap();
//         assert_eq!(message, retrieved_message);
//     }
// }
