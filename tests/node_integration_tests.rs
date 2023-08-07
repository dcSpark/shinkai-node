use async_channel::{Sender, bounded, Receiver};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;
use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_wasm::shinkai_utils::encryption::{
    decrypt_content_message, encryption_public_key_to_string, encryption_secret_key_to_string,
    unsafe_deterministic_encryption_keypair, EncryptionMethod,
};
use shinkai_message_wasm::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_wasm::shinkai_utils::shinkai_message_handler::ShinkaiMessageHandler;
use shinkai_message_wasm::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_message_wasm::shinkai_utils::utils::hash_string;
use shinkai_node::db::db_identity_registration::RegistrationCodeType;
use shinkai_node::managers::IdentityManager;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use shinkai_node::schemas::identity::{IdentityPermissions, IdentityType, StandardIdentity};
use tokio::runtime::Runtime;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use x25519_dalek::StaticSecret;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

fn get_node_identity_names() -> (String, String) {
    let node1_identity_name = "@@node1.shinkai";
    let node2_identity_name = "@@node2.shinkai";
    (node1_identity_name.to_string(), node2_identity_name.to_string())
}

#[test]
fn subidentity_registration() {
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_subidentity_name = "main_profile_node1";
        let node2_subidentity_name = "main_profile_node2";

        let (node1_identity_name, node2_identity_name) = get_node_identity_names();
        let (
            node1_encryption_sk,
            node1_encryption_pk,
            node1_identity_sk,
            node1_identity_pk,
            node2_encryption_sk,
            node2_encryption_pk,
            node2_identity_sk,
            node2_identity_pk,
        ) = get_encryption_and_identity_keys();

        let node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();
        let node2_encryption_sk_clone = node2_encryption_sk.clone();
        let node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);
        let node2_identity_sk_clone = clone_signature_secret_key(&node2_identity_sk);

        let (node1_subidentity_sk, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_subencryption_sk, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node2_subidentity_sk, node2_subidentity_pk) = unsafe_deterministic_signature_keypair(101);
        let (node2_subencryption_sk, node2_subencryption_pk) = unsafe_deterministic_encryption_keypair(101);

        let node1_subencryption_sk_clone = node1_subencryption_sk.clone();
        let node2_subencryption_sk_clone = node2_subencryption_sk.clone();
        let node2_subidentity_sk_clone = clone_signature_secret_key(&node2_subidentity_sk);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone().as_str()));
        let node2_db_path = format!("db_tests/{}", hash_string(node2_identity_name.clone().as_str()));

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
            eprintln!("Starting interactions");

            // Register a Profile in Node1 and verifies it
            {
                eprintln!("Register a Profile in Node1 and verifies it");
                registration_profile_node(
                    node1_commands_sender.clone(),
                    node1_subidentity_name,
                    node1_identity_name.as_str(),
                    node1_subencryption_sk_clone.clone(),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_subidentity_sk),
                )
                .await;
            }

            // Register a Profile in Node2 and verifies it
            {
                eprintln!("Register a Profile in Node2 and verifies it");
                registration_profile_node(
                    node2_commands_sender.clone(),
                    node2_subidentity_name,
                    node2_identity_name.as_str(),
                    node2_subencryption_sk_clone.clone(),
                    node2_encryption_pk,
                    clone_signature_secret_key(&node2_subidentity_sk),
                )
                .await;
            }

            // Connect Node 2 to Node 1
            {
                eprintln!("Connecting node 2 to node 1");
                tokio::time::sleep(Duration::from_secs(3)).await;
                node2_commands_sender
                    .clone()
                    .send(NodeCommand::Connect {
                        address: addr1,
                        profile_name: node1_identity_name.to_string(),
                    })
                    .await
                    .unwrap();

                // Waiting some safe assuring time for the Nodes to connect
                tokio::time::sleep(Duration::from_secs(3)).await;
            }

            // Send message from Node 2 subidentity to Node 1
            {
                eprintln!("node2_commands_sender: {:?}", node2_commands_sender);

                send_message_from_node_profile_to_other_node(
                    node1_commands_sender.clone(),
                    node2_commands_sender.clone(),
                    node1_identity_name.as_str(),
                    node2_identity_name.as_str(),
                    node1_encryption_sk_clone.clone(),
                    node1_encryption_pk,
                    node2_encryption_pk,
                    node2_subidentity_sk_clone,
                    node2_subencryption_pk,
                    node2_subidentity_name,
                    node1_subidentity_name,
                    node2_subencryption_sk,
                    node1_subencryption_pk,
                    clone_signature_secret_key(&node1_subidentity_sk),
                    node1_subencryption_sk.clone(),
                )
                .await;
            }
            // Create Node 1 subidentity
            // Send message from Node 1 subidentity to Node 2 subidentity
            {
                eprintln!("\n\n\n");
                eprintln!("Creating Node 1 subidentity");
                let (res1_registration_sender, res1_registraton_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::CreateRegistrationCode {
                        permissions: IdentityPermissions::Admin,
                        code_type: RegistrationCodeType::Profile,
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
                    IdentityType::Profile.to_string(),
                    IdentityPermissions::Admin.to_string(),
                    node1_subidentity_name.to_string().clone(),
                    node1_identity_name.to_string(),
                )
                .unwrap();

                let (res1_use_registration_sender, res1_use_registraton_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::UseRegistrationCode {
                        msg: code_message,
                        res: res1_use_registration_sender,
                    })
                    .await
                    .unwrap();
                let node1_use_registration_code = res1_use_registraton_receiver.recv().await.unwrap();
                assert_eq!(
                    node1_use_registration_code,
                    "true".to_string(),
                    "Node 1 used registration code"
                );

                let (res1_all_subidentities_sender, res1_all_subidentities_receiver): (
                    async_channel::Sender<Vec<StandardIdentity>>,
                    async_channel::Receiver<Vec<StandardIdentity>>,
                ) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::GetAllSubidentities {
                        res: res1_all_subidentities_sender,
                    })
                    .await
                    .unwrap();
                let node1_all_subidentities = res1_all_subidentities_receiver.recv().await.unwrap();
                let node1_just_subidentity_name = ShinkaiName::new(node1_subidentity_name.to_string()).unwrap();
                assert_eq!(
                    node1_all_subidentities[0].full_identity_name, node1_just_subidentity_name,
                    "Node 1 has the right subidentity"
                );

                // Send message from Node 1 subidentity to Node 2 subidentity
                println!("Final trick. Sending message from Node 1 subidentity to Node 2 subidentity");
                let message_content =
                    "test encrypted body content from node1 subidentity to node2 subidentity".to_string();
                let unchanged_message = ShinkaiMessageBuilder::new(
                    node1_subencryption_sk,
                    clone_signature_secret_key(&node1_subidentity_sk),
                    node2_subencryption_pk,
                )
                .body(message_content.clone())
                .no_body_encryption()
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node1_subidentity_name.to_string().clone(),
                    node2_subidentity_name.to_string().clone(),
                    "".to_string(),
                    EncryptionMethod::DiffieHellmanChaChaPoly1305,
                )
                .external_metadata_with_other(
                    node2_identity_name.to_string().clone(),
                    node1_identity_name.to_string().clone(),
                    encryption_public_key_to_string(node1_subencryption_pk.clone()),
                )
                .build()
                .unwrap();

                let (res1_send_msg_sender, res1_send_msg_receiver): (
                    async_channel::Sender<NodeCommand>,
                    async_channel::Receiver<NodeCommand>,
                ) = async_channel::bounded(1);

                node1_commands_sender
                    .send(NodeCommand::SendOnionizedMessage { msg: unchanged_message })
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

                // println!("\n\n");
                // println!("\n***********\n");
                // println!("\n***********\n");
                // println!("\n***********\n");
                // println!("Node 1 last messages: {:?}", node1_last_messages);
                // println!("\n\nNode 2 last messages: {:?}", node2_last_messages);

                let message_to_check = node2_last_messages[1].clone();

                // Check that the message is body encrypted
                assert_eq!(
                    ShinkaiMessageHandler::is_body_currently_encrypted(&message_to_check.clone()),
                    false,
                    "Message from Node 1 to Node 2 is body encrypted"
                );

                // Check that the content is encrypted
                // println!("Message to check: {:?}", message_to_check_body_unencrypted.clone());
                assert_eq!(
                    ShinkaiMessageHandler::is_content_currently_encrypted(&message_to_check.clone()),
                    true,
                    "Message from Node 1 to Node 2 is content encrypted"
                );

                {
                    let internal_metadata = &message_to_check.clone().body.unwrap().internal_metadata.unwrap();
                    assert_eq!(
                        internal_metadata.sender_subidentity,
                        node1_subidentity_name.to_string(),
                        "Node 2's profile send an encrypted message to Node 1. The message has the right sender."
                    );

                    assert_eq!(
                        internal_metadata.recipient_subidentity,
                        node2_subidentity_name.to_string(),
                        "Node 2's profile send an encrypted message to Node 1. The message has the right sender."
                    );
                }

                let message_to_check_content_unencrypted = decrypt_content_message(
                    message_to_check.clone().body.unwrap().content,
                    &message_to_check.clone().encryption.as_str(),
                    &node2_subencryption_sk_clone.clone(),
                    &node1_subencryption_pk,
                )
                .unwrap();

                // This check can't be done using a static value because the nonce is randomly generated
                assert_eq!(
                    message_content, message_to_check_content_unencrypted.0,
                    "Node 1's profile send an encrypted message to Node 1's profile"
                );
            }
        });

        // Wait for all tasks to complete
        let _ = tokio::try_join!(node1_handler, node2_handler, interactions_handler).unwrap();
    });
}

