use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use base64::Engine;
use chrono::TimeZone;
use chrono::Utc;
use ed25519_dalek::SigningKey;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, Ollama, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIConvertFilesAndSaveToFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder, APIVecFsDeleteItem,
    APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson, APIVecFsRetrieveSourceFile,
    APIVecFsRetrieveVectorSearchSimplifiedJson, APIVecFsSearchItems, MessageSchemaType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{clone_static_secret_key, EncryptionMethod};
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_vector_resources::resource_errors::VRError;
use std::path::Path;
use std::sync::Arc;
use utils::test_boilerplate::run_test_one_node_network;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::it::utils::shinkai_testing_framework::ShinkaiTestingFramework;

use super::utils;
use super::utils::db_handlers::setup_node_storage_path;
use super::utils::node_test_api::{api_initial_registration_with_no_code_for_device, api_llm_provider_registration};
use mockito::Server;

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

#[test]
fn vector_fs_api_tests() {
    setup_node_storage_path();
    std::env::set_var("WELCOME_MESSAGE", "false");
    std::env::set_var("ONLY_TESTING_JS_TOOLS", "true");


    let mut server = Server::new();

    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_agent = env.node1_llm_provider.clone();
            let node1_encryption_pk = env.node1_encryption_pk;
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let node1_abort_handler = env.node1_abort_handler;

            let node1_db_weak = Arc::downgrade(&env.node1_db);

            // For this test
            let symmetrical_sk = unsafe_deterministic_aes_encryption_key(0);

            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_initial_registration_with_no_code_for_device(
                    node1_commands_sender.clone(),
                    env.node1_profile_name.as_str(),
                    env.node1_identity_name.as_str(),
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name.as_str(),
                )
                .await;
            }
            {
                // Register an Agent
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let agent_name = ShinkaiName::new(
                    format!(
                        "{}/{}/agent/{}",
                        node1_identity_name.clone(),
                        node1_profile_name.clone(),
                        node1_agent.clone()
                    )
                    .to_string(),
                )
                .unwrap();

                // Note: this is mocked for Ollamas API
                let _m = server
                    .mock("POST", "/api/generate")
                    .with_status(200)
                    .with_header("content-type", "application/json")
                    .with_body(
                        r#"{
                            "model":"mixtral:8x7b-instruct-v0.1-q4_1",
                            "created_at":"2023-12-19T11:36:44.687874415Z",
                            "response":"{\"answer\": \"Why couldn't the bicycle stand up by itself? Because it was two-tired.\"}",
                            "done":true,
                            "context":[28705,733,16289,28793,28705,995,460,396,10023,13892,693,865,659,2735,298,272,3857,3036,304,574,1216,4788,298,4372,707,2996,272,2188,5312,28723,2378,459,1460,354,3629,2758,442,1871,297,574,4372,298,272,2188,28725,562,3768,1912,272,2188,390,1188,1871,390,2572,28723,415,2188,659,2261,28747,28705,387,1912,528,264,13015,28723,13,1047,368,506,2066,1871,298,5090,4372,272,2188,28742,28713,2996,28747,1992,19571,1413,272,2296,413,6848,28765,304,7771,2511,1112,28747,464,18437,464,24115,28742,464,14243,1423,464,11339,28705,1047,368,927,298,18896,680,1871,298,9222,4372,272,2188,28725,868,368,622,927,298,1073,9547,304,1605,27674,4916,28748,14104,272,6594,14060,395,680,1871,304,1073,302,264,3472,5709,298,1300,633,3036,28723,11147,354,28049,680,4842,567,10537,821,1552,28707,479,528,264,28832,28747,1992,19571,1413,272,2296,413,6848,28765,304,7771,2511,1112,28747,464,18437,464,2360,28742,464,14243,1423,28725,464,3499,1869,464,1427,28742,443,28742,28705,8789,3371,733,28748,16289,4490,28739,24115,1264,345,7638,3481,28742,28707,272,24521,1812,1876,582,486,3837,28804,5518,378,403,989,28733,28707,1360,611,28752],
                            "total_duration":29617027653,
                            "load_duration":7157879293,
                            "prompt_eval_count":203,
                            "prompt_eval_duration":19022360000,
                            "eval_count":25,
                            "eval_duration":3435284000
                        }"#,
                    )
                    .create();

                let ollama = Ollama {
                    model_type: "mixtral:8x7b-instruct-v0.1-q4_1".to_string(),
                };

                let agent = SerializedLLMProvider {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    api_key: Some("".to_string()),
                    external_url: Some(server.url()),
                    model: LLMProviderInterface::Ollama(ollama),
                };
                api_llm_provider_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    agent,
                )
                .await;
            }
            // Send message (APICreateFilesInboxWithSymmetricKey) from Device subidentity to Node 1
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
                // Initialize local PDF parser
                ShinkaiTestingFramework::initialize_pdfium().await;

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
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
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
                // Create Folder
                let payload = APIVecFsCreateFolder {
                    path: "/".to_string(),
                    folder_name: "test_folder2".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsCreateFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
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
                // Upload .vrkai file to inbox
                // Prepare the file to be read
                let filename = "../../files/shinkai_intro.vrkai";
                let file_path = Path::new(filename);

                // Read the file into a buffer
                let file_data = std::fs::read(file_path).map_err(|_| VRError::FailedPDFParsing).unwrap();

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
                    file_datetime: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::ConvertFilesAndSaveToFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
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
            let mut retrieved_fs_json = String::new();
            {
                // Recover file from path using APIVecFSRetrievePathSimplifiedJson
                let payload = APIVecFsRetrievePathSimplifiedJson {
                    path: "/test_folder/shinkai_intro".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
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

                // Assuming `resp` is now a serde_json::Value
                let resp_json = serde_json::to_string(&resp).expect("Failed to convert response to string");
                // eprintln!("resp for current file system files: {}", resp_json);

                // TODO: convert to json and then compare
                let expected_path = "/test_folder/shinkai_intro";
                assert!(
                    resp_json.contains(expected_path),
                    "Response does not contain the expected file path: {}",
                    expected_path
                );
                retrieved_fs_json = resp_json;
            }
            {
                // Upload .pdf file to inbox
                // Prepare the file to be read
                let filename = "../../files/shinkai_intro.pdf";
                let file_path = Path::new(filename);

                // Read the file into a buffer
                let file_data = std::fs::read(file_path).map_err(|_| VRError::FailedPDFParsing).unwrap();

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
                    file_datetime: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::ConvertFilesAndSaveToFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
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
                // Recover file from path using APIVecFSRetrievePathSimplifiedJson
                let payload = APIVecFsRetrievePathSimplifiedJson {
                    path: "/test_folder/shinkai_intro".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp for current file system files: {:?}", resp);

                // Convert `resp` from serde_json::Value to String for comparison
                let resp_json = serde_json::to_string(&resp).expect("Failed to convert response to string");

                let expected_path = "/test_folder/shinkai_intro";
                assert!(
                    resp_json.contains(expected_path),
                    "Response does not contain the expected file path: {}",
                    expected_path
                );
                // Assert that after updating the fs item with a new VR generated from the PDF (overwriting the one from the .vrkai),
                // the filesystem json is different (because different timestamps/id on the item).
                assert_ne!(resp_json, retrieved_fs_json);
            }
            {
                // Retrieve source file
                let payload = APIVecFsRetrieveSourceFile {
                    path: "/test_folder/shinkai_intro".to_string(),
                };

                let api_v2_key = std::env::var("API_V2_KEY").unwrap_or_else(|_| "SUPER_SECRET".to_string());

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::V2ApiRetrieveSourceFile {
                        bearer: api_v2_key,
                        payload,
                        res: res_sender,
                    })
                    .await
                    .unwrap();
                let resp_base64 = res_receiver.recv().await.unwrap().expect("Failed to receive response");

                // Compare the response with the original file
                let filename = "../../files/shinkai_intro.pdf";
                let file_path = Path::new(filename);
                let file_data = std::fs::read(file_path).map_err(|_| VRError::FailedPDFParsing).unwrap();

                let decoded_content = base64::engine::general_purpose::STANDARD
                    .decode(resp_base64.as_bytes())
                    .expect("Failed to decode base64");
                assert_eq!(file_data, decoded_content);
            }

            {
                // Copy Item (we required creating a new folder to copy the item to)
                {
                    // Create Folder
                    let payload = APIVecFsCreateFolder {
                        path: "/".to_string(),
                        folder_name: "test_folder3".to_string(),
                    };

                    let msg = generate_message_with_payload(
                        serde_json::to_string(&payload).unwrap(),
                        MessageSchemaType::VecFsCreateFolder,
                        node1_profile_encryption_sk.clone(),
                        clone_signature_secret_key(&node1_profile_identity_sk),
                        node1_encryption_pk,
                        node1_identity_name.as_str(),
                        node1_profile_name.as_str(),
                        node1_identity_name.as_str(),
                    );

                    // Prepare the response channel
                    let (res_sender, res_receiver) = async_channel::bounded(1);

                    // Send the command
                    node1_commands_sender
                        .send(NodeCommand::APIVecFSCreateFolder { msg, res: res_sender })
                        .await
                        .unwrap();
                    let _ = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                }
                // Copy item
                let payload = APIVecFsCopyItem {
                    origin_path: "/test_folder/shinkai_intro".to_string(),
                    destination_path: "/test_folder3".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsCopyItem,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSCopyItem { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                assert!(
                    resp.contains("Item copied successfully to /test_folder3"),
                    "Response does not contain the expected file path: /test_folder3/shinkai_intro"
                );
            }
            {
                // Move item
                let payload = APIVecFsMoveItem {
                    origin_path: "/test_folder3/shinkai_intro".to_string(),
                    destination_path: "/test_folder2".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsMoveItem, // Assuming you have a corresponding schema type for moving items
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSMoveItem { msg, res: res_sender }) // Assuming you have a corresponding NodeCommand for moving items
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp: {:?}", resp);
                assert!(
                    resp.contains("Item moved successfully to /test_folder"),
                    "Response does not contain the expected file path: /test_folder"
                );
            }
            {
                // Move Folder
                let payload = APIVecFsMoveFolder {
                    origin_path: "/test_folder".to_string(),
                    destination_path: "/test_folder2".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsMoveFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSMoveFolder { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp: {:?}", resp);
            }
            {
                // Recover file from path using APIVecFSRetrievePathSimplifiedJson
                let payload = APIVecFsRetrievePathSimplifiedJson { path: "/".to_string() };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
                    .await
                    .unwrap();
                let parsed_resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                // eprintln!("resp for current file system files: {:?}", parsed_resp);

                /*
                /
                ├── test_folder2
                │   ├── test_folder
                │   │   └── shinkai_intro
                │   └── shinkai_intro
                └── test_folder3
                 */

                // Assert the root contains 'test_folder2' and 'test_folder3'
                assert!(
                    parsed_resp["child_folders"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|folder| folder["name"] == "test_folder2"),
                    "test_folder2 is missing"
                );
                assert!(
                    parsed_resp["child_folders"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|folder| folder["name"] == "test_folder3"),
                    "test_folder3 is missing"
                );

                // Assert 'test_folder2' contains 'test_folder' and 'shinkai_intro'
                let test_folder2 = parsed_resp["child_folders"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|folder| folder["name"] == "test_folder2")
                    .expect("test_folder2 not found");
                assert!(
                    test_folder2["child_folders"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|folder| folder["name"] == "test_folder"),
                    "test_folder inside test_folder2 is missing"
                );
                assert!(
                    test_folder2["child_items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|item| item["name"] == "shinkai_intro"),
                    "shinkai_intro directly inside test_folder2 is missing"
                );

                // Assert 'test_folder' inside 'test_folder2' contains 'shinkai_intro'
                let test_folder_inside_test_folder2 = test_folder2["child_folders"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|folder| folder["name"] == "test_folder")
                    .expect("test_folder inside test_folder2 not found");
                assert!(
                    test_folder_inside_test_folder2["child_items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|item| item["name"] == "shinkai_intro"),
                    "shinkai_intro inside test_folder inside test_folder2 is missing"
                );
            }
            {
                // Do deep search
                let payload = APIVecFsRetrieveVectorSearchSimplifiedJson {
                    search: "who wrote Shinkai?".to_string(),
                    path: Some("/test_folder2".to_string()),
                    max_results: Some(10),
                    max_files_to_scan: Some(100),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrieveVectorSearchSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSRetrieveVectorSearchSimplifiedJson { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                for r in &resp {
                    eprintln!("\n\nSearch result: {:?}", r);
                }

                let check_first = &resp[0].0 == &"Shinkai Network Manifesto (Early Preview)".to_string()
                    && (&resp[0].1
                        == &vec![
                            "test_folder2".to_string(),
                            "test_folder".to_string(),
                            "shinkai_intro".to_string(),
                        ]
                        || &resp[0].1 == &vec!["test_folder2".to_string(), "shinkai_intro".to_string()]);

                let check_second = &resp[1].0 == &"Shinkai Network Manifesto (Early Preview)".to_string()
                    && (&resp[1].1
                        == &vec![
                            "test_folder2".to_string(),
                            "test_folder".to_string(),
                            "shinkai_intro".to_string(),
                        ]
                        || &resp[1].1 == &vec!["test_folder2".to_string(), "shinkai_intro".to_string()]);

                assert!(!resp.is_empty(), "Response is empty.");
                assert!(check_first && check_second);
            }
            {
                // Do file search
                let payload = APIVecFsSearchItems {
                    search: "shinkai".to_string(),
                    path: Some("/".to_string()),
                    max_results: Some(10),
                    max_files_to_scan: Some(100),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsSearchItems,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSSearchItems { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp seach items: {:?}", resp);
                assert_eq!(resp.len(), 2, "Expected 2 search results, but got {}", resp.len());
                assert!(
                    resp.contains(&"/test_folder2/test_folder/shinkai_intro".to_string()),
                    "Response does not contain the expected file path: /test_folder2/test_folder/shinkai_intro"
                );
                assert!(
                    resp.contains(&"/test_folder2/shinkai_intro".to_string()),
                    "Response does not contain the expected file path: /test_folder2/shinkai_intro"
                );
            }
            {
                // Remove file
                let payload = APIVecFsDeleteItem {
                    path: "/test_folder2/test_folder/shinkai_intro".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsDeleteItem,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSDeleteItem { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp seach items delete item: {:?}", resp);
            }
            {
                // remove folder
                let payload = APIVecFsDeleteFolder {
                    path: "/test_folder2/test_folder".to_string(),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsDeleteFolder,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSDeleteFolder { msg, res: res_sender })
                    .await
                    .unwrap();
                let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("resp seach items delete folder: {:?}", resp);
            }
            {
                let payload = APIVecFsRetrievePathSimplifiedJson { path: "/".to_string() };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                );

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res: res_sender })
                    .await
                    .unwrap();
                let parsed_resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");

                // Assert root contains 'test_folder2' and 'test_folder3'
                let root_folders = parsed_resp["child_folders"]
                    .as_array()
                    .expect("Expected child_folders to be an array");
                assert_eq!(
                    root_folders.len(),
                    2,
                    "Expected 2 root folders, found {}",
                    root_folders.len()
                );

                let test_folder2 = root_folders
                    .iter()
                    .find(|&f| f["name"] == "test_folder2")
                    .expect("test_folder2 not found");
                let test_folder3 = root_folders
                    .iter()
                    .find(|&f| f["name"] == "test_folder3")
                    .expect("test_folder3 not found");

                // Assert 'test_folder2' contains 'shinkai_intro'
                let test_folder2_items = test_folder2["child_items"]
                    .as_array()
                    .expect("Expected child_items to be an array in test_folder2");
                assert_eq!(
                    test_folder2_items.len(),
                    1,
                    "Expected 1 item in test_folder2, found {}",
                    test_folder2_items.len()
                );
                assert_eq!(
                    test_folder2_items[0]["name"], "shinkai_intro",
                    "Expected item 'shinkai_intro' in test_folder2"
                );

                // Assert 'test_folder3' is empty
                let test_folder3_folders = test_folder3["child_folders"]
                    .as_array()
                    .expect("Expected child_folders to be an array in test_folder3");
                let test_folder3_items = test_folder3["child_items"]
                    .as_array()
                    .expect("Expected child_items to be an array in test_folder3");
                assert!(test_folder3_folders.is_empty(), "Expected no folders in test_folder3");
                assert!(test_folder3_items.is_empty(), "Expected no items in test_folder3");
            }
            node1_abort_handler.abort();
        })
    });
}
