use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use async_channel::{bounded, Receiver, Sender};
use ed25519_dalek::SigningKey;
use serde_json::Value;
use shinkai_message_primitives::shinkai_utils::{
    encryption::{
        encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
    },
    shinkai_logging::init_default_tracing,
    shinkai_message_builder::ShinkaiMessageBuilder,
    signatures::{
        clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
        unsafe_deterministic_signature_keypair,
    },
};
use shinkai_node::network::{node_commands::NodeCommand, node_api_router::APIError, Node};
use shinkai_tcp_relayer::TCPProxy;
use shinkai_vector_resources::utils::hash_string;
use tokio::{net::TcpListener, runtime::Runtime, time::sleep};

use crate::it::utils::{
    node_test_local::local_registration_profile_node, shinkai_testing_framework::ShinkaiTestingFramework, test_boilerplate::{default_embedding_model, supported_embedding_models}, vecfs_test_utils::remove_timestamps_from_shared_folder_cache_response
};

use super::utils::db_handlers::setup;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

#[test]
fn tcp_proxy_test_identity() {
    std::env::set_var("WELCOME_MESSAGE", "false");
    init_default_tracing();
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test_with_proxy2.arb-sep-shinkai";
        let node2_identity_name = "@@node2_test.arb-sep-shinkai";
        let node1_profile_name = "main";
        let node2_profile_name = "main";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let _node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let node2_encryption_sk_clone = node2_encryption_sk.clone();

        let tcp_proxy_identity_name = "@@tcp_tests_proxy.arb-sep-shinkai";
        let (tcp_proxy_identity_sk, tcp_proxy_identity_pk) = unsafe_deterministic_signature_keypair(2);
        let (tcp_proxy_encryption_sk, tcp_proxy_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

        eprintln!(
            "TCP Proxy encryption sk: {:?}",
            encryption_secret_key_to_string(tcp_proxy_encryption_sk.clone())
        );
        eprintln!(
            "TCP Proxy encryption pk: {:?}",
            encryption_public_key_to_string(tcp_proxy_encryption_pk)
        );
        eprintln!(
            "TCP Proxy identity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&tcp_proxy_identity_sk))
        );
        eprintln!(
            "TCP Proxy identity pk: {:?}",
            signature_public_key_to_string(tcp_proxy_identity_pk)
        );

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
            node1_encryption_sk.clone(),
            0,
            node1_commands_receiver,
            node1_db_path,
            "".to_string(),
            Some(tcp_proxy_identity_name.to_string()),
            true,
            vec![],
            node1_fs_db_path,
            None,
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            None,
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
            None,
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

        // Setup a TCP listener
        // Info from: https://shinkai-contracts.pages.dev/identity/tcp_tests_proxy.arb-sep-shinkai

        // Spawn a task to accept connections
        let tcp_handle = tokio::spawn({
            // Creates a TCPProxy instance
            let proxy = TCPProxy::new(
                Some(tcp_proxy_identity_sk),
                Some(tcp_proxy_encryption_sk),
                Some(tcp_proxy_identity_name.to_string()),
                None,
                None,
                None,
            )
            .await
            .unwrap();

            let proxy = proxy.clone();
            let listener = TcpListener::bind("127.0.0.1:8084").await.unwrap();
            async move {
                loop {
                    if let Ok((socket, _)) = listener.accept().await {
                        proxy.handle_client(socket).await;
                    }
                    eprintln!("handle_client new loop");
                    sleep(Duration::from_millis(200)).await;
                }
            }
        });
        let tcp_abort_handler = tcp_handle.abort_handle();
        sleep(Duration::from_secs(3)).await;

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
                eprintln!("Register a Profile in Node1 and verify it");
                local_registration_profile_node(
                    node1_commands_sender.clone(),
                    node1_profile_name,
                    node1_identity_name,
                    node1_subencryption_sk_clone.clone(),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    1,
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

            tokio::time::sleep(Duration::from_secs(2)).await;

            // Shinkai Testing Framework
            let node_2_testing_framework = ShinkaiTestingFramework::new(
                node2_commands_sender.clone(),
                node2_profile_identity_sk.clone(),
                node2_profile_encryption_sk.clone(),
                node2_encryption_pk,
                node2_identity_name.to_string(),
                node2_profile_name.to_string(),
            );

            //
            // Creating a folder and uploading some files to the vector db
            //
            eprintln!("\n\n### Creating a folder and uploading some files to the vector db \n\n");
            {
                // Create /shinkai_sharing folder
                node_2_testing_framework.create_folder("/", "shinkai_sharing").await;
                node_2_testing_framework
                    .upload_file("/shinkai_sharing", "../../files/shinkai_intro.vrkai")
                    .await;
                node_2_testing_framework.make_folder_shareable("/shinkai_sharing").await;

                // For Debugging
                let node2_info = node_2_testing_framework.retrieve_file_info("/", true).await;
                eprintln!("Node 2 info: {:?}", node2_info);
                node_2_testing_framework.show_available_shared_items().await;
            }
            {
                eprintln!("\n\n### Sending message from node 1 to TCP Relay to node 2 requesting shared folders*\n");

                let mut expected_response = serde_json::json!({
                    "node_name": "@@node2_test.arb-sep-shinkai/main",
                    "last_ext_node_response": "2024-05-25T20:42:48.285935Z",
                    "last_request_to_ext_node": "2024-05-25T20:42:48.285935Z",
                    "last_updated": "2024-05-25T20:42:48.285935Z",
                    "state": "ResponseAvailable",
                    "response_last_updated": "2024-05-25T20:42:48.285935Z",
                    "response": {
                        "/shinkai_sharing": {
                            "path": "/shinkai_sharing",
                            "permission": "Public",
                            "profile": "main",
                            "tree": {
                                "name": "/",
                                "path": "/shinkai_sharing",
                                "web_link": null,
                                "last_modified": "2024-05-25T20:42:47.557583Z",
                                "children": {
                                    "shinkai_intro": {
                                        "name": "shinkai_intro",
                                        "path": "/shinkai_sharing/shinkai_intro",
                                        "last_modified": "2024-05-01T17:38:59.904492Z",
                                        "web_link": null,
                                        "children": {}
                                    }
                                }
                            },
                            "subscription_requirement": {
                                "minimum_token_delegation": 100,
                                "minimum_time_delegated_hours": 100,
                                "monthly_payment": {
                                    "USD": "10.00"
                                },
                                "has_web_alternative": false,
                                "is_free": false,
                                "folder_description": "This is a test folder"
                            }
                        }
                    }
                });

                let mut success = false;
                for _attempt in 0..15 {
                    let send_result = create_and_send_message(
                        node1_commands_sender.clone(),
                        node2_identity_name,
                        node2_profile_name,
                        &node1_profile_encryption_sk,
                        &node1_profile_identity_sk,
                        &node1_encryption_pk,
                        node1_identity_name,
                        node1_profile_name,
                    )
                    .await;

                    if let Ok(mut actual_response) = send_result {
                        remove_timestamps_from_shared_folder_cache_response(&mut expected_response);
                        remove_timestamps_from_shared_folder_cache_response(&mut actual_response);

                        if actual_response == expected_response {
                            success = true;
                            break;
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                }

                assert!(
                    success,
                    "Failed to match the expected shared folder information after multiple attempts"
                );
            }
            {
                // Dont forget to do this at the end
                node1_abort_handler.abort();
                node2_abort_handler.abort();
                tcp_abort_handler.abort();
            }
        });

        // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, node2_handler, tcp_handle, interactions_handler);
        match result {
            Ok(_) => {}
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    eprintln!("One of the tasks was aborted, but this is expected.");
                } else {
                    // If the error is not due to an abort, then it's unexpected
                    panic!("An unexpected error occurred: {:?}", e);
                }
            }
        }
    });

    rt.shutdown_background();
}

