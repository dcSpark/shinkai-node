#![recursion_limit = "256"]

use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_channel::{bounded, Receiver, Sender};
use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use fs_extra::dir::{self, CopyOptions};
use shinkai_fs_mirror::shinkai::shinkai_manager_for_sync::ShinkaiManagerForSync;
use shinkai_fs_mirror::synchronizer::{FilesystemSynchronizer, SyncInterval};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_node::network::node_api::{self, APIError};
use shinkai_node::schemas::identity::{Identity, IdentityType};
use shinkai_vector_resources::resource_errors::VRError;
use std::collections::HashMap;
use std::{fs, path::Path};
use tempfile::tempdir;
use tokio::runtime::Runtime;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIConvertFilesAndSaveToFolder, APIVecFsCreateFolder, APIVecFsRetrievePathSimplifiedJson, IdentityPermissions,
    MessageSchemaType, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
};

use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

fn persistence_setup() {
    let path = Path::new("db_tests_persistence/");
    let _ = fs::remove_dir_all(path);
}

fn folder_setup() {
    // Create a temporary directory
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Example: Copying "../knowledge" into the temporary directory
    let source_path = Path::new("../knowledge");
    dir::copy(source_path, temp_path, &CopyOptions::new()).expect("Failed to copy knowledge folder");

    // // Remove existing files in the temporary directory
    // for entry in fs::read_dir(temp_path).expect("Failed to read temp dir") {
    //     let entry = entry.expect("Failed to read entry");
    //     let path = entry.path();
    //     if path.is_file() {
    //         fs::remove_file(path).expect("Failed to remove file");
    //     }
    // }

    // // Add a new file to the temporary directory
    // let new_file_path = temp_path.join("new_file.txt");
    // let mut file = File::create(&new_file_path).expect("Failed to create new file");
    // writeln!(file, "This is a new file.").expect("Failed to write to new file");
}

// Done: We can import a ShinkaiNode and start it from scratch
// Done: Then we can register a new identity and profile (ready for testing)
// Done: Then we copy a starting folder and
// We sync the filesystem from a specific folder (files + subfolders with more items)
// We check that the files are correctly synced
// Then we add a new file and remove another file
// These changes should be reflected in the Node system

