use async_channel::{bounded, Receiver, Sender};
use chrono::Utc;
use ed25519_dalek::SigningKey;
use fs_extra::dir::{self, CopyOptions};
use serde_json::Value;
use shinkai_fs_mirror::shinkai::shinkai_manager_for_sync::ShinkaiManagerForSync;
use shinkai_fs_mirror::synchronizer::{FilesystemSynchronizer, SyncInterval};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_node::network::node_api::{self, APIError};
use shinkai_node::schemas::identity::{Identity, IdentityType};
use shinkai_vector_resources::utils::hash_string;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{fs, path::Path};
use tempfile::{tempdir, TempDir};
use tokio::runtime::Runtime;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIVecFsRetrievePathSimplifiedJson, IdentityPermissions, MessageSchemaType, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
};

use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
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

fn folder_setup() -> (PathBuf, TempDir) {
    // Create a temporary directory
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    let source_path = Path::new("../../knowledge");
    dir::copy(source_path, temp_path, &CopyOptions::new()).expect("Failed to copy knowledge folder");

    eprintln!("Created temp dir");
    print_dir(temp_path, 0);

    (temp_path.to_path_buf(), temp_dir)
}

fn modify_file_content(temp_dir: PathBuf) {
    let file_path = temp_dir.join("knowledge/test.txt");
    if file_path.exists() {
        // Debug
        // let before_content = fs::read_to_string(&file_path).unwrap_or_else(|_| "Failed to read file".to_string());
        // eprintln!("Content before modification: {}", before_content);

        // // Get and print the last modified time before modification
        // if let Ok(metadata) = fs::metadata(&file_path) {
        //     if let Ok(last_modified) = metadata.modified() {
        //         eprintln!("Last modified before modification: {:?}", last_modified);
        //     }
        // }

        let _ = fs::write(
            &file_path,
            "Shinkai whitepaper was written by Nico and Rob. Shinkai is an AI-Powered Operating System.",
        );

        // Debug
        // // Read and log the content after modification
        // let after_content = fs::read_to_string(&file_path).unwrap_or_else(|_| "Failed to read file".to_string());
        // eprintln!("Content after modification: {}", after_content);

        // // Get and print the last modified time after modification
        // if let Ok(metadata) = fs::metadata(&file_path) {
        //     if let Ok(last_modified) = metadata.modified() {
        //         eprintln!("Last modified after modification: {:?}", last_modified);
        //     }
        // }
    }

    eprintln!("Modified file content");
}

fn modify_temp_dir(temp_dir: PathBuf) {
    // Define paths based on the temporary directory
    let file_to_remove = temp_dir.join("knowledge/test_1/file1.txt");
    let file_to_move = temp_dir.join("knowledge/test_1/file2.txt");
    let file_destination = temp_dir.join("knowledge/test_1/file4.txt");
    let new_sub_folder = temp_dir.join("knowledge/test_1/sub_test1");
    let new_file_in_sub_folder = new_sub_folder.join("file1.txt");
    let folder_to_remove = temp_dir.join("knowledge/test_2");

    // Remove file1.txt
    if file_to_remove.exists() {
        let _ = fs::remove_file(&file_to_remove);
    }

    // Move file2.txt to file4.txt
    if file_to_move.exists() {
        let _ = fs::rename(&file_to_move, file_destination);
    }

    // Create a new subfolder and a new file within it
    if fs::create_dir_all(&new_sub_folder).is_ok() {
        let _ = File::create(new_file_in_sub_folder).map(|mut file| {
            let _ = writeln!(file, "This is a new file in the subfolder.");
        });
    }

    // Remove the test_2 folder
    if folder_to_remove.exists() {
        let _ = fs::remove_dir_all(&folder_to_remove);
    }

    eprintln!("Modified temp dir");
    print_dir(&temp_dir, 0);
}

fn print_dir(path: &Path, indent: usize) {
    if path.is_dir() {
        fs::read_dir(path)
            .unwrap()
            .flatten() // Use flatten to directly handle Ok values
            .for_each(|entry| {
                let path = entry.path();
                let metadata = fs::metadata(&path).unwrap();
                if metadata.is_dir() {
                    eprintln!(
                        "{}{}/",
                        " ".repeat(indent * 2),
                        path.file_name().unwrap().to_str().unwrap()
                    );
                    print_dir(&path, indent + 1);
                } else {
                    eprintln!(
                        "{}{}",
                        " ".repeat(indent * 2),
                        path.file_name().unwrap().to_str().unwrap()
                    );
                }
            });
    }
}

