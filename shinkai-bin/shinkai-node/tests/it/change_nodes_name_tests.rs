use std::{
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    path::Path,
    time::Duration,
};

use crate::it::utils::node_test_api::api_registration_device_node_profile_main;
use async_channel::{bounded, Receiver, Sender};
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::shinkai_utils::{
    encryption::unsafe_deterministic_encryption_keypair,
    shinkai_message_builder::ShinkaiMessageBuilder,
    signatures::{clone_signature_secret_key, hash_signature_public_key, unsafe_deterministic_signature_keypair},
};
use shinkai_node::network::Node;
use tokio::runtime::Runtime;

use super::utils::test_boilerplate::{default_embedding_model, supported_embedding_models};

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(path);
}

#[test]
fn change_nodes_name_test() {
    setup();
    std::env::set_var("SKIP_IMPORT_FROM_DIRECTORY", "true");
    std::env::set_var("IS_TESTING", "1");

    fn port_is_available(port: u16) -> bool {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    let rt = Runtime::new().unwrap();
    let e = rt.block_on(async {
        let new_node_name = "@@change_node_test.sep-shinkai";
        let node1_identity_name = "@@node1_test.sep-shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let (node1_profile_identity_sk, _node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, _node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node1_device_identity_sk, _node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, _node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let node1_db_path = format!("db_tests/{}", hash_signature_public_key(&node1_identity_pk));
        assert!(port_is_available(8080), "Port 8080 is not available");
        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
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
            vec![],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            None,
        );

        let node1_handler = tokio::spawn(async move {
            let _ = node1.await.lock().await.start().await;
        });

        let abort_handler = node1_handler.abort_handle();

        let interactions_handler = tokio::spawn(async move {
            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
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
            {
                // Change Nodes Name
                let job_message = ShinkaiMessageBuilder::change_node_name(
                    new_node_name.to_string(),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name.to_string(),
                    node1_profile_name.to_string(),
                    node1_identity_name.to_string(),
                    node1_profile_name.to_string(),
                )
                .unwrap();

                let (res_message_job_sender, res_message_job_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIChangeNodesName {
                        msg: job_message.clone(),
                        res: res_message_job_sender,
                    })
                    .await
                    .unwrap();
                let _resp = res_message_job_receiver.recv().await.unwrap();
            }
            {
                // Restart Node
                eprintln!("Restarting Node1");
                abort_handler.abort();
            }
        });

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

    let rt = Runtime::new().unwrap();
    let e = rt.block_on(async {
        let new_node_name = "@@change_node_test.sep-shinkai";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, _node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_signature_public_key(&node1_identity_pk));
        assert!(port_is_available(8080), "Port 8080 is not available");
        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            new_node_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk.clone(),
            None,
            None,
            0,
            node1_commands_receiver,
            node1_db_path,
            Path::new("db").join(".secret").to_str().unwrap().to_string(),
            None,
            true,
            vec![],
            None,
            None,
            default_embedding_model(),
            supported_embedding_models(),
            None,
        );

        let node1_handler = tokio::spawn(async move {
            let _ = node1.await.lock().await.start().await;
        });

        let abort_handler = node1_handler.abort_handle();

        let interactions_handler = tokio::spawn(async move {
            eprintln!("starting the node for a second time");
            {
                let (res_message_job_sender, res_message_job_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::IsPristine {
                        res: res_message_job_sender,
                    })
                    .await
                    .unwrap();
                let resp = res_message_job_receiver.recv().await.unwrap();
                assert!(!resp);
            }
            {
                let (res_message_job_sender, res_message_job_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::GetNodeName {
                        res: res_message_job_sender,
                    })
                    .await
                    .unwrap();
                let resp = res_message_job_receiver.recv().await.unwrap();
                assert_eq!(resp, new_node_name);
            }
            abort_handler.abort();
        });

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
    assert!(port_is_available(8080), "Port 8080 is not available");
}
