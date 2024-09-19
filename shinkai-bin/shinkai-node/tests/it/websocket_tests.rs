use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::Aead;
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_trait::async_trait;
use ed25519_dalek::SigningKey;
use futures::SinkExt;
use futures::StreamExt;
use shinkai_db::db::ShinkaiDB;
use shinkai_db::schemas::inbox_permission::InboxPermission;
use shinkai_db::schemas::ws_types::WSMessagePayload;
use shinkai_db::schemas::ws_types::WSMessageType;
use shinkai_message_primitives::schemas::identity::Identity;
use shinkai_message_primitives::schemas::identity::StandardIdentity;
use shinkai_message_primitives::schemas::identity::StandardIdentityType;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::IdentityPermissions;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::TopicSubscription;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::WSTopic;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use shinkai_message_primitives::shinkai_utils::file_encryption::aes_encryption_key_to_string;
use shinkai_message_primitives::shinkai_utils::file_encryption::unsafe_deterministic_aes_encryption_key;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;
use shinkai_node::managers::identity_manager::IdentityManagerTrait;
use shinkai_node::network::{ws_manager::WebSocketManager, ws_routes::run_ws_api};
use shinkai_vector_resources::utils::hash_string;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

// Mock struct for testing
#[derive(Clone, Debug)]
struct MockIdentityManager {
    dummy_standard_identity: Identity,
    // Add any fields you need for your mock
}

impl MockIdentityManager {
    pub fn new() -> Self {
        let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let dummy_standard_identity = Identity::Standard(StandardIdentity {
            full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            addr: None,
            node_encryption_public_key: node1_encryption_pk,
            node_signature_public_key: node1_identity_pk,
            profile_encryption_public_key: Some(node1_encryption_pk),
            profile_signature_public_key: Some(node1_identity_pk),
            identity_type: StandardIdentityType::Global,
            permission_type: IdentityPermissions::Admin,
        });

        Self {
            dummy_standard_identity,
            // initialize other fields...
        }
    }
}

#[async_trait]
impl IdentityManagerTrait for MockIdentityManager {
    fn find_by_identity_name(&self, _full_profile_name: ShinkaiName) -> Option<&Identity> {
        if _full_profile_name.to_string() == "@@node1.shinkai/main_profile_node1" {
            Some(&self.dummy_standard_identity)
        } else {
            None
        }
    }

    async fn search_identity(&self, _full_identity_name: &str) -> Option<Identity> {
        if _full_identity_name == "@@node1.shinkai/main_profile_node1" {
            Some(self.dummy_standard_identity.clone())
        } else {
            None
        }
    }

    fn clone_box(&self) -> Box<dyn IdentityManagerTrait + Send> {
        Box::new(self.clone())
    }

    async fn external_profile_to_global_identity(&self, _full_profile_name: &str) -> Result<StandardIdentity, String> {
        unimplemented!()
    }
}

fn decrypt_message(encrypted_hex: &str, shared_key: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Decode the hex string to bytes
    let encrypted_bytes = hex::decode(encrypted_hex)?;

    // Create the cipher using the shared key
    let shared_key_bytes = hex::decode(shared_key)?;
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&shared_key_bytes));

    // Create the nonce
    let nonce = GenericArray::from_slice(&[0u8; 12]);

    // Decrypt the message
    let decrypted_bytes = cipher
        .decrypt(nonce, encrypted_bytes.as_ref())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    // Convert the decrypted bytes to a string
    let decrypted_message = String::from_utf8(decrypted_bytes)?;

    Ok(decrypted_message)
}

