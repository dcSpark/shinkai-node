use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{LLMProviderInterface, OpenAI, SerializedLLMProvider};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_node::network::node::NodeCommand;
use std::time::Duration;
use std::time::Instant;
use utils::test_boilerplate::run_test_one_node_network;

use super::utils;
use super::utils::node_test_api::{
    api_agent_registration, api_create_job, api_initial_registration_with_no_code_for_device, api_message_job,
};
use mockito::Server;

async fn wait_for_response(node1_commands_sender: async_channel::Sender<NodeCommand>) {
    let (res1_sender, res1_receiver) = async_channel::bounded(1);
    node1_commands_sender
        .send(NodeCommand::FetchLastMessages {
            limit: 1,
            res: res1_sender,
        })
        .await
        .unwrap();
    let node1_last_messages = res1_receiver.recv().await.unwrap();
    let msg_hash = node1_last_messages[0].calculate_message_hash_for_pagination();

    let start = Instant::now();
    loop {
        let (res1_sender, res1_receiver) = async_channel::bounded(1);
        node1_commands_sender
            .send(NodeCommand::FetchLastMessages {
                limit: 2, // Set the limit to 8 to fetch up to 8 messages
                res: res1_sender,
            })
            .await
            .unwrap();
        let node1_last_messages = res1_receiver.recv().await.unwrap();
        // eprintln!("node1_last_messages: {:?}", node1_last_messages);
        // eprintln!("node1_last_messages[0] hash: {:?}", node1_last_messages[0].calculate_message_hash_for_pagination());
        // eprintln!("node1_last_messages[1] hash: {:?}", node1_last_messages[1].calculate_message_hash_for_pagination());

        if node1_last_messages.len() == 2 && node1_last_messages[1].calculate_message_hash_for_pagination() == msg_hash {
            break;
        }

        if start.elapsed() > Duration::from_secs(15) {
            panic!("Test failed: 3 seconds have passed without receiving the response");
        }

        tokio::time::sleep(Duration::from_millis(200)).await; // Short sleep to prevent tight looping
    }
}

