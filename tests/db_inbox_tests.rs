use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_time::ShinkaiTime;
use shinkai_message_primitives::shinkai_message::shinkai_message::{
    MessageBody, MessageData, ShinkaiMessage, ShinkaiVersion,
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{IdentityPermissions, MessageSchemaType};
use shinkai_message_primitives::shinkai_utils::encryption::{
    unsafe_deterministic_encryption_keypair, EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::db::db_errors::ShinkaiDBError;
use shinkai_node::db::ShinkaiDB;
use shinkai_node::schemas::identity::{StandardIdentity, StandardIdentityType};
use shinkai_node::schemas::inbox_permission::InboxPermission;
use std::fs;
use std::path::Path;

use ed25519_dalek::{SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

fn get_message_offset_db_key(message: &ShinkaiMessage) -> Result<String, ShinkaiDBError> {
    // Calculate the hash of the message for the key
    let hash_key = message.calculate_message_hash();

    // Clone the external_metadata first, then unwrap
    let ext_metadata = message.external_metadata.clone();

    // Get the scheduled time or calculate current time
    let time_key = match ext_metadata.scheduled_time.is_empty() {
        true => ShinkaiTime::generate_time_now(),
        false => ext_metadata.scheduled_time.clone(),
    };

    // Create the composite key by concatenating the time_key and the hash_key, with a separator
    let composite_key = format!("{}:::{}", time_key, hash_key);

    Ok(composite_key)
}

fn generate_message_with_text(
    content: String,
    my_encryption_secret_key: EncryptionStaticKey,
    my_signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey,
    recipient_subidentity_name: String,
    origin_destination_identity_name: String,
    timestamp: String,
) -> ShinkaiMessage {
    let inbox_name = InboxName::get_regular_inbox_name_from_params(
        origin_destination_identity_name.clone().to_string(),
        "".to_string(),
        origin_destination_identity_name.clone().to_string(),
        recipient_subidentity_name.clone().to_string(),
        false,
    )
    .unwrap();

    let inbox_name_value = match inbox_name {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
    };

    let message = ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .message_raw_content(content.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata_with_inbox(
            "".to_string(),
            recipient_subidentity_name.clone().to_string(),
            inbox_name_value,
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
fn test_insert_messages_with_tree_structure() {
    setup();

    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let (node1_subidentity_sk, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (node1_subencryption_sk, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

    let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));

    let mut shinkai_db = ShinkaiDB::new(&node1_db_path).unwrap();

    let mut parent_message = None;

    eprintln!("Inserting messages...\n\n");
    let mut parent_message_hash: Option<String> = None;
    let mut parent_message_hash_2: Option<String> = None;
    let mut parent_message_hash_4: Option<String> = None;
    /*
    The tree that we are creating looks like:
        1
        ├── 2
        │   ├── 4
        │   │   ├── 6
        │   │   └── 7
        │   │       └── 8
        │   └── 5
        └── 3
     */
    for i in 1..=8 {
        let message = generate_message_with_text(
            format!("Hello World {}", i),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_subencryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            format!("2023-07-02T20:53:34.81{}Z", i),
        );

        // Necessary to extract the inbox
        parent_message = Some(message.clone());

        let parent_hash: Option<String> = match i {
            2 | 3 => parent_message_hash.clone(),
            4 | 5 => parent_message_hash_2.clone(),
            6 | 7 => parent_message_hash.clone(),
            8 => parent_message_hash_4.clone(),
            _ => None,
        };

        shinkai_db
            .unsafe_insert_inbox_message(&message, parent_hash.clone())
            .unwrap();

        // Update the parent message according to the tree structure
        if i == 1 {
            parent_message_hash = Some(message.calculate_message_hash());
        } else if i == 2 {
            parent_message_hash_2 = Some(message.calculate_message_hash());
        } else if i == 4 {
            parent_message_hash = Some(message.calculate_message_hash());
        } else if i == 7 {
            parent_message_hash_4 = Some(message.calculate_message_hash());
        }

        // Print the message hash, content, and parent hash
        println!(
            "message hash: {} message content: {} message parent hash: {}",
            message.calculate_message_hash(),
            message.get_message_content().unwrap(),
            parent_hash.as_ref().map(|hash| hash.as_str()).unwrap_or("None")
        );
    }

    let inbox_name = InboxName::from_message(&parent_message.unwrap()).unwrap();

    let inbox_name_value = match inbox_name {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
    };

    eprintln!("\n\n\n Getting messages...");

    let last_messages_inbox = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 3, None)
        .unwrap();

    let last_messages_content: Vec<Vec<String>> = last_messages_inbox
        .iter()
        .map(|message_array| {
            message_array
                .iter()
                .map(|message| message.clone().get_message_content().unwrap())
                .collect()
        })
        .collect();

    eprintln!("Last messages: {:?}", last_messages_content);

    assert_eq!(last_messages_inbox.len(), 3);

    // Check the content of the first message array
    assert_eq!(last_messages_inbox[0].len(), 2);
    assert_eq!(
        last_messages_inbox[0][0].clone().get_message_content().unwrap(),
        "Hello World 4".to_string()
    );
    assert_eq!(
        last_messages_inbox[0][1].clone().get_message_content().unwrap(),
        "Hello World 5".to_string()
    );

    // Check the content of the second message array
    assert_eq!(last_messages_inbox[1].len(), 2);
    assert_eq!(
        last_messages_inbox[1][0].clone().get_message_content().unwrap(),
        "Hello World 7".to_string()
    );
    assert_eq!(
        last_messages_inbox[1][1].clone().get_message_content().unwrap(),
        "Hello World 6".to_string()
    );

    // Check the content of the third message array
    assert_eq!(last_messages_inbox[2].len(), 1);
    assert_eq!(
        last_messages_inbox[2][0].clone().get_message_content().unwrap(),
        "Hello World 8".to_string()
    );
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
        "2023-07-02T20:53:34.812Z".to_string(),
    );

    let mut shinkai_db = ShinkaiDB::new(&node1_db_path).unwrap();
    let _ = shinkai_db.unsafe_insert_inbox_message(&message.clone(), None);
    println!("Inserted message {:?}", message.encode_message());
    let result = ShinkaiMessage::decode_message_result(message.encode_message().unwrap());
    println!("Decoded message {:?}", result);

    let last_messages_all = shinkai_db.get_last_messages_from_all(10).unwrap();
    assert_eq!(last_messages_all.len(), 1);
    assert_eq!(
        last_messages_all[0].clone().get_message_content().unwrap(),
        "Hello World".to_string()
    );

    let inbox_name = InboxName::from_message(&message).unwrap();

    let inbox_name_value = match inbox_name {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
    };

    println!("Inbox name: {}", inbox_name_value);
    assert_eq!(
        inbox_name_value,
        "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false".to_string()
    );

    println!("Inbox name: {}", inbox_name_value.to_string());
    let last_messages_inbox = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.to_string(), 10, None)
        .unwrap();
    assert_eq!(last_messages_inbox.len(), 1);
    assert_eq!(
        last_messages_inbox[0][0].clone().get_message_content().unwrap(),
        "Hello World".to_string()
    );

    // Get last unread messages
    let last_unread = shinkai_db
        .get_last_unread_messages_from_inbox(inbox_name_value.clone().to_string(), 10, None)
        .unwrap();
    println!("Last unread messages: {:?}", last_unread);
    assert_eq!(last_unread.len(), 1);
    assert_eq!(
        last_unread[0].clone().get_message_content().unwrap(),
        "Hello World".to_string()
    );

    let message2 = generate_message_with_text(
        "Hello World 2".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.813Z".to_string(),
    );
    let message3 = generate_message_with_text(
        "Hello World 3".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.814Z".to_string(),
    );
    let message4 = generate_message_with_text(
        "Hello World 4".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:54:34.814Z".to_string(),
    );
    let message5 = generate_message_with_text(
        "Hello World 5".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:55:34.814Z".to_string(),
    );
    match shinkai_db.unsafe_insert_inbox_message(&message2.clone(), None) {
        Ok(_) => println!("message2 inserted successfully"),
        Err(e) => println!("Failed to insert message2: {}", e),
    }

    match shinkai_db.unsafe_insert_inbox_message(&message3.clone(), None) {
        Ok(_) => println!("message3 inserted successfully"),
        Err(e) => println!("Failed to insert message3: {}", e),
    }

    match shinkai_db.unsafe_insert_inbox_message(&message4.clone(), None) {
        Ok(_) => println!("message4 inserted successfully"),
        Err(e) => println!("Failed to insert message4: {}", e),
    }

    match shinkai_db.unsafe_insert_inbox_message(&message5.clone(), None) {
        Ok(_) => println!("message5 inserted successfully"),
        Err(e) => println!("Failed to insert message5: {}", e),
    }

    let all_messages_inbox = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 6, None)
        .unwrap();
    assert_eq!(all_messages_inbox.len(), 5);

    let last_messages_inbox = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 2, None)
        .unwrap();
    assert_eq!(last_messages_inbox.len(), 2);

    let last_unread_messages_inbox = shinkai_db
        .get_last_unread_messages_from_inbox(inbox_name_value.clone().to_string(), 2, None)
        .unwrap();
    assert_eq!(last_unread_messages_inbox.len(), 2);
    assert_eq!(
        last_unread_messages_inbox[0].clone().get_message_content().unwrap(),
        "Hello World".to_string()
    );
    assert_eq!(
        last_unread_messages_inbox[1].clone().get_message_content().unwrap(),
        "Hello World 2".to_string()
    );

    let offset = get_message_offset_db_key(&last_unread_messages_inbox[1].clone()).unwrap();
    println!("\n\n ### Offset: {}", offset);
    println!("Last unread messages: {:?}", last_unread_messages_inbox[1]);
    // check pagination for last unread
    let last_unread_messages_inbox_page2 = shinkai_db
        .get_last_unread_messages_from_inbox(inbox_name_value.clone().to_string(), 3, Some(offset.clone()))
        .unwrap();
    assert_eq!(last_unread_messages_inbox_page2.len(), 3);
    assert_eq!(
        last_unread_messages_inbox_page2[0]
            .clone()
            .get_message_content()
            .unwrap(),
        "Hello World 3".to_string()
    );

    // check pagination for inbox messages
    let last_unread_messages_inbox_page2 = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 3, Some(offset))
        .unwrap();
    assert_eq!(last_unread_messages_inbox_page2.len(), 1);
    assert_eq!(
        last_unread_messages_inbox_page2[0][0]
            .clone()
            .get_message_content()
            .unwrap(),
        "Hello World".to_string()
    );

    // Mark as read up to a certain time
    shinkai_db
        .mark_as_read_up_to(
            inbox_name_value.clone().to_string(),
            "2023-07-03T00:00:00.000Z".to_string(),
        )
        .unwrap();

    let last_messages_inbox = shinkai_db
        .get_last_unread_messages_from_inbox(inbox_name_value.clone().to_string(), 2, None)
        .unwrap();
    assert_eq!(last_messages_inbox.len(), 0);

    // Test permissions
    let subidentity_name = "device1";
    let full_subidentity_name =
        ShinkaiName::from_node_and_profile(node1_identity_name.to_string(), subidentity_name.to_string()).unwrap();

    let device1_subidentity = StandardIdentity::new(
        full_subidentity_name.clone(),
        None,
        node1_encryption_pk.clone(),
        node1_identity_pk.clone(),
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );

    let _ = shinkai_db.insert_profile(device1_subidentity.clone());
    println!("Inserted profile");

    shinkai_db
        .add_permission(&inbox_name_value, &device1_subidentity, InboxPermission::Admin)
        .unwrap();
    assert!(shinkai_db
        .has_permission(&inbox_name_value, &device1_subidentity, InboxPermission::Admin)
        .unwrap());

    let _ = shinkai_db
        .print_all_from_cf(format!("{}_perms", inbox_name_value).as_str())
        .unwrap();

    shinkai_db
        .remove_permission(&inbox_name_value, &device1_subidentity)
        .unwrap();
    assert!(!shinkai_db
        .has_permission(&inbox_name_value, &device1_subidentity, InboxPermission::Admin)
        .unwrap());

    let _ = shinkai_db
        .print_all_from_cf(format!("{}_perms", inbox_name_value).as_str())
        .unwrap();

    let message4 = generate_message_with_text(
        "Hello World 6".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        "other_inbox".to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.815Z".to_string(),
    );
    let message5 = generate_message_with_text(
        "Hello World 7".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        "yet_another_inbox".to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.816Z".to_string(),
    );
    shinkai_db.unsafe_insert_inbox_message(&message4, None).unwrap();
    shinkai_db.unsafe_insert_inbox_message(&message5, None).unwrap();

    // Test get_inboxes_for_profile
    let node1_profile_identity = StandardIdentity::new(
        ShinkaiName::from_node_and_profile(node1_identity_name.to_string(), node1_subidentity_name.to_string())
            .unwrap(),
        None,
        node1_encryption_pk.clone(),
        node1_identity_pk.clone(),
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );
    let _ = shinkai_db.insert_profile(node1_profile_identity.clone());
    let inboxes = shinkai_db
        .get_inboxes_for_profile(node1_profile_identity.clone())
        .unwrap();
    assert_eq!(inboxes.len(), 1);

    let node1_identity = StandardIdentity::new(
        ShinkaiName::new(node1_identity_name.to_string()).unwrap(),
        None,
        node1_encryption_pk.clone(),
        node1_identity_pk.clone(),
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );
    let inboxes = shinkai_db.get_inboxes_for_profile(node1_identity.clone()).unwrap();
    assert_eq!(inboxes.len(), 3);
    assert!(inboxes.contains(&inbox_name_value));
    assert!(inboxes.contains(&"inbox::@@node1.shinkai::@@node1.shinkai/other_inbox::false".to_string()));
    assert!(inboxes.contains(&"inbox::@@node1.shinkai::@@node1.shinkai/yet_another_inbox::false".to_string()));

    // Test get_smart_inboxes_for_profile
    let smart_inboxes = shinkai_db
        .get_all_smart_inboxes_for_profile(node1_identity.clone())
        .unwrap();
    assert_eq!(smart_inboxes.len(), 3);

    // Check if smart_inboxes contain the expected results
    let expected_inbox_ids = vec![
        "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false",
        "inbox::@@node1.shinkai::@@node1.shinkai/other_inbox::false",
        "inbox::@@node1.shinkai::@@node1.shinkai/yet_another_inbox::false",
    ];

    for smart_inbox in smart_inboxes {
        assert!(expected_inbox_ids.contains(&smart_inbox.inbox_id.as_str()));
        assert_eq!(smart_inbox.inbox_id, smart_inbox.custom_name);

        // Check the last_message of each smart_inbox
        if let Some(last_message) = smart_inbox.last_message {
            match last_message.body {
                MessageBody::Unencrypted(ref body) => match body.message_data {
                    MessageData::Unencrypted(ref data) => match smart_inbox.inbox_id.as_str() {
                        "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false" => {
                            assert_eq!(data.message_raw_content, "Hello World 5");
                        }
                        "inbox::@@node1.shinkai::@@node1.shinkai/other_inbox::false" => {
                            assert_eq!(data.message_raw_content, "Hello World 6");
                        }
                        "inbox::@@node1.shinkai::@@node1.shinkai/yet_another_inbox::false" => {
                            assert_eq!(data.message_raw_content, "Hello World 7");
                        }
                        _ => panic!("Unexpected inbox_id"),
                    },
                    _ => panic!("Expected unencrypted message data"),
                },
                _ => panic!("Expected unencrypted message body"),
            }
            assert_eq!(last_message.external_metadata.sender, "@@node1.shinkai");
            assert_eq!(last_message.external_metadata.recipient, "@@node1.shinkai");
            assert_eq!(last_message.encryption, EncryptionMethod::None);
            assert_eq!(last_message.version, ShinkaiVersion::V1_0);
        }
    }

    // Update the name of one of the inboxes
    let inbox_to_update = "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false";
    let new_name = "New Inbox Name";
    shinkai_db.update_smart_inbox_name(inbox_to_update, new_name).unwrap();

    // Get smart_inboxes again
    let updated_smart_inboxes = shinkai_db.get_all_smart_inboxes_for_profile(node1_identity).unwrap();

    // Check if the name of the updated inbox has been changed
    for smart_inbox in updated_smart_inboxes {
        if smart_inbox.inbox_id == inbox_to_update {
            eprintln!("Smart inbox: {:?}", smart_inbox);
            assert_eq!(smart_inbox.custom_name, new_name);
        }
    }
}

