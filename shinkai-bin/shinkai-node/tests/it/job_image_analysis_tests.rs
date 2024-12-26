use crate::it::utils::vecfs_test_utils::{get_files_for_job, get_folder_name_for_job, upload_file_to_job};

use super::utils::test_boilerplate::run_test_one_node_network;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, Ollama, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use super::utils::node_test_api::{
    api_create_job, api_initial_registration_with_no_code_for_device, api_llm_provider_registration, api_message_job,
};
use mockito::Server;

#[test]
#[ignore]
fn job_image_analysis() {
    let mut server = Server::new();

    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_api_key_bearer = env.node1_api_key;
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_agent = env.node1_llm_provider.clone();
            let node1_encryption_pk = env.node1_encryption_pk;
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);

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
                            "model":"llava",
                            "created_at":"2023-12-19T11:18:05.31733973Z",
                            "response":"{\"answer\": \"A bright blue, clear sky\"}",
                            "done":true,
                            "context":[29871,13,11889,29901,887,526,263,1407,8444,20255,393,29915,29879,1407,1781,472,1614,1259,263,3414,29889,448,450,1857,1667,3414,472,1361,338,29901,421,2783,29581,278,1967,29952,2538,2818,773,278,1494,382,29933,22498,322,13312,3078,1683,29901,525,10998,525,12011,29915,525,11283,1347,525,10162,29871,7521,3126,13,22933,5425,1254,13566,29901,8853,12011,1115,376,29909,11785,7254,29892,2821,14744,9092],
                            "total_duration":3482767354,
                            "load_duration":2553548600,
                            "prompt_eval_count":1,
                            "prompt_eval_duration":798772000,
                            "eval_count":11,
                            "eval_duration":127775000
                        }"#,
                    )
                    .create();

                let ollama = Ollama {
                    model_type: "llava".to_string(),
                };

                let agent = SerializedLLMProvider {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    external_url: Some(server.url()),
                    api_key: Some("mockapikey".to_string()),
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
            let job_message_content = "describe the image".to_string();
            {
                eprintln!("\n\n### Sending message (APIAddFileToInboxWithSymmetricKey) from profile subidentity to node 1\n\n");
                let file_path = Path::new("../../files/blue_64x64.png");
                upload_file_to_job(&node1_commands_sender, &job_id, file_path, &node1_api_key_bearer).await;

                // Retrieve the folder name for the job
                let folder_name = get_folder_name_for_job(&node1_commands_sender, &job_id, &node1_api_key_bearer)
                    .await
                    .unwrap();
                eprintln!("Folder name for job: {}", folder_name);

                // Retrieve the files for the job
                let files = get_files_for_job(&node1_commands_sender, &job_id, &node1_api_key_bearer)
                    .await
                    .unwrap();
                eprintln!("Files for job: {:?}", files);

                // Extract the path from the files
                let file_paths: Vec<String> = if let Some(files_array) = files.as_array() {
                    files_array
                        .iter()
                        .filter_map(|file| file.get("path").and_then(|name| name.as_str()).map(|s| s.to_string()))
                        .collect()
                } else {
                    panic!("Files is not an array");
                };

                // Convert Vec<String> to Vec<&str>
                let file_paths_str: Vec<&str> = file_paths.iter().map(|s| s.as_str()).collect();

                // Check that the files contain the expected file
                let expected_file_name = "blue_64x64.png";
                eprintln!("file_paths: {:?}", file_paths);
                assert!(
                    file_paths.iter().any(|file_name| file_name.ends_with(expected_file_name)),
                    "Expected file not found in job files"
                );

                let shinkai_path = ShinkaiPath::base_path();
                eprintln!("Shinkai Path: {}", shinkai_path.to_string_lossy());

                // Send a Message to the Job for processing
                eprintln!("\n\nSend a message for the Job");
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
                    &job_message_content,
                    &file_paths_str,
                    "",
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
        })
    });
}
