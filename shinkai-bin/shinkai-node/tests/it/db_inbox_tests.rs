use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use shinkai_message_primitives::schemas::identity::{StandardIdentity, StandardIdentityType};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::inbox_permission::InboxPermission;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
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
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;

use std::path::PathBuf;
use std::sync::Arc;
use tempfile::NamedTempFile;

use ed25519_dalek::SigningKey;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

fn setup_test_db() -> SqliteManager {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = PathBuf::from(temp_file.path());
    let api_url = String::new();
    let model_type =
        EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

    SqliteManager::new(db_path, api_url, model_type).unwrap()
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

    ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .message_raw_content(content.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata_with_inbox(
            "".to_string(),
            recipient_subidentity_name.clone().to_string(),
            inbox_name_value,
            EncryptionMethod::None,
            None,
        )
        .external_metadata_with_schedule(
            origin_destination_identity_name.clone().to_string(),
            origin_destination_identity_name.clone().to_string(),
            timestamp,
        )
        .build()
        .unwrap()
}

#[tokio::test]
async fn test_insert_single_message_and_retrieve() {
    let node_identity_name = "@@node.shinkai";
    let subidentity_name = "main";
    let (node_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let (node_encryption_sk, node_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let db = setup_test_db();
    let shinkai_db = Arc::new(db);

    // Insert a single message
    let message = generate_message_with_text(
        "Only Message".to_string(),
        node_encryption_sk,
        clone_signature_secret_key(&node_identity_sk),
        node_encryption_pk,
        subidentity_name.to_string(),
        node_identity_name.to_string(),
        "2023-07-03T10:00:00.000Z".to_string(),
    );

    shinkai_db
        .unsafe_insert_inbox_message(&message, None, None)
        .await
        .unwrap();

    // Retrieve the message and check
    let inbox_name = InboxName::from_message(&message).unwrap();
    let inbox_name_value = match inbox_name {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
    };

    let messages = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 1, None)
        .unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0][0].clone().get_message_content().unwrap(),
        "Only Message".to_string()
    );
}

#[tokio::test]
async fn test_insert_two_messages_and_check_order_and_parent() {
    let node_identity_name = "@@node.shinkai";
    let subidentity_name = "main_profile_node";
    let (node_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let (node_encryption_sk, node_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let db = setup_test_db();
    let shinkai_db = Arc::new(db);

    // Insert first message
    let message1 = generate_message_with_text(
        "First Message".to_string(),
        node_encryption_sk.clone(),
        clone_signature_secret_key(&node_identity_sk),
        node_encryption_pk,
        subidentity_name.to_string(),
        node_identity_name.to_string(),
        "2023-07-02T20:53:34.812Z".to_string(),
    );

    shinkai_db
        .unsafe_insert_inbox_message(&message1, None, None)
        .await
        .unwrap();

    // Insert second message with first message as parent
    let message2 = generate_message_with_text(
        "Second Message".to_string(),
        node_encryption_sk.clone(),
        clone_signature_secret_key(&node_identity_sk),
        node_encryption_pk,
        subidentity_name.to_string(),
        node_identity_name.to_string(),
        "2023-07-02T20:54:34.923Z".to_string(),
    );

    let parent_message_hash = Some(message1.calculate_message_hash_for_pagination());

    shinkai_db
        .unsafe_insert_inbox_message(&message2, parent_message_hash.clone(), None)
        .await
        .unwrap();

    // Retrieve messages and check order
    let inbox_name = InboxName::from_message(&message1).unwrap();
    let inbox_name_value = match inbox_name {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
    };

    let messages = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 2, None)
        .unwrap();
    eprintln!("\n\n\n Messages: {:?}", messages);

    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[0][0].clone().get_message_content().unwrap(),
        "First Message".to_string()
    );
    assert_eq!(
        messages[1][0].clone().get_message_content().unwrap(),
        "Second Message".to_string()
    );

    // Check parent of the second message
    let expected_parent_hash = if let MessageBody::Unencrypted(shinkai_body) = &messages[0][0].body {
        shinkai_body
            .internal_metadata
            .node_api_data
            .as_ref()
            .map(|data| data.node_message_hash.clone())
    } else {
        None
    };

    let actual_parent_hash = if let MessageBody::Unencrypted(shinkai_body) = &messages[1][0].body {
        shinkai_body
            .internal_metadata
            .node_api_data
            .as_ref()
            .map(|data| data.parent_hash.clone())
    } else {
        None
    };

    // eprintln!("Expected parent hash: {:?}", expected_parent_hash);
    // eprintln!("Actual parent hash: {:?}", actual_parent_hash);
    assert_eq!(actual_parent_hash, expected_parent_hash);

    // Retrieve messages with pagination using the last message's hash
    let pagination_hash = messages[1][0].calculate_message_hash_for_pagination();
    eprintln!("Pagination hash: {}", pagination_hash);
    let paginated_messages = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 2, Some(pagination_hash))
        .unwrap();

    eprintln!("Paginated messages: {:?}", paginated_messages);

    // Expecting to get only 1 message back due to pagination
    assert_eq!(paginated_messages.len(), 1);
    assert_eq!(
        paginated_messages[0][0].clone().get_message_content().unwrap(),
        "First Message".to_string()
    );
}

