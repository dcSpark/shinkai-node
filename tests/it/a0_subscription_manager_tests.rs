use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_channel::{bounded, Receiver, Sender};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use shinkai_message_primitives::schemas::shinkai_subscription::{ShinkaiSubscription, ShinkaiSubscriptionStatus};
use core::panic;
use std::collections::HashMap;
use ed25519_dalek::SigningKey;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription_req::FolderSubscription;
use shinkai_message_primitives::schemas::shinkai_subscription_req::{PaymentOption, SubscriptionPayment};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIAvailableSharedItems, APIConvertFilesAndSaveToFolder, APICreateShareableFolder, APIVecFsCreateFolder, APIVecFsDeleteFolder, APIVecFsDeleteItem, APIVecFsRetrievePathSimplifiedJson, MessageSchemaType
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair, EncryptionMethod
};
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::node_api::APIError;
use shinkai_node::network::Node;
use shinkai_vector_resources::resource_errors::VRError;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::utils::node_test_api::api_registration_device_node_profile_main;
use super::utils::node_test_local::local_registration_profile_node;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

#[allow(clippy::too_many_arguments)]
fn generate_message_with_payload<T: ToString>(
    payload: T,
    schema: MessageSchemaType,
    my_encryption_secret_key: EncryptionStaticKey,
    my_signature_secret_key: SigningKey,
    receiver_public_key: EncryptionPublicKey,
    sender: &str,
    sender_subidentity: &str,
    recipient: &str,
    recipient_subidentity: &str,
) -> ShinkaiMessage {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S%f").to_string();

    let message = ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .message_raw_content(payload.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(schema)
        .internal_metadata_with_inbox(
            sender_subidentity.to_string(),
            recipient_subidentity.to_string(),
            "".to_string(),
            EncryptionMethod::None,
            None,
        )
        .external_metadata_with_schedule(recipient.to_string(), sender.to_string(), timestamp)
        .build()
        .unwrap();
    message
}

// Function to recursively check if the actual response contains the expected structure
fn check_structure(actual: &serde_json::Value, expected: &serde_json::Value) -> bool {
    if let (Some(actual_folders), Some(expected_folders)) = (actual["child_folders"].as_array(), expected["child_folders"].as_array()) {
        if actual_folders.len() != expected_folders.len() {
            return false;
        }
        for (actual_folder, expected_folder) in actual_folders.iter().zip(expected_folders.iter()) {
            if !check_folder(actual_folder, expected_folder) {
                return false;
            }
        }
    } else {
        return false;
    }
    true
}

fn check_folder(actual_folder: &serde_json::Value, expected_folder: &serde_json::Value) -> bool {
    let actual_name = actual_folder["name"].as_str().unwrap_or("Unknown Folder");
    let expected_name = expected_folder["name"].as_str().unwrap_or("Unknown Folder");
    if actual_name != expected_name {
        return false;
    }

    let actual_path = actual_folder["path"].as_str().unwrap_or("Unknown Path");
    let expected_path = expected_folder["path"].as_str().unwrap_or("Unknown Path");
    if actual_path != expected_path {
        return false;
    }

    let empty_vec = vec![];
    let actual_subfolders = actual_folder["child_folders"].as_array().unwrap_or(&empty_vec);
    let expected_subfolders = expected_folder["child_folders"].as_array().unwrap_or(&empty_vec);
    if actual_subfolders.len() != expected_subfolders.len() {
        return false;
    }
    for (actual_subfolder, expected_subfolder) in actual_subfolders.iter().zip(expected_subfolders.iter()) {
        if !check_folder(actual_subfolder, expected_subfolder) {
            return false;
        }
    }

    let actual_items = actual_folder["child_items"].as_array().unwrap_or(&empty_vec);
    let expected_items = expected_folder["child_items"].as_array().unwrap_or(&empty_vec);
    if actual_items.len() != expected_items.len() {
        return false;
    }
    for (actual_item, expected_item) in actual_items.iter().zip(expected_items.iter()) {
        let actual_item_name = actual_item["name"].as_str().unwrap_or("Unknown Item");
        let expected_item_name = expected_item["name"].as_str().unwrap_or("Unknown Item");
        if actual_item_name != expected_item_name {
            return false;
        }

        let actual_item_path = actual_item["path"].as_str().unwrap_or("Unknown Path");
        let expected_item_path = expected_item["path"].as_str().unwrap_or("Unknown Path");
        if actual_item_path != expected_item_path {
            return false;
        }
    }

    true
}

async fn fetch_last_messages(
    commands_sender: &Sender<NodeCommand>,
    limit: usize,
) -> Result<Vec<ShinkaiMessage>, APIError> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    commands_sender
        .send(NodeCommand::FetchLastMessages { limit, res: res_sender })
        .await
        .unwrap();
    Ok(res_receiver.recv().await.unwrap())
}

async fn make_folder_shareable(
    commands_sender: &Sender<NodeCommand>,
    folder_path: &str,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
) {
    let payload = APICreateShareableFolder {
        path: folder_path.to_string(),
        subscription_req: FolderSubscription {
            minimum_token_delegation: Some(100),
            minimum_time_delegated_hours: Some(100),
            monthly_payment: Some(PaymentOption::USD(10.0)),
            is_free: false,
            has_web_alternative: Some(false),
            folder_description: "This is a test folder".to_string(),
        },
    };

    let msg = generate_message_with_payload(
        serde_json::to_string(&payload).unwrap(),
        MessageSchemaType::CreateShareableFolder,
        encryption_sk,
        signature_sk,
        encryption_pk,
        identity_name,
        profile_name,
        identity_name,
        profile_name,
    );

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command
    commands_sender
        .send(NodeCommand::APICreateShareableFolder { msg, res: res_sender })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("Make folder shareable resp: {:?}", resp);
}

#[allow(clippy::too_many_arguments)]
async fn show_available_shared_items(
    streamer_node_name: &str,
    streamer_profile_name: &str,
    commands_sender: &Sender<NodeCommand>,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
) {
    let payload = APIAvailableSharedItems {
        path: "/".to_string(), // Assuming you want to list items at the root
        streamer_node_name: streamer_node_name.to_string(),
        streamer_profile_name: streamer_profile_name.to_string(),
    };

    let msg = generate_message_with_payload(
        serde_json::to_string(&payload).unwrap(),
        MessageSchemaType::AvailableSharedItems,
        encryption_sk,
        signature_sk,
        encryption_pk,
        identity_name,
        profile_name,
        identity_name,
        streamer_profile_name,
    );

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command
    commands_sender
        .send(NodeCommand::APIAvailableSharedItems { msg, res: res_sender })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("Available shared items resp: {:?}", resp);
}

