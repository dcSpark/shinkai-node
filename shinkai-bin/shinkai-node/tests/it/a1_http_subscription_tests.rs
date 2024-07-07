use async_channel::{bounded, Receiver, Sender};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use shinkai_message_primitives::schemas::shinkai_subscription::{ShinkaiSubscription, ShinkaiSubscriptionStatus};
use shinkai_vector_resources::utils::hash_string;
use core::panic;
use std::collections::HashMap;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription_req::SubscriptionPayment;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIVecFsRetrievePathSimplifiedJson, FileDestinationCredentials, FileDestinationSourceType, MessageSchemaType
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair 
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::node_api::APIError;
use shinkai_node::network::Node;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

use super::utils::node_test_api::api_registration_device_node_profile_main;
use super::utils::node_test_local::local_registration_profile_node;
use crate::it::utils::db_handlers::setup;
use crate::it::utils::vecfs_test_utils::{check_structure, check_subscription_success, create_folder, fetch_last_messages, generate_message_with_payload, make_folder_shareable, make_folder_shareable_http_free, print_tree_simple, remove_folder, remove_item, remove_timestamps_from_shared_folder_cache_response, retrieve_file_info, show_available_shared_items, upload_file};

#[test]
fn http_subscription_manager_test() {
    std::env::set_var("WELCOME_MESSAGE", "false");
    std::env::set_var("SUBSCRIPTION_HTTP_UPLOAD_INTERVAL_MINUTES", "1000");
    init_default_tracing();
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.arb-sep-shinkai";
        let node2_identity_name = "@@node2_test.arb-sep-shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";
        let node2_profile_name = "main_profile_node2";

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

        // Read AWS credentials from environment variables
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").expect("AWS_ACCESS_KEY_ID not set");
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY").expect("AWS_SECRET_ACCESS_KEY not set");
        let aws_url = std::env::var("AWS_URL").expect("AWS_URL not set");

        // file_dest_credentials
        let file_dest_credentials = FileDestinationCredentials {
            source: FileDestinationSourceType::R2,
            access_key_id,
            secret_access_key,
            endpoint_uri: aws_url,
            bucket: "shinkai-streamer".to_string(),
        };

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

            //
            // Creating a folder and uploading some files to the vector db
            //
            eprintln!("\n\n### Creating a folder and uploading some files to the vector db \n\n");
            // Send message (APICreateFilesInboxWithSymmetricKey) from Device subidentity to Node 1
            {
                // Create /shared test folder
                create_folder(
                    &node1_commands_sender,
                    "/",
                    "shinkai_sharing_http_test",
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;

                // Upload File to /shinkai_sharing_http_test
                let file_path = Path::new("../../files/shinkai_intro.vrkai");
                upload_file(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    "/shinkai_sharing_http_test",
                    file_path,
                    0,
                )
                .await;

                // Upload File to /shinkai_sharing_http_test
                let file_path = Path::new("../../files/zeko.vrkai");
                upload_file(
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    "/shinkai_sharing_http_test",
                    file_path,
                    0,
                )
                .await;
            }
            {
                // Make /shared test folder shareable
                eprintln!("Make /shinkai_sharing_http_test shareable");
                make_folder_shareable_http_free(
                    &node1_commands_sender,
                    "/shinkai_sharing_http_test",
                    node1_profile_encryption_sk.clone(), 
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                    Some(file_dest_credentials),
                )
                .await;

                {
                    eprintln!("### Retrieve info");
                    // Retrieve info
                    retrieve_file_info(
                        &node1_commands_sender,
                        node1_profile_encryption_sk.clone(),
                        clone_signature_secret_key(&node1_profile_identity_sk),
                        node1_encryption_pk,
                        node1_identity_name,
                        node1_profile_name,
                        "/",
                        true,
                    )
                    .await;
                }
            }
            {
                // Show available shared items
                eprintln!("Show available shared items after making /shared test folder shareable");
                show_available_shared_items(
                    node1_identity_name,
                    node1_profile_name,
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
            }
            {
                // Double check that the files are uploaded
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::LocalHttpUploaderProcessSubscriptionUpdates { res: res_sender })
                    .await
                    .unwrap();
                res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("LocalHttpUploaderProcessSubscriptionUpdates done");
            }
            {
                // Show available shared items
                eprintln!("Show available shared items after making /shared test folder shareable");
                show_available_shared_items(
                    node1_identity_name,
                    node1_profile_name,
                    &node1_commands_sender,
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    node1_identity_name,
                    node1_profile_name,
                )
                .await;
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
            {
                eprintln!("\n\n### Sending message from node 2 to node 1 requesting shared folders*\n");

                let unchanged_message = ShinkaiMessageBuilder::vecfs_available_shared_items(
                    None,
                    node1_identity_name.to_string(),
                    node1_profile_name.to_string(),
                    node2_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node2_profile_identity_sk),
                    node2_encryption_pk,
                    node2_identity_name.to_string().clone(),
                    node2_profile_name.to_string().clone(),
                    node2_identity_name.to_string(),
                    node2_profile_name.to_string().clone(),
                    None,
                )
                .unwrap();

                // eprintln!("\n\n unchanged message: {:?}", unchanged_message);

                #[allow(clippy::type_complexity)]
                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<Value, APIError>>,
                    async_channel::Receiver<Result<Value, APIError>>,
                ) = async_channel::bounded(1);

                node2_commands_sender
                    .send(NodeCommand::APIAvailableSharedItems {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();

                let send_result = res_send_msg_receiver.recv().await.unwrap();
                eprint!("send_result: {:?}", send_result);
                assert!(send_result.is_ok(), "Failed to get APIAvailableSharedItems");
            }
            {
                eprintln!(">>> Subscribe to the shared folder");
                eprintln!(
                    "\n\n### Sending message from node 2 to node 1 requesting: subscription to shared test folder\n"
                );
                let requirements = SubscriptionPayment::Free;

                let unchanged_message = ShinkaiMessageBuilder::vecfs_subscribe_to_shared_folder(
                    "/shinkai_sharing_http_test".to_string(),
                    requirements,
                    Some(true),
                    None,
                    node1_identity_name.to_string(),
                    node1_profile_name.to_string(),
                    node2_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node2_profile_identity_sk),
                    node2_encryption_pk,
                    node2_identity_name.to_string().clone(),
                    node2_profile_name.to_string().clone(),
                    node2_identity_name.to_string(),
                    "".to_string(),
                    None,
                )
                .unwrap();

                #[allow(clippy::type_complexity)]
                let (res_send_msg_sender, res_send_msg_receiver): (
                    async_channel::Sender<Result<String, APIError>>,
                    async_channel::Receiver<Result<String, APIError>>,
                ) = async_channel::bounded(1);

                node2_commands_sender
                    .send(NodeCommand::APISubscribeToSharedFolder {
                        msg: unchanged_message,
                        res: res_send_msg_sender,
                    })
                    .await
                    .unwrap();

                let send_result = res_send_msg_receiver.recv().await.unwrap();
                eprint!("\n\nsend_result: {:?}", send_result);

                let subscription_success_message = "{\"subscription_details\":\"Subscribed to /shinkai_sharing_http_test\",\"shared_folder\":\"/shinkai_sharing_http_test\",\"status\":\"Success\",\"error\":null,\"metadata\":null}";
                let subscription_success = check_subscription_success(
                    &node2_commands_sender,
                    4, // attempts
                    2, // delay_secs
                    subscription_success_message,
                )
                .await;
                assert!(subscription_success, "Failed to subscribe to shared folder");                
            }
            {
                // Call Node1's MySubscripcion Job Message Processing (Subscriber)
                 let (res_sender, res_receiver) = async_channel::bounded(1);

                 // Send the command
                 node2_commands_sender
                     .send(NodeCommand::LocalMySubscriptionCallJobMessageProcessing { res: res_sender })
                     .await
                     .unwrap();
                 res_receiver.recv().await.unwrap().expect("Failed to receive response");
                 eprintln!("LocalHttpUploaderProcessSubscriptionUpdates done");
                
            }
            {
               // Call Node1's HTTPDownload Processing (Subscriber)
               let (res_sender, res_receiver) = async_channel::bounded(1);

               // Send the command
               node2_commands_sender
                   .send(NodeCommand::LocalMySubscriptionTriggerHttpDownload { res: res_sender })
                   .await
                   .unwrap();
               res_receiver.recv().await.unwrap().expect("Failed to receive response");
               eprintln!("LocalMySubscriptionTriggerHttpDownload done\n\n"); 
            }
            {
                let mut attempts = 0;
                let max_attempts = 100;
                let mut structure_matched = false;

                while attempts < max_attempts && !structure_matched {
                    
                    eprintln!("\n\n### (Send updates to subscribers) Sending message from node 2's identity to node 2 to check if the subscription synced\n");

                    let payload = APIVecFsRetrievePathSimplifiedJson { path: "/".to_string() };
                    let msg = generate_message_with_payload(
                        serde_json::to_string(&payload).unwrap(),
                        MessageSchemaType::VecFsRetrievePathSimplifiedJson,
                        node2_profile_encryption_sk.clone(),
                        clone_signature_secret_key(&node2_profile_identity_sk),
                        node2_encryption_pk,
                        &node2_identity_name.to_string().clone(),
                        &node2_profile_name.to_string().clone(),
                        node2_identity_name,
                        "",
                    );

                    // Prepare the response channel
                    let (res_sender, res_receiver) = async_channel::bounded(1);

                    // Send the command
                    node2_commands_sender
                        .send(NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res: res_sender })
                        .await
                        .unwrap();
                    let actual_resp_json = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                    print_tree_simple(actual_resp_json.clone());

                    let expected_structure = serde_json::json!({
                        "path": "/",
                        "child_folders": [
                            {
                                "name": "My Subscriptions",
                                "path": "/My Subscriptions",
                                "child_folders": [
                                    {
                                        "name": "shinkai_sharing_http_test",
                                        "path": "/My Subscriptions/shinkai_sharing_http_test",
                                        "child_folders": [],
                                        "child_items": [
                                            {
                                                "name": "shinkai_intro",
                                                "path": "/My Subscriptions/shinkai_sharing_http_test/shinkai_intro"
                                            },
                                            {
                                                "name": "Zeko_Mina_Rollup",
                                                "path": "/My Subscriptions/shinkai_sharing_http_test/Zeko_Mina_Rollup"
                                            }
                                        ]
                                    }
                                ],
                                "child_items": []
                            }
                        ],
                        "child_items": []
                    });

                    structure_matched = check_structure(&actual_resp_json, &expected_structure);
                    if structure_matched {
                        eprintln!("The actual folder structure matches the expected structure.");
                        break;
                    } else {
                        eprintln!("The actual folder structure does not match the expected structure. Retrying...");
                        eprintln!("Expected structure: {}", expected_structure);
                        eprintln!("Actual structure: {}", actual_resp_json);
                    }
                    attempts += 1;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                assert!(structure_matched, "The actual folder structure does not match the expected structure after all attempts.");
                if !structure_matched {
                    panic!("The actual folder structure does not match the expected structure after all attempts.");
                }
            }
            {
                // Dont forget to do this at the end
                node1_abort_handler.abort();
                node2_abort_handler.abort();
            }
        });

        // Wait for all tasks to complete
        let result = tokio::try_join!(node1_handler, node2_handler, interactions_handler);
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

