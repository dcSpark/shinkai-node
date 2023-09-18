use async_channel::{bounded, Receiver, Sender};
use shinkai_message_primitives::schemas::agents::serialized_agent::{SerializedAgent, SleepAPI, AgentLLMInterface};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, unsafe_deterministic_encryption_keypair,
};
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr};
use tokio::runtime::Runtime;

use crate::utils::node_test_api::{
    api_agent_registration, api_create_job, api_message_job, api_registration_device_node_profile_main,
};
use crate::utils::node_test_performance::{Category, PerformanceCheck};

mod utils;

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn node_agent_perf() {
    // WIP: need to find a way to test the agent registration
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.shinkai";
        let node1_subidentity_name = "main";
        let node1_device_name = "node1_device";
        let node1_agent = "node1_gpt_agent";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node1_device_identity_sk, node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk.clone(),
            0,
            node1_commands_receiver,
            node1_db_path,
            true,
        );

        println!("Starting Node");
        let node1_handler = tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 1");
            let _ = node1.await.start().await;
        });

        let interactions_handler = tokio::spawn(async move {
            println!("Registration of an Admin Profile");

            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_registration_device_node_profile_main(
                    node1_commands_sender.clone(),
                    node1_subidentity_name,
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
                // Register an Agent
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let sleep_ai = SleepAPI {};
                let agent_name = ShinkaiName::new(
                    format!(
                        "{}/{}/agent/{}",
                        node1_identity_name.clone(),
                        node1_subidentity_name.clone(),
                        node1_agent.clone()
                    )
                    .to_string(),
                )
                .unwrap();

                let agent = SerializedAgent {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    perform_locally: false,
                    external_url: None,
                    api_key: None,
                    model: AgentLLMInterface::Sleep(sleep_ai),
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
                };
                api_agent_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone(),
                    node1_subidentity_name.clone(),
                    agent,
                )
                .await;
            }

            let mut job_id = "".to_string();
            let agent_subidentity =
                format!("{}/agent/{}", node1_subidentity_name.clone(), node1_agent.clone()).to_string();
            {
                // Create a Job
                eprintln!("\n\nCreate a Job for the previous Agent in Node1 and verify it");
                job_id = api_create_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_profile_encryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_identity_name.clone(),
                    node1_subidentity_name.clone(),
                    &agent_subidentity.clone(),
                )
                .await;
            }
            // {
            //     // Send a Message to the Job for processing
            //     let performance_check = PerformanceCheck::new(Category::Medium);
            //     eprintln!("\n\nSend a message for a Job");
            //     let message = "Tell me. Who are you?".to_string();
            //     api_message_job(
            //         node1_commands_sender.clone(),
            //         clone_static_secret_key(&node1_profile_encryption_sk),
            //         node1_encryption_pk.clone(),
            //         clone_signature_secret_key(&node1_profile_identity_sk),
            //         node1_identity_name.clone(),
            //         node1_subidentity_name.clone(),
            //         &agent_subidentity.clone(),
            //         &job_id.clone().to_string(),
            //         &message,
            //     )
            //     .await;
            //     assert!(performance_check.check(), "The operation took too long");
            // }
            // Uncomment this when fixed!
            // {
            //     // Send another Message to the Job for processing
            //     let performance_check = PerformanceCheck::new(Category::Medium);
            //     eprintln!("\n\nSend a message for a Job");
            //     let message = "Tell me. Who are you?".to_string();
            //     api_message_job(
            //         node1_commands_sender.clone(),
            //         clone_static_secret_key(&node1_profile_encryption_sk),
            //         node1_encryption_pk.clone(),
            //         clone_signature_secret_key(&node1_profile_identity_sk),
            //         node1_identity_name.clone(),
            //         node1_subidentity_name.clone(),
            //         &agent_subidentity.clone(),
            //         &job_id.clone().to_string(),
            //         &message,
            //     )
            //     .await;
            //     assert!(performance_check.check(), "The operation took too long");
            // }
        });

        // Wait for all tasks to complete
        let _ = tokio::try_join!(node1_handler, interactions_handler).unwrap();
    });
}