fn get_encryption_and_identity_keys() -> (
    EncryptionStaticKey,
    EncryptionPublicKey,
    SignatureStaticKey,
    SignaturePublicKey,
    EncryptionStaticKey,
    EncryptionPublicKey,
    SignatureStaticKey,
    SignaturePublicKey,
) {
    let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
    let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
    let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    let node1_encryption_sk_clone = node1_encryption_sk.clone();
    let node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);

    let node2_encryption_sk_clone = node2_encryption_sk.clone();
    let node2_identity_sk_clone = clone_signature_secret_key(&node2_identity_sk);

    print_keys(
        node1_encryption_sk_clone,
        node1_encryption_pk,
        node2_encryption_sk_clone,
        node2_encryption_pk,
        node1_identity_sk_clone,
        node1_identity_pk,
        node2_identity_sk_clone,
        node2_identity_pk,
    );

    (
        node1_encryption_sk,
        node1_encryption_pk,
        node1_identity_sk,
        node1_identity_pk,
        node2_encryption_sk,
        node2_encryption_pk,
        node2_identity_sk,
        node2_identity_pk,
    )
}

fn print_keys(
    node1_encryption_sk: EncryptionStaticKey,
    node1_encryption_pk: EncryptionPublicKey,
    node2_encryption_sk: EncryptionStaticKey,
    node2_encryption_pk: EncryptionPublicKey,
    node1_subidentity_sk: SignatureStaticKey,
    node1_subidentity_pk: SignaturePublicKey,
    node2_subidentity_sk: SignatureStaticKey,
    node2_subidentity_pk: SignaturePublicKey,
) {
    eprintln!(
        "Node 1 encryption sk: {:?}",
        encryption_secret_key_to_string(node1_encryption_sk)
    );
    eprintln!(
        "Node 1 encryption pk: {:?}",
        encryption_public_key_to_string(node1_encryption_pk)
    );

    eprintln!(
        "Node 2 encryption sk: {:?}",
        encryption_secret_key_to_string(node2_encryption_sk)
    );
    eprintln!(
        "Node 2 encryption pk: {:?}",
        encryption_public_key_to_string(node2_encryption_pk)
    );

    eprintln!(
        "Node 1 subidentity sk: {:?}",
        signature_secret_key_to_string(clone_signature_secret_key(&node1_subidentity_sk))
    );
    eprintln!(
        "Node 1 subidentity pk: {:?}",
        signature_public_key_to_string(node1_subidentity_pk)
    );

    eprintln!(
        "Node 2 subidentity sk: {:?}",
        signature_secret_key_to_string(clone_signature_secret_key(&node2_subidentity_sk))
    );
    eprintln!(
        "Node 2 subidentity pk: {:?}",
        signature_public_key_to_string(node2_subidentity_pk)
    );
}

