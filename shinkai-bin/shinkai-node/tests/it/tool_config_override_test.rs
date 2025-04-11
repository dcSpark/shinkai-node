use async_channel::{bounded, Receiver, Sender};
use serde_json::{json, Map, Value};
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::agent::Agent;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::Node;
use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use tokio::runtime::Runtime;

use crate::it::utils::node_test_api::{api_registration_device_node_profile_main, wait_for_default_tools};
use crate::it::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

use mockito::Server;

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

#[test]
fn test_tools_config_override_via_api() {
    setup();
    std::env::set_var("WELCOME_MESSAGE", "false");

    let rt = Runtime::new().unwrap();

    let mut server = Server::new();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.sep-shinkai";
        let node1_subidentity_name = "main";
        let node1_device_name = "node1_device";

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

        // Setup mock LLM provider
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
                    "content": "Test response"
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
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };

        let node1_llm_provider_id = "node1_llm_provider";
        let llm_provider_name = ShinkaiName::new(
            format!(
                "{}/{}/agent/{}",
                node1_identity_name, node1_subidentity_name, node1_llm_provider_id
            )
            .to_string(),
        )
        .unwrap();

        let llm_provider = SerializedLLMProvider {
            id: node1_llm_provider_id.to_string(),
            full_identity_name: llm_provider_name.clone(),
            external_url: Some(server.url()),
            api_key: Some("mockapikey".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        // Create node
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            node1_identity_sk.clone(),
            node1_encryption_sk.clone(),
            None,
            None,
            0,
            node1_commands_receiver,
            node1_db_path,
            "".to_string(),
            None,
            true,
            vec![llm_provider],
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

        let _abort_handler = node1_handler.abort_handle();

        // Register profile and wait for tools
        {
            // Register a Profile in Node1 and verifies it
            eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
            api_registration_device_node_profile_main(
                node1_commands_sender.clone(),
                node1_subidentity_name,
                node1_identity_name,
                node1_encryption_pk,
                node1_device_encryption_sk.clone(),
                node1_device_identity_sk.clone(),
                node1_profile_encryption_sk.clone(),
                node1_profile_identity_sk.clone(),
                node1_device_name,
            )
            .await;

            // Wait for default tools to be ready
            let tools_ready = wait_for_default_tools(
                node1_commands_sender.clone(),
                api_key_bearer.clone(),
                20, // Wait up to 20 seconds
            )
            .await
            .expect("Failed to check for default tools");
            assert!(tools_ready, "Default tools should be ready within 20 seconds");
        }

        // Create a new agent with tools_config_override
        {
            eprintln!("\n\n### Creating an agent with tools_config_override\n\n");

            // Define the tool configuration
            let mut tools_config_override = HashMap::new();
            let mut config_values = HashMap::new();
            config_values.insert("api_key".to_string(), Value::String("test-api-key".to_string()));
            config_values.insert("timeout".to_string(), Value::Number(serde_json::Number::from(30)));
            tools_config_override.insert("local:::shinkai:::search-knowledge:::1.0".to_string(), config_values);

            let node1_agent_id = "node1_gpt_agent";
            let agent_name = ShinkaiName::new(
                format!(
                    "{}/{}/agent/{}",
                    node1_identity_name, node1_subidentity_name, node1_agent_id
                )
                .to_string(),
            )
            .unwrap();
            // Create the agent payload
            let agent_data = json!({
                "name": "Test Agent",
                "agent_id": node1_agent_id,
                "full_identity_name": agent_name.clone(),
                "llm_provider_id": node1_llm_provider_id,
                "ui_description": "Test agent description",
                "knowledge": [],
                "storage_path": "/test/storage/path",
                "tools": ["local:::shinkai:::search-knowledge:::1.0"],
                "debug_mode": true,
                "tools_config_override": tools_config_override
            });

            let agent: Agent = serde_json::from_value(agent_data).unwrap();
            // Add the agent
            let (res_sender, res_receiver) = async_channel::bounded(1);
            node1_commands_sender
                .send(NodeCommand::V2ApiAddAgent {
                    bearer: api_key_bearer.clone(),
                    agent,
                    res: res_sender,
                })
                .await
                .unwrap();

            // Verify the agent was added
            match res_receiver.recv().await.unwrap() {
                Ok(_) => eprintln!("Agent added successfully"),
                Err(e) => panic!("Failed to add agent: {:?}", e),
            }

            // Now retrieve the agent and verify the tools_config_override was saved
            let (get_sender, get_receiver) = async_channel::bounded(1);
            node1_commands_sender
                .send(NodeCommand::V2ApiGetAgent {
                    bearer: api_key_bearer.clone(),
                    agent_id: node1_agent_id.to_string(),
                    res: get_sender,
                })
                .await
                .unwrap();

            // Verify the retrieved agent has the correct tools_config_override
            match get_receiver.recv().await.unwrap() {
                Ok(agent) => {
                    eprintln!("Agent retrieved successfully");

                    // Assert agent has tools_config_override
                    assert!(
                        agent.tools_config_override.is_some(),
                        "Agent should have tools_config_override"
                    );

                    let config_map = agent.tools_config_override.unwrap();
                    assert!(
                        config_map.contains_key("local:::shinkai:::search-knowledge:::1.0"),
                        "tools_config_override should contain the configured tool"
                    );

                    let tool_config = &config_map["local:::shinkai:::search-knowledge:::1.0"];
                    assert_eq!(
                        tool_config.get("api_key").unwrap().as_str().unwrap(),
                        "test-api-key",
                        "api_key should match the configured value"
                    );
                    assert_eq!(
                        tool_config.get("timeout").unwrap().as_i64().unwrap(),
                        30,
                        "timeout should match the configured value"
                    );
                }
                Err(e) => panic!("Failed to retrieve agent: {:?}", e),
            }

            // Now update the agent with a modified tools_config_override
            eprintln!("\n\n### Updating the agent with modified tools_config_override\n\n");

            let tool_router_key = "local:::__official_shinkai:::shinkai_llm_prompt_processor";
            // Create updated config
            let mut updated_config_values = HashMap::new();
            updated_config_values.insert("api_key".to_string(), Value::String("updated-api-key".to_string()));
            updated_config_values.insert("timeout".to_string(), Value::Number(serde_json::Number::from(60)));

            let mut updated_tools_config = HashMap::new();
            updated_tools_config.insert(tool_router_key.to_string(), updated_config_values);

            // Create update payload
            let update_data = json!({
                "agent_id": node1_agent_id,
                "tools_config_override": updated_tools_config
            });

            // Update the agent
            let (update_sender, update_receiver) = async_channel::bounded(1);
            node1_commands_sender
                .send(NodeCommand::V2ApiUpdateAgent {
                    bearer: api_key_bearer.clone(),
                    partial_agent: update_data,
                    res: update_sender,
                })
                .await
                .unwrap();

            // Verify the agent was updated
            match update_receiver.recv().await.unwrap() {
                Ok(_) => eprintln!("Agent updated successfully"),
                Err(e) => panic!("Failed to update agent: {:?}", e),
            }

            // Retrieve the agent again to verify the updated tools_config_override
            let (get_sender2, get_receiver2) = async_channel::bounded(1);
            node1_commands_sender
                .send(NodeCommand::V2ApiGetAgent {
                    bearer: api_key_bearer.clone(),
                    agent_id: node1_agent_id.to_string(),
                    res: get_sender2,
                })
                .await
                .unwrap();

            // Verify the retrieved agent has the updated tools_config_override
            match get_receiver2.recv().await.unwrap() {
                Ok(agent) => {
                    eprintln!("Updated agent retrieved successfully");

                    // Assert agent has tools_config_override
                    assert!(
                        agent.tools_config_override.is_some(),
                        "Agent should have tools_config_override"
                    );

                    let config_map = agent.tools_config_override.unwrap();
                    assert!(
                        config_map.contains_key(tool_router_key),
                        "tools_config_override should contain the configured tool"
                    );

                    let tool_config = &config_map[&tool_router_key.to_string()];
                    assert_eq!(
                        tool_config.get("api_key").unwrap().as_str().unwrap(),
                        "updated-api-key",
                        "api_key should be updated to the new value"
                    );
                    assert_eq!(
                        tool_config.get("timeout").unwrap().as_i64().unwrap(),
                        60,
                        "timeout should be updated to the new value"
                    );
                }
                Err(e) => panic!("Failed to retrieve updated agent: {:?}", e),
            }

            // Test tool execution with tools_config_override
            eprintln!("\n\n### Testing tool execution using the tools_config_override\n\n");

            // Create parameters for tool execution
            let mut parameters = Map::new();
            parameters.insert("query".to_string(), Value::String("test query".to_string()));

            // Create extra_config for tool execution
            let extra_config = Map::new();

            // Execute the tool
            let result = crate::it::utils::node_test_api::api_execute_tool(
                node1_commands_sender.clone(),
                api_key_bearer.clone(),
                tool_router_key.to_string(),
                parameters,
                "shinkai__echo".to_string(),
                "test_app".to_string(),
                Some(node1_agent_id.to_string()),
                node1_llm_provider_id.to_string(),
                extra_config,
                Map::new(),
            )
            .await;

            assert!(result.is_ok(), "Tool execution should succeed");
            // We're not validating the actual tool execution results here, just
            // making sure the API flows work correctly
            eprintln!("Tool execution result: {:?}", result);
        }
        _abort_handler.abort();
    });
}