#[allow(clippy::too_many_arguments)]
fn generate_message_with_text(
    content: String,
    inbox_name: String,
    my_encryption_secret_key: EncryptionStaticKey,
    my_signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey,
    recipient_subidentity_name: String,
    origin_destination_identity_name: String,
    timestamp: String,
) -> ShinkaiMessage {
    ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .message_raw_content(content.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::WSMessage)
        .internal_metadata_with_inbox(
            recipient_subidentity_name.clone().to_string(),
            "".to_string(),
            inbox_name,
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

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

#[tokio::test]
async fn test_websocket() {
    // Setup
    setup();

    let job_id1 = "test_job".to_string();
    let job_id2 = "test_job2".to_string();
    let agent_id = "agent3".to_string();
    let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let shinkai_db = Arc::new(shinkai_db);
    let shinkai_db_weak = Arc::downgrade(&shinkai_db);

    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let node_name = ShinkaiName::new(node1_identity_name.to_string()).unwrap();
    let identity_manager_trait: Arc<Mutex<dyn IdentityManagerTrait + Send>> =
        Arc::new(Mutex::new(MockIdentityManager::new()));

    let inbox_name1 = InboxName::get_job_inbox_name_from_params(job_id1.to_string()).unwrap();
    let inbox_name2 = InboxName::get_job_inbox_name_from_params(job_id2.to_string()).unwrap();

    let inbox_name1_string = match inbox_name1 {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value.clone(),
    };
    let inbox_name2_string = match &inbox_name2 {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value.clone(),
    };

    // Start the WebSocket server
    let ws_manager = WebSocketManager::new(
        shinkai_db_weak.clone(),
        node_name,
        identity_manager_trait.clone(),
        clone_static_secret_key(&node1_encryption_sk),
    )
    .await;
    let ws_address = "127.0.0.1:8080".parse().expect("Failed to parse WebSocket address");
    tokio::spawn(run_ws_api(ws_address, Arc::clone(&ws_manager)));

    // Give the server a little time to start
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Connect to the server
    let connection_result = tokio_tungstenite::connect_async("ws://127.0.0.1:8080/ws").await;

    // Check if the connection was successful
    assert!(connection_result.is_ok(), "Failed to connect");

    let (mut ws_stream, _) = connection_result.expect("Failed to connect");

    // Create a shared encryption key Aes256Gcm
    let symmetrical_sk = unsafe_deterministic_aes_encryption_key(0);
    let shared_enc_string = aes_encryption_key_to_string(symmetrical_sk);
    eprintln!("shared_enc_string: {}", shared_enc_string);

    // Send a message to the server to establish the connection and subscribe to a topic
    let ws_message = WSMessage {
        subscriptions: vec![
            TopicSubscription {
                topic: WSTopic::Inbox,
                subtopic: Some("job_inbox::test_job::false".to_string()),
            },
            TopicSubscription {
                topic: WSTopic::Inbox,
                subtopic: Some("job_inbox::test_job2::false".to_string()),
            },
        ],
        unsubscriptions: vec![],
        shared_key: Some(shared_enc_string.to_string()),
    };

    // Serialize WSMessage to a JSON string
    let ws_message_json = serde_json::to_string(&ws_message).unwrap();

    // Generate a ShinkaiMessage
    let shinkai_message = generate_message_with_text(
        ws_message_json,
        inbox_name1_string.to_string(),
        node1_encryption_sk.clone(),
        node1_identity_sk.clone(),
        node1_encryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.810Z".to_string(),
    );

    {
        // Add identity to the database
        let sender_subidentity = {
            let shinkai_name = ShinkaiName::from_node_and_profile_names(
                node1_identity_name.to_string(),
                node1_subidentity_name.to_string(),
            )
            .unwrap();
            let identity_manager_lock = identity_manager_trait.lock().await;
            match identity_manager_lock.find_by_identity_name(shinkai_name).unwrap() {
                Identity::Standard(std_identity) => std_identity.clone(),
                _ => panic!("Identity is not of type StandardIdentity"),
            }
        };

        let _ = shinkai_db.insert_profile(sender_subidentity.clone());
        let scope = JobScope::new_default();
        match shinkai_db.create_new_job(job_id1, agent_id.clone(), scope.clone(), false, None, None) {
            Ok(_) => (),
            Err(e) => panic!("Failed to create a new job: {}", e),
        }
        match shinkai_db.create_new_job(job_id2, agent_id, scope, false, None, None) {
            Ok(_) => (),
            Err(e) => panic!("Failed to create a new job: {}", e),
        }
        shinkai_db
            .add_permission(&inbox_name1_string, &sender_subidentity, InboxPermission::Admin)
            .unwrap();
        shinkai_db
            .add_permission(&inbox_name2_string, &sender_subidentity, InboxPermission::Admin)
            .unwrap();
    }

    // Convert ShinkaiMessage to String
    let message_string = shinkai_message.to_string().unwrap();

    ws_stream
        .send(tungstenite::Message::Text(message_string))
        .await
        .expect("Failed to send message");

    // Wait for the server to process the subscription message
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Note: Manual way to push an update for testing purposes
    // Send a message to all connections that are subscribed to the topic
    ws_manager
        .lock()
        .await
        .handle_update(
            WSTopic::Inbox,
            "job_inbox::test_job::false".to_string(),
            "Hello, world!".to_string(),
            WSMessageType::None,
            false,
        )
        .await;

    // Check the response
    let msg = ws_stream
        .next()
        .await
        .expect("Failed to read message")
        .expect("Failed to read message");
    let encrypted_message = msg.to_text().unwrap();
    let decrypted_message = decrypt_message(encrypted_message, &shared_enc_string).expect("Failed to decrypt message");

    let ws_message_payload: WSMessagePayload =
        serde_json::from_str(&decrypted_message).expect("Failed to parse WSMessagePayload");
    eprintln!("ws_message_payload: {:?}", ws_message_payload);

    assert_eq!(ws_message_payload.message.unwrap(), "Hello, world!");

    // Note: We add a message and we expect to trigger an update
    {
        // Generate a ShinkaiMessage
        let shinkai_message = generate_message_with_text(
            "Hello, world!".to_string(),
            inbox_name1_string.to_string(),
            node1_encryption_sk.clone(),
            node1_identity_sk.clone(),
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.810Z".to_string(),
        );

        let _ = shinkai_db
            .unsafe_insert_inbox_message(&shinkai_message.clone(), None, Some(ws_manager.clone()))
            .await;
        // eprintln!("result: {:?}", result);
        // eprintln!("here after adding a message");

        // Check the response
        let msg = ws_stream
            .next()
            .await
            .expect("Failed to read message")
            .expect("Failed to read message");

        // TODO: it should decrypt the message with the symmetrical key

        let encrypted_msg_text = msg.to_text().unwrap();
        let decrypted_message =
            decrypt_message(encrypted_msg_text, &shared_enc_string).expect("Failed to decrypt message");
        let ws_message_payload: WSMessagePayload =
            serde_json::from_str(&decrypted_message).expect("Failed to parse WSMessagePayload");
        let recovered_shinkai = ShinkaiMessage::from_string(ws_message_payload.message.unwrap()).unwrap();
        let recovered_content = recovered_shinkai.get_message_content().unwrap();
        assert_eq!(recovered_content, "Hello, world!");
    }
    // Send a message to inbox_name2_string (Job2)
    {
        let shinkai_message = generate_message_with_text(
            "Hello, world 2!".to_string(),
            inbox_name2_string.to_string(),
            node1_encryption_sk.clone(),
            node1_identity_sk.clone(),
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.810Z".to_string(),
        );

        let _ = shinkai_db
            .unsafe_insert_inbox_message(&shinkai_message.clone(), None, Some(ws_manager.clone()))
            .await;

        // Wait for the server to process the message
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Check the response
        let msg = ws_stream
            .next()
            .await
            .expect("Failed to read message")
            .expect("Failed to read message");

        let encrypted_msg_text = msg.to_text().unwrap();
        // eprintln!("encrypted_msg_text: {}", encrypted_msg_text);
        let decrypted_message =
            decrypt_message(encrypted_msg_text, &shared_enc_string).expect("Failed to decrypt message");
        let ws_message_payload: WSMessagePayload =
            serde_json::from_str(&decrypted_message).expect("Failed to parse WSMessagePayload");
        let recovered_shinkai = ShinkaiMessage::from_string(ws_message_payload.message.unwrap()).unwrap();
        let recovered_content = recovered_shinkai.get_message_content().unwrap();
        assert_eq!(recovered_content, "Hello, world 2!");
    }

    // Unsubscribe from inbox_name1_string
    {
        let ws_message = WSMessage {
            subscriptions: vec![],
            unsubscriptions: vec![TopicSubscription {
                topic: WSTopic::Inbox,
                subtopic: Some("job_inbox::test_job::false".to_string()),
            }],
            shared_key: Some(shared_enc_string.to_string()),
        };

        // Serialize WSMessage to a JSON string
        let ws_message_json = serde_json::to_string(&ws_message).unwrap();

        // Generate a ShinkaiMessage
        let shinkai_message = generate_message_with_text(
            ws_message_json,
            inbox_name1_string.to_string(),
            node1_encryption_sk.clone(),
            node1_identity_sk.clone(),
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.810Z".to_string(),
        );

        // Convert ShinkaiMessage to String
        let message_string = shinkai_message.to_string().unwrap();

        ws_stream
            .send(tungstenite::Message::Text(message_string))
            .await
            .expect("Failed to send message");

        // Wait for the server to process the unsubscription message
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Send a new message to inbox_name1_string
    {
        let shinkai_message = generate_message_with_text(
            "Hello, world 3!".to_string(),
            inbox_name1_string.to_string(),
            node1_encryption_sk,
            node1_identity_sk,
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.810Z".to_string(),
        );

        let _ = shinkai_db
            .unsafe_insert_inbox_message(&shinkai_message.clone(), None, Some(ws_manager))
            .await;

        // Wait for the server to process the message
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Check that no message is received
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), ws_stream.next()).await;

        assert!(result.is_err());
    }

    // Send a close message
    ws_stream
        .send(tungstenite::Message::Close(None))
        .await
        .expect("Failed to send close message");

    std::mem::drop(shinkai_db);
}

#[tokio::test]
async fn test_websocket_smart_inbox() {
    // Setup
    setup();

    let job_id1 = "test_job".to_string();
    let no_access_job_id = "no_access_job_id".to_string();
    let agent_id = "agent4".to_string();
    let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
    let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
    let shinkai_db = Arc::new(shinkai_db);
    let shinkai_db_weak = Arc::downgrade(&shinkai_db);

    let node1_identity_name = "@@node1.shinkai";
    let node1_subidentity_name = "main_profile_node1";
    let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    let node_name = ShinkaiName::new(node1_identity_name.to_string()).unwrap();
    let identity_manager_trait: Arc<Mutex<dyn IdentityManagerTrait + Send>> =
        Arc::new(Mutex::new(MockIdentityManager::new()));

    let inbox_name1 = InboxName::get_job_inbox_name_from_params(job_id1.to_string()).unwrap();
    let inbox_name1_string = match inbox_name1 {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value.clone(),
    };

    let no_access_job_id_name = InboxName::get_job_inbox_name_from_params(no_access_job_id.to_string()).unwrap();
    let no_access_job_id_name_string = match no_access_job_id_name {
        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value.clone(),
    };

    // Start the WebSocket server
    let ws_manager = WebSocketManager::new(
        shinkai_db_weak.clone(),
        node_name,
        identity_manager_trait.clone(),
        clone_static_secret_key(&node1_encryption_sk),
    )
    .await;
    let ws_address = "127.0.0.1:8080".parse().expect("Failed to parse WebSocket address");
    tokio::spawn(run_ws_api(ws_address, Arc::clone(&ws_manager)));

    // Give the server a little time to start
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Connect to the server
    let connection_result = tokio_tungstenite::connect_async("ws://127.0.0.1:8080/ws").await;

    // Check if the connection was successful
    assert!(connection_result.is_ok(), "Failed to connect");

    let (mut ws_stream, _) = connection_result.expect("Failed to connect");

    // Create a shared encryption key Aes256Gcm
    let symmetrical_sk = unsafe_deterministic_aes_encryption_key(0);
    let shared_enc_string = aes_encryption_key_to_string(symmetrical_sk);

    // Send a message to the server to establish the connection and subscribe to a topic
    let ws_message = WSMessage {
        subscriptions: vec![TopicSubscription {
            topic: WSTopic::SmartInboxes,
            subtopic: None,
        }],
        unsubscriptions: vec![],
        shared_key: Some(shared_enc_string.to_string()),
    };

    // Serialize WSMessage to a JSON string
    let ws_message_json = serde_json::to_string(&ws_message).unwrap();

    // Generate a ShinkaiMessage
    let shinkai_message = generate_message_with_text(
        ws_message_json,
        "".to_string(),
        node1_encryption_sk.clone(),
        node1_identity_sk.clone(),
        node1_encryption_pk,
        node1_subidentity_name.to_string(),
        node1_identity_name.to_string(),
        "2023-07-02T20:53:34.810Z".to_string(),
    );

    {
        // Add identity to the database
        let sender_subidentity = {
            let shinkai_name = ShinkaiName::from_node_and_profile_names(
                node1_identity_name.to_string(),
                node1_subidentity_name.to_string(),
            )
            .unwrap();
            let identity_manager_lock = identity_manager_trait.lock().await;
            match identity_manager_lock.find_by_identity_name(shinkai_name).unwrap() {
                Identity::Standard(std_identity) => std_identity.clone(),
                _ => panic!("Identity is not of type StandardIdentity"),
            }
        };

        let _ = shinkai_db.insert_profile(sender_subidentity.clone());
        let scope = JobScope::new_default();
        match shinkai_db.create_new_job(job_id1, agent_id.clone(), scope.clone(), false, None, None) {
            Ok(_) => (),
            Err(e) => panic!("Failed to create a new job: {}", e),
        }
        shinkai_db
            .add_permission(&inbox_name1_string, &sender_subidentity, InboxPermission::Admin)
            .unwrap();

        match shinkai_db.create_new_job(no_access_job_id, agent_id, scope, false, None, None) {
            Ok(_) => (),
            Err(e) => panic!("Failed to create a new job: {}", e),
        }
    }

    // Convert ShinkaiMessage to String
    let message_string = shinkai_message.to_string().unwrap();

    ws_stream
        .send(tungstenite::Message::Text(message_string))
        .await
        .expect("Failed to send message");

    // Wait for the server to process the subscription message
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Send a new message to inbox_name1_string
    {
        // Generate a ShinkaiMessage
        let shinkai_message = generate_message_with_text(
            "Hello, world!".to_string(),
            inbox_name1_string.to_string(),
            node1_encryption_sk.clone(),
            node1_identity_sk.clone(),
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.810Z".to_string(),
        );

        let _ = shinkai_db
            .unsafe_insert_inbox_message(&shinkai_message.clone(), None, Some(ws_manager.clone()))
            .await;
    }

    // Check the response
    let msg = ws_stream
        .next()
        .await
        .expect("Failed to read message")
        .expect("Failed to read message");

    let encrypted_message = msg.to_text().unwrap();
    let decrypted_message = decrypt_message(encrypted_message, &shared_enc_string).expect("Failed to decrypt message");
    let ws_message_payload: WSMessagePayload =
        serde_json::from_str(&decrypted_message).expect("Failed to parse WSMessagePayload");
    let recovered_shinkai = ShinkaiMessage::from_string(ws_message_payload.message.unwrap()).unwrap();
    let recovered_content = recovered_shinkai.get_message_content().unwrap();
    assert_eq!(recovered_content, "Hello, world!");

    // Send a message to an inbox that the user DOES NOT have access. the user shouldn't receive a notification
    {
        let shinkai_message = generate_message_with_text(
            "Hello, no one!".to_string(),
            no_access_job_id_name_string.to_string(),
            node1_encryption_sk.clone(),
            node1_identity_sk.clone(),
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.810Z".to_string(),
        );

        let _ = shinkai_db
            .unsafe_insert_inbox_message(&shinkai_message.clone(), None, Some(ws_manager))
            .await;
    }

    // Check that no message is received
    let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), ws_stream.next()).await;
    eprintln!("result: {:?}", result);
    assert!(result.is_err());

    // Send a close message
    ws_stream
        .send(tungstenite::Message::Close(None))
        .await
        .expect("Failed to send close message");

    std::mem::drop(shinkai_db);
}

