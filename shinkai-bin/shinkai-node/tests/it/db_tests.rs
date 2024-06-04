use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_utils::encryption::{
    unsafe_deterministic_encryption_keypair, EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::db::ShinkaiDB;
use std::fs;
use std::path::Path;

use ed25519_dalek::SigningKey;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

fn generate_message_with_text(
    content: String,
    my_encryption_secret_key: EncryptionStaticKey,
    my_signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey,
    recipient_subidentity_name: String,
    origin_destination_identity_name: String,
    scheduled_time: String,
) -> ShinkaiMessage {
    ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .message_raw_content(content.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata(
            "".to_string(),
            recipient_subidentity_name.clone().to_string(),
            EncryptionMethod::None,
            None,
        )
        .external_metadata_with_schedule(
            origin_destination_identity_name.clone().to_string(),
            origin_destination_identity_name.clone().to_string(),
            scheduled_time,
        )
        .build()
        .unwrap()
}

#[test]
fn test_insert_message_to_all() {
    init_default_tracing();
    setup();

    // Initialization same as in db_inbox test
    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, _node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, _node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let (_node1_subidentity_sk, _node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (_node1_subencryption_sk, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);
    let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name));

    // Generate the message to be inserted
    let message = generate_message_with_text(
        "Hello All".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.450Z".to_string(),
    );

    // Create the DB and insert the message to all
    let shinkai_db = ShinkaiDB::new(&node1_db_path).unwrap();
    assert!(shinkai_db.insert_message_to_all(&message).is_ok());

    // Fetch the message using `get_last_messages_from_all` method
    let last_messages_all = shinkai_db.get_last_messages_from_all(1).unwrap();
    assert_eq!(last_messages_all.len(), 1);
    assert_eq!(
        last_messages_all[0].clone().get_message_content().unwrap(),
        "Hello All".to_string()
    );

    let message2 = generate_message_with_text(
        "Hello All 2".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.450Z".to_string(),
    );
    let message_before = generate_message_with_text(
        "Hello All before".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.440Z".to_string(),
    );
    let message_after = generate_message_with_text(
        "Hello All after".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.460Z".to_string(),
    );

    assert!(shinkai_db.insert_message_to_all(&message2).is_ok());
    assert!(shinkai_db.insert_message_to_all(&message_before).is_ok());
    assert!(shinkai_db.insert_message_to_all(&message_after).is_ok());

    // Fetch the message using `get_last_messages_from_all` method
    let last_messages_all = shinkai_db.get_last_messages_from_all(5).unwrap();
    assert_eq!(last_messages_all.len(), 4);
    assert_eq!(
        last_messages_all[0].clone().get_message_content().unwrap(),
        "Hello All after".to_string()
    );
    // Note: Hello All and Hello All 2 have the same scheduled time, so the order is
    // not guaranteed
    let expected_contents = ["Hello All", "Hello All 2"];
    assert!(expected_contents.contains(&last_messages_all[1].clone().get_message_content().unwrap().as_str()));
    assert!(expected_contents.contains(&last_messages_all[2].clone().get_message_content().unwrap().as_str()));
    assert_eq!(
        last_messages_all[3].clone().get_message_content().unwrap(),
        "Hello All before".to_string()
    );
}

#[test]
fn test_schedule_and_get_due_scheduled_messages() {
    init_default_tracing();
    setup();

    // Initialization same as in db_inbox test
    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, _node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, _node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let (_node1_subidentity_sk, _node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (_node1_subencryption_sk, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);
    let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name));

    // Generate the message to be inserted
    let message = generate_message_with_text(
        "Hello Scheduled".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34Z".to_string(),
    );
    let message2 = generate_message_with_text(
        "Hello Scheduled 2".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34Z".to_string(),
    );
    let message_before = generate_message_with_text(
        "Hello Scheduled before".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:33Z".to_string(),
    );
    let message_after = generate_message_with_text(
        "Hello Scheduled after".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:35Z".to_string(),
    );

    // Create the DB and schedule the message
    let shinkai_db = ShinkaiDB::new(&node1_db_path).unwrap();

    assert!(shinkai_db.schedule_message(&message).is_ok());
    assert!(shinkai_db.schedule_message(&message2).is_ok());
    assert!(shinkai_db.schedule_message(&message_before).is_ok());
    assert!(shinkai_db.schedule_message(&message_after).is_ok());

    // Fetch the due messages
    let due_messages = shinkai_db
        .get_due_scheduled_messages("2023-07-02T20:53:35Z".to_string())
        .unwrap();
    assert_eq!(due_messages.len(), 4);
    assert_eq!(
        due_messages[0].clone().get_message_content().unwrap(),
        "Hello Scheduled before".to_string()
    );

    let expected_contents = ["Hello Scheduled", "Hello Scheduled 2"];
    assert!(expected_contents.contains(&due_messages[1].clone().get_message_content().unwrap().as_str()));
    assert!(expected_contents.contains(&due_messages[2].clone().get_message_content().unwrap().as_str()));

    assert_eq!(
        due_messages[3].clone().get_message_content().unwrap(),
        "Hello Scheduled after".to_string()
    );

    let due_messages = shinkai_db
        .get_due_scheduled_messages("2023-07-02T20:53:33Z".to_string())
        .unwrap();
    assert_eq!(due_messages.len(), 1);
    assert_eq!(
        due_messages[0].clone().get_message_content().unwrap(),
        "Hello Scheduled before".to_string()
    );

    let due_messages = shinkai_db
        .get_due_scheduled_messages("2023-07-03T00:00:00Z".to_string())
        .unwrap();
    assert_eq!(due_messages.len(), 4);

    let due_messages = shinkai_db
        .get_due_scheduled_messages("2023-07-01T00:00:00Z".to_string())
        .unwrap();
    assert_eq!(due_messages.len(), 0);
}
