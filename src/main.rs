use async_channel::{bounded, Receiver, Sender};
// main.rs
use network::Node;
use shinkai_message::encryption::ephemeral_encryption_keys;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

use crate::network::node::NodeCommand;
use crate::network::node_api;
use crate::shinkai_message::encryption::{
    encryption_secret_key_to_string, hash_encryption_public_key, string_to_encryption_static_key,
};
use crate::shinkai_message::signatures::{string_to_signature_secret_key, ephemeral_signature_keypair, hash_signature_public_key, clone_signature_secret_key};
use crate::shinkai_message::signatures::{signature_public_key_to_string, signature_secret_key_to_string};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

mod db;
mod network;
mod shinkai_message;

mod shinkai_message_proto {
    include!(concat!(env!("OUT_DIR"), "/shinkai_message_proto.rs"));
}

fn main() {
    // let node1_identity_name = "@@node1.shinkai";
    // let node2_identity_name = "@@node2.shinkai";

    // let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
    // let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

    // let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
    // let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

    // let (node3_identity_sk, node3_identity_pk) = unsafe_deterministic_signature_keypair(2);
    // let (node3_encryption_sk, node3_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

    // println!(
    //     "node1 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}",
    //     signature_secret_key_to_string(node1_identity_sk),
    //     signature_public_key_to_string(node1_identity_pk),
    //     encryption_secret_key_to_string(node1_encryption_sk),
    //     encryption_public_key_to_string(node1_encryption_pk)
    // );

    // println!("node2 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}", signature_secret_key_to_string(node2_identity_sk), signature_public_key_to_string(node2_identity_pk), encryption_secret_key_to_string(node2_encryption_sk), encryption_public_key_to_string(node2_encryption_pk));
    // println!("node3 identity_secret_key: {} identity_public_key: {} encryption_secret_key: {} encryption_public_key: {}", signature_secret_key_to_string(node3_identity_sk), signature_public_key_to_string(node3_identity_pk), encryption_secret_key_to_string(node3_encryption_sk), encryption_public_key_to_string(node3_encryption_pk));

    // Placeholder for now. Maybe it should be a parameter that the user sets
    // and then it's checked with onchain data for matching with the keys provided
    let global_identity_name = "@@node1.shinkai";

    // Create Tokio runtime
    let mut rt = Runtime::new().unwrap();

    // Generate your keys here or load them from a file.
    let (identity_secret_key, identity_public_key) = match env::var("IDENTITY_SECRET_KEY") {
        Ok(secret_key_str) => {
            let secret_key = string_to_signature_secret_key(&secret_key_str).unwrap();
            let public_key = SignaturePublicKey::from(&secret_key);
            (secret_key, public_key)
        }
        _ => ephemeral_signature_keypair(),
    };

    let (encryption_secret_key, encryption_public_key) = match env::var("ENCRYPTION_SECRET_KEY") {
        Ok(secret_key_str) => {
            let secret_key = string_to_encryption_static_key(&secret_key_str).unwrap();
            let public_key = x25519_dalek::PublicKey::from(&secret_key); 
            (secret_key, public_key)
        }
        _ => ephemeral_encryption_keys(),
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

    let identity_secret_key_string = signature_secret_key_to_string(clone_signature_secret_key(&identity_secret_key));
    let identity_public_key_string = signature_public_key_to_string(identity_public_key.clone());

    let db_path = format!(
        "db/{}",
        hash_signature_public_key(&identity_public_key)
    );
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
        clone_signature_secret_key(&identity_secret_key),
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
                tokio::spawn(async move { connect_node.lock().await.start().await.unwrap() })
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