// Note: We need to mock up JobManager and change the depencency of SheetManager to a trait so we can swap between JobManager or the MockJobManager
// #[tokio::test]
// async fn test_websocket_sheet_update() {
//
//     // Setup
//     let agent_id = "agent".to_string();
//     let db_path = format!("db_tests/{}", hash_string(&agent_id.clone()));
//     let shinkai_db = ShinkaiDB::new(&db_path).unwrap();
//     let shinkai_db = Arc::new(shinkai_db);
//     let shinkai_db_weak = Arc::downgrade(&shinkai_db);

//     let node1_identity_name = "@@node1.shinkai";
//     let node1_subidentity_name = "main_profile_node1";
//     let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
//     let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

//     let node_name = ShinkaiName::new(node1_identity_name.to_string()).unwrap();
//     let identity_manager_trait: Arc<Mutex<dyn IdentityManagerTrait + Send>> =
//         Arc::new(Mutex::new(MockIdentityManager::new()));

//     // Start the WebSocket server
//     let ws_manager = WebSocketManager::new(
//         shinkai_db_weak.clone(),
//         node_name.clone(),
//         identity_manager_trait.clone(),
//         clone_static_secret_key(&node1_encryption_sk),
//     )
//     .await;
//     let ws_address = "127.0.0.1:8080".parse().expect("Failed to parse WebSocket address");
//     tokio::spawn(run_ws_api(ws_address, Arc::clone(&ws_manager)));