#[test]
fn mirror_sync_tests() {
    eprintln!("Starting sync tests");
    std::env::set_var("WELCOME_MESSAGE", "false");
    setup();
    persistence_setup();
    let (test_folder, _temp_dir) = folder_setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.arb-sep-shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let _node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let _node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let node1_subencryption_sk_clone = node1_profile_encryption_sk.clone();

        let _node1_subidentity_sk_clone = clone_signature_secret_key(&node1_profile_identity_sk);

        let (node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name));
        let node1_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node1_identity_name));

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
            None,
            true,
            vec![],
            None,
            node1_fs_db_path,
            None,
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
            let _ = node_api::run_api(
                node1_commands_sender_clone.clone(),
                api_listen_address,
                node1_identity_name.to_string(),
            )
            .await;
        });

        let node1_abort_handler = node1_handler.abort_handle();
        let api_server_handler = api_server.abort_handle();

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

            // Current folder structure
            //     knowledge/
            //     test_1/
            //         file2.txt
            //         file3.txt
            //         file1.txt
            //     test.txt
            //     test_2/
            //         file2.txt
            //         file3.txt
            //         file1.txt

            let syncing_folders = FilesystemSynchronizer::new(
                shinkai_manager_sync,
                test_folder.clone(),
                Path::new("./").to_path_buf(),
                "db_tests_persistence/".to_string(),
                SyncInterval::None,
                false,
                None,
            )
            .await
            .unwrap();

            let _ = syncing_folders.force_process_updates().await;

            {
                let mut attempt = 0;
                let mut success = false;
                while attempt < 10 && !success {
                    let payload = APIVecFsRetrievePathSimplifiedJson { path: "/".to_string() };

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

                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
                        .await
                        .unwrap();
                    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                    let mut parsed_resp = parse_and_extract_file_paths(resp);
                    parsed_resp.sort();
                    // println!("parsed_resp: {:?}", parsed_resp);

                    let mut expected_paths = vec![
                        PathBuf::from("/knowledge/test"),
                        PathBuf::from("/knowledge/test_2/file2"),
                        PathBuf::from("/knowledge/test_2/file3"),
                        PathBuf::from("/knowledge/test_2/file1"),
                        PathBuf::from("/knowledge/test_1/file3"),
                        PathBuf::from("/knowledge/test_1/file1"),
                        PathBuf::from("/knowledge/test_1/file2"),
                    ];
                    expected_paths.sort();

                    if parsed_resp == expected_paths {
                        success = true;
                    } else {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        attempt += 1;
                    }
                }

                assert!(success, "Failed to match the expected file paths after 10 attempts.");
            }
            {
                // we don't modify anything so the diff calculation should be empty
                let results = syncing_folders.get_scan_folders_and_calculate_difference().await;
                // results should be an empty vec
                assert_eq!(results, vec![], "The results should be an empty vec");
            }
            {
                // let resp = syncing_folders
                //     .shinkai_manager_for_sync
                //     .retrieve_vector_resource("/knowledge/test")
                //     .await
                //     .unwrap();
                // eprintln!("(before) data_string: {:?}", resp.data);

                // we modify just one file /knowledge/test.txt
                eprintln!("Modifying file content of /knowledge/test.txt");
                modify_file_content(test_folder.clone());
                let results = syncing_folders.get_scan_folders_and_calculate_difference().await;
                assert_eq!(
                    results.len(),
                    1,
                    "Expected results to contain exactly one modified file."
                );
                let file_path = results[0].0.to_str().unwrap();
                assert_eq!(
                    file_path, "knowledge/test.txt",
                    "Expected the modified file to be 'knowledge/test.txt'."
                );

                let _result = syncing_folders.force_process_updates().await;
                // eprintln!("result: {:?}", result);

                let _resp = syncing_folders
                    .shinkai_manager_for_sync
                    .retrieve_vector_resource("/knowledge/test")
                    .await
                    .unwrap();
                // eprintln!("(after) resp: {:?}", resp.data);
            }
            {
                // Some modifications are made to the folder
                // Updated directory structure:
                //     knowledge/
                //     test_1/
                //         file3.txt
                //         file4.txt
                //         sub_test1/
                //              file1.txt
                //     test.txt
                modify_temp_dir(test_folder.clone());
                let _ = syncing_folders.force_process_updates().await;
                let mut attempt = 0;
                let mut success = false;
                while attempt < 10 && !success {
                    let payload = APIVecFsRetrievePathSimplifiedJson { path: "/".to_string() };

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

                    let (res_sender, res_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
                        .await
                        .unwrap();
                    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                    let mut parsed_resp = parse_and_extract_file_paths(resp);
                    parsed_resp.sort();

                    let mut expected_paths = vec![
                        PathBuf::from("/knowledge/test"),
                        PathBuf::from("/knowledge/test_1/file1"),
                        PathBuf::from("/knowledge/test_1/file2"),
                        PathBuf::from("/knowledge/test_1/file3"),
                        PathBuf::from("/knowledge/test_1/file4"),
                        PathBuf::from("/knowledge/test_1/sub_test1/file1"),
                        PathBuf::from("/knowledge/test_2/file1"),
                        PathBuf::from("/knowledge/test_2/file2"),
                        PathBuf::from("/knowledge/test_2/file3"),
                    ];
                    expected_paths.sort();

                    if parsed_resp == expected_paths {
                        success = true;
                    } else {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        attempt += 1;
                    }
                }

                assert!(success, "Failed to match the expected file paths after 10 attempts.");
            }
            api_server_handler.abort();
            node1_abort_handler.abort();
            api_server_handler.abort();
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
    rt.shutdown_background();
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
            .send(NodeCommand::GetAllSubidentitiesDevicesAndLLMProviders(
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

#[allow(dead_code)]
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

fn extract_files_paths(folder: &Value, base_path: PathBuf) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Check if the current node has child items (files) and add them to the paths vector
    if let Some(items) = folder["child_items"].as_array() {
        for item in items {
            if let Some(name) = item["name"].as_str() {
                paths.push(base_path.join(name));
            }
        }
    }

    // Recursively process child folders
    if let Some(folders) = folder["child_folders"].as_array() {
        for subfolder in folders {
            if let Some(name) = subfolder["name"].as_str() {
                let new_base = base_path.join(name);
                paths.extend(extract_files_paths(subfolder, new_base));
            }
        }
    }

    paths
}

fn parse_and_extract_file_paths(json: Value) -> Vec<PathBuf> {
    extract_files_paths(&json, PathBuf::from("/"))
}
