use async_channel::{bounded, Receiver, Sender};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
};
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_node::network::agent_payments_manager::shinkai_tool_offering::{
    AssetPayment, ShinkaiToolOffering, ToolPrice, UsageType, UsageTypeInquiry,
};
use shinkai_node::network::node_commands::NodeCommand;
use shinkai_node::network::Node;
use shinkai_node::tools::argument::ToolArgument;
use shinkai_node::tools::network_tool::NetworkTool;
use shinkai_node::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader};
use shinkai_node::wallet::mixed::{Asset, NetworkIdentifier};
use shinkai_node::wallet::wallet_manager::WalletRole;
use shinkai_vector_resources::utils::hash_string;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

use super::utils::node_test_api::api_registration_device_node_profile_main;
use super::utils::node_test_local::local_registration_profile_node;
use crate::it::utils::db_handlers::setup;
use crate::it::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

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
    std::env::set_var("ONLY_TESTING_WORKFLOWS", "true");

    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.arb-sep-shinkai";
        let node2_identity_name = "@@node2_test.arb-sep-shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";
        let node2_profile_name = "main_profile_node2";

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
        let node1_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node1_identity_name));
        let node2_db_path = format!("db_tests/{}", hash_string(node2_identity_name));
        let node2_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node2_identity_name));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk,
            0,
            node1_commands_receiver,
            node1_db_path,
            "".to_string(),
            None,
            true,
            vec![],
            node1_fs_db_path,
            None,
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some(api_v2_key.to_string()),
        )
        .await;

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let node2 = Node::new(
            node2_identity_name.to_string(),
            addr2,
            clone_signature_secret_key(&node2_identity_sk),
            node2_encryption_sk,
            0,
            node2_commands_receiver,
            node2_db_path,
            "".to_string(),
            None,
            true,
            vec![],
            node2_fs_db_path,
            None,
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

            tokio::time::sleep(Duration::from_secs(3)).await;

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

            let test_network_tool_name = "@@node1_test.arb-sep-shinkai:::shinkai-tool-echo:::network__echo";
            let test_local_tool_key_name = "local:::shinkai-tool-echo:::network__echo";

            let shinkai_tool_offering = ShinkaiToolOffering {
                tool_key: test_local_tool_key_name.to_string(),
                usage_type: UsageType::PerUse(ToolPrice::Payment(vec![AssetPayment {
                    asset: Asset {
                        network_id: NetworkIdentifier::BaseSepolia,
                        asset_id: "USDC".to_string(),
                        decimals: Some(6),
                        contract_address: Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string()),
                    },
                    amount: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
                }])),
                meta_description: Some("Echo tool offering".to_string()),
            };

            let shinkai_tool_header = ShinkaiToolHeader {
                name: "network__echo".to_string(),
                toolkit_name: "shinkai-tool-echo".to_string(),
                description: "Echoes the input message".to_string(),
                tool_router_key: test_local_tool_key_name.to_string(),
                tool_type: "JS".to_string(),
                formatted_tool_summary_for_ui:
                    "Tool Name: network__echo\nToolkit Name: shinkai-tool-echo\nDescription: Echoes the input message"
                        .to_string(),
                input_args: vec![ToolArgument {
                    arg_type: "string".to_string(),
                    description: "".to_string(),
                    is_required: true,
                    name: "message".to_string(),
                }],
                author: "Shinkai".to_string(),
                version: "v0.1".to_string(),
                enabled: true,
                config: Some(vec![]),
                usage_type: None,
                tool_offering: Some(shinkai_tool_offering.clone()),
            };

            {
                eprintln!("Add tool to node1");
                // List all Shinkai tools
                let (sender, receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiListAllShinkaiTools {
                        bearer: api_v2_key.to_string(),
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp list all shinkai tools: {:?}", resp);

                // Retrieve the shinkai_tool from node1
                let (sender, receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiGetShinkaiTool {
                        bearer: api_v2_key.to_string(),
                        payload: "local:::shinkai-tool-echo:::shinkai__echo".to_string(),
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp get shinkai tool: {:?}", resp);

                // Modify the tool_key
                let mut shinkai_tool = match resp {
                    Ok(tool) => serde_json::from_value::<ShinkaiTool>(tool).unwrap(),
                    Err(e) => panic!("Failed to retrieve shinkai tool: {:?}", e),
                };

                if let ShinkaiTool::JS(ref mut js_tool, _) = shinkai_tool {
                    js_tool.name = "network__echo".to_string();
                }

                // Add the modified ShinkaiTool to node1
                let (sender, receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiAddShinkaiTool {
                        bearer: api_v2_key.to_string(),
                        shinkai_tool,
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp add modified shinkai tool to node1: {:?}", resp);

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
                    Err(e) => panic!("Expected Ok, got Err: {:?}", e),
                }
            }
            {
                eprintln!("Add wallet to node1");
                // Add wallet to node1
                let (sender, receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::V2ApiCreateLocalEthersWallet {
                        bearer: api_v2_key.to_string(),
                        network: NetworkIdentifier::BaseSepolia,
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
                // // Add wallet to node2
                // let (sender, receiver) = async_channel::bounded(1);
                // node2_commands_sender
                //     .send(NodeCommand::V2ApiRestoreLocalEthersWallet {
                //         bearer: api_v2_key.to_string(),
                //         network: NetworkIdentifier::BaseSepolia,
                //         source: WalletSource::Mnemonic(std::env::var("RESTORE_WALLET_MNEMONICS_NODE2").unwrap()),
                //         role: WalletRole::Both,
                //         res: sender,
                //     })
                //     .await
                //     .unwrap();
                // let resp = receiver.recv().await.unwrap();
                // eprintln!("resp restore wallet to node2: {:?}", resp);

                // Coinbase MPC Wallet
                // Add wallet to node2
                let (sender, receiver) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::V2ApiRestoreCoinbaseMPCWallet {
                        bearer: api_v2_key.to_string(),
                        network: NetworkIdentifier::BaseSepolia,
                        config: None,
                        wallet_id: std::env::var("COINBASE_API_WALLET_ID").unwrap(),
                        role: WalletRole::Both,
                        res: sender,
                    })
                    .await
                    .unwrap();

                let resp = receiver.recv().await.unwrap();
                eprintln!("resp restore wallet to node2: {:?}", resp);

                // Check if the response is an error and panic if it is
                if let Err(e) = resp {
                    panic!("Failed to restore wallet: {:?}", e);
                }
            }
            {
                eprintln!("Add network tool to node2");

                // Convert ShinkaiToolHeader to ShinkaiTool
                // Manually create NetworkTool
                let network_tool = NetworkTool {
                    name: shinkai_tool_header.name.clone(),
                    toolkit_name: shinkai_tool_header.toolkit_name.clone(),
                    description: shinkai_tool_header.description.clone(),
                    version: shinkai_tool_header.version.clone(),
                    provider: ShinkaiName::new(node1_identity_name.to_string()).unwrap(),
                    usage_type: shinkai_tool_offering.usage_type.clone(),
                    activated: shinkai_tool_header.enabled,
                    config: shinkai_tool_header.config.clone().unwrap_or_default(),
                    input_args: vec![],
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
                        shinkai_tool,
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
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp list all shinkai tools in node2: {:?}", resp);

                // Assert that "network__echo" is in the list of tools
                match resp {
                    Ok(tools) => {
                        let tool_names: Vec<String> = tools
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|tool| tool["name"].as_str().unwrap().to_string())
                            .collect();
                        assert!(
                            tool_names.contains(&"network__echo".to_string()),
                            "network__echo tool not found"
                        );
                    }
                    Err(e) => panic!("Expected Ok, got Err: {:?}", e),
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
                        res: sender,
                    })
                    .await
                    .unwrap();
                let resp = receiver.recv().await.unwrap();
                eprintln!("resp search shinkai tool: {:?}", resp);

                // Assert that "network__echo" is in the search results
                match resp {
                    Ok(tools) => {
                        let tool_names: Vec<String> = tools
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|tool| tool["name"].as_str().unwrap().to_string())
                            .collect();
                        assert!(
                            tool_names.contains(&"network__echo".to_string()),
                            "network__echo tool not found in search results"
                        );
                    }
                    Err(e) => panic!("Expected Ok, got Err: {:?}", e),
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

            let invoice_id: String;
            {
                eprintln!("Requesting invoice for 'network__echo' tool from node2");

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
                        invoice_id = invoice_resp["unique_id"].as_str().unwrap().to_string();
                    }
                    Err(e) => panic!("Failed to request invoice: {:?}", e),
                }
            }
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
                    panic!("Invoice not found after waiting");
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
                    Err(e) => panic!("Failed to pay invoice: {:?}", e),
                }
            }
            // TODO?: I need another loop command to check if the result was processed by node1?
            // Note: do I need to store that?

            // TODO: Check in node2 if it received the response from node1 of the tool execution

            // add sleep 5 seconds
            tokio::time::sleep(Duration::from_secs(10000)).await;

            node1_abort_handler.abort();
            node2_abort_handler.abort();
        });

        let _ = tokio::join!(node1_handler, node2_handler, interactions_handler);
    });
}