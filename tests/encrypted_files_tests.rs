use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_channel::{bounded, Receiver, Sender};
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, OpenAI, SerializedAgent};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{JobMessage, MessageSchemaType};
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, encrypt_with_chacha20poly1305, encryption_public_key_to_string,
    encryption_secret_key_to_string, ephemeral_encryption_keys, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::agent::agent;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::node_api::APIError;
use shinkai_node::network::Node;
use shinkai_vector_resources::resource_errors::VectorResourceError;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::time::Instant;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;
use utils::test_boilerplate::run_test_one_node_network;

mod utils;
use crate::utils::node_test_api::{
    api_agent_registration, api_create_job, api_initial_registration_with_no_code_for_device, api_message_job,
    api_registration_device_node_profile_main,
};
use crate::utils::node_test_local::local_registration_profile_node;
use mockito::Server;

#[test]
fn sandwich_messages_with_files_test() {
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
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive messages");
                eprintln!("response: {}", response);
            }
            {
                eprintln!("\n\n### Sending message (APIAddFileToInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

                // New File A
                // // Prepare the file to be sent
                // let file_path = Path::new("tmp_tests/test_file.txt");
                // // Create the directory if it does not exist
                // if let Some(parent) = file_path.parent() {
                //     fs::create_dir_all(parent).expect("Failed to create directory");
                // }
                // let file_content = "This is a test file";
                // fs::write(&file_path, file_content).expect("Unable to write file");

                // // Read the entire file into a Vec<u8>
                // let file_data = tokio::fs::read(&file_path).await.expect("Failed to read file");

                // New File B
                // Prepare the file to be read
                let file_path = Path::new("files/shinkai_intro.pdf");

                // Read the file into a buffer
                let file_data = std::fs::read(&file_path)
                    .map_err(|_| VectorResourceError::FailedPDFParsing)
                    .unwrap();

                // Encrypt the file using Aes256Gcm
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));
                let nonce = GenericArray::from_slice(&[0u8; 12]);
                let nonce_slice = nonce.as_slice();
                let nonce_str = aes_nonce_to_hex_string(nonce_slice);
                let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

                // Prepare the other parameters
                let filename = "shinkai_intro.pdf";

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
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("response: {}", response);
            }
            let mut server = Server::new();
            {
                // Register an Agent
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let open_ai = OpenAI {
                    model_type: "gpt-3.5-turbo".to_string(),
                };
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

                let _m = server
                    .mock("POST", "/v1/chat/completions")
                    .match_header("authorization", "Bearer mockapikey")
                    .with_status(200)
                    .with_header("content-type", "application/json")
                    .with_body(
                        r#"{
                    "id": "chatcmpl-123",
                    "object": "chat.completion",
                    "created": 1677652288,
                    "choices": [{
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": "\n\n{\"answer\": \"Hello there, how may I assist you today?\"}"
                        },
                        "finish_reason": "stop"
                    }],
                    "usage": {
                        "prompt_tokens": 9,
                        "completion_tokens": 12,
                        "total_tokens": 21
                    }
                }"#,
                    )
                    .create();

                let agent = SerializedAgent {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    perform_locally: false,
                    external_url: Some("https://api.openai.com".to_string()),
                    // external_url: Some(server.url()),
                    api_key: Some("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".to_string()),
                    // api_key: Some("mockapikey".to_string()),
                    model: AgentLLMInterface::OpenAI(open_ai),
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

            let mut job_id = "".to_string();
            let agent_subidentity = format!("{}/agent/{}", node1_profile_name.clone(), node1_agent.clone()).to_string();
            {
                // Create a Job
                eprintln!("\n\nCreate a Job for the previous Agent in Node1 and verify it");
                job_id = api_create_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                )
                .await;
            }
            {
                // Send a Message to the Job for processing
                eprintln!("\n\nSend a message for a Job");
                let message = "What's Shinkai?".to_string();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &message,
                    &hash_of_aes_encryption_key_hex(symmetrical_sk),
                )
                .await;
            }
            // {
            //     eprintln!("\n\n### Sending Second message (APIAddFileToInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

            //     // Prepare the file to be read
            //     let filename = "files/Zeko_Mina_Rollup.pdf";
            //     let file_path = Path::new(filename.clone());

            //     // Read the file into a buffer
            //     let file_data = std::fs::read(&file_path)
            //         .map_err(|_| VectorResourceError::FailedPDFParsing)
            //         .unwrap();

            //     // Encrypt the file using Aes256Gcm
            //     let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));
            //     let nonce = GenericArray::from_slice(&[0u8; 12]);
            //     let nonce_slice = nonce.as_slice();
            //     let nonce_str = aes_nonce_to_hex_string(nonce_slice);
            //     let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

            //     // Prepare the response channel
            //     let (res_sender, res_receiver) = async_channel::bounded(1);

            //     // Send the command
            //     node1_commands_sender
            //         .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
            //             filename: filename.to_string(),
            //             file: ciphertext,
            //             public_key: hash_of_aes_encryption_key_hex(symmetrical_sk),
            //             encrypted_nonce: nonce_str,
            //             res: res_sender,
            //         })
            //         .await
            //         .unwrap();

            //     // Receive the response
            //     let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
            //     eprintln!("response: {}", response);
            // }
            // {
            //     let _m = server
            //         .mock("POST", "/v1/chat/completions")
            //         .match_header("authorization", "Bearer mockapikey")
            //         .with_status(200)
            //         .with_header("content-type", "application/json")
            //         .with_body(
            //             r#"{
            //         "id": "chatcmpl-123",
            //         "object": "chat.completion",
            //         "created": 1677652288,
            //         "choices": [{
            //             "index": 0,
            //             "message": {
            //                 "role": "assistant",
            //                 "content": "\n\n{\"answer\": \"Hello there, how may I assist you today?\"}"
            //             },
            //             "finish_reason": "stop"
            //         }],
            //         "usage": {
            //             "prompt_tokens": 9,
            //             "completion_tokens": 12,
            //             "total_tokens": 21
            //         }
            //     }"#,
            //         )
            //         .create();
            // }
            // {
            //     // Send a Message to the Job for processing
            //     eprintln!("\n\nSend a message for the Job");
            //     let message = "How does Zeko work?".to_string();
            //     let start = Instant::now();
            //     api_message_job(
            //         node1_commands_sender.clone(),
            //         clone_static_secret_key(&node1_profile_encryption_sk),
            //         node1_encryption_pk.clone(),
            //         clone_signature_secret_key(&node1_profile_identity_sk),
            //         node1_identity_name.clone().as_str(),
            //         node1_profile_name.clone().as_str(),
            //         &agent_subidentity.clone(),
            //         &job_id.clone().to_string(),
            //         &message,
            //         &hash_of_aes_encryption_key_hex(symmetrical_sk),
            //     )
            //     .await;

            //     let duration = start.elapsed(); // Get the time elapsed since the start of the timer
            //     eprintln!("Time elapsed in api_message_job is: {:?}", duration);
            // }
        })
    });
}