#[test]
fn sync_tests() {
    eprintln!("Starting sync tests");
    setup();
    persistence_setup();
    // folder_setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.sepolia-shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let node1_subencryption_sk_clone = node1_profile_encryption_sk.clone();

        let node1_subidentity_sk_clone = clone_signature_secret_key(&node1_profile_identity_sk);

        let (node1_device_identity_sk, node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));
        let node1_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node1_identity_name.clone()));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk,
            0,
            node1_commands_receiver,
            node1_db_path,
            true,
            vec![],
            None,
            node1_fs_db_path,
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
            "Node 1 identity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node1_identity_sk))
        );
        eprintln!(
            "Node 1 identity pk: {:?}",
            signature_public_key_to_string(node1_identity_pk)
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
            "Node 1 subencryption sk: {:?}",
            encryption_secret_key_to_string(node1_subencryption_sk_clone.clone())
        );
        eprintln!(
            "Node 1 subencryption pk: {:?}",
            encryption_public_key_to_string(node1_profile_encryption_pk)
        );

        eprintln!("Starting node");
        // Start node1 and node2
        let node1_clone = Arc::clone(&node1);
        let node1_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 1");
            let _ = node1_clone.lock().await.start().await;
        });

        // add 2 sec delay
        tokio::time::sleep(Duration::from_secs(2)).await;
        // Setup API Server task
        let api_listen_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8082); 
        let node1_commands_sender_clone = node1_commands_sender.clone();
        let api_server = tokio::spawn(async move {
            node_api::run_api(
                node1_commands_sender_clone.clone(),
                api_listen_address,
                node1_identity_name.to_string(),
            )
            .await;
        });

        let node1_abort_handler = node1_handler.abort_handle();
        let symmetrical_sk = unsafe_deterministic_aes_encryption_key(0);

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

            // Create a New ShinkaiManagerForSync
            let node_address = format!("http://{}", api_listen_address);
            let shinkai_manager_sync = ShinkaiManagerForSync::new(
                node1_profile_encryption_sk.clone(),
                clone_signature_secret_key(&node1_profile_identity_sk),
                node1_encryption_pk,
                node1_identity_name.to_string(),
                node1_profile_name.to_string(),
                node1_identity_name.to_string(),
                node1_identity_name.to_string(),
                node_address,
            );
            // TODO: change the time to manual. Maybe create an Enum to pass it?

            let syncing_folders = FilesystemSynchronizer::new(
                shinkai_manager_sync, 
                Path::new("./knowledge").to_path_buf(), 
                Path::new("./knowledge").to_path_buf(), 
                "db_tests_persistence/".to_string(), 
                SyncInterval::Timed(Duration::from_secs(5))
            ).await.unwrap();
            
            eprintln!("syncing_folders: {:?}", syncing_folders);
            // sleep for 10 seconds
            tokio::time::sleep(Duration::from_secs(10)).await;
            // let res = syncing_folders.scan_folders();
            // eprintln!("res: {:?}", res);
            {   
                eprintln!("\n\nChecking the current file system files\n\n");
                let payload = APIVecFsRetrievePathSimplifiedJson {
                    path: "/".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    node1_identity_name,
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                // eprintln!("resp for current file system files: {}", resp);
                eprintln!("\n\n Checking the current file system files\n\n");
                print_tree_simple(&resp);   
                // TODO: check that the response matches the expectation
            }
            {
                // TODO: Make modifications
                // Add a new file
                // update a file
                // add sub folders
            }
            {
                // 
            }
            node1_abort_handler.abort();
            panic!("end of the road for now");
            // Testing Some Stuff
            {
                eprintln!("\n\n### Sending message (APICreateFilesInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

                let message_content = aes_encryption_key_to_string(symmetrical_sk);
                let msg = ShinkaiMessageBuilder::create_files_inbox_with_sym_key(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    "job::test::false".to_string(),
                    message_content.clone(),
                    node1_profile_name.to_string(),
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string(),
                )
                .unwrap();

                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res: res_sender })
                    .await
                    .unwrap();
                let _ = res_receiver.recv().await.unwrap().expect("Failed to receive messages");
            }
            {
                 // Create Folder
                 let payload = APIVecFsCreateFolder {
                    path: "/".to_string(),
                    folder_name: "test_folder".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsCreateFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    node1_identity_name,
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSCreateFolder { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp: {:?}", resp);
            }
            {
                // let filename = "files/shinkai_intro.vrkai";
                let filename = "files/Zeko_Mina_Rollup.pdf";
                let file_path = Path::new(filename);

                // Read the file into a buffer
                let file_data = std::fs::read(file_path)
                    .map_err(|_| VRError::FailedPDFParsing)
                    .unwrap();

                // Encrypt the file using Aes256Gcm
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));
                let nonce = GenericArray::from_slice(&[0u8; 12]);
                let nonce_slice = nonce.as_slice();
                let nonce_str = aes_nonce_to_hex_string(nonce_slice);
                let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
                        filename: filename.to_string(),
                        file: ciphertext,
                        public_key: hash_of_aes_encryption_key_hex(symmetrical_sk),
                        encrypted_nonce: nonce_str,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                // Receive the response
                let _ = res_receiver.recv().await.unwrap().expect("Failed to receive response"); 
            }
            {
                 // Convert File and Save to Folder
                 let payload = APIConvertFilesAndSaveToFolder {
                    path: "/test_folder".to_string(),
                    file_inbox: hash_of_aes_encryption_key_hex(symmetrical_sk),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::ConvertFilesAndSaveToFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    node1_identity_name,
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIConvertFilesAndSaveToFolder { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp: {:?}", resp);
            }
            {
                let payload = APIVecFsRetrievePathSimplifiedJson {
                    // path: "/test_folder/shinkai_intro".to_string(),
                    path: "/test_folder/Zeko_Mina_Rollup".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    node1_identity_name,
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp for current file system files: {}", resp);
            }
            node1_abort_handler.abort();
        });
        // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, api_server, interactions_handler);
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
}

