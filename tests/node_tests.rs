use async_channel::{bounded, Receiver, Sender};
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::{Node, Subidentity, SubIdentityManager};
use shinkai_node::shinkai_message::encryption::{
    encryption_public_key_to_string, hash_encryption_public_key,
    unsafe_deterministic_encryption_keypair, EncryptionMethod, decrypt_content_message,
};
use shinkai_node::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_node::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use shinkai_node::shinkai_message::signatures::{
    clone_signature_secret_key, signature_public_key_to_string,
    unsafe_deterministic_signature_keypair, sign_message,
};
use shinkai_node::shinkai_message::utils::hash_string;
use shinkai_node::shinkai_message_proto::Field;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn tcp_node_test() {
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1.shinkai";
        let node2_identity_name = "@@node2.shinkai";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, _) = unsafe_deterministic_encryption_keypair(1);

        let (node1_commands_sender, node1_commands_receiver): (
            Sender<NodeCommand>,
            Receiver<NodeCommand>,
        ) = bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (
            Sender<NodeCommand>,
            Receiver<NodeCommand>,
        ) = bounded(100);

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
        );

        println!("Starting nodes");
        // Start node1 and node2
        let node1_handler = tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 1");
            let _ = node1.await.start().await;
        });

        let node2_handler = tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 2");
            let _ = node2.await.start().await;
        });

        let interactions_handler = tokio::spawn(async move {
            println!("Starting interactions");
            println!("Connecting node 2 to node 1");
            tokio::time::sleep(Duration::from_secs(3)).await;
            node2_commands_sender
                .send(NodeCommand::Connect {
                    address: addr1,
                    profile_name: node1_identity_name.to_string(),
                })
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_secs(4)).await;
            // Get Node2 messages
            let (res2_sender, res2_receiver) = async_channel::bounded(1);
            node2_commands_sender
                .send(NodeCommand::FetchLastMessages {
                    limit: 5,
                    res: res2_sender,
                })
                .await
                .unwrap();
            let node2_last_messages = res2_receiver.recv().await.unwrap();

            // Get Node1 messages
            let (res1_sender, res1_receiver) = async_channel::bounded(1);
            node1_commands_sender
                .send(NodeCommand::FetchLastMessages {
                    limit: 5,
                    res: res1_sender,
                })
                .await
                .unwrap();
            let node1_last_messages = res1_receiver.recv().await.unwrap();

            println!("Node 1 last messages: {:?}", node1_last_messages);
            println!("Node 2 last messages: {:?}", node2_last_messages);

            assert_eq!(
                node1_last_messages.len(),
                3,
                "Node 1 (listening) should have 3 message"
            );
            assert_eq!(
                node2_last_messages.len(),
                3,
                "Node 2 (connecting) should have 3 messages"
            );

            // Node 1 (receiving the Ping, sending back a Pong)
            assert_eq!(
                node1_last_messages[1].body.as_ref().unwrap().content == "Pong".to_string(),
                true,
            );
            assert_eq!(
                node1_last_messages[1]
                    .external_metadata
                    .as_ref()
                    .unwrap()
                    .sender
                    == node1_identity_name.to_string(),
                true
            );
            assert_eq!(
                node1_last_messages[1]
                    .external_metadata
                    .as_ref()
                    .unwrap()
                    .recipient
                    == node2_identity_name.clone(),
                true
            );

            // Node 2 (sending the Ping, Receiving a Pong and confirming with ACK)
            assert_eq!(
                node2_last_messages[0].body.as_ref().unwrap().content == "ACK".to_string(),
                true
            );
            assert_eq!(
                node2_last_messages[0]
                    .external_metadata
                    .as_ref()
                    .unwrap()
                    .sender
                    == node2_identity_name.clone(),
                true
            );
            assert_eq!(
                node2_last_messages[0]
                    .external_metadata
                    .as_ref()
                    .unwrap()
                    .recipient
                    == node1_identity_name.clone(),
                true
            );
            assert_eq!(
                node2_last_messages[2].body.as_ref().unwrap().content == "Ping".to_string(),
                true
            );
            assert_eq!(
                node2_last_messages[2]
                    .external_metadata
                    .as_ref()
                    .unwrap()
                    .sender
                    == node2_identity_name.clone(),
                true
            );
            assert_eq!(
                node2_last_messages[2]
                    .external_metadata
                    .as_ref()
                    .unwrap()
                    .recipient
                    == node1_identity_name.clone(),
                true
            );

            // Messages should be equal
            assert_eq!(
                ShinkaiMessageHandler::calculate_hash(&node1_last_messages[0]),
                ShinkaiMessageHandler::calculate_hash(&node2_last_messages[0])
            );
            assert_eq!(
                ShinkaiMessageHandler::calculate_hash(&node1_last_messages[1]),
                ShinkaiMessageHandler::calculate_hash(&node2_last_messages[1])
            );
            assert_eq!(
                ShinkaiMessageHandler::calculate_hash(&node1_last_messages[2]),
                ShinkaiMessageHandler::calculate_hash(&node2_last_messages[2])
            );

            // {
            //     let shinkai_message = node1_last_messages[0].clone();
            //     let message_wrapper = ShinkaiMessageWrapper::from(&shinkai_message);
            //     let message_json = serde_json::to_string_pretty(&message_wrapper)
            //         .expect("Failed to serialize message to JSON");
            //     println!("Last message from Node 1: {}", message_json);
            // }
        });

        // Wait for all tasks to complete
        let _ = tokio::try_join!(node1_handler, node2_handler, interactions_handler).unwrap();
    });
}

