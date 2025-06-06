use async_channel::{bounded, Receiver, Sender};
use shinkai_http_api::node_api_router::{APIError, SendResponseBodyData};
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair, EncryptionMethod
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string, unsafe_deterministic_signature_keypair
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::Node;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::path::Path;
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;
use hex;

use crate::it::utils::node_test_api::{
    api_registration_device_node_profile_main, api_registration_profile_node, api_try_re_register_profile_node, wait_for_default_tools
};
use crate::it::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

use super::utils::node_test_local::local_registration_profile_node;

const NODE1_IDENTITY_NAME: &str = "@@node1_with_libp2p_relayer.sep-shinkai";
const NODE2_IDENTITY_NAME: &str = "@@node2_with_libp2p_relayer.sep-shinkai";
const RELAY_IDENTITY_NAME: &str = "@@libp2p_relayer.sep-shinkai";

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

#[test]
fn subidentity_registration() {
    std::env::set_var("SKIP_IMPORT_FROM_DIRECTORY", "true");
    std::env::set_var("IS_TESTING", "1");

    setup();
    let rt = Runtime::new().unwrap();

    let e = rt.block_on(async {
        let node1_identity_name = NODE1_IDENTITY_NAME;
        let node2_identity_name = NODE2_IDENTITY_NAME;
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";
        let node2_profile_name = "main";

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

        let (node2_subidentity_sk, node2_subidentity_pk) = unsafe_deterministic_signature_keypair(101);
        let (node2_subencryption_sk, node2_subencryption_pk) = unsafe_deterministic_encryption_keypair(101);

        let node1_subencryption_sk_clone = node1_profile_encryption_sk.clone();
        let node2_subencryption_sk_clone = node2_subencryption_sk.clone();

        let _node1_subidentity_sk_clone = clone_signature_secret_key(&node1_profile_identity_sk);
        let _node2_subidentity_sk_clone = clone_signature_secret_key(&node2_subidentity_sk);

        let (node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name));
        let node2_db_path = format!("db_tests/{}", hash_string(node2_identity_name));

        fn port_is_available(port: u16) -> bool {
            match TcpListener::bind(("127.0.0.1", port)) {
                Ok(_) => true,
                Err(_) => false,
            }
        }

        assert!(port_is_available(12006), "Port 12006 is not available");
        assert!(port_is_available(12007), "Port 12007 is not available");
        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12006);
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
            Some(RELAY_IDENTITY_NAME.to_string()),  // Use relay server for LibP2P networking
            true,
            vec![],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some("debug".to_string()),
        )
        .await;

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12007);
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
            Some(RELAY_IDENTITY_NAME.to_string()),  // Use relay server for LibP2P networking
            true,
            vec![],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some("debug".to_string()),
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
            signature_secret_key_to_string(clone_signature_secret_key(&node2_subidentity_sk))
        );
        eprintln!(
            "Node 2 subidentity pk: {:?}",
            signature_public_key_to_string(node2_subidentity_pk)
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
            encryption_public_key_to_string(node2_subencryption_pk)
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
                    clone_signature_secret_key(&node2_subidentity_sk),
                    1,
                )
                .await;
            }

            tokio::time::sleep(Duration::from_secs(3)).await;
            // Wait for default tools to be ready
            let tools_ready = wait_for_default_tools(
                node1_commands_sender.clone(),
                "debug".to_string(),
                120, // Wait up to 120 seconds
            )
            .await
            .expect("Failed to check for default tools");
            assert!(
                tools_ready,
                "Default tools for Node 1should be ready within 120 seconds"
            );

            // Wait for default tools to be ready
            let tools_ready = wait_for_default_tools(
                node2_commands_sender.clone(),
                "debug".to_string(),
                120, // Wait up to 120 seconds
            )
            .await
            .expect("Failed to check for default tools");
            assert!(
                tools_ready,
                "Default tools for Node 2 should be ready within 120 seconds"
            );

            // Send message from Node 2 subidentity to Node 1
            {
                eprintln!("\n\n### Sending message from a node 2 profile to node 1 profile\n\n");

                let message_content = "test body content".to_string();
                let unchanged_message = ShinkaiMessageBuilder::new(
                    node2_subencryption_sk.clone(),
                    clone_signature_secret_key(&node2_subidentity_sk),
                    node1_profile_encryption_pk,
                )
                .message_raw_content(message_content.clone())
                .no_body_encryption()
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node2_profile_name.to_string().clone(),
                    node1_profile_name.to_string(),
                    EncryptionMethod::DiffieHellmanChaChaPoly1305,
                    None,
                )
                .external_metadata_with_other(
                    node1_identity_name.to_string(),
                    node2_identity_name.to_string().clone(),
                    encryption_public_key_to_string(node2_subencryption_pk),
                )
                .build()
                .unwrap();

                eprintln!("\n\n unchanged message: {:?}", unchanged_message);

                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<SendResponseBodyData, APIError>>,
                    async_channel::Receiver<Result<SendResponseBodyData, APIError>>,
                ) = async_channel::bounded(1);

                node2_commands_sender
                    .send(NodeCommand::SendOnionizedMessage {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();

                let send_result = res_send_msg_receiver.recv().await.unwrap();
                assert!(
                    send_result.is_ok(),
                    "Failed to send onionized message {:?}",
                    send_result
                );
                tokio::time::sleep(Duration::from_secs(4)).await;

                // Get Node2 messages
                let (res2_sender, res2_receiver) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::FetchLastMessages {
                        limit: 2,
                        res: res2_sender,
                    })
                    .await
                    .unwrap();
                let node2_last_messages = res2_receiver.recv().await.unwrap();

                // Get Node1 messages
                let (res1_sender, res1_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::FetchLastMessages {
                        limit: 2,
                        res: res1_sender,
                    })
                    .await
                    .unwrap();
                let node1_last_messages = res1_receiver.recv().await.unwrap();

                eprintln!("\n\nNode 1 last messages: {:?}", node1_last_messages);
                eprintln!("\n\n");
                eprintln!("Node 2 last messages: {:?}", node2_last_messages);
                eprintln!("\n\n");

                let message_to_check = node1_last_messages[0].clone();
                // Check that the message is body encrypted
                assert_eq!(
                    ShinkaiMessage::is_body_currently_encrypted(&message_to_check.clone()),
                    false,
                    "Message from Node 2 to Node 1 is not body encrypted for Node 1 (receiver)"
                );

                let message_to_check = node2_last_messages[0].clone();
                // Check that the message is body encrypted
                assert_eq!(
                    ShinkaiMessage::is_body_currently_encrypted(&message_to_check.clone()),
                    false,
                    "Message from Node 2 to Node 1 is not body encrypted for Node 2 (sender)"
                );

                // Check that the content is encrypted
                eprintln!("Message to check: {:?}", message_to_check.clone());
                assert_eq!(
                    ShinkaiMessage::is_content_currently_encrypted(&message_to_check.clone()),
                    true,
                    "Message from Node 2 to Node 1 is content encrypted"
                );

                {
                    eprintln!("Checking that the message has the right sender {:?}", message_to_check);
                    assert_eq!(
                        message_to_check.get_sender_subidentity().unwrap(),
                        node2_profile_name.to_string(),
                        "Node 2's profile send an encrypted message to Node 1. The message has the right sender."
                    );
                }
                let message_to_check_content_unencrypted = message_to_check
                    .clone()
                    .decrypt_inner_layer(&node1_profile_encryption_sk.clone(), &node2_subencryption_pk)
                    .unwrap();

                // This check can't be done using a static value because the nonce is randomly generated
                assert_eq!(
                    message_content,
                    message_to_check_content_unencrypted.get_message_content().unwrap(),
                    "Node 2's profile send an encrypted message to Node 1"
                );

                assert_eq!(
                    node2_last_messages[0].external_metadata.clone().other,
                    encryption_public_key_to_string(node2_subencryption_pk),
                    "Node 2's profile send an encrypted message to Node 1. Node 2 sends the subidentity's pk in other"
                );

                assert_eq!(
                    node1_last_messages[0].external_metadata.clone().other,
                    encryption_public_key_to_string(node2_subencryption_pk),
                    "Node 2's profile send an encrypted message to Node 1. Node 1 has the other's public key"
                );
                eprintln!("Node 2 sent message to Node 1 successfully");
            }

            // Create Node 1 tries to recreate the same subidentity
            {
                api_try_re_register_profile_node(
                    node1_commands_sender.clone(),
                    node1_profile_name,
                    node1_identity_name,
                    node1_subencryption_sk_clone.clone(),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                )
                .await;
            }

            // Node 1 creates a new subidentity and that subidentity sends a message to the other one in Node 1
            {
                let node1_subidentity_name_2 = "node1_subidentity_2";
                let (_node1_subidentity_sk_2, _node1_subencryption_pk_2) = unsafe_deterministic_signature_keypair(3);
                let (_node1_subencryption_sk_2, node1_subencryption_pk_2) = unsafe_deterministic_encryption_keypair(3);

                eprintln!("Register another Profile in Node1 and verifies it");
                api_registration_profile_node(
                    node1_commands_sender.clone(),
                    node1_subidentity_name_2,
                    node1_identity_name,
                    node1_subencryption_sk_clone.clone(),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    2,
                )
                .await;

                eprintln!(
                    "Sending message from Node 1 subidentity to Node 1 subidentity 2 using the intra_sender feature"
                );
                let message_content =
                    "test encrypted body content from node1 subidentity to node1 subidentity 2".to_string();
                let unchanged_message = ShinkaiMessageBuilder::new(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_subencryption_pk_2,
                )
                .set_optional_second_public_key_receiver_node(node1_encryption_pk)
                .message_raw_content(message_content.clone())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node1_profile_name.to_string().clone(),
                    node1_subidentity_name_2.to_string().clone(),
                    EncryptionMethod::DiffieHellmanChaChaPoly1305,
                    None,
                )
                .external_metadata_with_other_and_intra_sender(
                    node1_identity_name.to_string().clone(),
                    node1_identity_name.to_string().clone(),
                    "".to_string(),
                    node1_profile_name.to_string().clone(),
                )
                .build()
                .unwrap();
                eprintln!("unchanged_message node 1 sub to node 1 sub 2: {:?}", unchanged_message);

                let (res1_send_msg_sender, res1_send_msg_receiver): (
                    async_channel::Sender<Result<SendResponseBodyData, APIError>>,
                    async_channel::Receiver<Result<SendResponseBodyData, APIError>>,
                ) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::SendOnionizedMessage {
                        msg: unchanged_message,
                        res: res1_send_msg_sender,
                    })
                    .await
                    .unwrap();

                let send_result = res1_send_msg_receiver.recv().await.unwrap();
                assert!(send_result.is_ok(), "Failed to send onionized message");

                let (res1_sender, res1_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::FetchLastMessages {
                        limit: 2,
                        res: res1_sender,
                    })
                    .await
                    .unwrap();
                let node1_last_messages = res1_receiver.recv().await.unwrap();

                // Check the last message
                let message_to_check = node1_last_messages[0].clone();

                // Check that the message is not body encrypted
                assert_eq!(
                    ShinkaiMessage::is_body_currently_encrypted(&message_to_check.clone()),
                    false,
                    "Message from Node 1 subidentity to Node 1 subidentity 2 is not body encrypted"
                );

                // Check that the content is encrypted
                assert_eq!(
                    ShinkaiMessage::is_content_currently_encrypted(&message_to_check.clone()),
                    true,
                    "Message from Node 1 subidentity to Node 1 subidentity 2 is content encrypted"
                );

                // Check the sender and recipient
                assert_eq!(
                    message_to_check.get_sender_subidentity().unwrap(),
                    node1_profile_name.to_string(),
                    "Node 1 subidentity sent a message to Node 1 subidentity 2. The message has the right sender."
                );
                assert_eq!(
                    message_to_check.get_recipient_subidentity().unwrap(),
                    node1_subidentity_name_2.to_string(),
                    "Node 1 subidentity sent a message to Node 1 subidentity 2. The message has the right recipient."
                );

                // TODO: Check that identity can be found using identity manager
            }

            // Send message from Node 1 subidentity to Node 2 subidentity
            {
                eprintln!("Final trick. Sending a fat message from Node 1 subidentity to Node 2 subidentity");
                let message_content = std::iter::repeat("hola-").take(10_000).collect::<String>();
                let unchanged_message = ShinkaiMessageBuilder::new(
                    node1_profile_encryption_sk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node2_subencryption_pk,
                )
                .message_raw_content(message_content.clone())
                .no_body_encryption()
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node1_profile_name.to_string().clone(),
                    node2_profile_name.to_string().clone(),
                    EncryptionMethod::DiffieHellmanChaChaPoly1305,
                    None,
                )
                .external_metadata_with_other(
                    node2_identity_name.to_string().clone(),
                    node1_identity_name.to_string().clone(),
                    encryption_public_key_to_string(node1_profile_encryption_pk.clone()),
                )
                .build()
                .unwrap();
                eprintln!("unchanged_message node 1 sub to node 2 sub: {:?}", unchanged_message);

                let (res1_send_msg_sender, res1_send_msg_receiver): (
                    async_channel::Sender<Result<SendResponseBodyData, APIError>>,
                    async_channel::Receiver<Result<SendResponseBodyData, APIError>>,
                ) = async_channel::bounded(1);

                node1_commands_sender
                    .send(NodeCommand::SendOnionizedMessage {
                        msg: unchanged_message.clone(),
                        res: res1_send_msg_sender,
                    })
                    .await
                    .unwrap();

                let send_result = res1_send_msg_receiver.recv().await.unwrap();
                assert!(send_result.is_ok(), "Failed to send onionized message");

                {
                    let mut is_successful = false;
                    for _ in 0..30 {
                        let (res2_sender, res2_receiver) = async_channel::bounded(1);
                        node2_commands_sender
                            .send(NodeCommand::FetchLastMessages {
                                limit: 1,
                                res: res2_sender,
                            })
                            .await
                            .unwrap();
                        let node2_last_messages = res2_receiver.recv().await.unwrap();
                        // eprintln!("node2_last_messages: {:?}", node2_last_messages);

                        let message_to_check = node2_last_messages[0].clone();

                        // Check if the message is not body encrypted
                        if ShinkaiMessage::is_body_currently_encrypted(&message_to_check.clone()) {
                            eprintln!("Message from Node 1 to Node 2 is not body encrypted as expected. Retrying...");
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }

                        // Check if the content is encrypted
                        if !ShinkaiMessage::is_content_currently_encrypted(&message_to_check.clone()) {
                            eprintln!(
                                "Message from Node 1 to Node 2 is not content encrypted as expected. Retrying..."
                            );
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }

                        // Check sender and recipient subidentity
                        if message_to_check.get_sender_subidentity().unwrap() != node1_profile_name.to_string() {
                            eprintln!("The message does not have the right sender. Retrying...");
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }

                        if message_to_check.get_recipient_subidentity().unwrap() != node2_profile_name.to_string() {
                            eprintln!("The message does not have the right recipient. Retrying...");
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }

                        // Decrypt the message content and check if it matches the expected content
                        let message_to_check_content_unencrypted = message_to_check
                            .clone()
                            .decrypt_inner_layer(&node2_subencryption_sk_clone.clone(), &node1_profile_encryption_pk)
                            .unwrap();

                        if message_content != message_to_check_content_unencrypted.get_message_content().unwrap() {
                            eprintln!("Decrypted message content does not match the expected content. Retrying...");
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }

                        is_successful = true;
                        break;
                    }
                    if !is_successful {
                        assert!(is_successful, "Failed to send fat message from Node 1 to Node 2");
                    }
                }

                node1_abort_handler.abort();
                node2_abort_handler.abort();
            }
        });

        // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, node2_handler, interactions_handler);
        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    eprintln!("One of the tasks was aborted, but this is expected.");
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