#[allow(clippy::too_many_arguments)]
async fn create_folder(
    commands_sender: &Sender<NodeCommand>,
    folder_path: &str,
    folder_name: &str,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
) {
    let payload = APIVecFsCreateFolder {
        path: folder_path.to_string(),
        folder_name: folder_name.to_string(),
    };

    let msg = generate_message_with_payload(
        serde_json::to_string(&payload).unwrap(),
        MessageSchemaType::VecFsCreateFolder,
        encryption_sk,
        signature_sk,
        encryption_pk,
        identity_name,
        profile_name,
        identity_name,
        profile_name,
    );

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command
    commands_sender
        .send(NodeCommand::APIVecFSCreateFolder { msg, res: res_sender })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("resp: {:?}", resp);
}

async fn remove_folder(
    commands_sender: &Sender<NodeCommand>,
    folder_path: &str,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
) {
    let payload = APIVecFsDeleteFolder {
        path: folder_path.to_string(),
    };

    let msg = generate_message_with_payload(
        serde_json::to_string(&payload).unwrap(),
        MessageSchemaType::VecFsDeleteFolder,
        encryption_sk,
        signature_sk,
        encryption_pk,
        identity_name,
        profile_name,
        identity_name,
        profile_name,
    );

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command
    commands_sender
        .send(NodeCommand::APIVecFSDeleteFolder { msg, res: res_sender })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("resp: {:?}", resp);
}

async fn remove_item(
    commands_sender: &Sender<NodeCommand>,
    item_path: &str,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
) {
    let payload = APIVecFsDeleteItem {
        path: item_path.to_string(),
    };

    let msg = generate_message_with_payload(
        serde_json::to_string(&payload).unwrap(),
        MessageSchemaType::VecFsDeleteItem,
        encryption_sk,
        signature_sk,
        encryption_pk,
        identity_name,
        profile_name,
        identity_name,
        profile_name,
    );

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command
    commands_sender
        .send(NodeCommand::APIVecFSDeleteItem { msg, res: res_sender })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("resp: {:?}", resp);
}


fn remove_timestamps_from_shared_folder_cache_response(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("last_ext_node_response");
            map.remove("last_request_to_ext_node");
            map.remove("last_updated");
            map.remove("response_last_updated");
            map.remove("last_modified");
            // Use a closure to explicitly call `remove_timestamps_from_response`
            map.values_mut()
                .for_each(remove_timestamps_from_shared_folder_cache_response);
        }
        serde_json::Value::Array(vec) => {
            vec.iter_mut()
                .for_each(remove_timestamps_from_shared_folder_cache_response);
        }
        _ => {}
    }
}

// Function to recursively remove date-related fields from a JSON value
fn remove_date_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let keys_to_remove: Vec<String> = map.iter()
                .filter_map(|(k, _)| if k.contains("datetime") || k.contains("date") { Some(k.clone()) } else { None })
                .collect();
            for key in keys_to_remove {
                map.remove(&key);
            }
            for v in map.values_mut() {
                remove_date_fields(v);
            }
        },
        serde_json::Value::Array(vec) => {
            for v in vec {
                remove_date_fields(v);
            }
        },
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
async fn retrieve_file_info(
    commands_sender: &Sender<NodeCommand>,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
    path: &str,
    is_simple: bool,
) -> Value {
    let payload = APIVecFsRetrievePathSimplifiedJson { path: path.to_string() };

    let msg = generate_message_with_payload(
        serde_json::to_string(&payload).unwrap(),
        MessageSchemaType::VecFsRetrievePathSimplifiedJson,
        encryption_sk.clone(),
        signature_sk.clone(),
        encryption_pk,
        identity_name,
        profile_name,
        identity_name,
        profile_name,
    );

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command
    commands_sender
        .send(NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res: res_sender })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");

    if is_simple {
        print_tree_simple(resp.clone());
    } else {
        eprintln!("resp for current file system files: {}", resp);
    }
    resp
}

fn print_tree_simple(json: Value) {
    // TODO: fix there is some extra space
    // /
    // ├── private_test_folder
    //     │   └── shinkai_intro
    // └── shared test folder
    //         ├── crypto
    //         │   └── shinkai_intro
    //         └── shinkai_intro
    // eprintln!("print_tree_simple JSON: {}", json_str);
    // Parse the JSON string into a serde_json::Value

    eprintln!("/");
    if let Some(folders) = json["child_folders"].as_array() {
        let folders_len = folders.len();
        for (index, folder) in folders.iter().enumerate() {
            let folder_name = folder["name"].as_str().unwrap_or("Unknown Folder");
            let prefix = if index < folders_len - 1 {
                "├── "
            } else {
                "└── "
            };
            eprintln!("{}{}", prefix, folder_name);
            print_subtree(folder, "    ", index == folders_len - 1);
        }
    }
}

async fn check_subscription_success(
    commands_sender: &Sender<NodeCommand>,
    attempts: usize,
    delay_secs: u64,
    success_message: &str,
) -> bool {
    for _ in 0..attempts {
        tokio::time::sleep(Duration::from_secs(delay_secs)).await;
        let node2_last_messages = fetch_last_messages(commands_sender, 2)
            .await
            .expect("Failed to fetch last messages");

        eprintln!("Node 2 last messages: {:?}", node2_last_messages);

        for message in &node2_last_messages {
            if message
                .get_message_content()
                .expect("should work")
                .contains(success_message)
            {
                eprintln!("Subscription successful.");
                return true;
            }
        }
    }

    eprintln!("Subscription was not successful within the expected time frame.");
    false
}

fn print_subtree(folder: &serde_json::Value, indent: &str, is_last: bool) {
    let mut new_indent = String::from(indent);
    if !is_last {
        new_indent.push_str("│   ");
    } else {
        new_indent.push_str("    ");
    }

    // Create a longer-lived empty Vec that can be borrowed
    let empty_vec = vec![];

    // Use a reference to `empty_vec` instead of creating a temporary value inline
    let subfolders = folder["child_folders"].as_array().unwrap_or(&empty_vec);
    let items = folder["child_items"].as_array().unwrap_or(&empty_vec);

    let subfolders_len = subfolders.len();
    let total_len = subfolders_len + items.len();

    for (index, subfolder) in subfolders.iter().enumerate() {
        let subfolder_name = subfolder["name"].as_str().unwrap_or("Unknown Subfolder");
        let prefix = if index < subfolders_len - 1 || !items.is_empty() {
            "├── "
        } else {
            "└── "
        };
        eprintln!("{}{}{}", new_indent, prefix, subfolder_name);
        print_subtree(subfolder, &new_indent, index == total_len - 1);
    }

    for (index, item) in items.iter().enumerate() {
        let item_name = item["name"].as_str().unwrap_or("Unknown Item");
        let prefix = if index < items.len() - 1 {
            "├── "
        } else {
            "└── "
        };
        eprintln!("{}{}{}", new_indent, prefix, item_name);
    }
}