#[test]
fn job_branchs_retries_tests() {
    std::env::set_var("WELCOME_MESSAGE", "false");
    init_default_tracing();
    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_agent = env.node1_agent.clone();
            let node1_encryption_pk = env.node1_encryption_pk;
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            let node1_abort_handler = env.node1_abort_handler;

            // For this test
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
                            "content": "\n# Answer\nHello there, how may I assist you today?"
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

                let agent = SerializedLLMProvider {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    perform_locally: false,
                    external_url: Some(server.url()),
                    api_key: Some("mockapikey".to_string()),
                    model: LLMProviderInterface::OpenAI(open_ai),
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
                };
                api_agent_registration(
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
            /*
            The tree that we are creating looks like:

                1
                ├── 2
                │   └── 5
                │       └── 6
                └── 3
                    └── 4
            */

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
                // Message 1
                let _ = api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    "hello are u there? (1)",
                    "",
                    "",
                    None,
                )
                .await;
                wait_for_response(node1_commands_sender.clone()).await;
            }

            /*
            The tree that we are creating looks like:

                1 (done)
                └── 2 (done)
                    │ ─── 5
                    │     └── 6
                    └── 3
                        └── 4
            */
            {
                // Message 3
                let _ = api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    "hello are u there? (3)",
                    "",
                    "",
                    None,
                )
                .await;
                wait_for_response(node1_commands_sender.clone()).await;
            }
            /*
            The tree that we are creating looks like:

                1 (done)
                └── 2 (done)
                    │ ─── 5
                    │     └── 6
                    └── 3 (done)
                        └── 4 (done)
            */
            let mut message2_hash: Option<String> = None;
            let mut inbox_name: Option<String> = None;
            {
                // Confirm that we receive 1, 2, 3, 4
                let start = Instant::now();
                loop {
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 4,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    eprintln!("node1_last_messages: {:?}", node1_last_messages);

                    if node1_last_messages.len() == 4
                        && node1_last_messages[1]
                            .get_message_content()
                            .unwrap()
                            .contains("hello are u there? (3)")
                    {
                        message2_hash = Some(node1_last_messages[2].calculate_message_hash_for_pagination());
                        inbox_name = Some(node1_last_messages[2].get_message_inbox().unwrap());
                        break;
                    }

                    if start.elapsed() > Duration::from_secs(10) {
                        panic!("Test failed: 3 seconds have passed without receiving the response");
                    }

                    tokio::time::sleep(Duration::from_millis(200)).await; // Short sleep to prevent tight looping
                }
            }
            {
                // Message 5
                let _ = api_message_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone().as_str(),
                    node1_profile_name.clone().as_str(),
                    &agent_subidentity.clone(),
                    &job_id.clone().to_string(),
                    "hello are u there? (5)",
                    "",
                    message2_hash.unwrap().as_str(),
                    None,
                )
                .await;
                wait_for_response(node1_commands_sender.clone()).await;
            }
            /*
            The tree that we are creating looks like:

                1 (done)
                └── 2 (done)
                    │ ─── 5 (done)
                    │     └── 6 (done)
                    └── 3 (done)
                        └── 4 (done)
            */
            {
                // Confirm that we receive 1, 2, 5, 6
                let start = Instant::now();
                loop {
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::GetLastMessagesFromInbox {
                            limit: 4,
                            inbox_name: inbox_name.clone().unwrap(),
                            offset_key: None,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    // eprintln!("\n\n node1_last_messages: {:?}", node1_last_messages);

                    if node1_last_messages.len() == 4
                        && node1_last_messages[2]
                            .get_message_content()
                            .unwrap()
                            .contains("hello are u there? (5)")
                    {
                        // // Print the content of each message if the condition passes
                        // for (index, msg) in node1_last_messages.iter().enumerate() {
                        //     eprintln!(
                        //         "Message position: {}, content: {}",
                        //         index,
                        //         msg.get_message_content().unwrap()
                        //     );
                        // }
                        break;
                    }

                    if start.elapsed() > Duration::from_secs(10) {
                        panic!("Test failed: 3 seconds have passed without receiving the response");
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await; // Short sleep to prevent tight looping
                }
            }
            // Check the endpoints that return all the branches
            {
                let start = Instant::now();
                loop {
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::GetLastMessagesFromInboxWithBranches {
                            limit: 6,
                            inbox_name: inbox_name.clone().unwrap(),
                            offset_key: None,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    // eprintln!("\n\n node1_last_messages: {:?}", node1_last_messages);

                    // Assuming each ShinkaiMessage can be uniquely identified or grouped by its content
                    // and that you have a way to determine the structure [[1],[2],[5,3],[6]] from these messages

                    // Since node1_last_messages is Vec<Vec<ShinkaiMessage>>, we need to iterate through both levels
                    let flattened_messages = node1_last_messages
                        .iter()
                        .flat_map(|msg_group| {
                            msg_group
                                .iter()
                                .map(|msg| {
                                    // Assuming get_message_content() is a method of ShinkaiMessage
                                    msg.get_message_content().unwrap() // Apply get_message_content to each ShinkaiMessage
                                })
                                .collect::<Vec<_>>() // Collects all message contents in the inner Vec<ShinkaiMessage>
                        })
                        .collect::<Vec<_>>(); // Collects all contents across all Vec<Vec<ShinkaiMessage>>

                    // Updated validation logic to check for specific content
                    let expected_contents = [
                        "hello are u there? (1)",
                        "Hello there, how may I assist you today?",
                        "hello are u there? (5)",
                        "hello are u there? (3)",
                        "Hello there, how may I assist you today?",
                    ];

                    let is_valid = flattened_messages
                        .iter()
                        .zip(expected_contents.iter())
                        .all(|(actual, &expected)| actual.contains(expected));

                    if is_valid && flattened_messages.len() == expected_contents.len() {
                        for (index, content) in flattened_messages.iter().enumerate() {
                            eprintln!("Message position: {}, content: {}", index, content);
                        }
                        break;
                    }

                    if start.elapsed() > Duration::from_secs(10) {
                        panic!("Test failed: 3 seconds have passed without receiving the response");
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await; // Short sleep to prevent tight looping
                }
            }
            node1_abort_handler.abort();
        })
    });
}
