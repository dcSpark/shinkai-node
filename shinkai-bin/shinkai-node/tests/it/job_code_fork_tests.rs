use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, Ollama, SerializedLLMProvider
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tools::CodeLanguage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_tools_primitives::tools::{
    parameters::Parameters, tool_playground::{ToolPlayground, ToolPlaygroundMetadata}, tool_types::{OperatingSystem, RunnerType, ToolResult}
};

use std::time::Duration;
use utils::test_boilerplate::run_test_one_node_network;

use super::utils;
use super::utils::node_test_api::{
    api_create_job, api_initial_registration_with_no_code_for_device, api_llm_provider_registration, wait_for_default_tools
};
use mockito::Server;

#[test]
fn test_job_code_fork() {
    std::env::set_var("WELCOME_MESSAGE", "false");

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

                // Wait for default tools to be ready
                let tools_ready = wait_for_default_tools(
                    node1_commands_sender.clone(),
                    node1_api_key.clone(),
                    20, // Wait up to 30 seconds
                )
                .await
                .expect("Failed to check for default tools");
                assert!(tools_ready, "Default tools should be ready within 30 seconds");
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
                // The code is non-valid, it's just a mock
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
                                "content": "```typescript\nimport { getHomePath } from './shinkai-local-support.ts';\n\ntype CONFIG = {};\ntype INPUTS = {};\ntype OUTPUT = {};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const homeDir = await getHomePath();\n  console.log(`The Shinkai Node is running in the directory: ${homeDir}`);\n  return {};\n}```"
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

            {
                // Generate tool implementation
                eprintln!("\n\nGenerate tool implementation");
                let message = JobMessage {
                    job_id: job_id.clone(),
                    content: "Create a simple tool that prints hello world".to_string(),
                    parent: None,
                    sheet_job_data: None,
                    tools: None,
                    callback: None,
                    metadata: None,
                    tool_key: None,
                    fs_files_paths: vec![],
                    job_filenames: vec![],
                };

                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiGenerateToolImplementation {
                        bearer: node1_api_key.clone(),
                        message,
                        language: CodeLanguage::Typescript,
                        tools: vec![],
                        post_check: false,
                        raw: false,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                let result = res_receiver.recv().await.unwrap();
                eprintln!("Tool implementation generation result: {:?}", result);
                assert!(result.is_ok(), "Tool implementation generation failed");
            }

            {
                // Store the generated tool
                eprintln!("\n\nStore the generated tool");
                let tool_playground = ToolPlayground {
                    metadata: ToolPlaygroundMetadata {
                        name: "Hello World".to_string(),
                        homepage: None,
                        version: "1.0.0".to_string(),
                        description: "A Shinkai Node running in the directory of its home path.".to_string(),
                        author: "@@localhost.sep-shinkai".to_string(),
                        keywords: vec![],
                        configurations: vec![],
                        parameters: Parameters {
                            schema_type: "object".to_string(),
                            properties: std::collections::HashMap::new(),
                            required: vec![],
                        },
                        result: ToolResult {
                            r#type: "object".to_string(),
                            properties: serde_json::json!({}),
                            required: vec![],
                        },
                        sql_tables: vec![],
                        sql_queries: vec![],
                        tools: None,
                        oauth: None,
                        runner: RunnerType::Any,
                        operating_system: vec![OperatingSystem::Linux, OperatingSystem::MacOS, OperatingSystem::Windows],
                        tool_set: None,
                    },
                    tool_router_key: None,
                    job_id: job_id.clone(),
                    job_id_history: vec![],
                    code: "import { getHomePath } from './shinkai-local-support.ts';\n\ntype CONFIG = {};\ntype INPUTS = {};\ntype OUTPUT = {};\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n  const homeDir = await getHomePath();\n  console.log(`The Shinkai Node is running in the directory: ${homeDir}`);\n  return {};\n}".to_string(),
                    language: CodeLanguage::Typescript,
                    assets: None,
                };

                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiSetPlaygroundTool {
                        bearer: node1_api_key.clone(),
                        payload: tool_playground,
                        tool_id: "task-id-1738841695936".to_string(),
                        app_id: "app-id-1738841695936".to_string(),
                        original_tool_key_path: None,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                let result = res_receiver.recv().await.unwrap();
                eprintln!("Tool storage result: {:?}", result);
                assert!(result.is_ok(), "Tool storage failed");
            }

            {
                // Wait for the implementation to be ready
                eprintln!("Waiting for the tool implementation to finish");
                tokio::time::sleep(Duration::from_secs(2)).await;
                let mut implementation_completed = false;
                for i in 0..10 {
                    eprintln!("Checking implementation completion attempt {}", i + 1);
                    let (res1_sender, res1_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::FetchLastMessages {
                            limit: 8,
                            res: res1_sender,
                        })
                        .await
                        .unwrap();
                    let node1_last_messages = res1_receiver.recv().await.unwrap();
                    println!("Node1 last messages: {:?}", node1_last_messages);

                    if node1_last_messages.len() >= 2 {
                        implementation_completed = true;
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                assert!(
                    implementation_completed,
                    "Tool implementation did not complete within the expected time"
                );
            }

            {
                // Fork the tool implementation
                eprintln!("\n\nFork the tool implementation");
                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiDuplicateTool {
                        bearer: node1_api_key.clone(),
                        tool_key_path: format!("local:::__localhost_sep_shinkai:::hello_world"),
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                let result = res_receiver.recv().await.unwrap();
                assert!(result.is_ok(), "Tool fork failed");
                let fork_result = result.unwrap();
                eprintln!("Fork result: {:?}", fork_result);
            }

            {
                // Verify the forked tool implementation
                eprintln!("Waiting for the tool fork to complete");
                tokio::time::sleep(Duration::from_secs(2)).await;
                let mut fork_completed = false;
                for i in 0..5 {
                    eprintln!("Checking fork completion attempt {}", i + 1);

                    // First get all smart inboxes
                    let (inbox_sender, inbox_receiver) = async_channel::bounded(1);
                    node1_commands_sender
                        .send(NodeCommand::V2ApiGetAllSmartInboxes {
                            bearer: node1_api_key.clone(),
                            limit: None,
                            offset: None,
                            show_hidden: None,
                            agent_id: None,
                            res: inbox_sender,
                        })
                        .await
                        .unwrap();
                    let inboxes = inbox_receiver.recv().await.unwrap();
                    eprintln!("Inboxes: {:?}", inboxes);

                    // Find the two job inboxes (original and forked)
                    let job_inboxes: Vec<_> = match inboxes {
                        Ok(inboxes) => inboxes
                            .iter()
                            .filter(|inbox| inbox.inbox_id.starts_with("job_inbox::"))
                            .cloned()
                            .collect(),
                        Err(_) => vec![],
                    };

                    if job_inboxes.len() >= 2 {
                        let original_inbox = &job_inboxes[0];
                        let forked_inbox = &job_inboxes[1];

                        // Get messages from original inbox
                        let (res1_sender, res1_receiver) = async_channel::bounded(1);
                        node1_commands_sender
                            .send(NodeCommand::V2ApiGetLastMessagesFromInbox {
                                bearer: node1_api_key.clone(),
                                inbox_name: original_inbox.inbox_id.clone(),
                                limit: 8,
                                offset_key: None,
                                res: res1_sender,
                            })
                            .await
                            .unwrap();
                        let original_messages = res1_receiver.recv().await.unwrap();

                        // Get messages from forked inbox
                        let (res2_sender, res2_receiver) = async_channel::bounded(1);
                        node1_commands_sender
                            .send(NodeCommand::V2ApiGetLastMessagesFromInbox {
                                bearer: node1_api_key.clone(),
                                inbox_name: forked_inbox.inbox_id.clone(),
                                limit: 8,
                                offset_key: None,
                                res: res2_sender,
                            })
                            .await
                            .unwrap();
                        let forked_messages = res2_receiver.recv().await.unwrap();

                        // Compare messages from both inboxes
                        if let (Ok(original_messages), Ok(forked_messages)) = (original_messages, forked_messages) {
                            assert!(
                                original_messages.len() == forked_messages.len(),
                                "Original and forked messages should have the same length"
                            );
                            assert!(original_messages.len() > 0, "Original messages should not be empty");

                            for (original_message, forked_message) in
                                original_messages.iter().zip(forked_messages.iter())
                            {
                                assert!(
                                    original_message.sender_subidentity == forked_message.sender_subidentity,
                                    "Original and forked messages should have the same sender subidentity"
                                );
                                assert!(
                                    original_message.sender == forked_message.sender,
                                    "Original and forked messages should have the same sender"
                                );
                                assert!(
                                    original_message.job_message.content == forked_message.job_message.content,
                                    "Original and forked messages should have the same content"
                                );
                            }
                            fork_completed = true;
                            break;
                        } else {
                            assert!(false, "Failed to get messages from inboxes");
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                assert!(
                    fork_completed,
                    "Fork did not complete with expected matching messages in both inboxes within the expected time"
                );
            }

            eprintln!("Job code fork test completed");
            node1_abort_handler.abort();
        })
    });
}