#[tokio::test]
async fn test_insert_messages_with_simple_tree_structure() {
    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let db = setup_test_db();
    let shinkai_db = Arc::new(db);

    let mut parent_message = None;

    eprintln!("Inserting messages...\n\n");
    let mut parent_message_hash: Option<String> = None;
    let mut parent_message_hash_2: Option<String> = None;

    /*
    The tree that we are creating looks like:
        1
        ├── 2
        │   ├── 4
        └── 3
     */
    for i in 1..=4 {
        let message = generate_message_with_text(
            format!("Hello World {}", i),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            format!("2023-07-02T20:53:34.81{}Z", i),
        );

        // Necessary to extract the inbox
        parent_message = Some(message.clone());

        let parent_hash: Option<String> = match i {
            2 | 3 => parent_message_hash.clone(),
            4 => parent_message_hash_2.clone(),
            _ => None,
        };

        shinkai_db
            .unsafe_insert_inbox_message(&message, parent_hash.clone(), None)
            .await
            .unwrap();

        // Update the parent message according to the tree structure
        if i == 1 {
            parent_message_hash = Some(message.calculate_message_hash_for_pagination());
        } else if i == 2 {
            parent_message_hash_2 = Some(message.calculate_message_hash_for_pagination());
        }

        // Print the message hash, content, and parent hash
        println!(
            "message hash: {} message content: {} message parent hash: {}",
            message.calculate_message_hash_for_pagination(),
            message.get_message_content().unwrap(),
            parent_hash.as_deref().unwrap_or("None")
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
    assert_eq!(last_messages_inbox[0].len(), 1);
    assert_eq!(
        last_messages_inbox[0][0].clone().get_message_content().unwrap(),
        "Hello World 1".to_string()
    );

    // Check the content of the second message array
    assert_eq!(last_messages_inbox[1].len(), 2);
    assert_eq!(
        last_messages_inbox[1][0].clone().get_message_content().unwrap(),
        "Hello World 2".to_string()
    );
    assert_eq!(
        last_messages_inbox[1][1].clone().get_message_content().unwrap(),
        "Hello World 3".to_string()
    );

    // Check the content of the third message array
    assert_eq!(last_messages_inbox[2].len(), 1);
    assert_eq!(
        last_messages_inbox[2][0].clone().get_message_content().unwrap(),
        "Hello World 4".to_string()
    );
}

#[tokio::test]
async fn test_insert_messages_with_simple_tree_structure_and_root() {
    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let db = setup_test_db();
    let shinkai_db = Arc::new(db);

    let mut parent_message = None;

    eprintln!("Inserting messages...\n\n");
    let mut parent_message_hash: Option<String> = None;
    let mut parent_message_hash_2: Option<String> = None;

    /*
    The tree that we are creating looks like:
        0
        1
        ├── 2
        │   ├── 4
        └── 3
     */
    for i in 0..=4 {
        let message = generate_message_with_text(
            format!("Hello World {}", i),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            format!("2023-07-02T20:53:34.81{}Z", i),
        );

        // Necessary to extract the inbox
        parent_message = Some(message.clone());

        let parent_hash: Option<String> = match i {
            2 | 3 => parent_message_hash.clone(),
            4 => parent_message_hash_2.clone(),
            _ => None,
        };

        shinkai_db
            .unsafe_insert_inbox_message(&message, parent_hash.clone(), None)
            .await
            .unwrap();

        // Update the parent message according to the tree structure
        if i == 1 {
            parent_message_hash = Some(message.calculate_message_hash_for_pagination());
        } else if i == 2 {
            parent_message_hash_2 = Some(message.calculate_message_hash_for_pagination());
        }

        // Print the message hash, content, and parent hash
        println!(
            "message hash: {} message content: {} message parent hash: {}",
            message.calculate_message_hash_for_pagination(),
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
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 4, None)
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

    assert_eq!(last_messages_inbox.len(), 4);

    // Check the content of the first message array
    assert_eq!(last_messages_inbox[0].len(), 1);
    assert_eq!(
        last_messages_inbox[0][0].clone().get_message_content().unwrap(),
        "Hello World 0".to_string()
    );

    // Check the content of the second message array
    assert_eq!(last_messages_inbox[1].len(), 1);
    assert_eq!(
        last_messages_inbox[1][0].clone().get_message_content().unwrap(),
        "Hello World 1".to_string()
    );

    // Check the content of the third message array
    assert_eq!(last_messages_inbox[2].len(), 2);
    assert_eq!(
        last_messages_inbox[2][0].clone().get_message_content().unwrap(),
        "Hello World 2".to_string()
    );
    assert_eq!(
        last_messages_inbox[2][1].clone().get_message_content().unwrap(),
        "Hello World 3".to_string()
    );

    // Check the content of the fourth message array
    assert_eq!(last_messages_inbox[3].len(), 1);
    assert_eq!(
        last_messages_inbox[3][0].clone().get_message_content().unwrap(),
        "Hello World 4".to_string()
    );

    // Testing Pagination
    // Get the hash of the first message of the first element of the first array returned
    let until_offset_key = last_messages_inbox[3][0].calculate_message_hash_for_pagination();

    // Get the last 2 messages with pagination
    let paginated_messages_inbox = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 2, Some(until_offset_key))
        .unwrap();

    let paginated_messages_content: Vec<Vec<String>> = paginated_messages_inbox
        .iter()
        .map(|message_array| {
            message_array
                .iter()
                .map(|message| message.clone().get_message_content().unwrap())
                .collect()
        })
        .collect();

    eprintln!("Paginated messages: {:?}", paginated_messages_content);

    // Check that the results obtained by using pagination match the ones that don't require pagination
    assert_eq!(paginated_messages_inbox.len(), 2);
    assert_eq!(paginated_messages_inbox[0].len(), 1);
    assert_eq!(
        paginated_messages_inbox[0][0].clone().get_message_content().unwrap(),
        "Hello World 1".to_string()
    );
    assert_eq!(paginated_messages_inbox[1].len(), 2);
    assert_eq!(
        paginated_messages_inbox[1][0].clone().get_message_content().unwrap(),
        "Hello World 2".to_string()
    );
    assert_eq!(
        paginated_messages_inbox[1][1].clone().get_message_content().unwrap(),
        "Hello World 3".to_string()
    );

    // New test for get_parent_message_hash
    let parent_hash_test = shinkai_db
        .get_parent_message_hash(
            &inbox_name_value,
            &last_messages_inbox[2][0].calculate_message_hash_for_pagination(),
        )
        .unwrap();

    assert_eq!(
        parent_hash_test,
        Some(last_messages_inbox[1][0].calculate_message_hash_for_pagination())
    );

    let parent_hash_test_2 = shinkai_db
        .get_parent_message_hash(
            &inbox_name_value,
            &last_messages_inbox[2][1].calculate_message_hash_for_pagination(),
        )
        .unwrap();

    assert_eq!(
        parent_hash_test_2,
        Some(last_messages_inbox[1][0].calculate_message_hash_for_pagination())
    );

    // Check for the root message, which should return None as it has no parent
    let root_message_parent_hash = shinkai_db
        .get_parent_message_hash(
            &inbox_name_value,
            &last_messages_inbox[0][0].calculate_message_hash_for_pagination(),
        )
        .unwrap();

    assert_eq!(root_message_parent_hash, None);
}

#[tokio::test]
async fn test_insert_messages_with_tree_structure() {
    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);

    let (_, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

    let db = setup_test_db();
    let shinkai_db = Arc::new(db);

    let mut parent_message = None;

    eprintln!("Inserting messages...\n\n");
    let mut parent_message_hash: Option<String> = None;
    let mut parent_message_hash_2: Option<String> = None;
    let mut parent_message_hash_4: Option<String> = None;
    let mut parent_message_hash_5: Option<String> = None;
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
            .unsafe_insert_inbox_message(&message, parent_hash.clone(), None)
            .await
            .unwrap();

        // Update the parent message according to the tree structure
        if i == 1 {
            parent_message_hash = Some(message.calculate_message_hash_for_pagination());
        } else if i == 2 {
            parent_message_hash_2 = Some(message.calculate_message_hash_for_pagination());
        } else if i == 4 {
            parent_message_hash = Some(message.calculate_message_hash_for_pagination());
        } else if i == 7 {
            parent_message_hash_4 = Some(message.calculate_message_hash_for_pagination());
        } else if i == 5 {
            parent_message_hash_5 = Some(message.calculate_message_hash_for_pagination());
        }

        // Print the message hash, content, and parent hash
        println!(
            "message hash: {} message content: {} message parent hash: {}",
            message.calculate_message_hash_for_pagination(),
            message.get_message_content().unwrap(),
            parent_hash.as_deref().unwrap_or("None")
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

    /*
    Now we are updating the tree to looks like this:
        1
        ├── 2
        │   ├── 4
        │   │   ├── 6
        │   │   └── 7
        │   │       └── 8
        │   └── 5
        |       └── 9
        └── 3

        So the new path should be: [1], [2,3], [5,4], [9] (if we request >5 for n)
     */

    // Add message 9 as a child of message 5
    let message = generate_message_with_text(
        "Hello World 9".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.819Z".to_string(),
    );

    // Get the hash of message 5 to set as the parent of message 9
    let parent_hash = parent_message_hash_5.clone();

    shinkai_db
        .unsafe_insert_inbox_message(&message, parent_hash.clone(), None)
        .await
        .unwrap();

    // Print the message hash, content, and parent hash
    println!(
        "message hash: {} message content: {} message parent hash: {}",
        message.calculate_message_hash_for_pagination(),
        message.get_message_content().unwrap(),
        parent_hash.as_deref().unwrap_or("None")
    );

    // Get the last 5 messages from the inbox
    let last_messages_inbox = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 5, None)
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

    assert_eq!(last_messages_inbox[3].len(), 1);
    assert_eq!(
        last_messages_inbox[3][0].clone().get_message_content().unwrap(),
        "Hello World 9".to_string()
    );

    // Check the content of the second message array
    assert_eq!(last_messages_inbox[2].len(), 2);
    assert_eq!(
        last_messages_inbox[2][0].clone().get_message_content().unwrap(),
        "Hello World 5".to_string()
    );
    assert_eq!(
        last_messages_inbox[2][1].clone().get_message_content().unwrap(),
        "Hello World 4".to_string()
    );

    // Check the content of the third message array
    assert_eq!(last_messages_inbox[1].len(), 2);
    assert_eq!(
        last_messages_inbox[1][0].clone().get_message_content().unwrap(),
        "Hello World 2".to_string()
    );
    assert_eq!(
        last_messages_inbox[1][1].clone().get_message_content().unwrap(),
        "Hello World 3".to_string()
    );

    assert_eq!(last_messages_inbox[0].len(), 1);
    assert_eq!(
        last_messages_inbox[0][0].clone().get_message_content().unwrap(),
        "Hello World 1".to_string()
    );
}

#[tokio::test]
async fn db_inbox() {
    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let (_, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (_, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

    let message = generate_message_with_text(
        "Hello World".to_string(),
        node1_encryption_sk.clone(),
        clone_signature_secret_key(&node1_identity_sk),
        node1_subencryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.812Z".to_string(),
    );

    let db = setup_test_db();
    let shinkai_db = Arc::new(db);
    let _ = shinkai_db
        .unsafe_insert_inbox_message(&message.clone(), None, None)
        .await;
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
    match shinkai_db
        .unsafe_insert_inbox_message(&message2.clone(), None, None)
        .await
    {
        Ok(_) => println!("message2 inserted successfully"),
        Err(e) => println!("Failed to insert message2: {}", e),
    }

    match shinkai_db
        .unsafe_insert_inbox_message(&message3.clone(), None, None)
        .await
    {
        Ok(_) => println!("message3 inserted successfully"),
        Err(e) => println!("Failed to insert message3: {}", e),
    }

    match shinkai_db
        .unsafe_insert_inbox_message(&message4.clone(), None, None)
        .await
    {
        Ok(_) => println!("message4 inserted successfully"),
        Err(e) => println!("Failed to insert message4: {}", e),
    }

    match shinkai_db
        .unsafe_insert_inbox_message(&message5.clone(), None, None)
        .await
    {
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
        "Hello World 4".to_string()
    );
    assert_eq!(
        last_unread_messages_inbox[1].clone().get_message_content().unwrap(),
        "Hello World 5".to_string()
    );

    let offset = last_unread_messages_inbox[1]
        .clone()
        .calculate_message_hash_for_pagination();
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
        "Hello World 2".to_string()
    );

    // check pagination for inbox messages
    let last_unread_messages_inbox_page2 = shinkai_db
        .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 3, Some(offset))
        .unwrap();
    assert_eq!(last_unread_messages_inbox_page2.len(), 3);
    assert_eq!(
        last_unread_messages_inbox_page2[0][0]
            .clone()
            .get_message_content()
            .unwrap(),
        "Hello World 2".to_string()
    );

    // Mark as read up to a certain time
    shinkai_db
        .mark_as_read_up_to(
            inbox_name_value.clone().to_string(),
            last_unread_messages_inbox_page2[2][0]
                .clone()
                .calculate_message_hash_for_pagination(),
        )
        .unwrap();

    let last_messages_inbox = shinkai_db
        .get_last_unread_messages_from_inbox(inbox_name_value.clone().to_string(), 2, None)
        .unwrap();
    assert_eq!(last_messages_inbox.len(), 1);

    // Test permissions
    let subidentity_name = "device1";
    let full_subidentity_name =
        ShinkaiName::from_node_and_profile_names(node1_identity_name.to_string(), subidentity_name.to_string())
            .unwrap();

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
    eprintln!("inbox name: {}", inbox_name_value);

    shinkai_db
        .add_permission(&inbox_name_value, &device1_subidentity, InboxPermission::Admin)
        .unwrap();
    assert!(shinkai_db
        .has_permission(&inbox_name_value, &device1_subidentity, InboxPermission::Admin)
        .unwrap());

    shinkai_db
        .remove_permission(&inbox_name_value, &device1_subidentity)
        .unwrap();
    assert!(!shinkai_db
        .has_permission(&inbox_name_value, &device1_subidentity, InboxPermission::Admin)
        .unwrap());

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
    shinkai_db
        .unsafe_insert_inbox_message(&message4, None, None)
        .await
        .unwrap();
    shinkai_db
        .unsafe_insert_inbox_message(&message5, None, None)
        .await
        .unwrap();

    // Test get_inboxes_for_profile
    let node1_profile_identity = StandardIdentity::new(
        ShinkaiName::from_node_and_profile_names(node1_identity_name.to_string(), node1_subidentity_name.to_string())
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

    let inboxes = shinkai_db
        .get_inboxes_for_profile(node1_profile_identity.clone())
        .unwrap();
    assert_eq!(inboxes.len(), 1);
    assert!(inboxes.contains(&"inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false".to_string()));

    // Test get_smart_inboxes_for_profile
    let smart_inboxes = shinkai_db
        .get_all_smart_inboxes_for_profile(node1_profile_identity.clone())
        .unwrap();
    assert_eq!(smart_inboxes.len(), 1);

    // Check if smart_inboxes contain the expected results
    let expected_inbox_ids = ["inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false"];

    for smart_inbox in smart_inboxes {
        assert!(expected_inbox_ids.contains(&smart_inbox.inbox_id.as_str()));
        assert_eq!(format!("New Inbox: {}", smart_inbox.inbox_id), smart_inbox.custom_name);

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
    let updated_smart_inboxes = shinkai_db
        .get_all_smart_inboxes_for_profile(node1_profile_identity)
        .unwrap();

    // Check if the name of the updated inbox has been changed
    for smart_inbox in updated_smart_inboxes {
        if smart_inbox.inbox_id == inbox_to_update {
            eprintln!("Smart inbox: {:?}", smart_inbox);
            assert_eq!(smart_inbox.custom_name, new_name);
        }
    }
}

#[tokio::test]
async fn test_permission_errors() {
    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";

    let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let (_, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
    let (_, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

    let db = setup_test_db();
    let shinkai_db = Arc::new(db);

    // Update local node keys
    shinkai_db
        .update_local_node_keys(
            ShinkaiName::new(node1_identity_name.to_string()).unwrap(),
            node1_encryption_pk,
            node1_identity_pk,
        )
        .unwrap();

    let subidentity_name = "device1";
    let full_subidentity_name =
        ShinkaiName::from_node_and_profile_names(node1_identity_name.to_string(), subidentity_name.to_string())
            .unwrap();

    let device1_subidentity = StandardIdentity::new(
        full_subidentity_name.clone(),
        None,
        node1_encryption_pk,
        node1_identity_pk,
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );
    let _ = shinkai_db.insert_profile(device1_subidentity.clone());

    // Create a fake identity for tests
    let nonexistent_identity = StandardIdentity::new(
        ShinkaiName::from_node_and_profile_names(node1_identity_name.to_string(), "nonexistent_identity".to_string())
            .unwrap(),
        None,
        node1_encryption_pk,
        node1_identity_pk,
        Some(node1_subencryption_pk),
        Some(node1_subidentity_pk),
        StandardIdentityType::Profile,
        IdentityPermissions::Standard,
    );

    // Test 1: Adding a permission to a nonexistent inbox should result in an error
    let result = shinkai_db.add_permission("nonexistent_inbox", &device1_subidentity, InboxPermission::Admin);
    assert!(result.is_err());

    // Test 2: Adding a permission for a nonexistent identity should result in an error
    let result = shinkai_db.add_permission(
        "job_inbox::not_existent::false",
        &nonexistent_identity,
        InboxPermission::Admin,
    );
    assert!(result.is_err());

    // Test 3: Removing a permission from a nonexistent inbox should result in an error
    let result = shinkai_db.remove_permission("job_inbox::not_existent::false", &device1_subidentity);
    assert!(result.is_err());

    // Test 4: Removing a permission for a nonexistent identity should result in an error
    let result = shinkai_db.remove_permission("existing_inbox", &nonexistent_identity);
    assert!(result.is_err());

    // Test 5: Checking permission of a nonexistent inbox should result in an error
    let result: Result<bool, SqliteManagerError> = shinkai_db.has_permission(
        "job_inbox::not_existent::false",
        &device1_subidentity,
        InboxPermission::Admin,
    );
    assert!(result.is_err());

    // Test 6: Checking permission for a nonexistent identity should result in an error
    let result = shinkai_db.has_permission("existing_inbox", &nonexistent_identity, InboxPermission::Admin);
    assert!(result.is_err());
}