#[allow(clippy::too_many_arguments)]
async fn upload_file(
    commands_sender: &Sender<NodeCommand>,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
    folder_name: &str,
    file_path: &Path,
    symmetric_key_index: u32,
) {
    let symmetrical_sk = unsafe_deterministic_aes_encryption_key(symmetric_key_index);
    eprintln!("\n\n### Sending message (APICreateFilesInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

    let message_content = aes_encryption_key_to_string(symmetrical_sk);
    let msg = ShinkaiMessageBuilder::create_files_inbox_with_sym_key(
        encryption_sk.clone(),
        signature_sk.clone(),
        encryption_pk,
        "job::test::false".to_string(),
        message_content.clone(),
        profile_name.to_string(),
        identity_name.to_string(),
        identity_name.to_string(),
    )
    .unwrap();

    let (res_sender, res_receiver) = async_channel::bounded(1);
    commands_sender
        .send(NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res: res_sender })
        .await
        .unwrap();
    let _ = res_receiver.recv().await.unwrap().expect("Failed to receive messages");

    // Upload file
    let file_data = std::fs::read(&file_path)
        .map_err(|_| VRError::FailedPDFParsing)
        .unwrap();

    let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));
    let nonce = GenericArray::from_slice(&[0u8; 12]);
    let nonce_slice = nonce.as_slice();
    let nonce_str = aes_nonce_to_hex_string(nonce_slice);
    let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

    let (res_sender, res_receiver) = async_channel::bounded(1);
    commands_sender
        .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
            filename: file_path.to_string_lossy().to_string(),
            file: ciphertext,
            public_key: hash_of_aes_encryption_key_hex(symmetrical_sk),
            encrypted_nonce: nonce_str,
            res: res_sender,
        })
        .await
        .unwrap();
    let res = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("upload_file resp to inbox: {:?}", res);

    // Convert File and Save to Folder
    let payload = APIConvertFilesAndSaveToFolder {
        path: folder_name.to_string(),
        file_inbox: hash_of_aes_encryption_key_hex(symmetrical_sk),
        file_datetime: Some(Utc.ymd(2024, 2, 1).and_hms(0, 0, 0)),
    };

    let msg = generate_message_with_payload(
        serde_json::to_string(&payload).unwrap(),
        MessageSchemaType::ConvertFilesAndSaveToFolder,
        encryption_sk.clone(),
        signature_sk.clone(),
        encryption_pk,
        identity_name,
        profile_name,
        identity_name,
        profile_name
    );

    let (res_sender, res_receiver) = async_channel::bounded(1);
    commands_sender
        .send(NodeCommand::APIConvertFilesAndSaveToFolder { msg, res: res_sender })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("upload_file resp processed: {:?}", resp);
}