#[test]
fn subidentity_registration() {
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1.shinkai";
        let node2_identity_name = "@@node2.shinkai";
        let node1_subidentity_name = "main_profile_node1";
        let node2_subidentity_name = "main_profile_node2";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node2_encryption_sk_clone = node2_encryption_sk.clone();
        let node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);
        let node2_identity_sk_clone = clone_signature_secret_key(&node2_identity_sk);

        let (node1_subidentity_sk, node1_subidentity_pk) =
            unsafe_deterministic_signature_keypair(100);
        let (node1_subencryption_sk, node1_subencryption_pk) =
            unsafe_deterministic_encryption_keypair(100);

        let (node2_subidentity_sk, node2_subidentity_pk) =
            unsafe_deterministic_signature_keypair(101);
        let (node2_subencryption_sk, node2_subencryption_pk) =
            unsafe_deterministic_encryption_keypair(101);
        let node1_subidentity_sk_clone = clone_signature_secret_key(&node1_subidentity_sk);
        let node2_subidentity_sk_clone = clone_signature_secret_key(&node2_subidentity_sk);

        let (node1_commands_sender, node1_commands_receiver): (
            Sender<NodeCommand>,
            Receiver<NodeCommand>,
        ) = bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (
            Sender<NodeCommand>,
            Receiver<NodeCommand>,
        ) = bounded(100);

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
        );

        println!("Starting nodes");
        // Start node1 and node2
        let node1_handler = tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 1");
            let _ = node1.await.start().await;
        });

        let node2_handler = tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 2");
            let _ = node2.await.start().await;
        });

        let interactions_handler = tokio::spawn(async move {
            println!("Starting interactions");
            println!("Registration of Subidentities");

            {
                let (res_registration_sender, res_registraton_receiver) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::CreateRegistrationCode {
                        res: res_registration_sender,
                    })
                    .await
                    .unwrap();
                let node2_registration_code = res_registraton_receiver.recv().await.unwrap();

                let code_message = ShinkaiMessageBuilder::code_registration(
                    node2_subencryption_sk.clone(),
                    clone_signature_secret_key(&node2_subidentity_sk),
                    node2_encryption_pk,
                    node2_registration_code.to_string(),
                    node2_subidentity_name.to_string().clone(),
                    node2_identity_name.to_string(),
                )
                .unwrap();

                let (res_use_registration_sender, res_use_registraton_receiver) =
                    async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::UseRegistrationCode {
                        msg: code_message,
                        res: res_use_registration_sender,
                    })
                    .await
                    .unwrap();
                let node2_use_registration_code =
                    res_use_registraton_receiver.recv().await.unwrap();
                assert_eq!(
                    node2_use_registration_code,
                    "true".to_string(),
                    "Node 2 used registration code"
                );

                let (res_all_subidentities_sender, res_all_subidentities_receiver): (
                    async_channel::Sender<Vec<Subidentity>>,
                    async_channel::Receiver<Vec<Subidentity>>,
                ) = async_channel::bounded(1);
                node2_commands_sender
                    .send(NodeCommand::GetAllSubidentities {
                        res: res_all_subidentities_sender,
                    })
                    .await
                    .unwrap();
                let node2_all_subidentities = res_all_subidentities_receiver.recv().await.unwrap();

                // TODO: add test that check that subidentity was correctly added
            }

            println!("Connecting node 2 to node 1");
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
                println!("Sending message from node 2 to node 1");
                let fields = vec![Field {
                    name: "field1".to_string(),
                    field_type: "type1".to_string(),
                }];

                let message_content = "test body content".to_string();
                let unchanged_message = ShinkaiMessageBuilder::new(
                    node2_subencryption_sk,
                    clone_signature_secret_key(&node2_subidentity_sk),
                    node1_encryption_pk,
                )
                .body(message_content.clone())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type("schema type".to_string(), fields)
                .internal_metadata(
                    node2_subidentity_name.to_string().clone(),
                    "".to_string(),
                    "".to_string(),
                    EncryptionMethod::None,
                )
                .external_metadata_with_other(
                    node1_identity_name.to_string(),
                    node2_identity_name.to_string().clone(),
                    encryption_public_key_to_string(node2_subencryption_pk.clone()),
                )
                .build()
                .unwrap();

                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<NodeCommand>,
                    async_channel::Receiver<NodeCommand>,
                ) = async_channel::bounded(1);

                node2_commands_sender
                    .send(NodeCommand::SendUnchangedMessage {
                        msg: unchanged_message,
                    })
                    .await
                    .unwrap();
                
                tokio::time::sleep(Duration::from_secs(2)).await;

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

                // println!("Node 1 last messages: {:?}", node1_last_messages);
                // println!("\n\n");
                println!("Node 2 last messages: {:?}", node2_last_messages);
                println!("\n\n");

                let encrypted_content = &node2_last_messages[1].clone().body.unwrap().content;
                let decrypted_content = decrypt_content_message(
                    encrypted_content.clone().to_string(),
                    &node2_last_messages[1].clone().encryption,
                    &node1_encryption_sk_clone.clone(),
                    &node2_subencryption_pk,
                ).unwrap();
                // This check can't be done using a static value because the nonce is randomly generated
                assert_eq!(
                    message_content,
                    decrypted_content,
                    "Node 2's profile send an encrypted message to Node 1"
                );

                assert_eq!(
                    node2_last_messages[1].external_metadata.clone().as_ref().unwrap().sender,
                    node2_subidentity_name.to_string(),
                    "Node 2's profile send an encrypted message to Node 1. The message has the right sender."
                );

                // You could think the subidentity signed it, but it's actually the node who re-signs it before sending it 
                let signature = sign_message(&node2_identity_sk_clone, node2_last_messages[1].clone());
                assert_eq!(
                    node2_last_messages[1].external_metadata.clone().as_ref().unwrap().signature,
                    signature,
                    "Node 2's profile send an encrypted message to Node 1. Node 2 sends the correct signature."
                );

                assert_eq!(
                    node2_last_messages[1].external_metadata.clone().as_ref().unwrap().other,
                    encryption_public_key_to_string(node2_subencryption_pk),
                    "Node 2's profile send an encrypted message to Node 1. Node 2 sends the subidentity's pk in other"
                );

                assert_eq!(
                    node1_last_messages[1].external_metadata.clone().as_ref().unwrap().other,
                    encryption_public_key_to_string(node2_subencryption_pk),
                    "Node 2's profile send an encrypted message to Node 1. Node 1 has the other's public key"
                );
                println!("Node 2 sent message to Node 1 successfully");
            }

            // Create Node 1 subidentity
            // Send message from Node 1 subidentity to Node 2 subidentity
            {
                println!("\n\n\n");
                println!("Creating Node 1 subidentity");
                let (res1_registration_sender, res1_registraton_receiver) =
                    async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::CreateRegistrationCode {
                        res: res1_registration_sender,
                    })
                    .await
                    .unwrap();
                let node1_registration_code = res1_registraton_receiver.recv().await.unwrap();

                let code_message = ShinkaiMessageBuilder::code_registration(
                    node1_subencryption_sk.clone(),
                    clone_signature_secret_key(&node1_subidentity_sk),
                    node1_encryption_pk,
                    node1_registration_code.to_string(),
                    node1_subidentity_name.to_string().clone(),
                    node1_identity_name.to_string(),
                )
                .unwrap();

                let (res1_use_registration_sender, res1_use_registraton_receiver) =
                    async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::UseRegistrationCode {
                        msg: code_message,
                        res: res1_use_registration_sender,
                    })
                    .await
                    .unwrap();
                let node1_use_registration_code =
                    res1_use_registraton_receiver.recv().await.unwrap();
                assert_eq!(
                    node1_use_registration_code,
                    "true".to_string(),
                    "Node 1 used registration code"
                );

                let (res1_all_subidentities_sender, res1_all_subidentities_receiver): (
                    async_channel::Sender<Vec<Subidentity>>,
                    async_channel::Receiver<Vec<Subidentity>>,
                ) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::GetAllSubidentities {
                        res: res1_all_subidentities_sender,
                    })
                    .await
                    .unwrap();
                let node1_all_subidentities = res1_all_subidentities_receiver.recv().await.unwrap();
                let node1_just_subidentity_name = SubIdentityManager::extract_subidentity(node1_subidentity_name);
                assert_eq!(node1_all_subidentities[0].name, node1_just_subidentity_name, "Node 1 has the right subidentity");
                // println!("Node 1 all subidentities: {:?}", node1_all_subidentities);

                // Send message from Node 1 subidentity to Node 2 subidentity
                let fields = vec![Field {
                    name: "field1".to_string(),
                    field_type: "type1".to_string(),
                }];

                let unchanged_message = ShinkaiMessageBuilder::new(
                    node1_subencryption_sk,
                    clone_signature_secret_key(&node1_subidentity_sk),
                    node2_subencryption_pk,
                )
                .body("test encrypted body content from node1 subidentity to node2 subidentity".to_string())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type("schema type".to_string(), fields)
                .internal_metadata(
                    node1_subidentity_name.to_string().clone(),
                    node2_subidentity_name.to_string().clone(),
                    "".to_string(),
                    EncryptionMethod::None,
                )
                .external_metadata_with_other(
                    node2_identity_name.to_string().clone(),
                    node1_identity_name.to_string().clone(),
                    encryption_public_key_to_string(node1_subencryption_pk.clone()),
                )
                .build()
                .unwrap();

                // let (res1_send_msg_sender, res1_send_msg_receiver): (
                //     async_channel::Sender<NodeCommand>,
                //     async_channel::Receiver<NodeCommand>,
                // ) = async_channel::bounded(1);

                // node1_commands_sender
                //     .send(NodeCommand::SendUnchangedMessage {
                //         msg: unchanged_message,
                //     })
                //     .await
                //     .unwrap();
                // let _ = res1_send_msg_receiver.recv().await.unwrap();
            }
        });

        // Wait for all tasks to complete
        let _ = tokio::try_join!(node1_handler, node2_handler, interactions_handler).unwrap();
    });
}