//     // Give the server a little time to start
//     tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

//     let sheet_manager = SheetManager::new(shinkai_db_weak.clone(), node_name.clone(), Some(ws_manager.clone()))
//         .await
//         .unwrap();
//     let node1_sheet_manager = Arc::new(Mutex::new(sheet_manager));

//     // Connect to the server
//     let connection_result = tokio_tungstenite::connect_async("ws://127.0.0.1:8080/ws").await;

//     // Check if the connection was successful
//     assert!(connection_result.is_ok(), "Failed to connect");

//     let (mut ws_stream, _) = connection_result.expect("Failed to connect");

//     // Create a shared encryption key Aes256Gcm
//     let symmetrical_sk = unsafe_deterministic_aes_encryption_key(0);
//     let shared_enc_string = aes_encryption_key_to_string(symmetrical_sk);

//     // Create a Sheet
//     let mut sheet_id = "".to_string();
//     let row_id;
//     let column_llm;
//     // Define columns with UUIDs
//     let column_text = ColumnDefinition {
//         id: Uuid::new_v4().to_string(),
//         name: "Column A".to_string(),
//         behavior: ColumnBehavior::Text,
//     };

//     {
//         let sheet_manager = node1_sheet_manager.clone();
//         let mut sheet_manager = sheet_manager.lock().await;

