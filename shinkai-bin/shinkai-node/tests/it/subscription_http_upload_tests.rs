use async_channel::{bounded, Receiver, Sender};
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::file_links::{FileLink, FileStatus};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionStatus, SubscriptionId,
};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    FileDestinationCredentials, FileDestinationSourceType,
};
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_node::network::Node;
use shinkai_subscription_manager::subscription_manager::http_manager::http_download_manager::{
    HttpDownloadJob, HttpDownloadManager,
};
use shinkai_subscription_manager::subscription_manager::http_manager::http_upload_manager::HttpSubscriptionUploadManager;
use shinkai_subscription_manager::subscription_manager::http_manager::subscription_file_uploader::{
    delete_all_in_folder, FileDestination,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use utils::test_boilerplate::run_test_one_node_network;

use super::utils;
use super::utils::node_test_api::api_initial_registration_with_no_code_for_device;
use super::utils::shinkai_testing_framework::ShinkaiTestingFramework;

#[test]
fn subscription_http_upload() {
    std::env::set_var("SUBSCRIPTION_HTTP_UPLOAD_INTERVAL_MINUTES", "1000");

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
            // let node1_my_subscription_manager = env.node1_my_subscriptions_manager.clone();
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
                // Use Unstructrued for PDF parsing until the local one is integrated
                let db_strong = node1_db_weak.upgrade().unwrap();
                db_strong.update_local_processing_preference(false).unwrap();

                // Create folder /shared_test_folder
                testing_framework.create_folder("/", "shinkai_sharing").await;
                testing_framework
                    .upload_file("/shinkai_sharing", "../../files/shinkai_intro.pdf")
                    .await;
                testing_framework
                    .upload_file("/shinkai_sharing", "../../files/zeko_mini.pdf")
                    .await;

                testing_framework
                    .make_folder_shareable_free_whttp("/shinkai_sharing", file_dest_credentials)
                    .await;
                testing_framework.show_available_shared_items().await;
            }
            {
                let external_subscriber_manager = node1_ext_subscription_manager.lock().await;
                let subscription_uploader = &external_subscriber_manager.http_subscription_upload_manager.clone();
                let shared_folders_trees_ref = &external_subscriber_manager.shared_folders_trees.clone();
                drop(external_subscriber_manager);

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

                let _ = HttpSubscriptionUploadManager::subscription_http_check_loop(
                    node1_db_weak.clone(),
                    node1_vecfs_weak.clone(),
                    ShinkaiName::new(node1_name.clone()).unwrap(),
                    subscription_uploader.subscription_file_map.clone(),
                    subscription_uploader.subscription_status.clone(),
                    shared_folders_trees_ref.clone(),
                    subscription_uploader.file_links.clone(),
                    1,
                )
                .await;

                // Check that subscription_file_map (cache) was updated correctly
                let expected_files = [
                    (
                        "/shinkai_sharing/shinkai_intro",
                        "e8f4ee5dda589611c6b5ac06b551031f5e314a7bc130534d12ffc0860d6dac9b",
                    ),
                    (
                        "/shinkai_sharing/zeko_mini.b90941d9.checksum",
                        "d7c996dc47390b3c1cb65e4c1ab03c035363b13b468195bfa6ab0d0eb90941d9",
                    ),
                    (
                        "/shinkai_sharing/shinkai_intro.0d6dac9b.checksum",
                        "e8f4ee5dda589611c6b5ac06b551031f5e314a7bc130534d12ffc0860d6dac9b",
                    ),
                    (
                        "/shinkai_sharing/zeko_mini",
                        "d7c996dc47390b3c1cb65e4c1ab03c035363b13b468195bfa6ab0d0eb90941d9",
                    ),
                ];

                // Print out the content of subscription_file_map and assert the values
                {
                    for entry in subscription_uploader.subscription_file_map.iter() {
                        let _key = entry.key();
                        let value = entry.value();
                        // println!("\n\n(In Test) After everything - Folder Subscription: {:?}", key);
                        for (file_path, status) in value.iter() {
                            // println!("  {} - {:?}", file_path, status);
                            // Find the expected hash for the current file path
                            if let Some((_, expected_hash)) = expected_files.iter().find(|(path, _)| path == file_path)
                            {
                                match status {
                                    FileStatus::Sync(actual_hash) => {
                                        assert_eq!(actual_hash, expected_hash, "Hash mismatch for file: {}", file_path);
                                    }
                                    _ => panic!("Expected Sync status for file: {}", file_path),
                                }
                            } else {
                                panic!("File path {} not found in expected files", file_path);
                            }
                        }
                    }
                }
                // // Print out the content of file_links
                // {
                //     let file_links = subscription_uploader.file_links;
                //     eprintln!("file_links address: {:p}", &file_links);
                //     println!("\n\n File Links Debug:");
                //     for entry in file_links.iter() {
                //         let folder_subscription = entry.key();
                //         eprintln!("Folder Subscription: {:?}", folder_subscription);
                //         let links_map = entry.value();
                //         println!("links map: {:?}", folder_subscription);
                //         for (file_path, link) in links_map.iter() {
                //             println!("  {} - {} - {}", file_path, link.last_8_hash, link.link);
                //         }
                //     }
                // }

                {
                    // Add the subscription to my_subscriptions
                    let new_subscription = SubscriptionId {
                        unique_id: "@@node1_test.arb-sep-shinkai:::main:::shinkai_sharing:::@@node1_test.arb-sep-shinkai:::main"
                            .to_string(),
                        include_folders: None,
                        exclude_folders: None,
                    };

                    let subscription = ShinkaiSubscription {
                        subscription_id: new_subscription,
                        shared_folder: "/shinkai_sharing".to_string(),
                        streaming_node: ShinkaiName::new("@@node1_test.arb-sep-shinkai".to_string()).unwrap(),
                        streaming_profile: "main".to_string(),
                        subscription_description: None,
                        subscriber_destination_path: None,
                        subscriber_node: ShinkaiName::new("@@node1_test.arb-sep-shinkai".to_string()).unwrap(),
                        subscriber_profile: "main".to_string(),
                        payment: None,
                        state: ShinkaiSubscriptionStatus::UnsubscribeConfirmed,
                        date_created: chrono::Utc::now(),
                        last_modified: chrono::Utc::now(),
                        last_sync: None,
                        http_preferred: None,
                    };
                    {
                        let db_strong = node1_db_weak.upgrade().unwrap();
                        let _ = db_strong.add_my_subscription(subscription.clone());
                    }
                    // Instantiate HttpDownloadManager and call process_job_queue
                    let http_download_manager = HttpDownloadManager::new(
                        node1_db_weak.clone(),
                        node1_vecfs_weak.clone(),
                        ShinkaiName::new(node1_name.clone()).unwrap(),
                    )
                    .await;

                    {
                        // let's call the api to download the files
                        // Call the API to download the files and then add them to the job queue manager
                        let db_clone = node1_db_weak.upgrade().unwrap();
                        let node_name_clone = ShinkaiName::new(node1_name.clone()).unwrap();
                        let ext_subscription_manager_clone = node1_ext_subscription_manager.clone();
                        let subscription_profile_path = "main:::/shinkai_sharing".to_string();

                        // Create a channel for sending results
                        #[allow(clippy::complexity)]
                        let (sender, receiver): (
                            Sender<Result<serde_json::Value, APIError>>,
                            Receiver<Result<serde_json::Value, APIError>>,
                        ) = bounded(1);

                        let _ = Node::api_get_http_free_subscription_links(
                            db_clone,
                            node_name_clone,
                            ext_subscription_manager_clone,
                            subscription_profile_path,
                            sender,
                        )
                        .await;

                        match receiver.recv().await {
                            Ok(result) => match result {
                                Ok(value) => {
                                    eprintln!("Received response: {:?}", value);
                                    // Deserialize JSON value to Vec<FileLink>
                                    let file_links: Vec<FileLink> =
                                        serde_json::from_value(value).unwrap_or_else(|_| vec![]);
                                    for file_link in file_links {
                                        let job = HttpDownloadJob {
                                            subscription_id: subscription.subscription_id.clone(), // Assuming FileLink has a field subscription_id
                                            info: file_link.clone(),
                                            url: file_link.link.clone(),
                                            date_created: chrono::Utc::now().to_string(),
                                        };
                                        // Add job to the download queue
                                        http_download_manager.add_job_to_download_queue(job).await.unwrap();
                                    }
                                }
                                Err(e) => eprintln!("Error processing request: {:?}", e),
                            },
                            Err(e) => eprintln!("Failed to receive response: {:?}", e),
                        }
                    }
                    eprintln!("\n\nProcessing download queue");
                    let semaphore = Arc::new(Semaphore::new(1));
                    let mut continue_processing = false;
                    let mut handles = Vec::new();
                    let active_jobs = Arc::new(RwLock::new(HashMap::new()));

                    loop {
                        // Process the job queue using the associated function syntax
                        let new_handles = HttpDownloadManager::process_job_queue(
                            http_download_manager.job_queue_manager.clone(),
                            node1_vecfs_weak.clone(),
                            node1_db_weak.clone(),
                            1,
                            semaphore.clone(),
                            active_jobs.clone(),
                            &mut continue_processing,
                        )
                        .await;

                        // If new_handles is empty, break the loop
                        if new_handles.is_empty() {
                            break;
                        }

                        handles.extend(new_handles);

                        // Wait for all current jobs to complete
                        let handles_to_join = std::mem::take(&mut handles);
                        futures::future::join_all(handles_to_join).await;
                        handles.clear();
                        eprintln!("Download queue processed. New Loop");
                    }
                    eprintln!("Download queue processed");
                    let _res = testing_framework.retrieve_and_print_path_simplified("/", true).await;

                    // After processing the download queue, retrieve file information for specific files
                    let file_info_shinkai_intro = testing_framework
                        .retrieve_file_info("/My Subscriptions/shinkai_sharing/shinkai_intro", true)
                        .await;
                    eprintln!(
                        "File info for /shinkai_sharing/shinkai_intro: {:?}",
                        file_info_shinkai_intro
                    );

                    let file_info_zeko_mini = testing_framework
                        .retrieve_file_info("/My Subscriptions/shinkai_sharing/zeko_mini", true)
                        .await;
                    eprintln!("File info for /shinkai_sharing/zeko_mini: {:?}", file_info_zeko_mini);
                }
            }
            node1_abort_handler.abort();
        })
    });
}
