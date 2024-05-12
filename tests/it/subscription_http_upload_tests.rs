use std::sync::Arc;

use dashmap::DashMap;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    FileDestinationCredentials, FileDestinationSourceType,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_node::network::subscription_manager::http_manager::http_upload_manager::HttpSubscriptionUploadManager;
use shinkai_node::network::subscription_manager::http_manager::subscription_file_uploader::{delete_all_in_folder, FileDestination};
use utils::test_boilerplate::run_test_one_node_network;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::utils;
use super::utils::node_test_api::api_initial_registration_with_no_code_for_device;
use super::utils::shinkai_testing_framework::ShinkaiTestingFramework;

#[test]
fn subscription_http_upload() {
    std::env::set_var("SUBSCRIPTION_HTTP_UPLOAD_INTERVAL_MINUTES", "1000");
    init_default_tracing();
    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_encryption_pk = env.node1_encryption_pk;
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);
            // let node1_db = env.node1_db.clone();
            // let node1_vecfs = env.node1_vecfs.clone();
            let node1_ext_subscription_manager = env.node1_ext_subscription_manager.clone();
            let node1_name = env.node1_identity_name.clone();
            let node1_abort_handler = env.node1_abort_handler;

            // Downgrade node1_db and node1_vecfs from Arc to Weak
            let node1_db_weak = Arc::downgrade(&env.node1_db);
            let node1_vecfs_weak = Arc::downgrade(&env.node1_vecfs);

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

            // Shinkai Testing Framework
            let testing_framework = ShinkaiTestingFramework::new(
                node1_commands_sender.clone(),
                env.node1_profile_identity_sk.clone(),
                env.node1_profile_encryption_sk.clone(),
                env.node1_encryption_pk,
                env.node1_identity_name.clone(),
                env.node1_profile_name.clone(),
            );

            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_initial_registration_with_no_code_for_device(
                    node1_commands_sender.clone(),
                    env.node1_profile_name.as_str(),
                    env.node1_identity_name.as_str(),
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name.as_str(),
                )
                .await;
            }
            {
                // Create folder /shared_test_folder
                testing_framework.create_folder("/", "shinkai_sharing").await;
                testing_framework
                    .upload_file("/shinkai_sharing", "files/shinkai_intro.pdf")
                    .await;
                testing_framework
                    .upload_file("/shinkai_sharing", "files/zeko_mini.pdf")
                    .await;

                testing_framework
                    .make_folder_shareable_free_whttp("/shinkai_sharing", file_dest_credentials)
                    .await;
                testing_framework.show_available_shared_items().await;
            }
            {
                let shared_folders_trees_ref = node1_ext_subscription_manager.lock().await.shared_folders_trees.clone();

                let _subscription_uploader = HttpSubscriptionUploadManager::new(
                    node1_db_weak.clone(),
                    node1_vecfs_weak.clone(),
                    ShinkaiName::new(node1_name.clone()).unwrap(),
                    shared_folders_trees_ref.clone(),
                )
                .await;

                {
                    // Setting up initial conditions
                    // Retrieve upload credentials from the database
                    let db_strong = node1_db_weak.upgrade().unwrap();
                    let path = "/shinkai_sharing";
                    let profile = "main";
                    let credentials = db_strong.get_upload_credentials(path, profile).unwrap();

                    let destination = FileDestination::from_credentials(credentials).await.unwrap();

                    // clean up the testing folder
                    let _ = delete_all_in_folder(&destination.clone(), "/shinkai_sharing").await;

                    // Adds:
                    // two random files (should get deleted)
                    // a file that has the wrong hash (it should be re-uploaded)
                    let dummy_data1 = vec![1, 2, 3, 4, 5];
                    let dummy_data2 = vec![6, 7, 8, 9, 10];
                    let dummy_file_name1 = "dummy_file1";
                    let dummy_file_name2 = "dummy_file2";
                    let outdated_shinkai_file = "shinkai_intro";

                    // Upload dummy files to the folder /shinkai_sharing
                    testing_framework
                        .update_file_to_http(
                            destination.clone(),
                            dummy_data1.clone(),
                            "/shinkai_sharing",
                            dummy_file_name1,
                        )
                        .await;
                    testing_framework
                        .update_file_to_http(
                            destination.clone(),
                            dummy_data2.clone(),
                            "/shinkai_sharing",
                            dummy_file_name2,
                        )
                        .await;
                    testing_framework
                        .update_file_to_http(
                            destination.clone(),
                            dummy_data2.clone(),
                            "/shinkai_sharing",
                            outdated_shinkai_file,
                        )
                        .await;

                    let checksum_file_name1 = "dummy_file1.4aaabb39.checksum";
                    let checksum_file_name2 = "dummy_file2.2bbbbb39.checksum";
                    let checksum_outdated_shinkai = "shinkai_intro.aaaaaaaa.checksum";

                    testing_framework
                        .update_file_to_http(
                            destination.clone(),
                            dummy_data1.clone(),
                            "/shinkai_sharing",
                            checksum_file_name1,
                        )
                        .await;
                    testing_framework
                        .update_file_to_http(
                            destination.clone(),
                            dummy_data1,
                            "/shinkai_sharing",
                            checksum_outdated_shinkai,
                        )
                        .await;
                    testing_framework
                        .update_file_to_http(destination, dummy_data2, "/shinkai_sharing", checksum_file_name2)
                        .await;
                }

                let subscriptions_whttp_support =
                    HttpSubscriptionUploadManager::fetch_subscriptions_with_http_support(&node1_db_weak.clone()).await;

                assert_eq!(
                    subscriptions_whttp_support.len(),
                    1,
                    "Expected one subscription with HTTP support"
                );
                let subscription = &subscriptions_whttp_support[0];
                assert_eq!(subscription.path, "/shinkai_sharing", "Path does not match");
                assert!(subscription.folder_subscription.is_free, "Subscription should be free");
                assert_eq!(
                    subscription.folder_subscription.has_web_alternative,
                    Some(true),
                    "Should have a web alternative"
                );

                // TODO: do I need to remove these and connect them to the subscription manager?
                let subscription_file_map = DashMap::new();
                let subscription_status = DashMap::new();

                let _ = HttpSubscriptionUploadManager::subscription_http_check_loop(
                    node1_db_weak.clone(),
                    node1_vecfs_weak.clone(),
                    ShinkaiName::new(node1_name.clone()).unwrap(),
                    subscription_file_map, // TODO: change to the one read from above
                    subscription_status,
                    shared_folders_trees_ref.clone(),
                    1,
                )
                .await;
            }
            node1_abort_handler.abort();
        })
    });
}
