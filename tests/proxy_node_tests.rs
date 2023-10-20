use async_channel::{bounded, Receiver, Sender};
use async_std::println;
use core::panic;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    IdentityPermissions, MessageSchemaType, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::node_api::{self, APIError};
use shinkai_node::network::node_proxy::{IsProxyConf, NodeProxyMode, ProxyIdentity};
use shinkai_node::network::Node;
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

mod utils;
use crate::utils::node_test_api::{
    api_registration_device_node_profile_main, api_registration_profile_node, api_try_re_register_profile_node,
};
use crate::utils::node_test_local::local_registration_profile_node;

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn api_to_node_proxy() {
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1.shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";

        let proxy_identity_name = "@@node1_proxy.shinkai";
        let proxy_profile_name = "proxy";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let node2_encryption_sk_clone = node2_encryption_sk.clone();

        let node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);
        let node2_identity_sk_clone = clone_signature_secret_key(&node2_identity_sk);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node2_subidentity_sk, node2_subidentity_pk) = unsafe_deterministic_signature_keypair(101);
        let (node2_subencryption_sk, node2_subencryption_pk) = unsafe_deterministic_encryption_keypair(101);

        let node1_subencryption_sk_clone = node1_profile_encryption_sk.clone();
        let node2_subencryption_sk_clone = node2_subencryption_sk.clone();

        let node1_subidentity_sk_clone = clone_signature_secret_key(&node1_profile_identity_sk);
        let node2_subidentity_sk_clone = clone_signature_secret_key(&node2_subidentity_sk);

        let (node1_device_identity_sk, node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));
        let node2_db_path = format!("db_tests/{}", hash_string(proxy_identity_name.clone()));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9550);
        let addr1_api = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9551);
        let proxy_identity = ProxyIdentity {
            api_peer: addr2,
            tcp_peer: addr2,
            shinkai_name: ShinkaiName::new(proxy_identity_name.to_string()).unwrap(),
        };
        let node_conf = NodeProxyMode::IsProxied(vec![proxy_identity.clone()]);

        let mut node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            node1_identity_sk,
            node1_encryption_sk,
            0,
            node1_commands_receiver,
            node1_db_path,
            true,
            None,
            NodeProxyMode::NoProxy,
        );

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9560);
        let addr2_api = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9561);
        let proxy_node_conf = NodeProxyMode::IsProxy(IsProxyConf {
            allow_new_identities: true,
            proxy_node_identities: {
                let mut map = HashMap::new();
                map.insert(
                    node1_identity_name.to_string(),
                    ProxyIdentity {
                        api_peer: addr1,
                        tcp_peer: addr1,
                        shinkai_name: ShinkaiName::new(node1_identity_name.to_string()).unwrap(),
                    },
                );
                map
            },
        });
        let mut proxy_node = Node::new(
            proxy_identity_name.to_string(),
            addr2,
            node2_identity_sk,
            node2_encryption_sk,
            0,
            node2_commands_receiver,
            node2_db_path,
            true,
            None,
            proxy_node_conf,
        );

        // Start node1 and node2
        let node1_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 1");
            let _ = node1.await.start().await;
        });

        let node1_api_handler = tokio::spawn(async move {
            node_api::run_api(node1_commands_sender, addr1_api, NodeProxyMode::NoProxy).await;
        });

        let proxy_node_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting proxy node");
            let _ = proxy_node.await.start().await;
        });

        let proxy_node_api_handler = tokio::spawn(async move {
            node_api::run_api(node2_commands_sender, addr2_api, NodeProxyMode::NoProxy).await;
        });

        let interactions_handler = tokio::spawn(async move {
            eprintln!("Starting interactions");
            eprintln!("Registration of Subidentities");

            // Wait for the API servers to start
            tokio::time::sleep(Duration::from_secs(1)).await;

            // Send a GET request to the v1/shinkai_health endpoint of each node
            let client = reqwest::Client::new();
            let node1_health = client
                .get(format!(
                    "http://{}:{}/v1/shinkai_health",
                    addr1_api.ip(),
                    addr1_api.port()
                ))
                .send()
                .await;
            let proxy_node_health = client
                .get(format!(
                    "http://{}:{}/v1/shinkai_health",
                    addr2_api.ip(),
                    addr2_api.port()
                ))
                .send()
                .await;

            match node1_health {
                Ok(response) => {
                    if response.status().is_success() {
                        eprintln!("Node 1 is healthy");
                    } else {
                        eprintln!("Node 1 is not healthy");
                    }
                }
                Err(e) => eprintln!("Failed to check Node 1 health: {}", e),
            }

            match proxy_node_health {
                Ok(response) => {
                    if response.status().is_success() {
                        eprintln!("Node 2 is healthy");
                    } else {
                        eprintln!("Node 2 is not healthy");
                    }
                }
                Err(e) => eprintln!("Failed to check Node 2 health: {}", e),
            }
        });

        // Wait for all tasks to complete
        let _ = tokio::try_join!(
            node1_handler,
            node1_api_handler,
            proxy_node_handler,
            interactions_handler
        )
        .unwrap();
    });
}
