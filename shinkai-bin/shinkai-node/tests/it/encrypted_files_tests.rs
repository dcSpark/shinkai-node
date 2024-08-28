use super::utils::test_boilerplate::run_test_one_node_network;
use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{JobMessage, MessageSchemaType};
use shinkai_message_primitives::shinkai_utils::encryption::{clone_static_secret_key, EncryptionMethod};
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_node::network::node_commands::NodeCommand;
use shinkai_vector_resources::resource_errors::VRError;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use super::utils::node_test_api::{
    api_llm_provider_registration, api_create_job, api_get_all_inboxes_from_profile, api_get_all_smart_inboxes_from_profile,
    api_initial_registration_with_no_code_for_device, api_message_job,
};
use mockito::Server;

#[test]
fn sandwich_messages_with_files_test() {
    unsafe { std::env::set_var("WELCOME_MESSAGE", "false") };
    init_default_tracing();
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
                            "content": "Hello there, how may I assist you today?"
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

                let open_ai = OpenAI {
                    model_type: "gpt-4-1106-preview".to_string(),
                };

                // let generic_api = GenericAPI {
                //     model_type: "togethercomputer/llama-2-70b-chat".to_string(),
                // };

                let agent = SerializedLLMProvider {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    perform_locally: false,
                    // external_url: Some("https://api.openai.com".to_string()),
                    // api_key: env::var("INITIAL_AGENT_API_KEY").ok(),
                    external_url: Some(server.url()),
                    api_key: Some("mockapikey".to_string()),
                    // external_url: Some("https://api.together.xyz".to_string()),
                    model: LLMProviderInterface::OpenAI(open_ai),
                    // model: LLMProviderInterface::GenericAPI(generic_api),
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
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

            let mut job_id = "".to_string();
            let agent_subidentity = format!("{}/agent/{}", node1_profile_name.clone(), node1_agent.clone()).to_string();
            {
                // Create a Job
                eprintln!("\n\nCreate a Job for the previous Agent in Node1 and verify it");
                job_id = api_create_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                )
                .await;
            }
            {
                // api_get_all_inboxes_from_profile
                eprintln!("\n\nGet All Profiles");
                let inboxes = api_get_all_inboxes_from_profile(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    node1_identity_name.clone().as_str(),
                )
                .await;
                assert_eq!(inboxes.len(), 1);
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
                let _response = res_receiver.recv().await.unwrap().expect("Failed to receive messages");
            }
            {
                eprintln!("\n\n### Sending Second message (APIAddFileToInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

                // Prepare the file to be read
                let filename = "../../files/zeko_mini.pdf";
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
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("response: {}", response);
            }
            {
                // Get filenames in inbox
                let message_content = hash_of_aes_encryption_key_hex(symmetrical_sk);
                let msg = ShinkaiMessageBuilder::new(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                )
                .message_raw_content(message_content.clone())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node1_profile_name.to_string().clone(),
                    "".to_string(),
                    EncryptionMethod::None,
                    None,
                )
                .external_metadata_with_intra_sender(
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string().clone(),
                    node1_profile_name.to_string().clone(),
                )
                .build()
                .unwrap();

                let (res_sender, res_receiver) = async_channel::bounded(1);
                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIGetFilenamesInInbox { msg, res: res_sender })
                    .await
                    .unwrap();

                // Receive the response
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                assert_eq!(response, vec!["../../files/zeko_mini.pdf"]);
            }
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

            let job_message_content = "What's Zeko?".to_string();
            {
                // Send a Message to the Job for processing
                eprintln!("\n\nSend a message for the Job");
                let start = Instant::now();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &job_message_content,
                    &hash_of_aes_encryption_key_hex(symmetrical_sk),
                    "",
                    None,
                )
                .await;

                let duration = start.elapsed(); // Get the time elapsed since the start of the timer
                eprintln!("Time elapsed in api_message_job is: {:?}", duration);
            }
            {
                // api_get_all_smart_inboxes_from_profile
                eprintln!("\n\n Get All Smart Inboxes");
                let inboxes = api_get_all_smart_inboxes_from_profile(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    node1_identity_name.clone().as_str(),
                )
                .await;
                assert_eq!(inboxes.len(), 1);
                assert_eq!(inboxes[0].custom_name, "What's Zeko?");
            }

            {
                eprintln!("Waiting for the Job to finish");
                for _ in 0..50 {
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 2,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    eprintln!("node1_last_messages: {:?}", node1_last_messages);

                    match node1_last_messages[0].get_message_content() {
                        Ok(message_content) => match serde_json::from_str::<JobMessage>(&message_content) {
                            Ok(job_message) => {
                                eprintln!("message_content: {}", message_content);
                                if job_message.content != job_message_content {
                                    assert!(true);
                                    break;
                                }
                            }
                            Err(_) => {
                                eprintln!("error: message_content: {}", message_content);
                            }
                        },
                        Err(_) => {
                            // nothing
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }

            let new_job_message_content = "Can you output markdown for an FAQ where my previous question is the heading and your response is the text underneath?".to_string();
            {
                // Send a Message to the Job for processing
                eprintln!("\n\nSend a message for the Job");
                let start = Instant::now();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &new_job_message_content,
                    "",
                    "",
                    None,
                )
                .await;

                let duration = start.elapsed(); // Get the time elapsed since the start of the timer
                eprintln!("Time elapsed in api_message_job is: {:?}", duration);
            }

            {
                eprintln!("Waiting for the Job to finish");
                for _ in 0..50 {
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 2,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    eprintln!("node1_last_messages: {:?}", node1_last_messages);

                    match node1_last_messages[0].get_message_content() {
                        Ok(message_content) => match serde_json::from_str::<JobMessage>(&message_content) {
                            Ok(job_message) => {
                                eprintln!("message_content: {}", message_content);
                                if job_message.content != new_job_message_content {
                                    assert!(true);
                                    break;
                                }
                            }
                            Err(_) => {
                                eprintln!("error: message_content: {}", message_content);
                            }
                        },
                        Err(_) => {
                            // nothing
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
            node1_abort_handler.abort();
        })
    });
}