async fn registration_profile_node(
    node_commands_sender: Sender<NodeCommand>,
    node_profile_name: &str,
    node_identity_name: &str,
    node_profile_encryption_sk: EncryptionStaticKey,
    node_encryption_pk: EncryptionPublicKey,
    node_subidentity_sk: SignatureStaticKey,
) {
    {
        let (res_registration_sender, res_registraton_receiver) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::CreateRegistrationCode {
                permissions: IdentityPermissions::Admin,
                code_type: RegistrationCodeType::Profile,
                res: res_registration_sender,
            })
            .await
            .unwrap();
        let node_registration_code = res_registraton_receiver.recv().await.unwrap();

        let code_message = ShinkaiMessageBuilder::code_registration(
            node_profile_encryption_sk.clone(),
            clone_signature_secret_key(&node_subidentity_sk),
            node_encryption_pk,
            node_registration_code.to_string(),
            IdentityType::Profile.to_string(),
            IdentityPermissions::Admin.to_string(),
            node_profile_name.to_string().clone(),
            node_identity_name.to_string(),
        )
        .unwrap();

        eprintln!("code_message: {:?}", code_message);

        tokio::time::sleep(Duration::from_secs(1)).await;

        let (res_use_registration_sender, res_use_registraton_receiver) = async_channel::bounded(2);

        eprintln!("node_commands_sender: {:?}", node_commands_sender);
        eprintln!("res_use_registration_sender: {:?}", res_use_registration_sender);
        node_commands_sender
            .send(NodeCommand::UseRegistrationCode {
                msg: code_message,
                res: res_use_registration_sender,
            })
            .await
            .unwrap();
        let node2_use_registration_code = res_use_registraton_receiver.recv().await.unwrap();
        eprintln!("node2_use_registration_code: {:?}", node2_use_registration_code);
        assert_eq!(
            node2_use_registration_code,
            "true".to_string(),
            "{} used registration code",
            node_profile_name
        );

        let (res_all_subidentities_sender, res_all_subidentities_receiver): (
            async_channel::Sender<Vec<StandardIdentity>>,
            async_channel::Receiver<Vec<StandardIdentity>>,
        ) = async_channel::bounded(1);
        node_commands_sender
            .send(NodeCommand::GetAllSubidentities {
                res: res_all_subidentities_sender,
            })
            .await
            .unwrap();
        let node2_all_subidentities = res_all_subidentities_receiver.recv().await.unwrap();

        assert_eq!(node2_all_subidentities.len(), 1, "Node has 1 subidentity");
        eprintln!(
            "{}",
            format!(
                "{} subidentity: {:?}",
                node_profile_name, node2_all_subidentities[0].full_identity_name
            )
        );
        assert_eq!(
            node2_all_subidentities[0].full_identity_name,
            ShinkaiName::from_node_and_profile(node_identity_name.to_string(), node_profile_name.to_string()).unwrap(),
            "Node has the right subidentity"
        );
    }
}

