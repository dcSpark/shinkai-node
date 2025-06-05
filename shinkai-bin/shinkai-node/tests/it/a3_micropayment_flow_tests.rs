use async_channel::{bounded, Receiver, Sender};
use serde_json::Value;
use shinkai_http_api::node_api_router::APIError;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::invoices::{Invoice, InvoiceStatusEnum};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tool_offering::{
    ShinkaiToolOffering, ToolPrice, UsageType, UsageTypeInquiry
};
use shinkai_message_primitives::schemas::wallet_complementary::{WalletRole, WalletSource};
use shinkai_message_primitives::schemas::x402_types::{self, Network, PaymentRequirements};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair
};
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string, unsafe_deterministic_signature_keypair
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::Node;
use shinkai_tools_primitives::tools::deno_tools::DenoTool;
use shinkai_tools_primitives::tools::network_tool::NetworkTool;
use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader, ShinkaiToolWithAssets};
use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

use super::utils::node_test_api::api_registration_device_node_profile_main;
use super::utils::node_test_local::local_registration_profile_node;
use crate::it::utils::db_handlers::setup;
use crate::it::utils::node_test_api::wait_for_default_tools;
use crate::it::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

// Helper function to provide the JSON for the new Deno tool
fn get_new_deno_tool_json_string() -> String {
    r#"{
        "content": [
            {
                "activated": false,
                "assets": [],
                "author": "@@localhost.sep-shinkai",
                "config": [],
                "configFormData": {},
                "configurations": {
                    "properties": {},
                    "required": [],
                    "type": "object"
                },
                "description": "A function that echoes back the input message.",
                "embedding": null,
                "file_inbox": null,
                "homepage": null,
                "input_args": {
                    "properties": {
                        "message": {
                            "description": "The message to echo",
                            "type": "string"
                        }
                    },
                    "required": [
                        "message"
                    ],
                    "type": "object"
                },
                "js_code": "type CONFIG = {};\ntype INPUTS = { message: string };\ntype OUTPUT = { echoed: string };\n\nexport async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> {\n    return { echoed: inputs.message };\n}",
                "keywords": [
                    "echo",
                    "message",
                    "repeat"
                ],
                "mcp_enabled": false,
                "name": "Echo Function",
                "oauth": [],
                "operating_system": [
                    "linux",
                    "macos",
                    "windows"
                ],
                "output_arg": {
                    "json": "{}"
                },
                "result": {
                    "properties": {
                        "echoed": {
                            "description": "The echoed message",
                            "type": "string"
                        }
                    },
                    "required": [
                        "echoed"
                    ],
                    "type": "object"
                },
                "runner": "any",
                "sql_queries": [],
                "sql_tables": [],
                "tool_router_key": "local:::__localhost_sep_shinkai:::echo_function",
                "tool_set": "",
                "tools": [],
                "version": "1.0.0"
            },
            true
        ],
        "type": "Deno"
    }"#.to_string()
}

#[cfg(feature = "console")]
use console_subscriber;

