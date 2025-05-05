use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use std::time::{Duration, Instant};

use super::utils::node_test_api::{
    api_create_job, api_initial_registration_with_no_code_for_device, api_llm_provider_registration, api_message_job, wait_for_default_tools, wait_for_rust_tools
};
use super::utils::test_boilerplate::run_test_one_node_network;
use mockito::Server;

async fn wait_for_response(node1_commands_sender: async_channel::Sender<NodeCommand>) {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node1_commands_sender
        .send(NodeCommand::FetchLastMessages {
            limit: 1,
            res: res_sender,
        })
        .await
        .unwrap();
    let node1_last_messages = res_receiver.recv().await.unwrap();
    let msg_hash = node1_last_messages[0].calculate_message_hash_for_pagination();

    let start = Instant::now();
    loop {
        let (res_sender, res_receiver) = async_channel::bounded(1);
        node1_commands_sender
            .send(NodeCommand::FetchLastMessages {
                limit: 2, // Get the last 2 messages (query and response)
                res: res_sender,
            })
            .await
            .unwrap();
        let node1_last_messages = res_receiver.recv().await.unwrap();

        if node1_last_messages.len() == 2 && node1_last_messages[1].calculate_message_hash_for_pagination() == msg_hash
        {
            break;
        }

        if start.elapsed() > Duration::from_secs(10) {
            panic!("Test failed: 10 seconds have passed without receiving the response");
        }

        tokio::time::sleep(Duration::from_millis(200)).await; // Short sleep to prevent tight looping
    }
}

#[test]
fn simple_job_message_test() {
    // Set required environment variables
    std::env::set_var("WELCOME_MESSAGE", "false");

    // Create a mock server for OpenAI API
    let mut server = Server::new();

    run_test_one_node_network(|env| {
        Box::pin(async move {
            // Extract environment variables from the test setup
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
            let node1_api_key = env.node1_api_key.clone();
            let node1_abort_handler = env.node1_abort_handler;

            {
                // 1. Setup mock OpenAI response
                eprintln!("\n\nSetting up mock OpenAI server");
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
                                    "content": "This is a test response from the mock server"
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
            }

            {
                // 2. Register device and profile
                eprintln!("\n\nRegistering device and profile");
                api_initial_registration_with_no_code_for_device(
                    node1_commands_sender.clone(),
                    node1_profile_name.as_str(),
                    node1_identity_name.as_str(),
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    node1_device_identity_sk,
                    node1_profile_encryption_sk.clone(),
                    node1_profile_identity_sk.clone(),
                    node1_device_name.as_str(),
                )
                .await;
            }

            {
                // Wait for default tools to be ready
                eprintln!("\n\nWaiting for default tools to be ready");
                let tools_ready = wait_for_default_tools(
                    node1_commands_sender.clone(),
                    node1_api_key.clone(),
                    20, // Wait up to 20 seconds
                )
                .await
                .expect("Failed to check for default tools");
                assert!(tools_ready, "Default tools should be ready within 20 seconds");
            }

            {
                // Check that Rust tools are installed
                eprintln!("\n\nWaiting for Rust tools installation");
                match wait_for_rust_tools(node1_commands_sender.clone(), 20).await {
                    Ok(retry_count) => {
                        eprintln!("Rust tools were installed after {} retries", retry_count);
                    }
                    Err(e) => {
                        panic!("{}", e);
                    }
                }
            }

            {
                // 3. Register an LLM provider (agent)
                eprintln!("\n\nRegistering LLM provider");
                let agent_name = ShinkaiName::new(
                    format!("{}/{}/agent/{}", node1_identity_name, node1_profile_name, node1_agent).to_string(),
                )
                .unwrap();

                let open_ai = OpenAI {
                    model_type: "gpt-4-turbo".to_string(),
                };

                let agent = SerializedLLMProvider {
                    id: node1_agent.to_string(),
                    full_identity_name: agent_name,
                    name: Some("Test Agent".to_string()),
                    description: Some("Test Agent Description".to_string()),
                    external_url: Some(server.url()),
                    api_key: Some("mockapikey".to_string()),
                    model: LLMProviderInterface::OpenAI(open_ai),
                };

                api_llm_provider_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    node1_profile_identity_sk.clone(),
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    agent,
                )
                .await;
            }

            let mut job_id: String;
            let agent_subidentity = format!("{}/agent/{}", node1_profile_name, node1_agent);

            {
                // 4. Create a job
                eprintln!("\n\nCreating a job");
                job_id = api_create_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    node1_profile_identity_sk.clone(),
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    &agent_subidentity,
                )
                .await;
            }

            {
                // Update job config to turn off streaming
                eprintln!("\n\nUpdating job config to turn off streaming");
                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiUpdateJobConfig {
                        bearer: node1_api_key.clone(),
                        job_id: job_id.clone(),
                        config: JobConfig {
                            stream: Some(false),
                            ..JobConfig::empty()
                        },
                        res: res_sender,
                    })
                    .await
                    .unwrap();
                let result = res_receiver.recv().await.unwrap();
                assert!(result.is_ok(), "Failed to update job config: {:?}", result);
            }

            {
                // 5. Send a message to the job
                eprintln!("\n\nSending a message to the job");
                let message_content = "This is a test message";
                api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    node1_profile_identity_sk.clone(),
                    node1_identity_name.as_str(),
                    node1_profile_name.as_str(),
                    &agent_subidentity,
                    &job_id,
                    message_content,
                    &[], // No file paths
                    "",  // No parent message
                )
                .await;
            }

            {
                // 6. Wait for and verify the response
                eprintln!("\n\nWaiting for response");
                wait_for_response(node1_commands_sender.clone()).await;

                // Verify the response content
                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::FetchLastMessages {
                        limit: 2,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                let messages = res_receiver.recv().await.unwrap();
                let response_content = messages[0]
                    .get_message_content()
                    .expect("Failed to get message content");

                assert!(
                    response_content.contains("This is a test response from the mock server"),
                    "Response content did not match expected: {}",
                    response_content
                );

                eprintln!("Test completed successfully");
                node1_abort_handler.abort();
                return;
            }
        })
    });
}