#[test]
fn tcp_proxy_test_localhost() {
    std::env::set_var("WELCOME_MESSAGE", "false");
    init_default_tracing();
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@localhost.arb-sep-shinkai";
        let node2_identity_name = "@@node2_test.arb-sep-shinkai";
        let node1_profile_name = "main";
        let node2_profile_name = "main";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let _node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let node2_encryption_sk_clone = node2_encryption_sk.clone();

        let tcp_proxy_identity_name = "@@tcp_tests_proxy.arb-sep-shinkai";
        let (tcp_proxy_identity_sk, tcp_proxy_identity_pk) = unsafe_deterministic_signature_keypair(2);
        let (tcp_proxy_encryption_sk, tcp_proxy_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

        eprintln!(
            "TCP Proxy encryption sk: {:?}",
            encryption_secret_key_to_string(tcp_proxy_encryption_sk.clone())
        );
        eprintln!(
            "TCP Proxy encryption pk: {:?}",
            encryption_public_key_to_string(tcp_proxy_encryption_pk)
        );
        eprintln!(
            "TCP Proxy identity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&tcp_proxy_identity_sk))
        );
        eprintln!(
            "TCP Proxy identity pk: {:?}",
            signature_public_key_to_string(tcp_proxy_identity_pk)
        );

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

        let (_node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (_node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

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
            node1_encryption_sk.clone(),
            0,
            node1_commands_receiver,
            node1_db_path,
            "".to_string(),
            Some(tcp_proxy_identity_name.to_string()),
            true,
            vec![],
            node1_fs_db_path,
            None,
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            None,
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
            None,
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

        // Setup a TCP listener
        // Info from: https://shinkai-contracts.pages.dev/identity/tcp_tests_proxy.arb-sep-shinkai

        // Spawn a task to accept connections
        let tcp_handle = tokio::spawn({
            // Creates a TCPProxy instance
            let proxy = TCPProxy::new(
                Some(tcp_proxy_identity_sk),
                Some(tcp_proxy_encryption_sk),
                Some(tcp_proxy_identity_name.to_string()),
                None,
                None,
                None,
            )
            .await
            .unwrap();

            let proxy = proxy.clone();
            let listener = TcpListener::bind("127.0.0.1:8084").await.unwrap();
            async move {
                loop {
                    if let Ok((socket, _)) = listener.accept().await {
                        proxy.handle_client(socket).await;
                    }
                    eprintln!("handle_client new loop");
                    sleep(Duration::from_millis(200)).await;
                }
            }
        });
        let tcp_abort_handler = tcp_handle.abort_handle();
        sleep(Duration::from_secs(3)).await;

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

            tokio::time::sleep(Duration::from_secs(3)).await;

            // Register a Profile in Node1 and verifies it
            {
                eprintln!("Register a Profile in Node1 and verify it");
                local_registration_profile_node(
                    node1_commands_sender.clone(),
                    node1_profile_name,
                    node1_identity_name,
                    node1_subencryption_sk_clone.clone(),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    1,
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

            tokio::time::sleep(Duration::from_secs(2)).await;

            // Shinkai Testing Framework
            let node_2_testing_framework = ShinkaiTestingFramework::new(
                node2_commands_sender.clone(),
                node2_profile_identity_sk.clone(),
                node2_profile_encryption_sk.clone(),
                node2_encryption_pk,
                node2_identity_name.to_string(),
                node2_profile_name.to_string(),
            );

            //
            // Creating a folder and uploading some files to the vector db
            //
            eprintln!("\n\n### Creating a folder and uploading some files to the vector db \n\n");
            {
                // Create /shinkai_sharing folder
                node_2_testing_framework.create_folder("/", "shinkai_sharing").await;
                node_2_testing_framework
                    .upload_file("/shinkai_sharing", "../../files/shinkai_intro.vrkai")
                    .await;
                node_2_testing_framework.make_folder_shareable("/shinkai_sharing").await;

                // For Debugging
                let node2_info = node_2_testing_framework.retrieve_file_info("/", true).await;
                eprintln!("Node 2 info: {:?}", node2_info);
                node_2_testing_framework.show_available_shared_items().await;
            }
            {
                eprintln!("\n\n### Sending message from node 1 to TCP Relay to node 2 requesting shared folders*\n");

                let _send_result = create_and_send_message(
                    node1_commands_sender.clone(),
                    node2_identity_name,
                    node2_profile_name,
                    &node1_profile_encryption_sk,
                    &node1_profile_identity_sk,
                    &node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                let mut expected_response = serde_json::json!({
                "node_name": "@@node2_test.arb-sep-shinkai/main",
                "last_ext_node_response": "2024-05-25T20:42:48.285935Z",
                "last_request_to_ext_node": "2024-05-25T20:42:48.285935Z",
                "last_updated": "2024-05-25T20:42:48.285935Z",
                "state": "ResponseAvailable",
                "response_last_updated": "2024-05-25T20:42:48.285935Z",
                "response": {
                    "/shinkai_sharing": {
                        "path": "/shinkai_sharing",
                            "permission": "Public",
                            "profile": "main",
                            "tree": {
                                "name": "/",
                                "path": "/shinkai_sharing",
                                "last_modified": "2024-05-25T20:42:47.557583Z",
                                "web_link": null,
                                "children": {
                                    "shinkai_intro": {
                                        "name": "shinkai_intro",
                                        "path": "/shinkai_sharing/shinkai_intro",
                                        "web_link": null,
                                        "last_modified": "2024-05-01T17:38:59.904492Z",
                                        "children": {}
                                    }
                                }
                            },
                            "subscription_requirement": {
                                "minimum_token_delegation": 100,
                                "minimum_time_delegated_hours": 100,
                                "monthly_payment": {
                                    "USD": "10.00"
                                },
                                "has_web_alternative": false,
                                "is_free": false,
                                "folder_description": "This is a test folder"
                            }
                        }
                    }
                });

                let mut success = false;
                for _ in 0..15 {
                    // Check up to 15 times (4 seconds each, total 60 seconds)
                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                    let send_result = create_and_send_message(
                        node1_commands_sender.clone(),
                        node2_identity_name,
                        node2_profile_name,
                        &node1_profile_encryption_sk,
                        &node1_profile_identity_sk,
                        &node1_encryption_pk,
                        node1_identity_name,
                        node1_profile_name,
                    )
                    .await;

                    if let Ok(mut actual_response) = send_result {
                        remove_timestamps_from_shared_folder_cache_response(&mut expected_response);
                        remove_timestamps_from_shared_folder_cache_response(&mut actual_response);

                        if actual_response == expected_response {
                            success = true;
                            break;
                        }
                    }
                }

                assert!(
                    success,
                    "Failed to match the expected shared folder information after multiple attempts"
                );
            }
            {
                // Dont forget to do this at the end
                node1_abort_handler.abort();
                node2_abort_handler.abort();
                tcp_abort_handler.abort();
            }
        });

        // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, node2_handler, tcp_handle, interactions_handler);
        match result {
            Ok(_) => {}
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    eprintln!("One of the tasks was aborted, but this is expected.");
                } else {
                    // If the error is not due to an abort, then it's unexpected
                    panic!("An unexpected error occurred: {:?}", e);
                }
            }
        }
    });

    rt.shutdown_background();
}

