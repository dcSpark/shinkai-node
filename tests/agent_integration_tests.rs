use async_channel::{bounded, Receiver, Sender};
use shinkai_message_wasm::schemas::agents::serialized_agent::{AgentAPIModel, OpenAI, SerializedAgent};
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;
use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_wasm::shinkai_utils::encryption::{unsafe_deterministic_encryption_keypair, EncryptionMethod, clone_static_secret_key};
use shinkai_message_wasm::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_wasm::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_wasm::shinkai_utils::utils::hash_string;
use shinkai_node::managers::agent;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::node_api::APIError;
use shinkai_node::network::Node;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

use crate::utils::node_test_api::{api_agent_registration, api_registration_device_node_profile_main, api_create_job};

mod utils;

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn node_agent_registration() {
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

        let (node1_subidentity_sk, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_subencryption_sk, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

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
        );

        println!("Starting Node");
        let node1_handler = tokio::spawn(async move {
            println!("\n\n");
            println!("Starting node 1");
            let _ = node1.await.start().await;
        });

        let interactions_handler = tokio::spawn(async move {
            println!("Registration of an Admin Profile");

            // Register a Profile in Node1 and verifies it
            {
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_registration_device_node_profile_main(
                    node1_commands_sender.clone(),
                    node1_subidentity_name,
                    node1_identity_name,
                    node1_subencryption_sk.clone(),
                    node1_encryption_pk,
                    clone_signature_secret_key(&node1_subidentity_sk),
                    node1_device_name,
                )
                .await;
            }

            // Register an Agent
            {
                eprintln!("\n\nRegister an Agent in Node1 and verify it");
                let open_ai = OpenAI {
                    model_type: "gpt-3.5-turbo".to_string(),
                };
                let agent_name = ShinkaiName::new(
                    format!(
                        "{}/{}/agent/{}",
                        node1_identity_name.clone(), node1_subidentity_name.clone(), node1_agent.clone()
                    )
                    .to_string(),
                )
                .unwrap();
                let agent = SerializedAgent {
                    id: node1_agent.clone().to_string(),
                    full_identity_name: agent_name,
                    perform_locally: false,
                    external_url: Some("http://localhost:808080".to_string()),
                    api_key: Some("test_api_key".to_string()),
                    model: AgentAPIModel::OpenAI(open_ai),
                    toolkit_permissions: vec![],
                    storage_bucket_permissions: vec![],
                    allowed_message_senders: vec![],
                };
                api_agent_registration(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_subencryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_subidentity_sk),
                    node1_identity_name.clone(),
                    node1_subidentity_name.clone(),
                    agent,
                )
                .await;
            }

            // Create a Job
            {
                eprintln!("\n\nCreate a Job for the previous Agent in Node1 and verify it");
                let subidentity_name = ShinkaiName::new(
                    format!(
                        "{}/{}/agent/{}",
                        node1_identity_name.clone(), node1_subidentity_name.clone(), node1_agent.clone()
                    )
                    .to_string(),
                ).unwrap();
                api_create_job(
                    node1_commands_sender.clone(),
                    clone_static_secret_key(&node1_subencryption_sk),
                    node1_encryption_pk.clone(),
                    clone_signature_secret_key(&node1_subidentity_sk),
                    node1_identity_name.clone(),
                    &subidentity_name.get_agent_name().unwrap(),
                )
                .await;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        // Wait for all tasks to complete
        let _ = tokio::try_join!(node1_handler, interactions_handler).unwrap(); // node2_handler,
    });
}