//         // Create a new empty sheet
//         sheet_manager.create_empty_sheet().unwrap();

//         // Get the ID of the newly created sheet
//         let sheets = sheet_manager.get_user_sheets().await.unwrap();
//         sheet_id.clone_from(&sheets.last().unwrap().uuid);

//         let workflow_str = r#"
//         workflow WorkflowTest v0.1 {
//             step Main {
//                 $RESULT = call opinionated_inference($INPUT)
//             }
//         }
//         "#;
//         let workflow = parse_workflow(workflow_str).unwrap();

//         column_llm = ColumnDefinition {
//             id: Uuid::new_v4().to_string(),
//             name: "Column B".to_string(),
//             behavior: ColumnBehavior::LLMCall {
//                 input: "=A".to_string(),
//                 workflow: Some(workflow),
//                 workflow_name: None,
//                 llm_provider_name: agent_id.clone(),
//                 input_hash: None,
//             },
//         };

//         let column_formula = ColumnDefinition {
//             id: Uuid::new_v4().to_string(),
//             name: "Column C".to_string(),
//             behavior: ColumnBehavior::Formula("=B + \" And Space\"".to_string()),
//         };

//         // Set columns
//         sheet_manager.set_column(&sheet_id, column_text.clone()).await.unwrap();
//         sheet_manager.set_column(&sheet_id, column_llm.clone()).await.unwrap();
//         sheet_manager
//             .set_column(&sheet_id, column_formula.clone())
//             .await
//             .unwrap();