#[test]
fn subscription_manager_test() {
    std::env::set_var("WELCOME_MESSAGE", "false");
    init_default_tracing();
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.sepolia-shinkai";
        let node2_identity_name = "@@node2_test.sepolia-shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";
        let node2_profile_name = "main_profile_node2";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let _node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let node2_encryption_sk_clone = node2_encryption_sk.clone();

        let _node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);
        let _node2_identity_sk_clone = clone_signature_secret_key(&node2_identity_sk);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node2_profile_identity_sk, node2_profile_identity_pk) = unsafe_deterministic_signature_keypair(101);
        let (node2_profile_encryption_sk, node2_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(101);

        let node1_subencryption_sk_clone = node1_profile_encryption_sk.clone();
        let node2_subencryption_sk_clone = node2_profile_encryption_sk.clone();

        let _node1_subidentity_sk_clone = clone_signature_secret_key(&node1_profile_identity_sk);
        let _node2_subidentity_sk_clone = clone_signature_secret_key(&node2_profile_identity_sk);

        let (node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name));
        let node1_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node1_identity_name));
        let node2_db_path = format!("db_tests/{}", hash_string(node2_identity_name));
        let node2_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node2_identity_name));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk,
            0,
            node1_commands_receiver,
            node1_db_path,
            "".to_string(),
            true,
            vec![],
            None,
            node1_fs_db_path,
            None,
            None,
        )
        .await;

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let node2 = Node::new(
            node2_identity_name.to_string(),
            addr2,
            clone_signature_secret_key(&node2_identity_sk),
            node2_encryption_sk,
            0,
            node2_commands_receiver,
            node2_db_path,
            "".to_string(),
            true,
            vec![],
            None,
            node2_fs_db_path,
            None,
            None,
        )
        .await;

        // Printing
        eprintln!(
            "Node 1 encryption sk: {:?}",
            encryption_secret_key_to_string(node1_encryption_sk_clone2.clone())
        );
        eprintln!(
            "Node 1 encryption pk: {:?}",
            encryption_public_key_to_string(node1_encryption_pk)
        );

        eprintln!(
            "Node 2 encryption sk: {:?}",
            encryption_secret_key_to_string(node2_encryption_sk_clone)
        );
        eprintln!(
            "Node 2 encryption pk: {:?}",
            encryption_public_key_to_string(node2_encryption_pk)
        );

        eprintln!(
            "Node 1 identity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node1_identity_sk))
        );
        eprintln!(
            "Node 1 identity pk: {:?}",
            signature_public_key_to_string(node1_identity_pk)
        );

        eprintln!(
            "Node 2 identity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node2_identity_sk))
        );
        eprintln!(
            "Node 2 identity pk: {:?}",
            signature_public_key_to_string(node2_identity_pk)
        );

        eprintln!(
            "Node 1 subidentity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node1_profile_identity_sk))
        );
        eprintln!(
            "Node 1 subidentity pk: {:?}",
            signature_public_key_to_string(node1_profile_identity_pk)
        );

        eprintln!(
            "Node 2 subidentity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node2_profile_identity_sk))
        );
        eprintln!(
            "Node 2 subidentity pk: {:?}",
            signature_public_key_to_string(node2_profile_identity_pk)
        );

        eprintln!(
            "Node 1 subencryption sk: {:?}",
            encryption_secret_key_to_string(node1_subencryption_sk_clone.clone())
        );
        eprintln!(
            "Node 1 subencryption pk: {:?}",
            encryption_public_key_to_string(node1_profile_encryption_pk)
        );

        eprintln!(
            "Node 2 subencryption sk: {:?}",
            encryption_secret_key_to_string(node2_subencryption_sk_clone.clone())
        );
        eprintln!(
            "Node 2 subencryption pk: {:?}",
            encryption_public_key_to_string(node2_profile_encryption_pk)
        );

        eprintln!("Starting nodes");
        // Start node1 and node2
        let node1_clone = Arc::clone(&node1);
        let node1_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 1");
            let _ = node1_clone.lock().await.start().await;
        });

        let node1_abort_handler = node1_handler.abort_handle();

        let node2_clone = Arc::clone(&node2);
        let node2_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 2");
            let _ = node2_clone.lock().await.start().await;
        });
        let node2_abort_handler = node2_handler.abort_handle();

        let interactions_handler = tokio::spawn(async move {
            eprintln!("Starting interactions");
            eprintln!("Registration of Subidentities");

            // Register a Profile in Node1 and verifies it
            {
                eprintln!("Register a Device with main profile in Node1 and verify it");
                api_registration_device_node_profile_main(
                    node1_commands_sender.clone(),
                    node1_profile_name,
                    node1_identity_name,
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name,
                )
                .await;
            }

            // Register a Profile in Node2 and verifies it
            {
                eprintln!("Register a Profile in Node2 and verify it");
                local_registration_profile_node(
                    node2_commands_sender.clone(),
                    node2_profile_name,
                    node2_identity_name,
                    node2_subencryption_sk_clone.clone(),
                    node2_encryption_pk,
                    clone_signature_secret_key(&node2_profile_identity_sk),
                    1,
                )
                .await;
            }

            tokio::time::sleep(Duration::from_secs(3)).await;

            //
            // Creating a folder and uploading some files to the vector db
            //
            eprintln!("\n\n### Creating a folder and uploading some files to the vector db \n\n");
            // Send message (APICreateFilesInboxWithSymmetricKey) from Device subidentity to Node 1
            {
                // Create /shared test folder
                create_folder(
                    &node1_commands_sender,
                    "/",
                    "shared test folder",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                // Create /private_test_folder
                create_folder(
                    &node1_commands_sender,
                    "/",
                    "private_test_folder",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
            }
            {
                // Retrieve info
                retrieve_file_info(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    "/",
                    true,
                )
                .await;
            }
            {
                // Upload File to /private_test_folder
                let file_path = Path::new("files/shinkai_intro.vrkai");
                upload_file(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    "/private_test_folder",
                    file_path,
                    0,
                )
                .await;

                // Upload File to /shared test folder
                let file_path = Path::new("files/shinkai_intro.vrkai");
                upload_file(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    "/shared test folder",
                    file_path,
                    0,
                )
                .await;
            }
            {
                // Retrieve info
                retrieve_file_info(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    "/",
                    true,
                )
                .await;
            }
            {
                // Show available shared items
                eprintln!("Show available shared items before making /shared test folder shareable");
                show_available_shared_items(
                    node1_identity_name,
                    node1_profile_name,
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
            }
            {
                // Create /shared test folder/crypto
                create_folder(
                    &node1_commands_sender,
                    "/shared test folder",
                    "crypto",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                // Upload File to /shared test folder/crypto
                let file_path = Path::new("files/shinkai_intro.vrkai");
                upload_file(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    "/shared test folder/crypto",
                    file_path,
                    0,
                )
                .await;

                tokio::time::sleep(Duration::from_secs(2)).await;

                {
                    // Retrieve info
                    retrieve_file_info(
                        &node1_commands_sender,
                        node1_profile_encryption_sk.clone(),
                        clone_signature_secret_key(&node1_profile_identity_sk),
                        node1_encryption_pk,
                        node1_identity_name,
                        node1_profile_name,
                        "/",
                        true,
                    )
                    .await;
                }
            }
            {
                // Make /shared test folder shareable
                eprintln!("Make /shared test folder shareable");
                make_folder_shareable(
                    &node1_commands_sender,
                    "/shared test folder",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
                {
                    eprintln!("### Retrieve info");
                    // Retrieve info
                    retrieve_file_info(
                        &node1_commands_sender,
                        node1_profile_encryption_sk.clone(),
                        clone_signature_secret_key(&node1_profile_identity_sk),
                        node1_encryption_pk,
                        node1_identity_name,
                        node1_profile_name,
                        "/",
                        true,
                    )
                    .await;
                }
            }
            {
                // Show available shared items
                eprintln!("Show available shared items after making /shared test folder shareable");
                show_available_shared_items(
                    node1_identity_name,
                    node1_profile_name,
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
            }
            //
            // Second Part of the Test
            //
            //      _   _      _                      _
            //     | \ | |    | |                    | |
            //     |  \| | ___| |___      _____  _ __| | __
            //     | . ` |/ _ \ __\ \ /\ / / _ \| '__| |/ /
            //     | |\  |  __/ |_ \ V  V / (_) | |  |   <
            //     |_| \_|\___|\__| \_/\_/ \___/|_|  |_|\_\
            //
            //
            {
                // Remove this after the other stuff is working
                eprintln!("\n\n### Sending message from node 2 to node 1 requesting shared folders*\n");

                let unchanged_message = ShinkaiMessageBuilder::vecfs_available_shared_items(
                    None,
                    node1_identity_name.to_string(),
                    node1_profile_name.to_string(),
                    node2_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node2_profile_identity_sk),
                    node2_encryption_pk,
                    node2_identity_name.to_string().clone(),
                    node2_profile_name.to_string().clone(),
                    node2_identity_name.to_string(),
                    node2_profile_name.to_string().clone(),
                )
                .unwrap();

                // eprintln!("\n\n unchanged message: {:?}", unchanged_message);

                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<Value, APIError>>,
                    async_channel::Receiver<Result<Value, APIError>>,
                ) = async_channel::bounded(1);

                node2_commands_sender
                    .send(NodeCommand::APIAvailableSharedItems {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();

                let send_result = res_send_msg_receiver.recv().await.unwrap();
                eprint!("send_result: {:?}", send_result);
                assert!(send_result.is_ok(), "Failed to get APIAvailableSharedItems");
                // tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

                let node2_last_messages = fetch_last_messages(&node2_commands_sender, 2)
                    .await
                    .expect("Failed to fetch last messages for node 2");

                eprintln!("Node 2 last messages: {:?}", node2_last_messages);
                eprintln!("\n\n");

                let node1_last_messages = fetch_last_messages(&node1_commands_sender, 2)
                    .await
                    .expect("Failed to fetch last messages for node 1");

                eprintln!("\n\nNode 1 last messages: {:?}", node1_last_messages);
                eprintln!("\n\n");
            }
            {
                // add 1 sec delay
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                eprintln!("\n\n### (RETRY!) Sending message from node 2 to node 1 requesting shared folders*\n");
                eprintln!("shared folders should be updated this time!");

                let unchanged_message = ShinkaiMessageBuilder::vecfs_available_shared_items(
                    None,
                    node1_identity_name.to_string(),
                    node1_profile_name.to_string(),
                    node2_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node2_profile_identity_sk),
                    node2_encryption_pk,
                    node2_identity_name.to_string().clone(),
                    node2_profile_name.to_string().clone(),
                    node2_identity_name.to_string(),
                    node2_profile_name.to_string().clone(),
                )
                .unwrap();

                // eprintln!("\n\n unchanged message: {:?}", unchanged_message);

                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<Value, APIError>>,
                    async_channel::Receiver<Result<Value, APIError>>,
                ) = async_channel::bounded(1);

                node2_commands_sender
                    .send(NodeCommand::APIAvailableSharedItems {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();

                let send_result = res_send_msg_receiver.recv().await.unwrap();
                eprint!("\n\nsend_result (after retry): {:?}", send_result);

                let mut expected_response = serde_json::json!({
                    "node_name": "@@node1_test.sepolia-shinkai/main",
                    "last_ext_node_response": "2024-03-24T00:47:22.292345Z",
                    "last_request_to_ext_node": "2024-03-24T00:47:22.292346Z",
                    "last_updated": "2024-03-24T00:47:22.292346Z",
                    "state": "ResponseAvailable",
                    "response_last_updated": "2024-03-24T00:47:22.292347Z",
                    "response": {
                        "/shared test folder": {
                            "path": "/shared test folder",
                            "permission": "Public",
                            "profile": "main",
                            "tree": {
                                "name": "/",
                                "path": "/shared test folder",
                                "last_modified": "2024-03-24T00:47:20.713156+00:00",
                                "children": {
                                    "crypto": {
                                        "name": "crypto",
                                        "path": "/shared test folder/crypto",
                                        "last_modified": "2024-03-24T00:47:18.657987+00:00",
                                        "children": {
                                            "shinkai_intro": {
                                                "name": "shinkai_intro",
                                                "path": "/shared test folder/crypto/shinkai_intro",
                                                "last_modified": "2024-02-26T23:06:00.019065981+00:00",
                                                "children": {}
                                            }
                                        }
                                    },
                                    "shinkai_intro": {
                                        "name": "shinkai_intro",
                                        "path": "/shared test folder/shinkai_intro",
                                        "last_modified": "2024-02-26T23:06:00.019065981+00:00",
                                        "children": {}
                                    }
                                }
                            },
                            "subscription_requirement": {
                                "minimum_token_delegation": 100,
                                "minimum_time_delegated_hours": 100,
                                "monthly_payment": {
                                    "USD": 10.0
                                },
                                "has_web_alternative": false,
                                "is_free": false,
                                "folder_description": "This is a test folder"
                            }
                        }
                    }
                });

                let mut actual_response: serde_json::Value = send_result.clone().unwrap();

                // Remove timestamps from both expected and actual responses using the new function
                remove_timestamps_from_shared_folder_cache_response(&mut expected_response);
                remove_timestamps_from_shared_folder_cache_response(&mut actual_response);

                // Perform the assertion
                assert_eq!(
                    actual_response, expected_response,
                    "Failed to match the expected shared folder information"
                );
                assert!(send_result.is_ok(), "Failed to get APIAvailableSharedItems");
            }
            {
                eprintln!(">>> Subscribe to the shared folder");
                eprintln!(
                    "\n\n### Sending message from node 2 to node 1 requesting: subscription to shared test folder\n"
                );
                let requirements = SubscriptionPayment::Free;

                let unchanged_message = ShinkaiMessageBuilder::vecfs_subscribe_to_shared_folder(
                    "/shared test folder".to_string(),
                    requirements,
                    node1_identity_name.to_string(),
                    node1_profile_name.to_string(),
                    node2_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node2_profile_identity_sk),
                    node2_encryption_pk,
                    node2_identity_name.to_string().clone(),
                    node2_profile_name.to_string().clone(),
                    node2_identity_name.to_string(),
                    "".to_string(),
                )
                .unwrap();

                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<String, APIError>>,
                    async_channel::Receiver<Result<String, APIError>>,
                ) = async_channel::bounded(1);

                node2_commands_sender
                    .send(NodeCommand::APISubscribeToSharedFolder {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();

                let send_result = res_send_msg_receiver.recv().await.unwrap();
                eprint!("\n\nsend_result: {:?}", send_result);

                let subscription_success_message = "{\"subscription_details\":\"Subscribed to /shared test folder\",\"shared_folder\":\"/shared test folder\",\"status\":\"Success\",\"error\":null,\"metadata\":null}";
                let subscription_success = check_subscription_success(
                    &node2_commands_sender,
                    4, // attempts
                    2, // delay_secs
                    subscription_success_message,
                )
                .await;
                assert!(subscription_success, "Failed to subscribe to shared folder");
            }
            {
                let msg = ShinkaiMessageBuilder::my_subscriptions(
                    node2_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node2_profile_identity_sk),
                    node2_encryption_pk,
                    node2_identity_name.to_string().clone(),
                    node2_profile_name.to_string().clone(),
                    node2_identity_name.to_string(),
                    "".to_string(),
                ).unwrap();
            
                // Prepare the response channel
                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<Value, APIError>>,
                    async_channel::Receiver<Result<Value, APIError>>,
                ) = async_channel::bounded(1);
            
                // Send the command
                node2_commands_sender
                .send(NodeCommand::APIMySubscriptions {
                    msg,
                    res: res_send_msg_sender,
                })
                .await
                .unwrap();

                let mut actual_resp_json = res_send_msg_receiver.recv().await.unwrap().expect("Failed to receive response");    
                
                // Expected response template without dates for comparison
                let expected_resp_template = r#"[{
                    "subscription_id": {
                        "unique_id": "@@node1_test.sepolia-shinkai:::main:::/shared test folder:::@@node2_test.sepolia-shinkai:::main_profile_node2",
                        "exclude_folders": null,
                        "include_folders": null,
                        "http_upload_destination": null
                    },
                    "shared_folder": "/shared test folder",
                    "streaming_node": "@@node1_test.sepolia-shinkai",
                    "streaming_profile": "main",
                    "subscriber_node": "@@node2_test.sepolia-shinkai",
                    "subscriber_profile": "main_profile_node2",
                    "payment": "Free",
                    "state": "SubscriptionConfirmed",
                    "subscriber_destination_path": null,
                    "subscription_description": null
                }]"#;
                let mut expected_resp_json: serde_json::Value = serde_json::from_str(expected_resp_template).expect("Failed to parse expected JSON");

                // Remove dates from the actual response for comparison
                if let Some(array) = actual_resp_json.as_array_mut() {
                    for item in array.iter_mut() {
                        if let Some(obj) = item.as_object_mut() {
                            obj.remove("date_created");
                            obj.remove("last_modified");
                            obj.remove("last_sync");
                        }
                    }
                }

                // Assert that the modified actual response matches the expected response template
                assert_eq!(actual_resp_json, expected_resp_json, "The response does not match the expected subscriptions response without dates.");
            }
            {
                eprintln!("Trigger External Manager Subscription Review in Node 1 (Streamer)");
                {
                    
                    let (res_send_msg_sender, res_send_msg_receiver): (
                        async_channel::Sender<Result<(), String>>,
                        async_channel::Receiver<Result<(), String>>,
                    ) = async_channel::bounded(1);

                    node1_commands_sender
                        .send(NodeCommand::LocalExtManagerProcessSubscriptionUpdates {
                            res: res_send_msg_sender,
                        })
                        .await
                        .unwrap(); 

                    res_send_msg_receiver.recv().await.unwrap().expect("Failed to receive response");
                }

                eprintln!("Send updates to subscribers");
                let mut attempts = 0;
                let max_attempts = 100;
                let mut structure_matched = false;

                while attempts < max_attempts && !structure_matched {
                    
                    eprintln!("\n\n### (Send updates to subscribers) Sending message from node 2's identity to node 2 to check if the subscription synced\n");

                    let payload = APIVecFsRetrievePathSimplifiedJson { path: "/".to_string() };
                    let msg = generate_message_with_payload(
                        serde_json::to_string(&payload).unwrap(),
                        MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                        node2_profile_encryption_sk.clone(),
                        clone_signature_secret_key(&node2_profile_identity_sk),
                        node2_encryption_pk,
                        &node2_identity_name.to_string().clone(),
                        &node2_profile_name.to_string().clone(),
                        node2_identity_name,
                        "",
                    );

                    // Prepare the response channel
                    let (res_sender, res_receiver) = async_channel::bounded(1);

                    // Send the command
                    node2_commands_sender
                        .send(NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res: res_sender })
                        .await
                        .unwrap();
                    let actual_resp_json = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                    print_tree_simple(actual_resp_json.clone());

                   let expected_structure = serde_json::json!({
                        "path": "/",
                        "child_folders": [
                            {
                                "name": "shared test folder",
                                "path": "/shared test folder",
                                "child_folders": [
                                    {
                                        "name": "crypto",
                                        "path": "/shared test folder/crypto",
                                        "child_folders": [],
                                        "child_items": [
                                            {
                                                "name": "shinkai_intro",
                                                "path": "/shared test folder/crypto/shinkai_intro"
                                            }
                                        ]
                                    }
                                ],
                                "child_items": [
                                    {
                                        "name": "shinkai_intro",
                                        "path": "/shared test folder/shinkai_intro"
                                    }
                                ]
                            }
                        ],
                        "child_items": []
                    });

                    structure_matched = check_structure(&actual_resp_json, &expected_structure);
                    if structure_matched {
                        eprintln!("The actual folder structure matches the expected structure.");
                        break;
                    } else {
                        eprintln!("The actual folder structure does not match the expected structure. Retrying...");
                        eprintln!("Actual structure: {:?}", actual_resp_json);
                    }
                    attempts += 1;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                assert!(structure_matched, "The actual folder structure does not match the expected structure after all attempts.");
            }
            {
                eprintln!("Add a new file to the streamer");
                 // Create /shared test folder/zeko
                 create_folder(
                    &node1_commands_sender,
                    "/shared test folder",
                    "zeko",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                // Create /shared test folder/zeko/paper
                create_folder(
                    &node1_commands_sender,
                    "/shared test folder/zeko",
                    "paper",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                // Upload File to /shared test folder/crypto
                let file_path = Path::new("files/zeko.vrkai");
                upload_file(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    "/shared test folder/zeko/paper",
                    file_path,
                    0,
                )
                .await;
            }
            {
                eprintln!("Check that new file was received");
                let mut attempts = 0;
                let max_attempts = 20;
                let mut structure_matched = false;

                eprintln!("Trigger External Manager Subscription Review in Node 1 (Streamer)");
                {
                    
                    let (res_send_msg_sender, res_send_msg_receiver): (
                        async_channel::Sender<Result<(), String>>,
                        async_channel::Receiver<Result<(), String>>,
                    ) = async_channel::bounded(1);

                    node1_commands_sender
                        .send(NodeCommand::LocalExtManagerProcessSubscriptionUpdates {
                            res: res_send_msg_sender,
                        })
                        .await
                        .unwrap(); 

                    res_send_msg_receiver.recv().await.unwrap().expect("Failed to receive response");
                }

                while attempts < max_attempts && !structure_matched {
                    
                    eprintln!("\n\n### (Check that new file was received) Sending message from node 2's identity to node 2 to check if the subscription synced\n");

                    let payload = APIVecFsRetrievePathSimplifiedJson { path: "/".to_string() };
                    let msg = generate_message_with_payload(
                        serde_json::to_string(&payload).unwrap(),
                        MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                        node2_profile_encryption_sk.clone(),
                        clone_signature_secret_key(&node2_profile_identity_sk),
                        node2_encryption_pk,
                        &node2_identity_name.to_string().clone(),
                        &node2_profile_name.to_string().clone(),
                        node2_identity_name,
                        "",
                    );

                    // Prepare the response channel
                    let (res_sender, res_receiver) = async_channel::bounded(1);

                    // Send the command
                    node2_commands_sender
                        .send(NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res: res_sender })
                        .await
                        .unwrap();
                    let actual_resp_json = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                    print_tree_simple(actual_resp_json.clone());

                   let expected_structure = serde_json::json!({
                        "path": "/",
                        "child_folders": [
                            {
                                "name": "shared test folder",
                                "path": "/shared test folder",
                                "child_folders": [ 
                                    {
                                        "name": "zeko",
                                        "path": "/shared test folder/zeko",
                                        "child_folders": [
                                            {
                                                "name": "paper",
                                                "path": "/shared test folder/zeko/paper",
                                                "child_folders": [],
                                                "child_items": [
                                                    {
                                                        "name": "Zeko_Mina_Rollup",
                                                        "path": "/shared test folder/zeko/paper/Zeko_Mina_Rollup"
                                                    },
                                                ]
                                            }
                                        ],
                                        "child_items": []
                                    },
                                    {
                                        "name": "crypto",
                                        "path": "/shared test folder/crypto",
                                        "child_folders": [],
                                        "child_items": [
                                            {
                                                "name": "shinkai_intro",
                                                "path": "/shared test folder/crypto/shinkai_intro"
                                            },
                                        ]
                                    }
                                ],
                                "child_items": [
                                    {
                                        "name": "shinkai_intro",
                                        "path": "/shared test folder/shinkai_intro"
                                    }
                                ]
                            }
                        ],
                        "child_items": []
                    });

                    eprintln!("\n\nExpected structure: {:?}", expected_structure);
                    eprintln!("-");
                    eprintln!("Actual structure: {:?}", actual_resp_json);
                    structure_matched = check_structure(&actual_resp_json, &expected_structure);
                    if structure_matched {
                        eprintln!("The actual folder structure matches the expected structure.");
                        break;
                    } else {
                        eprintln!("The actual folder structure does not match the expected structure. Retrying...");
                        eprintln!("Actual structure: {:?}", actual_resp_json);
                    }
                    attempts += 1;
                    tokio::time::sleep(Duration::from_secs(4)).await;
                }
                assert!(structure_matched, "The actual folder structure does not match the expected structure after all attempts.");
            }
            {
                eprintln!("Removing a file from the streamer");

                // Create /shared test folder/zeko/paper
                remove_item(
                    &node1_commands_sender,
                    "/shared test folder/zeko/paper/Zeko_Mina_Rollup",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                // Remove /shared test folder/zeko/paper
                 
                remove_folder(
                    &node1_commands_sender,
                    "/shared test folder/zeko/paper",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                // Remove /shared test folder/zeko/paper
                remove_folder(
                    &node1_commands_sender,
                    "/shared test folder/zeko",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;             

                // force cache update
                show_available_shared_items(
                    node1_identity_name,
                    node1_profile_name,
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
            }
            {
                eprintln!("Check that removed file were updated");
                let mut attempts = 0;
                let max_attempts = 15;
                let mut structure_matched = false;

                {
                    let (res_send_msg_sender, res_send_msg_receiver): (
                        async_channel::Sender<Result<(), String>>,
                        async_channel::Receiver<Result<(), String>>,
                    ) = async_channel::bounded(1);

                    node1_commands_sender
                        .send(NodeCommand::LocalExtManagerProcessSubscriptionUpdates {
                            res: res_send_msg_sender,
                        })
                        .await
                        .unwrap(); 

                    res_send_msg_receiver.recv().await.unwrap().expect("Failed to receive response"); 
                }

                while attempts < max_attempts && !structure_matched {
                    
                    eprintln!("\n\n### (Check that structure with removed folder (zeko) was updated) Sending message from node 2's identity to node 2 to check if the subscription synced\n");

                    let payload = APIVecFsRetrievePathSimplifiedJson { path: "/".to_string() };
                    let msg = generate_message_with_payload(
                        serde_json::to_string(&payload).unwrap(),
                        MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                        node2_profile_encryption_sk.clone(),
                        clone_signature_secret_key(&node2_profile_identity_sk),
                        node2_encryption_pk,
                        &node2_identity_name.to_string().clone(),
                        &node2_profile_name.to_string().clone(),
                        node2_identity_name,
                        "",
                    );

                    // Prepare the response channel
                    let (res_sender, res_receiver) = async_channel::bounded(1);

                    // Send the command
                    node2_commands_sender
                        .send(NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res: res_sender })
                        .await
                        .unwrap();
                    let actual_resp_json = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                    print_tree_simple(actual_resp_json.clone());

                   let expected_structure = serde_json::json!({
                        "path": "/",
                        "child_folders": [
                            {
                                "name": "shared test folder",
                                "path": "/shared test folder",
                                "child_folders": [
                                    {
                                        "name": "crypto",
                                        "path": "/shared test folder/crypto",
                                        "child_folders": [],
                                        "child_items": [
                                            {
                                                "name": "shinkai_intro",
                                                "path": "/shared test folder/crypto/shinkai_intro"
                                            },
                                        ]
                                    }
                                ],
                                "child_items": [
                                    {
                                        "name": "shinkai_intro",
                                        "path": "/shared test folder/shinkai_intro"
                                    }
                                ]
                            }
                        ],
                        "child_items": []
                    });

                    structure_matched = check_structure(&actual_resp_json, &expected_structure);
                    if structure_matched {
                        eprintln!("The actual folder structure matches the expected structure.");
                        break;
                    } else {
                        eprintln!("The actual folder structure does not match the expected structure. Retrying...");
                        // eprintln!("Actual structure: {:?}", actual_resp_json);
                    }
                    attempts += 1;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                assert!(structure_matched, "The actual folder structure does not match the expected structure after all attempts.");
            }
            {
                 // check that the unsubscription was processed in the other node
                 eprintln!("Check current subscribers for node 1");
                 let unchanged_message = ShinkaiMessageBuilder::get_my_subscribers(
                     None,
                     node1_profile_encryption_sk.clone(),
                     clone_signature_secret_key(&node1_profile_identity_sk),
                     node1_encryption_pk,
                     node1_identity_name.to_string().clone(),
                     node1_profile_name.to_string().clone(),
                     node1_identity_name.to_string(),
                     node1_profile_name.to_string(),
                 ).unwrap(); 
 
                 let (res_send_msg_sender, res_send_msg_receiver): (
                     async_channel::Sender<Result<HashMap<String, Vec<ShinkaiSubscription>>, APIError>>,
                     async_channel::Receiver<Result<HashMap<String, Vec<ShinkaiSubscription>>, APIError>>,
                 ) = async_channel::bounded(1);
 
                 node1_commands_sender
                    .send(NodeCommand::APIGetMySubscribers {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();
 
                    let send_result = res_send_msg_receiver.recv().await.unwrap();
                    // eprint!("\n\nsend_result subscribers: {:?}", send_result);
                     
                    // Assuming send_result is Ok, directly access the HashMap for comparison
                    let mut actual_subscriptions = send_result.expect("Failed to get subscribers");

                    // Prepare the expected subscriptions for comparison
                    let mut expected_subscriptions = HashMap::from([
                        ("/shared test folder".to_string(), vec![
                            ShinkaiSubscription::new(
                                "/shared test folder".to_string(),
                                ShinkaiName::new("@@node1_test.sepolia-shinkai".to_string()).unwrap(),
                                "main".to_string(),
                                ShinkaiName::new("@@node2_test.sepolia-shinkai".to_string()).unwrap(),
                                "main_profile_node2".to_string(),
                                ShinkaiSubscriptionStatus::SubscriptionConfirmed,
                                Some(SubscriptionPayment::Free),
                            )
                        ])
                    ]);

                    let dummy_date: DateTime<Utc> = Utc.ymd(2000, 1, 1).and_hms(0, 0, 0);

                    // Remove date fields from both actual and expected subscriptions for comparison
                    for subscriptions in actual_subscriptions.values_mut() {
                        for subscription in subscriptions {
                            subscription.date_created = dummy_date;
                            subscription.last_modified = dummy_date;
                            subscription.last_sync = None;
                        }
                    }

                    for subscriptions in expected_subscriptions.values_mut() {
                        for subscription in subscriptions {
                            subscription.date_created = dummy_date;
                            subscription.last_modified = dummy_date;
                            subscription.last_sync = None;
                        }
                    }

                    // Compare the actual subscriptions with the expected ones
                    assert_eq!(actual_subscriptions, expected_subscriptions, "The actual subscriptions do not match the expected ones.");
            }
            {
                // Unsubscribe from the shared folder
                eprintln!("\n\nUnsubscribe from the shared folder");
                let unchanged_message = ShinkaiMessageBuilder::vecfs_unsubscribe_to_shared_folder(
                    "/shared test folder".to_string(),
                    node1_identity_name.to_string(),
                    node1_profile_name.to_string(),
                    node2_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node2_profile_identity_sk),
                    node2_encryption_pk,
                    node2_identity_name.to_string().clone(),
                    node2_profile_name.to_string().clone(),
                    node2_identity_name.to_string(),
                    "".to_string(),
                )
                .unwrap();

                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<String, APIError>>,
                    async_channel::Receiver<Result<String, APIError>>,
                ) = async_channel::bounded(1);

                node2_commands_sender
                    .send(NodeCommand::APIUnsubscribe {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();

                    let send_result = res_send_msg_receiver.recv().await.unwrap();
                    // eprint!("\n\nsend_result unsubscribe: {:?}", send_result);
                    assert!(matches!(send_result, Ok(ref s) if s == "Unsubscribed"), "Expected to unsubscribe successfully.");
            }
            {
                // check that the unsubscription was processed in the other node
                eprintln!("\n\nCheck that the unsubscribe was processed in the node 1");
                // add two second delay
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                let unchanged_message = ShinkaiMessageBuilder::get_my_subscribers(
                    None,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.to_string().clone(),
                    node1_profile_name.to_string().clone(),
                    node1_identity_name.to_string(),
                    "".to_string(),
                ).unwrap(); 

                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<HashMap<String, Vec<ShinkaiSubscription>>, APIError>>,
                    async_channel::Receiver<Result<HashMap<String, Vec<ShinkaiSubscription>>, APIError>>,
                ) = async_channel::bounded(1);

                node1_commands_sender
                    .send(NodeCommand::APIGetMySubscribers {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();

                    let send_result = res_send_msg_receiver.recv().await.unwrap().expect("Failed to receive response");

                    // Assert that the response is empty, indicating no subscriptions
                    let expected_resp: HashMap<String, Vec<ShinkaiSubscription>> = HashMap::new();
                    assert_eq!(send_result, expected_resp, "Expected no subscriptions, but found some.");
            }
            {
                // unshare folder
                let msg = ShinkaiMessageBuilder::subscriptions_unshare_folder(
                    "/shared test folder".to_string(),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.to_string().clone(),
                    node1_profile_name.to_string().clone(),
                    node1_identity_name.to_string(),
                    "".to_string(),
                ).unwrap();
            
                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);
            
                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIUnshareFolder { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("unshare folder resp: {:?}", resp);    

                show_available_shared_items(
                    node1_identity_name,
                    node1_profile_name,
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
            }
            {
                // eprintln!("Get All Inboxes for Profile Node1");
                // TODO: modify to see your messages
                // TODO: check that inboxes are being deleted after uploading a file and converting it
                // let full_profile = format!("{}/{}", node1_identity_name.clone(), node1_profile_name.clone());
                // let msg = ShinkaiMessageBuilder::get_all_inboxes_for_profile(
                //   clone_static_secret_key(&node1_profile_encryption_sk),
                //   clone_signature_secret_key(&node1_profile_identity_sk),
                //   node1_encryption_pk.clone(),
                //   full_profile.clone().to_string(),
                //   node1_profile_name.clone().to_string(),
                //   node1_identity_name.clone().to_string(),
                //   node1_identity_name.clone().to_string(),
                // )
                // .unwrap();

                // let (res2_sender, res2_receiver) = async_channel::bounded(1);
                //   node1_commands_sender
                //       .send(NodeCommand::APIGetAllInboxesForProfile { msg, res: res2_sender })
                //       .await
                //       .unwrap();
                // let node2_last_messages = res2_receiver.recv().await.unwrap().expect("Failed to receive messages");
                // eprintln!("node1_all_profiles: {:?}", node2_last_messages);

                // let inboxes = api_get_all_smart_inboxes_from_profile(
                //     node1_commands_sender.clone(),
                //     clone_static_secret_key(&node1_profile_encryption_sk),
                //     node1_encryption_pk.clone(),
                //     clone_signature_secret_key(&node1_profile_identity_sk),
                //     node1_identity_name.clone(),
                //     node1_profile_name.clone(),
                //     node1_identity_name.clone(),
                // )
                // .await;
                // eprintln!("node1_all_profiles smart inboxes: {:?}", inboxes);

                // let inboxes = api_get_all_smart_inboxes_from_profile(
                //     node2_commands_sender.clone(),
                //     clone_static_secret_key(&node2_profile_encryption_sk),
                //     node2_encryption_pk.clone(),
                //     clone_signature_secret_key(&node2_profile_identity_sk),
                //     node2_identity_name.clone(),
                //     node2_profile_name.clone(),
                //     node2_identity_name.clone(),
                // )
                // .await;
                // eprintln!("node2_all_profiles smart inboxes: {:?}", inboxes);
            }
            {
                // Dont forget to do this at the end
                node1_abort_handler.abort();
                node2_abort_handler.abort();
            }
        });

        // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, node2_handler, interactions_handler);
        match result {
            Ok(_) => {}
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    eprintln!("One of the tasks was aborted, but this is expected.");
                } else {
                    // If the error is not due to an abort, then it's unexpected
                    panic!("An unexpected error occurred: {:?}", e);
                }
            }
        }
    });

    rt.shutdown_background();
}

