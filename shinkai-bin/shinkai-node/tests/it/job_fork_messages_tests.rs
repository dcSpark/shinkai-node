use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, Ollama, SerializedLLMProvider
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;

use std::time::Duration;
use std::time::Instant;
use utils::test_boilerplate::run_test_one_node_network;

use super::utils;
use super::utils::node_test_api::{
    api_create_job, api_initial_registration_with_no_code_for_device, api_llm_provider_registration, api_message_job
};
use mockito::Server;

#[test]
fn test_fork_job_messages() {
    let mut server = Server::new();

    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_llm_provider = env.node1_llm_provider.clone();
            let node1_encryption_pk = env.node1_encryption_pk.clone();
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let node1_api_key = env.node1_api_key.clone();
            let node1_abort_handler = env.node1_abort_handler;

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
                        node1_llm_provider.clone()
                    )
                    .to_string(),
                )
                .unwrap();

                // Note: this is mocked for Ollamas API
                let _m = server
                    .mock("POST", "/api/chat")
                    .with_status(200)
                    .with_header("content-type", "application/json")
                    .with_body(
                        r#"{
                            "model": "mixtral:8x7b-instruct-v0.1-q4_1",
                            "created_at": "2023-12-19T11:36:44.687874415Z",
                            "message": {
                                "role": "assistant",
                                "content": "Paris is the capital of France."
                            },
                            "done": true,
                            "total_duration": 29617027653,
                            "load_duration": 7157879293,
                            "prompt_eval_count": 203,
                            "prompt_eval_duration": 19022360000,
                            "eval_count": 25,
                            "eval_duration": 3435284000
                        }"#,
                    )
                    .create();

                let ollama = Ollama {
                    model_type: "mixtral:8x7b-instruct-v0.1-q4_1".to_string(),
                };

                let agent = SerializedLLMProvider {
                    id: node1_llm_provider.clone().to_string(),
                    full_identity_name: agent_name,
                    external_url: Some(server.url()),
                    api_key: Some("".to_string()),
                    model: LLMProviderInterface::Ollama(ollama),
                };
                api_llm_provider_registration(
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
            let agent_subidentity =
                format!("{}/agent/{}", node1_profile_name.clone(), node1_llm_provider.clone()).to_string();
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

            let first_message = "What is the capital of France?".to_string();
            {
                // Send first message to the Job
                eprintln!("\n\nSend first message for the Job");
                let start = Instant::now();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &first_message,
                    &[],
                    "",
                )
                .await;

                let duration = start.elapsed();
                eprintln!("Time elapsed in api_message_job is: {:?}", duration);
            }

            {
                eprintln!("Waiting for the first message Job to finish");
                tokio::time::sleep(Duration::from_secs(2)).await;
                let mut job_completed = false;
                for i in 0..10 {
                    eprintln!("Checking job completion attempt {}", i + 1);
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 4,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    eprintln!("Number of messages received: {}", node1_last_messages.len());
                    eprintln!("Last messages: {:?}", node1_last_messages);

                    if node1_last_messages.len() >= 2 {
                        match node1_last_messages[1].get_message_content() {
                            Ok(message_content) => match serde_json::from_str::<JobMessage>(&message_content) {
                                Ok(job_message) => {
                                    eprintln!("Successfully parsed job message: {}", job_message.content);
                                    job_completed = true;
                                    break;
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse job message: {}, error: {}", message_content, e);
                                }
                            },
                            Err(e) => {
                                eprintln!("Failed to get message content: {}", e);
                            }
                        }
                    }

                    if job_completed {
                        eprintln!("Job completed within the expected time");
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                assert!(job_completed, "Job did not complete within the expected time");
            }

            let second_message = "Can you tell me more about its history?".to_string();
            {
                // Send second message to the Job
                eprintln!("\n\nSend second message for the Job");
                let start = Instant::now();
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    &second_message,
                    &[],
                    "",
                )
                .await;

                let duration = start.elapsed();
                eprintln!("Time elapsed in api_message_job is: {:?}", duration);
            }

            {
                eprintln!("Waiting for the second message Job to finish");
                tokio::time::sleep(Duration::from_secs(2)).await;
                let mut job_completed = false;
                let mut message_to_fork_id = String::new();
                for i in 0..5 {
                    eprintln!("Checking job completion attempt {}", i + 1);
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 4,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    eprintln!("Number of messages received: {}", node1_last_messages.len());
                    eprintln!("Last messages: {:?}", node1_last_messages);

                    if node1_last_messages.len() >= 4 {
                        match node1_last_messages[3].get_message_content() {
                            Ok(message_content) => match serde_json::from_str::<JobMessage>(&message_content) {
                                Ok(job_message) => {
                                    eprintln!("Successfully parsed job message: {}", job_message.content);
                                    message_to_fork_id = node1_last_messages[0].calculate_message_hash_for_pagination();
                                    job_completed = true;
                                    break;
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse job message: {}, error: {}", message_content, e);
                                }
                            },
                            Err(e) => {
                                eprintln!("Failed to get message content: {}", e);
                            }
                        }
                    }

                    if job_completed {
                        eprintln!("Job completed within the expected time");
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                assert!(job_completed, "Job did not complete within the expected time");

                // Fork the conversation
                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiForkJobMessages {
                        bearer: node1_api_key.to_string(),
                        job_id: job_id.clone(),
                        message_id: message_to_fork_id.clone(),
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                let _ = res_receiver.recv().await.unwrap();

                // Verify the forked conversation
                let (res2_sender, res2_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::FetchLastMessages {
                        limit: 8,
                        res: res2_sender,
                    })
                    .await
                    .unwrap();
                let forked_messages = res2_receiver.recv().await.unwrap();
                println!("Forked messages: {:?}", forked_messages);

                assert_eq!(
                    forked_messages.len(),
                    4,
                    "Forked messages should match original message count"
                );

                eprintln!("Job fork messages test completed");
                node1_abort_handler.abort();
            }
        })
    });
}
