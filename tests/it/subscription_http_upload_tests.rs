use std::sync::Arc;

use dashmap::DashMap;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_node::network::subscription_manager::http_upload_manager::HttpSubscriptionUploadManager;
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
                testing_framework.create_folder("/", "shared_test_folder").await;
                testing_framework
                    .upload_file("/shared_test_folder", "files/shinkai_intro.pdf")
                    .await;
                testing_framework
                    .upload_file("/shared_test_folder", "files/zeko_mini.pdf")
                    .await;
                testing_framework.make_folder_shareable_free_whttp("/shared_test_folder").await;
                testing_framework.show_available_shared_items().await;
            }
            {
                let shared_folders_trees_ref = node1_ext_subscription_manager.lock().await.shared_folders_trees.clone();

                let subscription_uploader = HttpSubscriptionUploadManager::new(
                    node1_db_weak.clone(),
                    node1_vecfs_weak.clone(),
                    ShinkaiName::new(node1_name.clone()).unwrap(),
                    shared_folders_trees_ref.clone(),
                )
                .await;

                let subscriptions_whttp_support =
                    HttpSubscriptionUploadManager::fetch_subscriptions_with_http_support(&node1_db_weak.clone()).await;
                eprintln!("subscriptions_whttp_support: {:?}", subscriptions_whttp_support);
            }
            node1_abort_handler.abort();
        })
    });
}