#[test]
fn test_permission_errors() {
    setup();

    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";

    let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let (_, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (_, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

    let node1_db_path = format!("db_tests/{}", hash_string(node1_subidentity_name.clone()));

    let mut shinkai_db = ShinkaiDB::new(&node1_db_path).unwrap();
    let subidentity_name = "device1";
    let full_subidentity_name =
        ShinkaiName::from_node_and_profile(node1_identity_name.to_string(), subidentity_name.to_string()).unwrap();

    let device1_subidentity = StandardIdentity::new(
        full_subidentity_name.clone(),
        None,
        node1_encryption_pk.clone(),
        node1_identity_pk.clone(),
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );
    let _ = shinkai_db.insert_profile(device1_subidentity.clone());

    // Create a fake identity for tests
    let nonexistent_identity = StandardIdentity::new(
        ShinkaiName::from_node_and_profile(node1_identity_name.to_string(), "nonexistent_identity".to_string())
            .unwrap(),
        None,
        node1_encryption_pk.clone(),
        node1_identity_pk.clone(),
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );

    // Test 1: Adding a permission to a nonexistent inbox should result in an error
    let result = shinkai_db.add_permission("nonexistent_inbox", &device1_subidentity, InboxPermission::Admin);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ShinkaiDBError::InboxNotFound("Inbox not found: nonexistent_inbox".to_string())
    );

    // Test 2: Adding a permission for a nonexistent identity should result in an error
    let result = shinkai_db.add_permission("existing_inbox", &nonexistent_identity, InboxPermission::Admin);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ShinkaiDBError::IdentityNotFound(format!(
            "Identity not found for: {}",
            nonexistent_identity
                .clone()
                .full_identity_name
                .get_profile_name()
                .unwrap()
                .to_string()
        ))
    );

    // Test 3: Removing a permission from a nonexistent inbox should result in an error
    let result = shinkai_db.remove_permission("nonexistent_inbox", &device1_subidentity);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ShinkaiDBError::InboxNotFound("Inbox not found: nonexistent_inbox".to_string())
    );

    // Test 4: Removing a permission for a nonexistent identity should result in an error
    let result = shinkai_db.remove_permission("existing_inbox", &nonexistent_identity);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ShinkaiDBError::IdentityNotFound(format!(
            "Identity not found for: {}",
            nonexistent_identity.full_identity_name.clone().to_string()
        ))
    );

    // Test 5: Checking permission of a nonexistent inbox should result in an error
    let result = shinkai_db.has_permission("nonexistent_inbox", &device1_subidentity, InboxPermission::Admin);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ShinkaiDBError::InboxNotFound("Inbox not found: nonexistent_inbox".to_string())
    );

    // Test 6: Checking permission for a nonexistent identity should result in an error
    let result = shinkai_db.has_permission("existing_inbox", &nonexistent_identity, InboxPermission::Admin);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ShinkaiDBError::IdentityNotFound(format!(
            "Identity not found for: {}",
            nonexistent_identity.full_identity_name.clone().to_string()
        ))
    );
}
