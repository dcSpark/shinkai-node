use async_channel::{bounded, Receiver, Sender};
use prost::Message;
use shinkai_message_wasm::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_wasm::shinkai_utils::encryption::{unsafe_deterministic_encryption_keypair, EncryptionMethod};
use shinkai_message_wasm::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_wasm::shinkai_utils::shinkai_message_handler::ShinkaiMessageHandler;
use shinkai_message_wasm::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_wasm::shinkai_utils::utils::hash_string;
use shinkai_node::db::db_errors::ShinkaiDBError;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::managers::identity_manager::{IdentityType, StandardIdentity};
use shinkai_node::managers::{IdentityManager, InboxNameManager};
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use shinkai_node::schemas::inbox_permission::InboxPermission;
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

fn get_message_offset_db_key(message: &ShinkaiMessage) -> Result<String, ShinkaiDBError> {
    // Calculate the hash of the message for the key
    let hash_key = ShinkaiMessageHandler::calculate_hash(&message);

    // Clone the external_metadata first, then unwrap
    let cloned_external_metadata = message.external_metadata.clone();
    let ext_metadata = cloned_external_metadata.expect("Failed to clone external metadata");

    // Get the scheduled time or calculate current time
    let time_key = match ext_metadata.scheduled_time.is_empty() {
        true => ShinkaiMessageHandler::generate_time_now(),
        false => ext_metadata.scheduled_time.clone(),
    };

    // Create the composite key by concatenating the time_key and the hash_key, with a separator
    let composite_key = format!("{}:{}", time_key, hash_key);

    Ok(composite_key)
}