#[allow(clippy::complexity)]
async fn create_and_send_message(
    node1_commands_sender: Sender<NodeCommand>,
    node2_identity_name: &str,
    node2_profile_name: &str,
    node1_profile_encryption_sk: &EncryptionStaticKey,
    node1_profile_identity_sk: &SigningKey,
    node1_encryption_pk: &EncryptionPublicKey,
    node1_identity_name: &str,
    node1_profile_name: &str,
) -> Result<Value, APIError> {
    let unchanged_message = ShinkaiMessageBuilder::vecfs_available_shared_items(
        None,
        node2_identity_name.to_string(),
        node2_profile_name.to_string(),
        node1_profile_encryption_sk.clone(),
        clone_signature_secret_key(node1_profile_identity_sk),
        node1_encryption_pk.clone(),
        node1_identity_name.to_string(),
        node1_profile_name.to_string(),
        node1_identity_name.to_string(),
        "".to_string(),
        None,
    )
    .unwrap();

    #[allow(clippy::type_complexity)]
    let (res_send_msg_sender, res_send_msg_receiver): (
        async_channel::Sender<Result<Value, APIError>>,
        async_channel::Receiver<Result<Value, APIError>>,
    ) = async_channel::bounded(1);

    node1_commands_sender
        .send(NodeCommand::APIAvailableSharedItems {
            msg: unchanged_message,
            res: res_send_msg_sender,
        })
        .await
        .unwrap();

    res_send_msg_receiver.recv().await.unwrap()
}
