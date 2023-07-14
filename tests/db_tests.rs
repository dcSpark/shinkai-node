use async_channel::{bounded, Receiver, Sender};
use shinkai_node::db::db_errors::ShinkaiMessageDBError;
use shinkai_node::db::db_inbox::Permission;
use shinkai_node::db::ShinkaiMessageDB;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::{Node, IdentityManager, Identity};
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
    scheduled_time: String,
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
            scheduled_time,
        )
        .build()
        .unwrap();
    message
}

#[test]
fn test_insert_message_to_all() {
    setup();

    // Initialization same as in db_inbox test
    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let (node1_subidentity_sk, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (node1_subencryption_sk, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);
    let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));

    // Generate the message to be inserted
    let message = generate_message_with_text(
        "Hello All".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "20230702T20533481345".to_string()
    );

    // Create the DB and insert the message to all
    let mut shinkai_db = ShinkaiMessageDB::new(&node1_db_path).unwrap();
    assert!(shinkai_db.insert_message_to_all(&message).is_ok());

    // Fetch the message using `get_last_messages_from_all` method
    let last_messages_all = shinkai_db.get_last_messages_from_all(1).unwrap();
    assert_eq!(last_messages_all.len(), 1);
    assert_eq!(
        last_messages_all[0].clone().body.unwrap().content,
        "Hello All".to_string()
    );

    let message2 = generate_message_with_text(
        "Hello All 2".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "20230702T20533481345".to_string()
    );
    let message_before = generate_message_with_text(
        "Hello All before".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "20230702T20533481344".to_string()
    );
    let message_after = generate_message_with_text(
        "Hello All after".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "20230702T20533481346".to_string()
    );

    assert!(shinkai_db.insert_message_to_all(&message2).is_ok());
    assert!(shinkai_db.insert_message_to_all(&message_before).is_ok());
    assert!(shinkai_db.insert_message_to_all(&message_after).is_ok());

    // Fetch the message using `get_last_messages_from_all` method
    let last_messages_all = shinkai_db.get_last_messages_from_all(5).unwrap();
    assert_eq!(last_messages_all.len(), 4);
    assert_eq!(
        last_messages_all[0].clone().body.unwrap().content,
        "Hello All after".to_string()
    );
    // Note: Hello All and Hello All 2 have the same scheduled time, so the order is not guaranteed
    assert_eq!(
        last_messages_all[1].clone().body.unwrap().content,
        "Hello All".to_string()
    );
    assert_eq!(
        last_messages_all[2].clone().body.unwrap().content,
        "Hello All 2".to_string()
    );
    assert_eq!(
        last_messages_all[3].clone().body.unwrap().content,
        "Hello All before".to_string()
    );
}

#[test]
fn test_schedule_and_get_scheduled_due_messages() {
    setup();

    // Initialization same as in db_inbox test
    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let (node1_subidentity_sk, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (node1_subencryption_sk, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);
    let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));

    // Generate the message to be inserted
    let message = generate_message_with_text(
        "Hello Scheduled".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "20230702T20533481345".to_string()
    );
    let message2 = generate_message_with_text(
        "Hello Scheduled 2".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "20230702T20533481345".to_string()
    );
    let message_before = generate_message_with_text(
        "Hello Scheduled before".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "20230702T20533481344".to_string()
    );
    let message_after = generate_message_with_text(
        "Hello Scheduled after".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "20230702T20533481346".to_string()
    );

    // Create the DB and schedule the message
    let mut shinkai_db = ShinkaiMessageDB::new(&node1_db_path).unwrap();

    assert!(shinkai_db.schedule_message(&message).is_ok());
    assert!(shinkai_db.schedule_message(&message2).is_ok());
    assert!(shinkai_db.schedule_message(&message_before).is_ok());
    assert!(shinkai_db.schedule_message(&message_after).is_ok());

    // Fetch the due messages
    let due_messages = shinkai_db.get_scheduled_due_messages("20230702T20533481346".to_string()).unwrap();
    assert_eq!(due_messages.len(), 4);
    assert_eq!(
        due_messages[0].clone().body.unwrap().content,
        "Hello Scheduled before".to_string()
    );
    assert_eq!(
        due_messages[1].clone().body.unwrap().content,
        "Hello Scheduled 2".to_string()
    );
    assert_eq!(
        due_messages[2].clone().body.unwrap().content,
        "Hello Scheduled".to_string()
    );
    assert_eq!(
        due_messages[3].clone().body.unwrap().content,
        "Hello Scheduled after".to_string()
    );

    let due_messages = shinkai_db.get_scheduled_due_messages("20230702T20533481344".to_string()).unwrap();
    assert_eq!(due_messages.len(), 1);
    assert_eq!(
        due_messages[0].clone().body.unwrap().content,
        "Hello Scheduled before".to_string()
    );

    let due_messages = shinkai_db.get_scheduled_due_messages("20230703".to_string()).unwrap();
    assert_eq!(due_messages.len(), 4);

    let due_messages = shinkai_db.get_scheduled_due_messages("20230701".to_string()).unwrap();
    assert_eq!(due_messages.len(), 0);
}