fn generate_message_with_text(
    content: String,
    my_encryption_secret_key: EncryptionStaticKey,
    my_signature_secret_key: SignatureStaticKey,
    receiver_public_key: EncryptionPublicKey,
    recipient_subidentity_name: String,
    origin_destination_identity_name: String,
    timestamp: String,
) -> ShinkaiMessage {
    let message = ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .body(content.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata(
            "".to_string(),
            recipient_subidentity_name.clone().to_string(),
            "".to_string(),
            EncryptionMethod::None,
        )
        .external_metadata_with_schedule(
            origin_destination_identity_name.clone().to_string(),
            origin_destination_identity_name.clone().to_string(),
            timestamp,
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
        "20230702T20533481345".to_string()
    );

    let mut shinkai_db = ShinkaiDB::new(&node1_db_path).unwrap();
    let _ = shinkai_db.insert_inbox_message(&message.clone());

    let last_messages_all = shinkai_db.get_last_messages_from_all(10).unwrap();
    assert_eq!(last_messages_all.len(), 1);
    assert_eq!(
        last_messages_all[0].clone().body.unwrap().content,
        "Hello World".to_string()
    );

    let inbox_name = InboxNameManager::get_inbox_name_from_message(&message).unwrap();
    println!("Inbox name: {}", inbox_name);
    assert_eq!(
        inbox_name,
        "inbox::@@node1.shinkai|::@@node1.shinkai|main_profile_node1::false".to_string()
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
        "20230702T20533481346".to_string()
    );
    let message3 = generate_message_with_text(
        "Hello World 3".to_string(),
        node1_encryption_sk,
        node1_identity_sk,
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "20230702T20533481347".to_string()
    );
    match shinkai_db.insert_inbox_message(&message2.clone()) {
        Ok(_) => println!("message2 inserted successfully"),
        Err(e) => println!("Failed to insert message2: {}", e),
    }

    match shinkai_db.insert_inbox_message(&message3.clone()) {
        Ok(_) => println!("message3 inserted successfully"),
        Err(e) => println!("Failed to insert message3: {}", e),
    }

    let last_messages_inbox = shinkai_db
        .get_last_messages_from_inbox(inbox_name.clone().to_string(), 2)
        .unwrap();
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

    let offset = get_message_offset_db_key(&last_unread_messages_inbox[1].clone()).unwrap();
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
    let subidentity_name = "device1";
    let full_subidentity_name =
        IdentityManager::merge_to_full_identity_name(node1_identity_name.to_string(), subidentity_name.to_string());
    let device1_subidentity = StandardIdentity::new(
        full_subidentity_name.clone().to_string(),
        None,
        node1_encryption_pk.clone(),
        node1_identity_pk.clone(),
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        IdentityType::Device,
    );

    let _ = shinkai_db.insert_sub_identity(device1_subidentity.clone());

    println!("before adding perms> Inbox name: {}", inbox_name);
    shinkai_db
        .add_permission(&inbox_name, &device1_subidentity, InboxPermission::Admin)
        .unwrap();
    assert!(shinkai_db
        .has_permission(&inbox_name, &device1_subidentity, InboxPermission::Admin)
        .unwrap());

    let resp = shinkai_db
        .print_all_from_cf(format!("{}_perms", inbox_name).as_str())
        .unwrap();

    shinkai_db.remove_permission(&inbox_name, &device1_subidentity).unwrap();
    assert!(!shinkai_db
        .has_permission(&inbox_name, &device1_subidentity, InboxPermission::Admin)
        .unwrap());

    let resp = shinkai_db
        .print_all_from_cf(format!("{}_perms", inbox_name).as_str())
        .unwrap();
}

#[test]
fn test_permission_errors() {
    setup();

    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";

    let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let (node1_subidentity_sk, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (node1_subencryption_sk, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

    let node1_db_path = format!("db_tests/{}", hash_string(node1_subidentity_name.clone()));

    // Assuming the shinkai_db is created and node1_subencryption_pk, node1_subidentity_pk are defined
    let mut shinkai_db = ShinkaiDB::new(&node1_db_path).unwrap();
    let subidentity_name = "device1";
    let full_subidentity_name =
        IdentityManager::merge_to_full_identity_name(node1_identity_name.to_string(), subidentity_name.to_string());

    let device1_subidentity = StandardIdentity::new(
        full_subidentity_name.clone().to_string(),
        None,
        node1_encryption_pk.clone(),
        node1_identity_pk.clone(),
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        IdentityType::Device,
    );
    let _ = shinkai_db.insert_sub_identity(device1_subidentity.clone());

    println!("full_subidentity_name: {}", full_subidentity_name);
    println!("subidentity: {}", device1_subidentity);

    // Create a fake identity for tests
    let nonexistent_identity = StandardIdentity::new(
        "nonexistent_identity".to_string(),
        None,
        node1_encryption_pk.clone(),
        node1_identity_pk.clone(),
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        IdentityType::Device,
    );

    // Test 1: Adding a permission to a nonexistent inbox should result in an error
    let result = shinkai_db.add_permission("nonexistent_inbox", &device1_subidentity, InboxPermission::Admin);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ShinkaiDBError::InboxNotFound);

    // Test 2: Adding a permission for a nonexistent identity should result in an error
    let result = shinkai_db.add_permission("existing_inbox", &nonexistent_identity, InboxPermission::Admin);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ShinkaiDBError::IdentityNotFound);

    // Test 3: Removing a permission from a nonexistent inbox should result in an error
    let result = shinkai_db.remove_permission("nonexistent_inbox", &device1_subidentity);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ShinkaiDBError::InboxNotFound);

    // Test 4: Removing a permission for a nonexistent identity should result in an error
    let result = shinkai_db.remove_permission("existing_inbox", &nonexistent_identity);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ShinkaiDBError::IdentityNotFound);

    // Test 5: Checking permission of a nonexistent inbox should result in an error
    let result = shinkai_db.has_permission("nonexistent_inbox", &device1_subidentity, InboxPermission::Admin);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ShinkaiDBError::InboxNotFound);

    // Test 6: Checking permission for a nonexistent identity should result in an error
    let result = shinkai_db.has_permission("existing_inbox", &nonexistent_identity, InboxPermission::Admin);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), ShinkaiDBError::IdentityNotFound);
}