#[test]
fn micropayment_flow_test() {
    #[cfg(feature = "console")]
    {
        console_subscriber::init();
        eprintln!("> tokio-console is enabled");
    }

    std::env::set_var("WELCOME_MESSAGE", "false");
    std::env::set_var("ONLY_TESTING_JS_TOOLS", "true");
    std::env::set_var("SKIP_IMPORT_FROM_DIRECTORY", "true");
    std::env::set_var("IS_TESTING", "1");
    std::env::set_var("ADD_TESTING_NETWORK_ECHO", "true");
    std::env::set_var("ADD_TESTING_EXTERNAL_NETWORK_ECHO", "true");

    setup();
    let rt = Runtime::new().unwrap();

    let e = rt.block_on(async {
        let node1_identity_name = "@@node1_test.sep-shinkai";
        let node2_identity_name = "@@node2_test.sep-shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";
        let node2_profile_name = "main";

        let api_v2_key = "Human";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let _node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let node2_encryption_sk_clone = node2_encryption_sk.clone();

        let _node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);
        let _node2_identity_sk_clone = clone_signature_secret_key(&node2_identity_sk);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node2_profile_identity_sk, node2_profile_identity_pk) = unsafe_deterministic_signature_keypair(101);
        let (node2_profile_encryption_sk, node2_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(101);

        let node1_subencryption_sk_clone = node1_profile_encryption_sk.clone();
        let node2_subencryption_sk_clone = node2_profile_encryption_sk.clone();

        let _node1_subidentity_sk_clone = clone_signature_secret_key(&node1_profile_identity_sk);
        let _node2_subidentity_sk_clone = clone_signature_secret_key(&node2_profile_identity_sk);

        let (node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name));
        let node2_db_path = format!("db_tests/{}", hash_string(node2_identity_name));

        // Create node1 and node2
        fn port_is_available(port: u16) -> bool {
            match TcpListener::bind(("127.0.0.1", port)) {
                Ok(_) => true,
                Err(_) => false,
            }
        }

        // Create node1 and node2
        assert!(port_is_available(12000), "Port 12000 is not available");
        assert!(port_is_available(12001), "Port 12001 is not available");
        
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk,
            None,
            None,
            0,
            node1_commands_receiver,
            node1_db_path,
            "".to_string(),
            None,
            true,
            vec![],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some(api_v2_key.to_string()),
        )
        .await;

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12001);
        let node2 = Node::new(
            node2_identity_name.to_string(),
            addr2,
            clone_signature_secret_key(&node2_identity_sk),
            node2_encryption_sk,
            None,
            None,
            0,
            node2_commands_receiver,
            node2_db_path,
            "".to_string(),
            None,
            true,
            vec![],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some(api_v2_key.to_string()),
        )
        .await;

        // Printing
        eprintln!(
            "Node 1 encryption sk: {:?}",
            encryption_secret_key_to_string(node1_encryption_sk_clone2.clone())
        );
        eprintln!(
            "Node 1 encryption pk: {:?}",
            encryption_public_key_to_string(node1_encryption_pk)
        );

        eprintln!(
            "Node 2 encryption sk: {:?}",
            encryption_secret_key_to_string(node2_encryption_sk_clone)
        );
        eprintln!(
            "Node 2 encryption pk: {:?}",
            encryption_public_key_to_string(node2_encryption_pk)
        );

        eprintln!(
            "Node 1 identity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node1_identity_sk))
        );
        eprintln!(
            "Node 1 identity pk: {:?}",
            signature_public_key_to_string(node1_identity_pk)
        );

        eprintln!(
            "Node 2 identity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node2_identity_sk))
        );
        eprintln!(
            "Node 2 identity pk: {:?}",
            signature_public_key_to_string(node2_identity_pk)
        );

        eprintln!(
            "Node 1 subidentity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node1_profile_identity_sk))
        );
        eprintln!(
            "Node 1 subidentity pk: {:?}",
            signature_public_key_to_string(node1_profile_identity_pk)
        );

        eprintln!(
            "Node 2 subidentity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node2_profile_identity_sk))
        );
        eprintln!(
            "Node 2 subidentity pk: {:?}",
            signature_public_key_to_string(node2_profile_identity_pk)
        );

        eprintln!(
            "Node 1 subencryption sk: {:?}",
            encryption_secret_key_to_string(node1_subencryption_sk_clone.clone())
        );
        eprintln!(
            "Node 1 subencryption pk: {:?}",
            encryption_public_key_to_string(node1_profile_encryption_pk)
        );

        eprintln!(
            "Node 2 subencryption sk: {:?}",
            encryption_secret_key_to_string(node2_subencryption_sk_clone.clone())
        );
        eprintln!(
            "Node 2 subencryption pk: {:?}",
            encryption_public_key_to_string(node2_profile_encryption_pk)
        );

        eprintln!("Starting nodes");
        // Start node1 and node2
        let node1_clone = Arc::clone(&node1);
        let node1_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 1");
            let _ = node1_clone.lock().await.start().await;
        });

        let node1_abort_handler = node1_handler.abort_handle();

        let node2_clone = Arc::clone(&node2);
        let node2_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 2");
            let _ = node2_clone.lock().await.start().await;
        });
        let node2_abort_handler = node2_handler.abort_handle();

        let interactions_handler = tokio::spawn(async move {
            eprintln!("Starting interactions");
            eprintln!("Registration of Subidentities");

            // Register a Profile in Node1 and verifies it
            {
                eprintln!("Register a Device with main profile in Node1 and verify it");
                api_registration_device_node_profile_main(
                    node1_commands_sender.clone(),
                    node1_profile_name,
                    node1_identity_name,
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name,
                )
                .await;
            }

            // Register a Profile in Node2 and verifies it
            {
                eprintln!("Register a Profile in Node2 and verify it");
                local_registration_profile_node(
                    node2_commands_sender.clone(),
                    node2_profile_name,
                    node2_identity_name,
                    node2_subencryption_sk_clone.clone(),
                    node2_encryption_pk,
                    clone_signature_secret_key(&node2_profile_identity_sk),
                    1,
                )
                .await;
            }

            {
                // Wait for default tools to be ready Node 1
                let tools_ready = wait_for_default_tools(
                    node1_commands_sender.clone(),
                    api_v2_key.to_string(),
                    120, // Wait up to 20 seconds
                )
                .await
                .expect("Failed to check for default tools");
                assert!(tools_ready, "Default tools should be ready within 20 seconds");

                // Wait for default tools to be ready Node 2
                let tools_ready = wait_for_default_tools(
                    node2_commands_sender.clone(),
                    api_v2_key.to_string(),
                    120, // Wait up to 20 seconds
                )
                .await
                .expect("Failed to check for default tools");
                assert!(tools_ready, "Default tools should be ready within 20 seconds");
            }

            tokio::time::sleep(Duration::from_secs(5)).await;

            // ASCII Art
            eprintln!(
                "
                +--------------------------+       +--------------------------+
                |  Node 1                  |       |  Node 2                  |
                |  Agent Offering Provider |       |  Subscriber to Agent     |
                |                          |       |  Offering                |
                +--------------------------+       +--------------------------+
                "
            );

            // TODO:
            // Add tool to node1 (Done automatically)
            // and make it available with an offering (Done)

            // Add wallet to node2 and node1
            // Add network tool to node2
            // node2 does a vector search and finds the tool to do X
            // it asks for a quote to node1 <- here
            // node1 computes the quote and sends it to node2
            // node2 receives the quote, makes the payment
            // node 2 sends the payment receipt to node1 with the data to process X
            // node1 processes X and sends the result to node2
            // node2 receives the result and stores it
            // done

            let test_network_tool_name = "__node1_test_sep_shinkai:::__localhost_sep_shinkai:::echo_function";
            let test_local_tool_key_name = "local:::__localhost_sep_shinkai:::echo_function";

            let shinkai_tool_offering = ShinkaiToolOffering {
                tool_key: test_local_tool_key_name.to_string(),
                usage_type: UsageType::PerUse(ToolPrice::Payment(vec![PaymentRequirements {
                    scheme: "exact".to_string(),
                    description: "Echo tool payment".to_string(),
                    network: Network::BaseSepolia,
                    max_amount_required: "1".to_string(), // This does "include decimals"
                    resource: "https://shinkai.com".to_string(),
                    mime_type: "application/json".to_string(),
                    pay_to: "0xd68b44BcAB515C326226392922fC08c3C4913746".to_string(),
                    max_timeout_seconds: 300,
                    asset: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(),
                    output_schema: Some(serde_json::json!({})),
                    extra: Some(serde_json::json!({
                        "name": "USDC",
                        "version": "2"
                    })),
                }])),
                meta_description: Some("Echo tool offering".to_string()),
            };

            let mut input_args = Parameters::new();
            input_args.add_property(
                "message".to_string(),
                "string".to_string(),
                "The message to echo".to_string(),
                true,
                None,
            );

            let shinkai_tool_header = ShinkaiToolHeader {
                name: "Echo Function".to_string(),

                description: "A function that echoes back the input message.".to_string(),
                tool_router_key: test_local_tool_key_name.to_string(),
                tool_type: "Deno".to_string(),
                mcp_enabled: Some(false),
                formatted_tool_summary_for_ui:
                    "Tool Name: Echo Function\nAuthor: @@localhost.sep-shinkai\nDescription: A function that echoes back the input message."
                        .to_string(),
                input_args: input_args.clone(),
                output_arg: ToolOutputArg::empty(),
                author: "@@localhost.sep-shinkai".to_string(),
                version: "1.0.0".to_string(),
                enabled: true,
                config: Some(vec![]),
                usage_type: None,
                tool_offering: Some(shinkai_tool_offering.clone()),
            };

            {
                eprintln!("Add tool to node1");

                let new_tool_json_str = get_new_deno_tool_json_string();
                let parsed_json: serde_json::Value =
                    serde_json::from_str(&new_tool_json_str).expect("Failed to parse new tool JSON");

                let tool_content = parsed_json["content"][0].clone();
                let is_active = parsed_json["content"][1]
                    .as_bool()
                    .expect("is_active flag not found or not a boolean");

                let deno_tool: DenoTool =
                    serde_json::from_value(tool_content).expect("Failed to deserialize DenoTool from JSON");
                let shinkai_tool = ShinkaiTool::Deno(deno_tool, is_active);

                let (sender, receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiAddShinkaiTool {
                        bearer: api_v2_key.to_string(),
                        shinkai_tool: ShinkaiToolWithAssets {
                            tool: shinkai_tool,
                            assets: None,
                        },
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp add deno js tool to node2: {:?}", resp);
                assert!(resp.is_ok(), "Failed to add Deno JS Tool to node2: {:?}", resp.err());

                // Note: just for testing purposes, we will not list all tools
                // List all Shinkai tools
                let (sender, receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiListAllShinkaiTools {
                        bearer: api_v2_key.to_string(),
                        category: None,
                        res: sender,
                    })
                    .await
                    .unwrap();
                let all_tools_resp = receiver.recv().await.unwrap();
                eprintln!("\nAll available Shinkai tools (name, tool_router_key):");
                
                if let Ok(tools) = &all_tools_resp {
                    if let Some(array) = tools.as_array() {
                        for tool in array {
                            let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                            let tool_router_key = tool.get("tool_router_key").and_then(|v| v.as_str()).unwrap_or("-");
                            eprintln!("name: {}, tool_router_key: {}", name, tool_router_key);
                        }
                    }
                }

                // // Retrieve the shinkai_tool from node1
                // let (sender, receiver) = async_channel::bounded(1);
                // node1_commands_sender
                //     .send(NodeCommand::V2ApiGetShinkaiTool {
                //         bearer: api_v2_key.to_string(),
                //         payload: "__node1_test_sep_shinkai:::__official_shinkai:::echo_function".to_string(),
                //         serialize_config: false,
                //         res: sender,
                //     })
                //     .await
                //     .unwrap();
                // let resp = receiver.recv().await.unwrap();

                // // Print only minimal fields from the response
                // if let Ok(obj) = &resp {
                //     // Try to get the "content" field if it exists
                //     let content = obj.get("content").unwrap_or(obj);
                //     if let Some(name) = content.get("name") {
                //         let tool_router_key = content.get("tool_router_key").and_then(|v| v.as_str()).unwrap_or("-");
                //         eprintln!(
                //             "name: {}, tool_router_key: {}",
                //             name.as_str().unwrap_or("-"),
                //             tool_router_key
                //         );
                //     } else if let Some(array) = content.as_array() {
                //         for tool in array {
                //             let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                //             let tool_router_key = tool.get("tool_router_key").and_then(|v|
                // v.as_str()).unwrap_or("-");             eprintln!("name: {}, tool_router_key: {}",
                // name, tool_router_key);         }
                //     }
                // }

                // Add Offering
                let (sender, receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiSetToolOffering {
                        bearer: api_v2_key.to_string(),
                        tool_offering: shinkai_tool_offering.clone(),
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp set tool offering: {:?}", resp);
                if let Err(api_error) = &resp {
                    // Assuming api_error is of a type that has a `message` field (e.g., APIError from shinkai_http_api)
                    if api_error.message == "Tool does not exist" {
                        assert!(
                            false,
                            "Set tool offering failed because the tool does not exist. Full error: {:?}",
                            api_error
                        );
                    }
                }
                assert!(
                    resp.is_ok(),
                    "Set tool offering failed with an unexpected error: {:?}",
                    resp.err()
                );
            }
            {
                // Check if the tool is available
                let (sender, receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiGetAllToolOfferings {
                        bearer: api_v2_key.to_string(),
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp get all tool offerings: {:?}", resp);

                let expected_response = vec![shinkai_tool_header.clone()];

                match resp {
                    Ok(actual_response) => assert_eq!(actual_response, expected_response),
                    Err(e) => assert!(false, "Expected Ok, got Err: {:?}", e),
                }
            }
            {
                eprintln!("Add wallet to node1");
                // Add wallet to node1
                let (sender, receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiCreateLocalEthersWallet {
                        bearer: api_v2_key.to_string(),
                        network: x402_types::Network::BaseSepolia,
                        role: WalletRole::Both,
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp add wallet to node1: {:?}", resp);
            }
            {
                eprintln!("Add wallet to node2");
                // Local Ethers Wallet
                // Add wallet to node2
                let (sender, receiver) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::V2ApiRestoreLocalEthersWallet {
                        bearer: api_v2_key.to_string(),
                        network: x402_types::Network::BaseSepolia,
                        source: WalletSource::Mnemonic(std::env::var("RESTORE_WALLET_MNEMONICS_NODE2").unwrap()),
                        role: WalletRole::Both,
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp restore wallet to node2: {:?}", resp);

                // Coinbase MPC Wallet
                // For Development purposes, we use the Coinbase MPC Wallet
                // Add wallet to node2
                // let (sender, receiver) = async_channel::bounded(1);
                // node2_commands_sender
                //     .send(NodeCommand::V2ApiRestoreCoinbaseMPCWallet {
                //         bearer: api_v2_key.to_string(),
                //         network: Network::BaseSepolia,
                //         config: None,
                //         wallet_id: std::env::var("COINBASE_API_WALLET_ID").unwrap(),
                //         role: WalletRole::Both,
                //         res: sender,
                //     })
                //     .await
                //     .unwrap();

                // let resp = receiver.recv().await.unwrap();
                // eprintln!("resp restore wallet to node2: {:?}", resp);

                // Check if the response is an error and panic if it is
                // if let Err(e) = resp {
                //     assert!(false, "Failed to restore wallet: {:?}", e);
                // }
            }
            {
                eprintln!("Add network tool to node2");

                // Convert ShinkaiToolHeader to ShinkaiTool
                // Manually create NetworkTool
                let provider = ShinkaiName::new(node1_identity_name.to_string()).unwrap();
                let tool_router_key = ToolRouterKey::new(
                    provider.to_string(),
                    shinkai_tool_header.author.clone(),
                    shinkai_tool_header.name.clone(),
                    None,
                );
                let network_tool = NetworkTool {
                    name: shinkai_tool_header.name.clone(),
                    author: shinkai_tool_header.author.clone(),
                    description: shinkai_tool_header.description.clone(),
                    version: shinkai_tool_header.version.clone(),
                    mcp_enabled: shinkai_tool_header.mcp_enabled.clone(),
                    provider,
                    tool_router_key: tool_router_key.to_string_without_version(),
                    usage_type: shinkai_tool_offering.usage_type.clone(),
                    activated: shinkai_tool_header.enabled,
                    config: shinkai_tool_header.config.clone().unwrap_or_default(),
                    input_args: Parameters::new(),
                    output_arg: ToolOutputArg::empty(),
                    embedding: None,
                    restrictions: None,
                };

                let shinkai_tool = ShinkaiTool::Network(network_tool, true);

                let serialized_shinkai_tool = serde_json::to_value(&shinkai_tool).unwrap();
                eprintln!("serialized_shinkai_tool: {:?}", serialized_shinkai_tool);

                // Add the ShinkaiTool to node2
                let (sender, receiver) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::V2ApiAddShinkaiTool {
                        bearer: api_v2_key.to_string(),
                        shinkai_tool: ShinkaiToolWithAssets {
                            tool: shinkai_tool,
                            assets: None,
                        },
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp add shinkai tool to node2: {:?}", resp);

                // List all Shinkai tools
                let (sender, receiver) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::V2ApiListAllShinkaiTools {
                        bearer: api_v2_key.to_string(),
                        category: None,
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("\nAll available Shinkai tools in node2 (name, tool_router_key):");

                if let Ok(tools) = &resp {
                    if let Some(array) = tools.as_array() {
                        for tool in array {
                            let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("-");
                            let tool_router_key = tool.get("tool_router_key").and_then(|v| v.as_str()).unwrap_or("-");
                            eprintln!("name: {}, tool_router_key: {}", name, tool_router_key);
                        }
                    }
                }
            }
            {
                eprintln!("Search for 'echo' tool in node2");

                // Search for the tool using the V2ApiSearchShinkaiTool command
                let (sender, receiver) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::V2ApiSearchShinkaiTool {
                        bearer: api_v2_key.to_string(),
                        query: "echo".to_string(),
                        agent_or_llm: None,
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp search shinkai tool: {:?}", resp);

                // Print only the tool_router_key for each tool in the search results
                match &resp {
                    Ok(tools) => {
                        if let Some(array) = tools.as_array() {
                            eprintln!("Search results (tool_router_key):");
                            for tool in array {
                                let tool_router_key = tool.get("tool_router_key").and_then(|v| v.as_str()).unwrap_or("-");
                                eprintln!("tool_router_key: {}", tool_router_key);
                            }
                        }
                    }
                    Err(e) => eprintln!("Error searching tools: {:?}", e),
                }
            }

            //
            // Second Part of the Test
            //
            //      _   _      _                      _
            //     | \ | |    | |                    | |
            //     |  \| | ___| |___      _____  _ __| | __
            //     | . ` |/ _ \ __\ \ /\ / / _ \| '__| |/ /
            //     | |\  |  __/ |_ \ V  V / (_) | |  |   <
            //     |_| \_|\___|\__| \_/\_/ \___/|_|  |_|\_\
            //
            //

            let invoice_id: String = {
                eprintln!("Requesting invoice for \'echo_function\' tool from node2");

                // Request an invoice using the V2ApiRequestInvoice command
                let (sender, receiver) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::V2ApiRequestInvoice {
                        bearer: api_v2_key.to_string(),
                        tool_key_name: test_network_tool_name.to_string(),
                        usage: UsageTypeInquiry::PerUse,
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp request invoice: {:?}", resp);

                // Handle the response
                match resp {
                    Ok(invoice_resp) => {
                        eprintln!("Received invoice: {:?}", invoice_resp);
                        invoice_resp["unique_id"].as_str().unwrap().to_string()
                    }
                    Err(e) => panic!("Failed to request invoice: {:?}", e),
                }
            };
            // TODO: we need to wait for the invoice to be created and received by node2!
            {
                eprintln!("Waiting for invoice to be created and received by node2");

                let mut found_invoice = false;
                for _ in 0..20 {
                    let (sender, receiver) = async_channel::bounded(1);
                    node2_commands_sender
                        .send(NodeCommand::V2ApiListInvoices {
                            bearer: api_v2_key.to_string(),
                            res: sender,
                        })
                        .await
                        .unwrap();
                    let resp = receiver.recv().await.unwrap();
                    eprintln!("resp list invoices: {:?}", resp);

                    if let Ok(invoices) = resp {
                        if let Some(invoices_array) = invoices.as_array() {
                            if invoices_array
                                .iter()
                                .any(|inv| inv["invoice_id"].as_str() == Some(&invoice_id))
                            {
                                found_invoice = true;
                                break;
                            }
                        }
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                if !found_invoice {
                    assert!(false, "Invoice not found after waiting");
                }
            }
            {
                eprintln!("Paying invoice for 'network__echo' tool from node2");

                let (sender, receiver) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::V2ApiPayInvoice {
                        bearer: api_v2_key.to_string(),
                        invoice_id: invoice_id.clone(),
                        data_for_tool: serde_json::json!({ "message": "Hello, Shinkai!" }),
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp pay invoice: {:?}", resp);

                // Handle the response
                match resp {
                    Ok(payment_receipt) => eprintln!("Payment successful: {:?}", payment_receipt),
                    Err(e) => assert!(false, "Failed to pay invoice: {:?}", e),
                }
            }
            // Optional but it could help to debug in between issues
            // TODO?: I need another loop command to check if the result was processed by node1?
            // TODO: Check in node2 if it received the response from node1 of the tool execution

            // Check if the invoice is processed and has a result on node2
            {
                eprintln!("Waiting for invoice to be processed and have a result on node2");

                let mut found_processed_invoice = false;
                let mut resp: Result<Value, APIError> = Ok(serde_json::json!({}));
                for _ in 0..20 {
                    let (sender, receiver) = async_channel::bounded(1);
                    node2_commands_sender
                        .send(NodeCommand::V2ApiListInvoices {
                            bearer: api_v2_key.to_string(),
                            res: sender,
                        })
                        .await
                        .unwrap();
                    resp = receiver.recv().await.unwrap();

                    if let Ok(invoices) = resp.clone() {
                        if let Some(invoices_array) = invoices.as_array() {
                            for inv in invoices_array {
                                if let Ok(invoice) = serde_json::from_value::<Invoice>(inv.clone()) {
                                    if invoice.invoice_id == invoice_id
                                        && invoice.status == InvoiceStatusEnum::Processed
                                        && invoice.result_str.is_some()
                                    {
                                        found_processed_invoice = true;
                                        eprintln!("Found processed invoice: {:?}", invoice);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if found_processed_invoice {
                        eprintln!("resp list invoices on node2: {:?}", resp);
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                if !found_processed_invoice {
                    eprintln!("resp list invoices on node2: {:?}", resp);
                    assert!(false, "Processed invoice with result not found after waiting");
                }
            }

            node1_abort_handler.abort();
            node2_abort_handler.abort();
        });

        let result = tokio::try_join!(node1_handler, node2_handler, interactions_handler);
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
}
