use async_channel::{bounded, Receiver, Sender};
use serde_json::{json, Map};
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, unsafe_deterministic_encryption_keypair
};
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_message_primitives::shinkai_utils::search_mode::VectorSearchMode;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::Node;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::time::Duration;
use tokio::runtime::Runtime;

use crate::it::utils::node_test_api::{api_create_job_with_scope, api_execute_tool};
use crate::it::utils::vecfs_test_utils::{create_folder, upload_file};

use super::utils::db_handlers::setup_node_storage_path;
use super::utils::node_test_api::{api_registration_device_node_profile_main, wait_for_default_tools};
use super::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

use mockito::Server;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

#[test]
fn native_tool_test_knowledge() {
    setup_node_storage_path();
    std::env::set_var("WELCOME_MESSAGE", "false");
    std::env::set_var("SKIP_IMPORT_FROM_DIRECTORY", "true");
    std::env::set_var("IS_TESTING", "1");
    // WIP: need to find a way to test the agent registration
    setup();
    let rt = Runtime::new().unwrap();

    let mut server = Server::new();
    fn port_is_available(port: u16) -> bool {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    let e = rt.block_on(async {
        let node1_identity_name = "@@node1_test.sep-shinkai";
        let node1_subidentity_name = "main";
        let node1_device_name = "node1_device";
        let node1_agent = "node1_gpt_agent";

        let (node1_identity_sk, _node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let (node1_profile_identity_sk, _node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, _node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name));

        let api_key_bearer = "my_api_key".to_string();

        // Agent pre-creation
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
                    "content": "The Roman Empire is very interesting"
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

        let agent_name = ShinkaiName::new(
            format!(
                "{}/{}/agent/{}",
                node1_identity_name, node1_subidentity_name, node1_agent
            )
            .to_string(),
        )
        .unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
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

        assert!(port_is_available(12005), "Port 12005 is not available");
        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12005);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk.clone(),
            None,
            None,
            0,
            node1_commands_receiver,
            node1_db_path,
            "".to_string(),
            None,
            true,
            vec![agent],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some(api_key_bearer.clone()),
        );

        let node1_handler = tokio::spawn(async move {
            shinkai_log(ShinkaiLogOption::Tests, ShinkaiLogLevel::Debug, "Starting Node 1");
            let _ = node1.await.lock().await.start().await;
        });

        let abort_handler = node1_handler.abort_handle();

        let interactions_handler = tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::Tests,
                ShinkaiLogLevel::Debug,
                "\n\nRegistration of an Admin Profile",
            );

            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_registration_device_node_profile_main(
                    node1_commands_sender.clone(),
                    node1_subidentity_name,
                    node1_identity_name,
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name,
                )
                .await;

                // Wait for default tools to be ready
                let tools_ready = wait_for_default_tools(
                    node1_commands_sender.clone(),
                    api_key_bearer.clone(),
                    120, // Wait up to 120 seconds
                )
                .await
                .expect("Failed to check for default tools");
                assert!(tools_ready, "Default tools should be ready within 20 seconds");
            }
            {
                //
                // Creating a folder and uploading some files to the vector db
                //
                eprintln!("\n\n### Creating a folder and uploading some files to the vector db \n\n");
                // Send message (APICreateFilesInboxWithSymmetricKey) from Device subidentity to Node 1
                {
                    // Create test folder
                    create_folder(&node1_commands_sender, "/", "test_folder", &api_key_bearer.clone()).await;

                    // Upload File to /test_folder
                    let file_path = Path::new("../../files/shinkai_intro.vrkai");
                    upload_file(
                        &node1_commands_sender,
                        "/test_folder",
                        file_path,
                        &api_key_bearer.clone(),
                    )
                    .await;
                }

                #[allow(unused_assignments)]
                let mut job_id: String;
                let agent_subidentity = format!("{}/agent/{}", node1_subidentity_name, node1_agent).to_string();
                {
                    // Create a Job
                    shinkai_log(
                        ShinkaiLogOption::Tests,
                        ShinkaiLogLevel::Debug,
                        &format!("Creating a Job for Agent {}", agent_subidentity.clone()),
                    );
                    let vector_fs_folder = ShinkaiPath::from_string("test_folder".to_string());

                    let job_scope = MinimalJobScope {
                        vector_fs_items: vec![],
                        vector_fs_folders: vec![vector_fs_folder],
                        vector_search_mode: VectorSearchMode::FillUpTo25k,
                    };

                    job_id = api_create_job_with_scope(
                        node1_commands_sender.clone(),
                        clone_static_secret_key(&node1_profile_encryption_sk),
                        node1_encryption_pk,
                        clone_signature_secret_key(&node1_profile_identity_sk),
                        node1_identity_name,
                        node1_subidentity_name,
                        &agent_subidentity.clone(),
                        job_scope,
                    )
                    .await;
                }
                {
                    // Implement the tool execution here
                    // Add tool call code here
                    let mut parameters = Map::new();
                    parameters.insert("job_id".to_string(), json!(job_id));

                    let tool_execution_result = api_execute_tool(
                        node1_commands_sender.clone(),
                        api_key_bearer.clone(),
                        "local:::__official_shinkai:::shinkai_process_embeddings".to_string(),
                        parameters,
                        "your_tool_id".to_string(),
                        "your_app_id".to_string(),
                        Some(node1_agent.to_string()),
                        node1_agent.to_string(),
                        Map::new(),
                        Map::new(),
                    )
                    .await;

                    // Handle the result
                    match tool_execution_result {
                        Ok(response) => {
                            // Process the successful response
                            println!("Tool executed successfully: {:?}", response);
                        }
                        Err(error) => {
                            // Handle the error
                            eprintln!("Tool execution failed: {:?}", error);
                            panic!("Tool execution failed: {:?}", error);
                        }
                    }
                }

                abort_handler.abort();
            }
        });

        // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, interactions_handler);

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    println!("One of the tasks was aborted, but this is expected.");
                    Ok(())
                } else {
                    // If the error is not due to an abort, then it's unexpected
                    Err(e)
                }
            }
        }
    });
    rt.shutdown_timeout(Duration::from_secs(10));
    if let Err(e) = e {
        assert!(false, "An unexpected error occurred: {:?}", e);
    }
    assert!(port_is_available(12005), "Port 12005 is not available after test");
}
