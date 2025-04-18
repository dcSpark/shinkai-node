use async_channel::{bounded, Receiver, Sender};
use rand::Rng;
use serde_json::{json, Map, Value};
use shinkai_http_api::node_api_router;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::agent::Agent;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, OpenAI, SerializedLLMProvider
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair
};
use shinkai_node::network::Node;
use shinkai_node::tools::tool_implementation::native_tools::sql_processor::get_database_path_from_db_name_config;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};
use tokio::runtime::Runtime;

use crate::it::utils::node_test_api::{
    api_execute_tool, api_registration_device_node_profile_main, wait_for_default_tools
};
use crate::it::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

use mockito::Server;

#[test]
fn test_tool_execution_with_config_override() {
    std::env::set_var("WELCOME_MESSAGE", "false");
    let api_key_bearer = std::env::var("API_V2_KEY").unwrap_or_else(|_| "my_api_v2_key".to_string());
    std::env::set_var("API_V2_KEY", api_key_bearer.clone());
    std::env::set_var("NODE_API_PORT", "9550");

    let rt = Runtime::new().unwrap();
    let server = Server::new();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.sep-shinkai";
        let node1_subidentity_name = "main";
        let node1_device_name = "node1_device";
        let node1_agent_id = "node1_gpt_agent";
        let node1_llm_provider_id = "node1_llm_provider";

        let (node1_identity_sk, _node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let (node1_profile_identity_sk, _node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, _node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let node1_db_path = tempfile::tempdir().unwrap().path().to_str().unwrap().to_string();

        // Create the LLM provider
        let agent_name = ShinkaiName::new(
            format!(
                "{}/{}/agent/{}",
                node1_identity_name, node1_subidentity_name, node1_agent_id
            )
            .to_string(),
        )
        .unwrap();

        let open_ai = OpenAI {
            model_type: "gpt-3.5-turbo-1106".to_string(),
        };

        let llm_provider = SerializedLLMProvider {
            id: node1_llm_provider_id.to_string(),
            full_identity_name: agent_name.clone(),
            external_url: Some(server.url()),
            api_key: Some("mockapikey".to_string()),
            model: LLMProviderInterface::OpenAI(open_ai),
        };

        // Create node
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk,
            None,
            None,
            0,
            node1_commands_receiver,
            node1_db_path.clone(),
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

        let abort_handler = node1_handler.abort_handle();

        // Register device node profile main
        api_registration_device_node_profile_main(
            node1_commands_sender.clone(),
            "main",
            &node1_identity_name,
            node1_encryption_pk,
            node1_device_encryption_sk.clone(),
            node1_device_identity_sk.clone(),
            node1_profile_encryption_sk.clone(),
            node1_profile_identity_sk.clone(),
            &node1_device_name,
        )
        .await;

        // Setup API Server task
        let api_listen_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9550);
        let api_https_listen_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9560);

        let node1_commands_sender_clone = node1_commands_sender.clone();
        let api_server = tokio::spawn(async move {
            if let Err(e) = node_api_router::run_api(
                node1_commands_sender_clone,
                api_listen_address,
                api_https_listen_address,
                node1_identity_name.to_string(),
                None,
                None,
            )
            .await
            {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    &format!("API server failed to start: {}", e),
                );
                panic!("API server failed to start: {}", e);
            }
        });

        // Wait for default tools to be ready
        wait_for_default_tools(node1_commands_sender.clone(), api_key_bearer.clone(), 10)
            .await
            .unwrap();

        let random_database_name = format!("potato_database_{}", rand::thread_rng().gen_range(0..1000000));
        // Create a new agent with tools_config_override
        let mut tools_config_override = HashMap::new();
        let mut config_values = HashMap::new();
        config_values.insert("database_name".to_string(), Value::String(random_database_name.clone()));
        tools_config_override.insert(
            "local:::__official_shinkai:::memory_management".to_string(),
            config_values,
        );

        // Create the agent payload
        let agent_data = json!({
            "name": "Test Agent",
            "agent_id": node1_agent_id,
            "full_identity_name": agent_name.clone(),
            "llm_provider_id": node1_llm_provider_id,
            "ui_description": "Test agent description",
            "knowledge": [],
            "storage_path": "/test/storage/path",
            "tools": ["local:::__official_shinkai:::memory_management:::1.0.0"],
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

        // Create parameters for tool execution
        let mut parameters = Map::new();
        parameters.insert("memory_key".to_string(), Value::String("potato key value".to_string()));

        // Execute the tool with the agent
        let result = api_execute_tool(
            node1_commands_sender.clone(),
            api_key_bearer.clone(),
            "local:::__official_shinkai:::memory_management".to_string(),
            parameters,
            "shinkai__echo".to_string(),
            "test_app".to_string(),
            Some(node1_agent_id.to_string()),
            node1_llm_provider_id.to_string(),
            Map::new(),
            Map::new(),
        )
        .await;

        abort_handler.abort();

        assert!(result.is_ok(), "Tool execution should succeed");

        // Verify the shared SQLite database exists
        let db_file_path = get_database_path_from_db_name_config(random_database_name).unwrap();

        assert!(
            std::path::Path::new(&db_file_path).exists(),
            "Shared SQLite database should exist at {}",
            db_file_path.display()
        );
    });
}