async fn send_message_from_node_profile_to_other_node(
    node1_commands_sender: Sender<NodeCommand>,
    node2_commands_sender: Sender<NodeCommand>,
    node1_identity_name: &str,
    node2_identity_name: &str,
    node1_encryption_sk_clone: EncryptionStaticKey,
    node1_encryption_pk: EncryptionPublicKey,
    node2_encryption_pk: EncryptionPublicKey,
    node2_subidentity_sk_clone: SignatureStaticKey,
    node2_subencryption_pk: EncryptionPublicKey,
    node2_subidentity_name: &str,
    node1_subidentity_name: &str,
    node2_subencryption_sk: EncryptionStaticKey,
    node1_subencryption_pk: EncryptionPublicKey,
    node1_subidentity_sk: SignatureStaticKey,
    node1_subencryption_sk: EncryptionStaticKey,
) {
    // TODO: how is this being sent if there is not node 1 profile registered?
    // eprintln!("\n\n### Sending message from a node 1 profile to node 2\n\n");

    // let message_content = "test body content".to_string();
    // let unchanged_message = ShinkaiMessageBuilder::new(
    //     node1_subencryption_sk.clone(),
    //     clone_signature_secret_key(&node1_subidentity_sk),
    //     node2_encryption_pk,
    // )
    // .body(message_content.clone())
    // .no_body_encryption()
    // .message_schema_type(MessageSchemaType::TextContent)
    // .internal_metadata(
    //     node1_subidentity_name.to_string().clone(),
    //     "".to_string(),
    //     "".to_string(),
    //     EncryptionMethod::DiffieHellmanChaChaPoly1305,
    // )
    // .external_metadata_with_other(
    //     node2_identity_name.to_string().clone(),
    //     node1_identity_name.to_string().clone(),
    //     encryption_public_key_to_string(node1_subencryption_pk.clone()),
    // )
    // .build()
    // .unwrap();

    // eprintln!("here Message: {:?}", unchanged_message);

    // // let (res_send_msg_sender, res_send_msg_receiver): (
    // //     async_channel::Sender<NodeCommand>,
    // //     async_channel::Receiver<NodeCommand>,
    // // ) = async_channel::bounded(1);

    // node1_commands_sender
    //     .send(NodeCommand::SendOnionizedMessage { msg: unchanged_message })
    //     .await
    //     .unwrap();

    // tokio::time::sleep(Duration::from_secs(2)).await;

    // Get Node2 messages
    println!("\n\n### Sending message from a node 2 profile to node 1\n\n");

    let message_content = "test body content".to_string();
    let unchanged_message = ShinkaiMessageBuilder::new(
        node2_subencryption_sk.clone(),
        clone_signature_secret_key(&node2_subidentity_sk_clone),
        node1_encryption_pk,
    )
    .body(message_content.clone())
    .no_body_encryption()
    .message_schema_type(MessageSchemaType::TextContent)
    .internal_metadata(
        node2_subidentity_name.to_string().clone(),
        "".to_string(),
        "".to_string(),
        EncryptionMethod::DiffieHellmanChaChaPoly1305,
    )
    .external_metadata_with_other(
        node1_identity_name.to_string(),
        node2_identity_name.to_string().clone(),
        encryption_public_key_to_string(node2_subencryption_pk.clone()),
    )
    .build()
    .unwrap();

    eprintln!("here Message: {:?}", unchanged_message);
    eprintln!("node2_commands_sender: {:?}", node2_commands_sender);

    match node2_commands_sender
        .send(NodeCommand::SendOnionizedMessage { msg: unchanged_message })
        .await
    {
        Ok(_) => { /* success */ }
        Err(e) => eprintln!("Failed to send message: {:?}", e),
    }

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

    eprintln!("### Some code ###");
    eprintln!("\n\nNode 1 last messages: {:?}", node1_last_messages);
    eprintln!("\n\n");
    eprintln!("Node 2 last messages: {:?}", node2_last_messages);
    eprintln!("\n\n");

    let message_to_check = node1_last_messages[1].clone();
    // Check that the message is body encrypted
    assert_eq!(
        ShinkaiMessageHandler::is_body_currently_encrypted(&message_to_check.clone()),
        false,
        "Message from Node 2 to Node 1 is not body encrypted for Node 1 (receiver)"
    );

    let message_to_check = node2_last_messages[1].clone();
    // Check that the message is body encrypted
    assert_eq!(
        ShinkaiMessageHandler::is_body_currently_encrypted(&message_to_check.clone()),
        false,
        "Message from Node 2 to Node 1 is not body encrypted for Node 2 (sender)"
    );

    // Check that the content is encrypted
    println!("Message to check: {:?}", message_to_check.clone());
    println!("Message to check: {:?}", message_to_check.clone().body.unwrap().content);
    assert_eq!(
        ShinkaiMessageHandler::is_content_currently_encrypted(&message_to_check.clone()),
        false,
        "Message from Node 2 to Node 1 is not content encrypted"
    );

    {
        eprintln!("Checking that the message has the right sender {:?}", message_to_check);
        let internal_metadata = message_to_check.clone().body.unwrap().internal_metadata.unwrap();
        assert_eq!(
            internal_metadata.sender_subidentity,
            node2_subidentity_name.to_string(),
            "Node 2's profile send an encrypted message to Node 1. The message has the right sender."
        );
    }

    let message_to_check_content_unencrypted = decrypt_content_message(
        message_to_check.clone().body.unwrap().content,
        &message_to_check.clone().encryption.as_str(),
        &node1_encryption_sk_clone.clone(),
        &node2_subencryption_pk,
    )
    .unwrap();

    // This check can't be done using a static value because the nonce is randomly generated
    assert_eq!(
        message_content, message_to_check_content_unencrypted.0,
        "Node 2's profile send an encrypted message to Node 1"
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