#[test]
fn test_relay_server_communication() {
    std::env::set_var("SKIP_IMPORT_FROM_DIRECTORY", "true");
    std::env::set_var("IS_TESTING", "1");

    setup();
    let rt = Runtime::new().unwrap();

    let e: Result<(), tokio::task::JoinError> = rt.block_on(async {
        let node1_identity_name = NODE1_IDENTITY_NAME;  
        let node2_identity_name = NODE2_IDENTITY_NAME;

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name));
        let node2_db_path = format!("db_tests/{}", hash_string(node2_identity_name));

        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8082);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8083);
        
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
            Some(RELAY_IDENTITY_NAME.to_string()),  // Use real relay server
            true,
            vec![],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some("debug".to_string()),
        )
        .await;

        let node2 = Node::new(
            node2_identity_name.to_string(),
            addr2,
            clone_signature_secret_key(&node2_identity_sk),
            node2_encryption_sk.clone(),
            None,
            None,
            0,
            node2_commands_receiver,
            node2_db_path,
            "".to_string(),
            Some(RELAY_IDENTITY_NAME.to_string()),  // Use real relay server
            true,
            vec![],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some("debug".to_string()),
        )
        .await;

        eprintln!(">> Starting relay test with real identities and relay server");
        
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

        // Wait a bit for nodes to start
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Test basic bidirectional messaging
        let messaging_test = tokio::spawn(async move {
            eprintln!(">> Testing bidirectional messaging between real nodes via relay");

            // Wait a bit more for nodes to fully initialize
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Register profiles on both nodes first (required for messaging)
            eprintln!(">> Registering profiles on both nodes");
            
            let (profile1_sk, profile1_pk) = unsafe_deterministic_signature_keypair(100);
            let (profile1_encryption_sk, profile1_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

            let _registration_result1 = local_registration_profile_node(
                node1_commands_sender.clone(),
                "main",
                node1_identity_name,
                profile1_encryption_sk.clone(),
                node1_encryption_pk,
                clone_signature_secret_key(&profile1_sk),
                1,
            ).await;
            eprintln!(">> Node 1 ({}) profile registration completed", NODE1_IDENTITY_NAME);

            let (profile2_sk, profile2_pk) = unsafe_deterministic_signature_keypair(101);
            let (profile2_encryption_sk, profile2_encryption_pk) = unsafe_deterministic_encryption_keypair(101);

            let _registration_result2 = local_registration_profile_node(
                node2_commands_sender.clone(),
                "main",
                node2_identity_name,
                profile2_encryption_sk.clone(),
                node2_encryption_pk,
                clone_signature_secret_key(&profile2_sk),
                1,
            ).await;
            eprintln!(">> Node 2 ({}) profile registration completed", NODE2_IDENTITY_NAME);

            eprintln!("=== NODE KEYS ===");
            eprintln!("Node 1 Identity Secret Key: {}", signature_secret_key_to_string(clone_signature_secret_key(&node1_identity_sk)));
            eprintln!("Node 1 Identity Public Key:  {}", signature_public_key_to_string(node1_identity_pk));
            eprintln!("Node 2 Identity Secret Key: {}", signature_secret_key_to_string(clone_signature_secret_key(&node2_identity_sk)));
            eprintln!("Node 2 Identity Public Key:  {}", signature_public_key_to_string(node2_identity_pk));
            eprintln!("=== PROFILE KEYS ===");
            eprintln!("Profile 1 Secret Key: {}", signature_secret_key_to_string(clone_signature_secret_key(&profile1_sk)));
            eprintln!("Profile 1 Public Key: {}", signature_public_key_to_string(profile1_pk));
            eprintln!("Profile 2 Secret Key: {}", signature_secret_key_to_string(clone_signature_secret_key(&profile2_sk)));
            eprintln!("Profile 2 Public Key: {}", signature_public_key_to_string(profile2_pk));               

            // Wait for tools to be ready
            let tools_ready1 = wait_for_default_tools(
                node1_commands_sender.clone(),
                "debug".to_string(),
                60,
            ).await.unwrap_or(false);
            
            let tools_ready2 = wait_for_default_tools(
                node2_commands_sender.clone(),
                "debug".to_string(), 
                60,
            ).await.unwrap_or(false);

            eprintln!(">> Tools ready - Node 1: {}, Node 2: {}", tools_ready1, tools_ready2);

            // Wait for LibP2P mesh to stabilize and relay connections
            eprintln!(">> Waiting for relay connections to establish...");
            tokio::time::sleep(Duration::from_secs(10)).await;

            // Test sending message from node1 to node2
            eprintln!(">> Sending message from Node 1 ({}) to Node 2 ({})", NODE1_IDENTITY_NAME, NODE2_IDENTITY_NAME);
            let message_content_1to2 = "Hello from node1 to node2 via relay!".to_string();
            let message_1to2 = ShinkaiMessageBuilder::new(
                profile1_encryption_sk.clone(),
                clone_signature_secret_key(&profile1_sk),
                profile2_encryption_pk,
            )
            .message_raw_content(message_content_1to2.clone())
            .no_body_encryption()
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata(
                "main".to_string(),
                "main".to_string(),
                EncryptionMethod::DiffieHellmanChaChaPoly1305,
                None,
            )
            .external_metadata_with_other(
                node2_identity_name.to_string(),
                node1_identity_name.to_string(),
                encryption_public_key_to_string(profile1_encryption_pk),
            )
            .build()
            .unwrap();

            let (res_1to2_sender, res_1to2_receiver) = async_channel::bounded(1);
            node1_commands_sender
                .send(NodeCommand::SendOnionizedMessage {
                    msg: message_1to2,
                    res: res_1to2_sender,
                })
                .await
                .unwrap();

            // Test sending message from node2 to node1
            eprintln!(">> Sending message from Node 2 ({}) to Node 1 ({})", NODE2_IDENTITY_NAME, NODE1_IDENTITY_NAME);
            let message_content_2to1 = "Hello from node2 to node1 via relay!".to_string();
            let message_2to1 = ShinkaiMessageBuilder::new(
                profile2_encryption_sk.clone(),
                clone_signature_secret_key(&profile2_sk),
                profile1_encryption_pk,
            )
            .message_raw_content(message_content_2to1.clone())
            .no_body_encryption()
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata(
                "main".to_string(),
                "main".to_string(),
                EncryptionMethod::DiffieHellmanChaChaPoly1305,
                None,
            )
            .external_metadata_with_other(
                node1_identity_name.to_string(),
                node2_identity_name.to_string(),
                encryption_public_key_to_string(profile2_encryption_pk),
            )
            .build()
            .unwrap();

            let (res_2to1_sender, res_2to1_receiver) = async_channel::bounded(1);
            node2_commands_sender
                .send(NodeCommand::SendOnionizedMessage {
                    msg: message_2to1,
                    res: res_2to1_sender,
                })
                .await
                .unwrap();

            // Wait for message sending attempts
            let send_result_1to2 = res_1to2_receiver.recv().await.unwrap();
            let send_result_2to1 = res_2to1_receiver.recv().await.unwrap();

            eprintln!(">> Node 1 to Node 2 send result: {:?}", send_result_1to2.is_ok());
            eprintln!(">> Node 2 to Node 1 send result: {:?}", send_result_2to1.is_ok());

            assert_eq!(send_result_1to2.is_ok(), true, "Node 1 to Node 2 send should be successful");
            assert_eq!(send_result_2to1.is_ok(), true, "Node 2 to Node 1 send should be successful");

            // Wait longer for messages to potentially be delivered via relay
            eprintln!(">> Waiting for relay message delivery...");
            tokio::time::sleep(Duration::from_secs(15)).await;

            // Check if messages were received
            let (res1_check_sender, res1_check_receiver) = async_channel::bounded(1);
            node1_commands_sender
                .send(NodeCommand::FetchLastMessages {
                    limit: 5,
                    res: res1_check_sender,
                })
                .await
                .unwrap();

            let (res2_check_sender, res2_check_receiver) = async_channel::bounded(1);
            node2_commands_sender
                .send(NodeCommand::FetchLastMessages {
                    limit: 5,
                    res: res2_check_sender,
                })
                .await
                .unwrap();

            let node1_messages = res1_check_receiver.recv().await.unwrap();
            let node2_messages = res2_check_receiver.recv().await.unwrap();

            eprintln!(">> Node 1 message count: {}", node1_messages.len());
            eprintln!(">> Node 2 message count: {}", node2_messages.len());

            if !node1_messages.is_empty() {
                eprintln!(">> ✅ Node 1 received messages via relay");
                
                // Display received message content on Node 1
                for (i, message) in node1_messages.iter().enumerate() {
                    eprintln!(">> Node 1 - Message {}: {:?}", i + 1, message.get_message_content());
                    
                    // Try to decrypt the message if it's encrypted
                    if ShinkaiMessage::is_content_currently_encrypted(message) {
                        match message.clone().decrypt_inner_layer(&profile1_encryption_sk, &profile2_encryption_pk) {
                            Ok(decrypted_msg) => {
                                eprintln!(">> Node 1 - Decrypted content: {:?}", decrypted_msg.get_message_content());
                            },
                            Err(e) => {
                                eprintln!(">> Node 1 - Failed to decrypt: {:?}", e);
                            }
                        }
                    }
                }
            }
            
            if !node2_messages.is_empty() {
                eprintln!(">> ✅ Node 2 received messages via relay");
                
                // Display received message content on Node 2
                for (i, message) in node2_messages.iter().enumerate() {
                    eprintln!(">> Node 2 - Message {}: {:?}", i + 1, message.get_message_content());
                    
                    // Try to decrypt the message if it's encrypted
                    if ShinkaiMessage::is_content_currently_encrypted(message) {
                        match message.clone().decrypt_inner_layer(&profile2_encryption_sk, &profile1_encryption_pk) {
                            Ok(decrypted_msg) => {
                                eprintln!(">> Node 2 - Decrypted content: {:?}", decrypted_msg.get_message_content());
                            },
                            Err(e) => {
                                eprintln!(">> Node 2 - Failed to decrypt: {:?}", e);
                            }
                        }
                    }
                }
            }

            // Display the original messages that were sent
            eprintln!("\n>> Original messages sent:");
            eprintln!(">> Node 1 → Node 2: '{}'", message_content_1to2);
            eprintln!(">> Node 2 → Node 1: '{}'", message_content_2to1);

            assert_eq!(node2_messages.len(), 2, "Node 2 should have two messages.");
            assert_eq!(node1_messages.len(), 2, "Node 1 should have two messages.");

            eprintln!(">> Relay messaging test completed with real identities");
            node1_abort_handler.abort();
            node2_abort_handler.abort();
        });

        // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, node2_handler, messaging_test);
        match result {
            Ok(_) => {
                eprintln!(">> Relay test completed - nodes can communicate via relay server");
                Ok(())
            },
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    eprintln!("One of the tasks was aborted, but this is expected.");
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