//         // Add a new row
//         row_id = sheet_manager.add_row(&sheet_id, None).await.unwrap();

//         // Set value in Column A
//         sheet_manager
//             .set_cell_value(&sheet_id, row_id.clone(), column_text.id.clone(), "Hello".to_string())
//             .await
//             .unwrap();
//     }

//     // Send a message to the server to establish the connection and subscribe to the sheet updates
//     let ws_message = WSMessage {
//         subscriptions: vec![TopicSubscription {
//             topic: WSTopic::Sheet,
//             subtopic: Some(sheet_id.clone()),
//         }],
//         unsubscriptions: vec![],
//         shared_key: Some(shared_enc_string.to_string()),
//     };

//     // Serialize WSMessage to a JSON string
//     let ws_message_json = serde_json::to_string(&ws_message).unwrap();

//     // Generate a ShinkaiMessage
//     let shinkai_message = generate_message_with_text(
//         ws_message_json,
//         "".to_string(),
//         node1_encryption_sk.clone(),
//         node1_identity_sk.clone(),
//         node1_encryption_pk,
//         node1_subidentity_name.to_string(),
//         node1_identity_name.to_string(),
//         "2023-07-02T20:53:34.810Z".to_string(),
//     );

//     // Convert ShinkaiMessage to String
//     let message_string = shinkai_message.to_string().unwrap();

