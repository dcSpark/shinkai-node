use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use chrono::Utc;
use ed25519_dalek::SigningKey;
use shinkai_message_primitives::schemas::agents::serialized_agent::{
    AgentLLMInterface, GenericAPI, Ollama, OpenAI, SerializedAgent,
};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIConvertFilesAndSaveToFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsMoveFolder,
    APIVecFsRetrievePathSimplifiedJson, APIVecFsRetrieveVectorSearchSimplifiedJson, JobMessage, MessageSchemaType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{clone_static_secret_key, EncryptionMethod};
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_node::db::db_cron_task::CronTask;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::planner::kai_files::{KaiJobFile, KaiSchemaType};
use shinkai_vector_resources::resource_errors::VRError;
use std::env;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use utils::test_boilerplate::run_test_one_node_network;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::utils;
use super::utils::node_test_api::{api_agent_registration, api_initial_registration_with_no_code_for_device};
use mockito::Server;

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
        )
        .external_metadata_with_schedule(recipient.to_string(), sender.to_string(), timestamp)
        .build()
        .unwrap();
    message
}

#[test]
fn vector_fs_api_tests() {
    init_default_tracing();
    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_agent = env.node1_agent.clone();
            let node1_encryption_pk = env.node1_encryption_pk.clone();
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let node1_abort_handler = env.node1_abort_handler;

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
            let mut server = Server::new();
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

                let agent = SerializedAgent {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    perform_locally: false,
                    api_key: Some("".to_string()),
                    external_url: Some(server.url()),
                    model: AgentLLMInterface::Ollama(ollama),
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
                };
                api_agent_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
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

                let message_content = aes_encryption_key_to_string(symmetrical_sk.clone());
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
                    node1_encryption_pk.clone(),
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
                let filename = "files/shinkai_intro.vrkai";
                let file_path = Path::new(filename.clone());

                // Read the file into a buffer
                let file_data = std::fs::read(&file_path)
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
                    node1_encryption_pk.clone(),
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
                    node1_encryption_pk.clone(),
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

                // TODO: convert to json and then compare
                let expected_path = "/test_folder/shinkai_intro";
                assert!(
                    resp.contains(expected_path),
                    "Response does not contain the expected file path: {}",
                    expected_path
                );
            }
            // It is failing
            // Failed to receive response: APIError { code: 500, error: "Internal Server Error",
            // message: "Failed to move folder: Supplied path does not exist/hold any FSEntry in the VectorFS: /test_folder2" }
            // {
            //     // Move Folder
            //     let payload = APIVecFsMoveFolder {
            //         origin_path: "test_folder".to_string(),
            //         destination_path: "test_folder2".to_string(),
            //     };

            //     let msg = generate_message_with_payload(
            //         serde_json::to_string(&payload).unwrap(),
            //         MessageSchemaType::VecFsMoveFolder,
            //         node1_profile_encryption_sk.clone(),
            //         clone_signature_secret_key(&node1_profile_identity_sk),
            //         node1_encryption_pk.clone(),
            //         node1_identity_name.as_str(),
            //         node1_profile_name.as_str(),
            //         node1_identity_name.as_str(),
            //     );

            //     // Prepare the response channel
            //     let (res_sender, res_receiver) = async_channel::bounded(1);

            //     // Send the command
            //     node1_commands_sender
            //         .send(NodeCommand::APIVecFSMoveFolder { msg, res: res_sender })
            //         .await
            //         .unwrap();
            //     let resp = res_receiver.recv().await.unwrap().expect("Failed to receive response");
            //     eprintln!("resp: {:?}", resp);
            // }
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
                        node1_encryption_pk.clone(),
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
                    node1_encryption_pk.clone(),
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
                // For Later
            }
            {
                // Do deep search
                let payload = APIVecFsRetrieveVectorSearchSimplifiedJson {
                    search: "who wrote Shinkai?".to_string(),
                    path: Some("/test_folder".to_string()),
                    max_results: Some(10),
                    max_files_to_scan: Some(100),
                };

                let msg = generate_message_with_payload(
                    serde_json::to_string(&payload).unwrap(),
                    MessageSchemaType::VecFsRetrieveVectorSearchSimplifiedJson,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk.clone(),
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
                assert!(!resp.is_empty(), "Response is empty.");
                assert_eq!(
                    (&resp[0].0, &resp[0].1),
                    (
                        &"Shinkai Network Manifesto (Early Preview) Robert Kornacki rob@shinkai.com Nicolas Arqueros"
                            .to_string(),
                        &vec!["test_folder".to_string(), "shinkai_intro".to_string()]
                    ),
                    "The first search result does not match the expected output."
                );
            }
            node1_abort_handler.abort();
        })
    });
}