#[allow(clippy::too_many_arguments)]
pub async fn api_registration_device_node_profile_main(
    node_commands_sender: Sender<NodeCommand>,
    node_profile_name: &str,
    node_identity_name: &str,
    node_encryption_pk: EncryptionPublicKey,
    device_encryption_sk: EncryptionStaticKey,
    device_signature_sk: SigningKey,
    profile_encryption_sk: EncryptionStaticKey,
    profile_signature_sk: SigningKey,
    device_name_for_profile: &str,
) {
    {
        let (res_registration_sender, res_registraton_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::LocalCreateRegistrationCode {
                permissions: IdentityPermissions::Admin,
                code_type: RegistrationCodeType::Device("main".to_string()),
                res: res_registration_sender,
            })
            .await
            .unwrap();
        let node_registration_code = res_registraton_receiver.recv().await.unwrap();

        let code_message = ShinkaiMessageBuilder::use_code_registration_for_device(
            device_encryption_sk.clone(),
            clone_signature_secret_key(&device_signature_sk),
            profile_encryption_sk.clone(),
            clone_signature_secret_key(&profile_signature_sk),
            node_encryption_pk,
            node_registration_code.to_string(),
            IdentityType::Device.to_string(),
            IdentityPermissions::Admin.to_string(),
            device_name_for_profile.to_string().clone(),
            "".to_string(),
            node_identity_name.to_string(),
            node_identity_name.to_string(),
        )
        .unwrap();

        eprintln!("code_message: {:?}", code_message);

        tokio::time::sleep(Duration::from_secs(1)).await;

        let (res_use_registration_sender, res_use_registraton_receiver) = async_channel::bounded(2);

        eprintln!("node_commands_sender: {:?}", node_commands_sender);
        eprintln!("res_use_registration_sender: {:?}", res_use_registration_sender);
        node_commands_sender
            .send(NodeCommand::APIUseRegistrationCode {
                msg: code_message,
                res: res_use_registration_sender,
            })
            .await
            .unwrap();
        let node2_use_registration_code = res_use_registraton_receiver.recv().await.unwrap();
        eprintln!("node_use_registration_code: {:?}", node2_use_registration_code);
        match node2_use_registration_code {
            Ok(code) => assert_eq!(
                code.message,
                "true".to_string(),
                "{} used registration code",
                node_profile_name
            ),
            Err(e) => panic!("Registration code error: {:?}", e),
        }

        // tokio::time::sleep(Duration::from_secs(1)).await;

        #[allow(clippy::type_complexity)]
        let (res_all_subidentities_sender, res_all_subidentities_receiver): (
            async_channel::Sender<Result<Vec<Identity>, APIError>>,
            async_channel::Receiver<Result<Vec<Identity>, APIError>>,
        ) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::GetAllSubidentitiesDevicesAndAgents(
                res_all_subidentities_sender,
            ))
            .await
            .unwrap();
        let node2_all_subidentities = res_all_subidentities_receiver.recv().await.unwrap().unwrap();
        eprintln!("node_all_subidentities: {:?}", node2_all_subidentities);
        shinkai_log(
            ShinkaiLogOption::Tests,
            ShinkaiLogLevel::Debug,
            format!(
                "{} subidentity: {:?}",
                node_profile_name,
                node2_all_subidentities[0].get_full_identity_name()
            )
            .as_str(),
        );
        assert_eq!(
            node2_all_subidentities[1].get_full_identity_name(),
            format!("{}/main/device/{}", node_identity_name, device_name_for_profile),
            "Node has the right subidentity"
        );
    }
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
) -> ShinkaiMessage {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S%f").to_string();

    ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
        .message_raw_content(payload.to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(schema)
        .internal_metadata_with_inbox(
            sender_subidentity.to_string(),
            "".to_string(),
            "".to_string(),
            EncryptionMethod::None,
            None,
        )
        .external_metadata_with_schedule(recipient.to_string(), sender.to_string(), timestamp)
        .build()
        .unwrap()
}

fn print_tree_simple(json_str: &str) {
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

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
        eprintln!("/");
        if let Some(folders) = val["child_folders"].as_array() {
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
    } else {
        eprintln!("Failed to parse JSON");
    }
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