//     ws_stream
//         .send(tungstenite::Message::Text(message_string))
//         .await
//         .expect("Failed to send message");

//     // Wait for the server to process the subscription message
//     tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

//     // Update the sheet
//     {
//         let sheet_manager = node1_sheet_manager.clone();
//         let mut sheet_manager = sheet_manager.lock().await;

//         // Set value in Column A
//         sheet_manager
//             .set_cell_value(
//                 &sheet_id,
//                 row_id.clone(),
//                 column_text.id.clone(),
//                 "Updated Hello".to_string(),
//             )
//             .await
//             .unwrap();
//     }

//     // Check the response
//     let msg = ws_stream
//         .next()
//         .await
//         .expect("Failed to read message")
//         .expect("Failed to read message");

//     let encrypted_message = msg.to_text().unwrap();
//     let decrypted_message = decrypt_message(encrypted_message, &shared_enc_string).expect("Failed to decrypt message");
//     let ws_message_payload: WSMessagePayload =
//         serde_json::from_str(&decrypted_message).expect("Failed to parse WSMessagePayload");
//     let recovered_shinkai = ShinkaiMessage::from_string(ws_message_payload.message.unwrap()).unwrap();
//     let recovered_content = recovered_shinkai.get_message_content().unwrap();
//     assert_eq!(recovered_content, "Updated Hello");

//     // Send a close message
//     ws_stream
//         .send(tungstenite::Message::Close(None))
//         .await
//         .expect("Failed to send close message");

//     std::mem::drop(shinkai_db);
// }
