use async_channel::{bounded, Receiver, Sender};
use async_std::println;
use core::panic;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    IdentityPermissions, MessageSchemaType, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::{NodeCommand, NodeProxyMode};
use shinkai_node::network::node_api::APIError;
use shinkai_node::network::Node;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

mod utils;
use crate::utils::node_test_api::{
    api_registration_device_node_profile_main, api_registration_profile_node, api_try_re_register_profile_node,
};
use crate::utils::node_test_local::local_registration_profile_node;

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn subidentity_registration() {
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.shinkai";
        let node2_identity_name = "@@node2_test.shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";
        let node2_profile_name = "main_profile_node2";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let node2_encryption_sk_clone = node2_encryption_sk.clone();

        let node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);
        let node2_identity_sk_clone = clone_signature_secret_key(&node2_identity_sk);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node2_subidentity_sk, node2_subidentity_pk) = unsafe_deterministic_signature_keypair(101);
        let (node2_subencryption_sk, node2_subencryption_pk) = unsafe_deterministic_encryption_keypair(101);

        let node1_subencryption_sk_clone = node1_profile_encryption_sk.clone();
        let node2_subencryption_sk_clone = node2_subencryption_sk.clone();

        let node1_subidentity_sk_clone = clone_signature_secret_key(&node1_profile_identity_sk);
        let node2_subidentity_sk_clone = clone_signature_secret_key(&node2_subidentity_sk);

        let (node1_device_identity_sk, node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));
        let node2_db_path = format!("db_tests/{}", hash_string(node2_identity_name.clone()));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            node1_identity_sk,
            node1_encryption_sk,
            0,
            node1_commands_receiver,
            node1_db_path,
            true,
            None,
            NodeProxyMode::NoProxy
        );

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let mut node2 = Node::new(
            node2_identity_name.to_string(),
            addr2,
            node2_identity_sk,
            node2_encryption_sk,
            0,
            node2_commands_receiver,
            node2_db_path,
            true,
            None,
            NodeProxyMode::NoProxy
        );

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
        let node1_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 1");
            let _ = node1.await.start().await;
        });

        let node2_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 2");
            let _ = node2.await.start().await;
        });

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
                    node1_encryption_pk.clone(),
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

            eprintln!("Connecting node 2 to node 1");
            tokio::time::sleep(Duration::from_secs(3)).await;
            node2_commands_sender
                .send(NodeCommand::Connect {
                    address: addr1,
                    profile_name: node1_identity_name.to_string(),
                })
                .await
                .unwrap();

            // Waiting some safe assuring time for the Nodes to connect
            tokio::time::sleep(Duration::from_secs(3)).await;

            // Send message from Node 2 subidentity to Node 1
            {
                eprintln!("\n\n### Sending message from a node 2 profile to node 1 profile\n\n");

                let message_content = "test body content".to_string();
                let unchanged_message = ShinkaiMessageBuilder::new(
                    node2_subencryption_sk.clone(),
                    clone_signature_secret_key(&node2_subidentity_sk),
                    node1_profile_encryption_pk.clone(),
                )
                .message_raw_content(message_content.clone())
                .no_body_encryption()
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node2_profile_name.to_string().clone(),
                    node1_profile_name.to_string(),
                    EncryptionMethod::DiffieHellmanChaChaPoly1305,
                )
                .external_metadata_with_other(
                    node1_identity_name.to_string(),
                    node2_identity_name.to_string().clone(),
                    encryption_public_key_to_string(node2_subencryption_pk.clone()),
                )
                .build()
                .unwrap();

                eprintln!("\n\n unchanged message: {:?}", unchanged_message);

                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<(), APIError>>,
                    async_channel::Receiver<Result<(), APIError>>,
                ) = async_channel::bounded(1);

                node2_commands_sender
                    .send(NodeCommand::SendOnionizedMessage {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();

                let send_result = res_send_msg_receiver.recv().await.unwrap();
                assert!(send_result.is_ok(), "Failed to send onionized message");
                tokio::time::sleep(Duration::from_secs(1)).await;

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

                let message_to_check = node1_last_messages[1].clone();
                // Check that the message is body encrypted
                assert_eq!(
                    ShinkaiMessage::is_body_currently_encrypted(&message_to_check.clone()),
                    false,
                    "Message from Node 2 to Node 1 is not body encrypted for Node 1 (receiver)"
                );

                let message_to_check = node2_last_messages[1].clone();
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
                    .decrypt_inner_layer(&&node1_profile_encryption_sk.clone(), &node2_subencryption_pk)
                    .unwrap();

                // This check can't be done using a static value because the nonce is randomly generated
                assert_eq!(
                    message_content,
                    message_to_check_content_unencrypted.get_message_content().unwrap(),
                    "Node 2's profile send an encrypted message to Node 1"
                );

                assert_eq!(
                    node2_last_messages[1].external_metadata.clone().other,
                    encryption_public_key_to_string(node2_subencryption_pk),
                    "Node 2's profile send an encrypted message to Node 1. Node 2 sends the subidentity's pk in other"
                );

                assert_eq!(
                    node1_last_messages[0].external_metadata.clone().other,
                    encryption_public_key_to_string(node2_subencryption_pk),
                    "Node 2's profile send an encrypted message to Node 1. Node 1 has the other's public key"
                );
                println!("Node 2 sent message to Node 1 successfully");
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
                let (node1_subidentity_sk_2, node1_subencryption_pk_2) = unsafe_deterministic_signature_keypair(3);
                let (node1_subencryption_sk_2, node1_subencryption_pk_2) = unsafe_deterministic_encryption_keypair(3);

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

                eprintln!("Sending message from Node 1 subidentity to Node 1 subidentity 2 using the intra_sender feature");
                let message_content =
                    "test encrypted body content from node1 subidentity to node1 subidentity 2".to_string();
                let unchanged_message = ShinkaiMessageBuilder::new(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_subencryption_pk_2,
                )
                .set_optional_second_public_key_receiver_node(node1_encryption_pk.clone())
                .message_raw_content(message_content.clone())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node1_profile_name.to_string().clone(),
                    node1_subidentity_name_2.to_string().clone(),
                    EncryptionMethod::DiffieHellmanChaChaPoly1305,
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
                    async_channel::Sender<Result<(), APIError>>,
                    async_channel::Receiver<Result<(), APIError>>,
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

                // Check that identity exists in identity manager
            }

            // Send message from Node 1 subidentity to Node 2 subidentity
            {
                eprintln!("Final trick. Sending a fat message from Node 1 subidentity to Node 2 subidentity");
                // let message_content = "test encrypted body content from node1 subidentity to node2 subidentity".to_string();
                let message_content = std::iter::repeat("hola-").take(100_000).collect::<String>();
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
                    async_channel::Sender<Result<(), APIError>>,
                    async_channel::Receiver<Result<(), APIError>>,
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
                        eprintln!("node2_last_messages: {:?}", node2_last_messages);

                        match node2_last_messages[0].get_message_content() {
                            Ok(message) => {
                                if message == message_content {
                                    break;
                                }
                            }
                            Err(_) => {
                                // nothing
                            }
                        }

                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }

                tokio::time::sleep(Duration::from_millis(1000)).await;
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

                // println!("\n\n");
                // println!("\n***********\n");
                // println!("\n***********\n");
                // println!("\n***********\n");
                // println!("Node 1 last messages: {:?}", node1_last_messages);
                // println!("\n\nNode 2 last messages: {:?}", node2_last_messages);

                let message_to_check = node2_last_messages[0].clone();

                // Check that the message is body encrypted
                assert_eq!(
                    ShinkaiMessage::is_body_currently_encrypted(&message_to_check.clone()),
                    false,
                    "Message from Node 1 to Node 2 is body encrypted"
                );

                // Check that the content is encrypted
                eprintln!("Message to check: {:?}", message_to_check.clone());
                assert_eq!(
                    ShinkaiMessage::is_content_currently_encrypted(&message_to_check.clone()),
                    true,
                    "Message from Node 1 to Node 2 is content encrypted"
                );

                {
                    assert_eq!(
                        message_to_check.get_sender_subidentity().unwrap(),
                        node1_profile_name.to_string(),
                        "Node 2's profile send an encrypted message to Node 1. The message has the right sender."
                    );

                    assert_eq!(
                        message_to_check.get_recipient_subidentity().unwrap(),
                        node2_profile_name.to_string(),
                        "Node 2's profile send an encrypted message to Node 1. The message has the right sender."
                    );
                }

                let message_to_check_content_unencrypted = message_to_check
                    .clone()
                    .decrypt_inner_layer(&node2_subencryption_sk_clone.clone(), &node1_profile_encryption_pk)
                    .unwrap();

                // This check can't be done using a static value because the nonce is randomly generated
                assert_eq!(
                    message_content,
                    message_to_check_content_unencrypted.get_message_content().unwrap(),
                    "Node 1's profile send an encrypted message to Node 1's profile"
                );
            }
        });

        // Wait for all tasks to complete
        let _ = tokio::try_join!(node1_handler, node2_handler, interactions_handler).unwrap();
    });
}
