use async_channel::{bounded, Receiver, Sender};
// main.rs
use network::Node;
use shinkai_message::encryption::ephemeral_keys;
use shinkai_node::shinkai_message::encryption::{public_key_to_string, string_to_public_key};
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use x25519_dalek::PublicKey;

use crate::network::node::NodeCommand;
use crate::network::node_api;
use crate::shinkai_message::encryption::{
    hash_public_key, secret_key_to_string, string_to_static_key, unsafe_deterministic_double_private_key,
};

mod db;
mod network;
mod shinkai_message;

mod shinkai_message_proto {
    include!(concat!(env!("OUT_DIR"), "/shinkai_message_proto.rs"));
}

fn main() {
    // let (node1_identity, node1_encryption) = unsafe_deterministic_double_private_key(0);
    // let (node2_identity, node2_encryption) = unsafe_deterministic_double_private_key(1);
    // let (node3_identity, node3_encryption) = unsafe_deterministic_double_private_key(2);

    // println!("node1 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}", secret_key_to_string(node1_identity.0), public_key_to_string(node1_identity.1), secret_key_to_string(node1_encryption.0), public_key_to_string(node1_encryption.1));
    // println!("node2 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}", secret_key_to_string(node2_identity.0), public_key_to_string(node2_identity.1), secret_key_to_string(node2_encryption.0), public_key_to_string(node2_encryption.1));
    // println!("node3 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}", secret_key_to_string(node3_identity.0), public_key_to_string(node3_identity.1), secret_key_to_string(node3_encryption.0), public_key_to_string(node3_encryption.1));

    // Placeholder for now. Maybe it should be a parameter that the user sets
    // and then it's checked with onchain data for matching with the keys provided
    let global_identity_name = "@@globalIdentity.shinkai";

    // Create Tokio runtime
    let mut rt = Runtime::new().unwrap();

    // Generate your keys here or load them from a file.
    let (identity_secret_key, identity_public_key) = match env::var("IDENTITY_SECRET_KEY") {
        Ok(secret_key_str) => {
            let secret_key = string_to_static_key(&secret_key_str).unwrap();
            let public_key = PublicKey::from(&secret_key);
            (secret_key, public_key)
        }
        _ => ephemeral_keys(),
    };

    let (encryption_secret_key, encryption_public_key) = match env::var("ENCRYPTION_SECRET_KEY") {
        Ok(secret_key_str) => {
            let secret_key = string_to_static_key(&secret_key_str).unwrap();
            let public_key = PublicKey::from(&secret_key);
            (secret_key, public_key)
        }
        _ => ephemeral_keys(),
    };

    // Fetch the environment variables for the IP and port, or use default values
    let ip: IpAddr = env::var("NODE_IP")
        .unwrap_or_else(|_| "0.0.0.0".to_string())
        .parse()
        .expect("Failed to parse IP address");
    let port: u16 = env::var("NODE_PORT")
        .unwrap_or_else(|_| "8000".to_string())
        .parse()
        .expect("Failed to parse port number");
    let ping_interval: u64 = env::var("PING_INTERVAL_SECS")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .expect("Failed to parse ping interval");

    // Node API configuration
    let api_ip: IpAddr = env::var("NODE_API_IP")
        .unwrap_or_else(|_| "0.0.0.0".to_string())
        .parse()
        .expect("Failed to parse IP address");
    let api_port: u16 = env::var("NODE_API_PORT")
        .unwrap_or_else(|_| "3030".to_string())
        .parse()
        .expect("Failed to parse port number");

    // Define the address and port where your node will listen
    let listen_address = SocketAddr::new(ip, port);
    let api_listen_address = SocketAddr::new(api_ip, api_port);

    let identity_secret_key_string = secret_key_to_string(identity_secret_key.clone());
    let identity_public_key_string = public_key_to_string(identity_public_key.clone());

    let db_path = format!("db/{}", hash_public_key(identity_public_key.clone()));
    // Log the address, port, and public_key
    println!(
        "Starting node with address: {}, port: {}, secret_key {}, public_key: {} and db path: {}",
        ip, port, identity_secret_key_string, identity_public_key_string, db_path
    );

    let (node_commands_sender, node_commands_receiver): (
        Sender<NodeCommand>,
        Receiver<NodeCommand>,
    ) = bounded(100);

    // Create a new node
    let node = Arc::new(Mutex::new(Node::new(
        global_identity_name.to_string(),
        listen_address,
        identity_secret_key.clone(),
        encryption_secret_key.clone(),
        ping_interval,
        node_commands_receiver,
        db_path,
    )));

    // Clone the Arc<Mutex<Node>> for use in each task
    let connect_node = Arc::clone(&node);
    let start_node = Arc::clone(&node);

    // Create a new Tokio runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    // Run the API server and node in separate tasks
    rt.block_on(async {
        // API Server task
        let api_server = tokio::spawn(async move {
            node_api::run_api(node_commands_sender, api_listen_address).await;
        });

        // Node task
        // TODO: this needs redo after node refactoring
        let node_task = if let Ok(_) = env::var("CONNECT_ADDR") {
            if let Ok(_) = env::var("CONNECT_PK") {
                tokio::spawn(async move {
                    connect_node
                        .lock()
                        .await
                        .start()
                        .await
                        .unwrap()
                })
            } else {
                eprintln!("CONNECT_PK environment variable is not set.");
                tokio::spawn(async move { start_node.lock().await.start().await.unwrap() })
            }
        } else {
            tokio::spawn(async move { start_node.lock().await.start().await.unwrap() })
        };

        let _ = tokio::try_join!(api_server, node_task);
    });
}
