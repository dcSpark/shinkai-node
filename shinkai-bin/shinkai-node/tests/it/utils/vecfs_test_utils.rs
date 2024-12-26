use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_channel::Sender;
use chrono::{TimeZone, Utc};
use ed25519_dalek::SigningKey;
use rust_decimal::Decimal;
use serde_json::Value;

use shinkai_fs::shinkai_fs_error::ShinkaiFsError;
use shinkai_message_primitives::schemas::shinkai_subscription_req::FolderSubscription;
use shinkai_message_primitives::schemas::shinkai_subscription_req::PaymentOption;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIAvailableSharedItems, APIConvertFilesAndSaveToFolder, APICreateShareableFolder, APIVecFsCreateFolder,
    APIVecFsDeleteFolder, APIVecFsDeleteItem, APIVecFsRetrievePathSimplifiedJson, FileDestinationCredentials,
    MessageSchemaType,
};
use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_http_api::node_api_router::APIError;

use std::path::Path;
use std::time::Duration;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub fn print_tree_simple(json: Value) {
    // TODO: fix there is some extra space
    // /
    // ├── private_test_folder
    //     │   └── shinkai_intro
    // └── shared_test_folder
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

pub async fn remove_folder(
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

pub async fn remove_item(
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

#[allow(clippy::too_many_arguments)]
pub async fn retrieve_file_info(
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

#[allow(clippy::too_many_arguments)]
pub fn generate_message_with_payload<T: ToString>(
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

    ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
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
        .unwrap()
}

// Function to recursively check if the actual response contains the expected structure
pub fn check_structure(actual: &Value, expected: &Value) -> bool {
    if let (Some(mut actual_folders), Some(mut expected_folders)) = (
        actual["child_folders"].as_array().cloned(),
        expected["child_folders"].as_array().cloned(),
    ) {
        if actual_folders.len() != expected_folders.len() {
            eprintln!("Folder count mismatch: expected {}, found {}", expected_folders.len(), actual_folders.len());
            return false;
        }
        sort_folders(&mut actual_folders);
        sort_folders(&mut expected_folders);
        for (actual_folder, expected_folder) in actual_folders.iter().zip(expected_folders.iter()) {
            if !check_folder(actual_folder, expected_folder) {
                return false;
            }
        }
    } else {
        eprintln!("Expected and actual folders structure mismatch");
        return false;
    }
    true
}

pub fn sort_folders(folders: &mut [Value]) {
    folders.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
}

pub fn sort_items(items: &mut [Value]) {
    items.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
}

pub fn check_folder(actual_folder: &Value, expected_folder: &Value) -> bool {
    let actual_name = actual_folder["name"].as_str().unwrap_or("Unknown Folder");
    let expected_name = expected_folder["name"].as_str().unwrap_or("Unknown Folder");
    if actual_name != expected_name {
        eprintln!("Folder name mismatch: expected '{}', found '{}'", expected_name, actual_name);
        return false;
    }

    let actual_path = actual_folder["path"].as_str().unwrap_or("Unknown Path");
    let expected_path = expected_folder["path"].as_str().unwrap_or("Unknown Path");
    if actual_path != expected_path {
        eprintln!("Folder path mismatch: expected '{}', found '{}'", expected_path, actual_path);
        return false;
    }

    let mut actual_subfolders = actual_folder["child_folders"].as_array().unwrap_or(&vec![]).to_vec();
    let mut expected_subfolders = expected_folder["child_folders"].as_array().unwrap_or(&vec![]).to_vec();
    if actual_subfolders.len() != expected_subfolders.len() {
        eprintln!("Subfolder count mismatch in '{}': expected {}, found {}", actual_name, expected_subfolders.len(), actual_subfolders.len());
        return false;
    }
    sort_folders(&mut actual_subfolders);
    sort_folders(&mut expected_subfolders);
    for (actual_subfolder, expected_subfolder) in actual_subfolders.iter().zip(expected_subfolders.iter()) {
        if !check_folder(actual_subfolder, expected_subfolder) {
            return false;
        }
    }

    let mut actual_items = actual_folder["child_items"].as_array().unwrap_or(&vec![]).to_vec();
    let mut expected_items = expected_folder["child_items"].as_array().unwrap_or(&vec![]).to_vec();
    if actual_items.len() != expected_items.len() {
        eprintln!("Item count mismatch in '{}': expected {}, found {}", actual_name, expected_items.len(), actual_items.len());
        return false;
    }
    sort_items(&mut actual_items);
    sort_items(&mut expected_items);
    for (actual_item, expected_item) in actual_items.iter().zip(expected_items.iter()) {
        if !check_item(actual_item, expected_item) {
            return false;
        }
    }

    true
}

pub fn check_item(actual_item: &Value, expected_item: &Value) -> bool {
    let actual_name = actual_item["name"].as_str().unwrap_or("Unknown Item");
    let expected_name = expected_item["name"].as_str().unwrap_or("Unknown Item");
    if actual_name != expected_name {
        eprintln!("Item name mismatch: expected '{}', found '{}'", expected_name, actual_name);
        return false;
    }

    let actual_path = actual_item["path"].as_str().unwrap_or("Unknown Path");
    let expected_path = expected_item["path"].as_str().unwrap_or("Unknown Path");
    if actual_path != expected_path {
        eprintln!("Item path mismatch: expected '{}', found '{}'", expected_path, actual_path);
        return false;
    }

    true
}

pub async fn fetch_last_messages(
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

#[allow(clippy::too_many_arguments)]
pub async fn make_folder_shareable(
    commands_sender: &Sender<NodeCommand>,
    folder_path: &str,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
    credentials: Option<FileDestinationCredentials>,
) {
    let has_web_alternative = credentials.is_some();
    let payload = APICreateShareableFolder {
        path: folder_path.to_string(),
        subscription_req: FolderSubscription {
            minimum_token_delegation: Some(100),
            minimum_time_delegated_hours: Some(100),
            monthly_payment: Some(PaymentOption::USD(Decimal::new(1000, 2))), // Represents 10.00
            is_free: false,
            has_web_alternative: Some(has_web_alternative),
            folder_description: "This is a test folder".to_string(),
        },
        credentials,
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
pub async fn make_folder_shareable_http_free(
    commands_sender: &Sender<NodeCommand>,
    folder_path: &str,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
    credentials: Option<FileDestinationCredentials>,
) {
    let payload = APICreateShareableFolder {
        path: folder_path.to_string(),
        subscription_req: FolderSubscription {
            minimum_token_delegation: None,
            minimum_time_delegated_hours: None,
            monthly_payment: None,
            is_free: true,
            has_web_alternative: Some(true),
            folder_description: "This is a test folder".to_string(),
        },
        credentials,
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
pub async fn show_available_shared_items(
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
pub async fn create_folder(
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

pub fn remove_timestamps_from_shared_folder_cache_response(value: &mut serde_json::Value) {
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

pub async fn check_subscription_success(
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

pub fn print_subtree(folder: &serde_json::Value, indent: &str, is_last: bool) {
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
pub async fn upload_file(
    commands_sender: &Sender<NodeCommand>,
    folder_name: &str,
    file_path: &Path,
    bearer_token: &str,
) {
    eprintln!("file_path: {:?}", file_path);

    // Print current directory
    let current_dir = std::env::current_dir().unwrap();
    println!("Current directory: {:?}", current_dir);

    // Read file data
    let file_data = std::fs::read(file_path).map_err(|_| ShinkaiFsError::FailedPDFParsing).unwrap();

    // Extract the file name and extension
    let filename = file_path.file_name().unwrap().to_string_lossy().to_string();

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command using V2ApiUploadFileToFolder
    commands_sender
        .send(NodeCommand::V2ApiUploadFileToFolder {
            bearer: bearer_token.to_string(),
            filename, // Use the extracted filename
            file: file_data,
            path: folder_name.to_string(),
            file_datetime: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
            res: res_sender,
        })
        .await
        .unwrap();

    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("upload_file resp to folder: {:?}", resp);
}

#[allow(clippy::too_many_arguments)]
pub async fn upload_file_to_job(
    commands_sender: &Sender<NodeCommand>,
    job_id: &str,
    file_path: &Path,
    bearer_token: &str,
) {
    eprintln!("file_path: {:?}", file_path);

    // Print current directory
    let current_dir = std::env::current_dir().unwrap();
    println!("Current directory: {:?}", current_dir);

    // Read file data
    let file_data = std::fs::read(file_path).map_err(|_| ShinkaiFsError::FailedPDFParsing).unwrap();

    // Extract the file name with extension
    let filename = file_path.file_name().unwrap().to_string_lossy().to_string();

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command using V2ApiUploadFileToJob
    commands_sender
        .send(NodeCommand::V2ApiUploadFileToJob {
            bearer: bearer_token.to_string(),
            job_id: job_id.to_string(),
            filename, // Use the extracted filename
            file: file_data,
            file_datetime: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
            res: res_sender,
        })
        .await
        .unwrap();

    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("upload_file_to_job resp: {:?}", resp);
}

pub async fn get_folder_name_for_job(
    commands_sender: &Sender<NodeCommand>,
    job_id: &str,
    bearer_token: &str,
) -> Result<String, APIError> {
    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command to get the folder name for the job
    commands_sender
        .send(NodeCommand::V2ApiVecFSGetFolderNameForJob {
            bearer: bearer_token.to_string(),
            job_id: job_id.to_string(),
            res: res_sender,
        })
        .await
        .unwrap();

    // Receive and return the folder name
    res_receiver.recv().await.unwrap()
}

pub async fn get_files_for_job(
    commands_sender: &Sender<NodeCommand>,
    job_id: &str,
    bearer_token: &str,
) -> Result<Value, APIError> {
    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command to retrieve files for the job
    commands_sender
        .send(NodeCommand::V2ApiVecFSRetrieveFilesForJob {
            bearer: bearer_token.to_string(),
            job_id: job_id.to_string(),
            res: res_sender,
        })
        .await
        .unwrap();

    // Receive and return the files as a JSON value
    res_receiver.recv().await.unwrap()
}
