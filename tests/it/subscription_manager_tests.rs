use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_channel::{bounded, Receiver, Sender};
use async_std::println;
use chrono::Utc;
use core::panic;
use ed25519_dalek::{SigningKey, VerifyingKey};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription_req::PaymentOption;
use shinkai_message_primitives::schemas::shinkai_subscription_req::ShinkaiFolderSubscription;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIAvailableSharedItems, APIConvertFilesAndSaveToFolder, APICreateShareableFolder, APIVecFsCreateFolder,
    APIVecFsRetrievePathSimplifiedJson, IdentityPermissions, MessageSchemaType, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
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
use shinkai_node::network::node_api::{APIError, SendResponseBodyData};
use shinkai_node::network::Node;
use shinkai_vector_resources::resource_errors::VRError;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::utils::node_test_api::{
    api_registration_device_node_profile_main, api_registration_profile_node, api_try_re_register_profile_node,
};
use super::utils::node_test_local::local_registration_profile_node;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

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

    let message = ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
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
        .unwrap();
    message
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
        subscription_req: ShinkaiFolderSubscription {
            minimum_token_delegation: Some(100),
            minimum_time_delegated_hours: Some(100),
            monthly_payment: Some(PaymentOption::USD(10.0)),
            is_free: false,
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

async fn show_available_shared_items(
    commands_sender: &Sender<NodeCommand>,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
) {
    let payload = APIAvailableSharedItems {
        path: "/".to_string(), // Assuming you want to list items at the root
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

async fn retrieve_file_info(
    commands_sender: &Sender<NodeCommand>,
    encryption_sk: EncryptionStaticKey,
    signature_sk: SigningKey,
    encryption_pk: EncryptionPublicKey,
    identity_name: &str,
    profile_name: &str,
    path: &str,
) {
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
    );

    // Prepare the response channel
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command
    commands_sender
        .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("resp for current file system files: {}", resp);
}

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

    let message_content = aes_encryption_key_to_string(symmetrical_sk.clone());
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
    eprintln!("upload_file resp: {:?}", res);

    // Convert File and Save to Folder
    let payload = APIConvertFilesAndSaveToFolder {
        path: folder_name.to_string(),
        file_inbox: hash_of_aes_encryption_key_hex(symmetrical_sk),
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
    );

    let (res_sender, res_receiver) = async_channel::bounded(1);
    commands_sender
        .send(NodeCommand::APIConvertFilesAndSaveToFolder { msg, res: res_sender })
        .await
        .unwrap();
    let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
    eprintln!("upload_file resp: {:?}", resp);
}

#[test]
fn subscription_manager_test() {
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
        let node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let node2_encryption_sk_clone = node2_encryption_sk.clone();

        let node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);
        let node2_identity_sk_clone = clone_signature_secret_key(&node2_identity_sk);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node2_subidentity_sk, node2_subidentity_pk) = unsafe_deterministic_signature_keypair(101);
        let (node2_subencryption_sk, node2_subencryption_pk) = unsafe_deterministic_encryption_keypair(101);

        let node1_subencryption_sk_clone = node1_profile_encryption_sk.clone();
        let node2_subencryption_sk_clone = node2_subencryption_sk.clone();

        let node1_subidentity_sk_clone = clone_signature_secret_key(&node1_profile_identity_sk);
        let node2_subidentity_sk_clone = clone_signature_secret_key(&node2_subidentity_sk);

        let (node1_device_identity_sk, node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));
        let node1_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node1_identity_name.clone()));
        let node2_db_path = format!("db_tests/{}", hash_string(node2_identity_name.clone()));
        let node2_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node2_identity_name.clone()));

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

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let mut node2 = Node::new(
            node2_identity_name.to_string(),
            addr2,
            clone_signature_secret_key(&node2_identity_sk),
            node2_encryption_sk,
            0,
            node2_commands_receiver,
            node2_db_path,
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
            signature_secret_key_to_string(clone_signature_secret_key(&node2_subidentity_sk))
        );
        eprintln!(
            "Node 2 subidentity pk: {:?}",
            signature_public_key_to_string(node2_subidentity_pk)
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
            encryption_public_key_to_string(node2_subencryption_pk)
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
                    node1_encryption_pk.clone(),
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
                    clone_signature_secret_key(&node2_subidentity_sk),
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
                // Create /shared_test_folder
                create_folder(
                    &node1_commands_sender,
                    "/",
                    "shared_test_folder",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
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
                    node1_encryption_pk.clone(),
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
                    node1_encryption_pk.clone(),
                    node1_identity_name,
                    node1_profile_name,
                    "/",
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
                    node1_encryption_pk.clone(),
                    node1_identity_name,
                    node1_profile_name,
                    "/private_test_folder",
                    &file_path,
                    0,
                )
                .await;

                // Upload File to /shared_test_folder
                let file_path = Path::new("files/shinkai_intro.vrkai");
                upload_file(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    node1_identity_name,
                    node1_profile_name,
                    "/shared_test_folder",
                    &file_path,
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
                    node1_encryption_pk.clone(),
                    node1_identity_name,
                    node1_profile_name,
                    "/",
                )
                .await;
            }
            {
                // Make /shared_test_folder shareable
                make_folder_shareable(
                    &node1_commands_sender,
                    "/shared_test_folder",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
            }
            {
                // Show available shared items
                show_available_shared_items(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
            }
            {
                // Create /shared_test_folder/crypto
                create_folder(
                    &node1_commands_sender,
                    "/shared_test_folder",
                    "crypto",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                // Upload File to /shared_test_folder/crypto
                let file_path = Path::new("files/shinkai_intro.vrkai");
                upload_file(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    node1_identity_name,
                    node1_profile_name,
                    "/shared_test_folder/crypto",
                    &file_path,
                    0,
                )
                .await;
            }
              {
                // Show available shared items
                show_available_shared_items(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
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
