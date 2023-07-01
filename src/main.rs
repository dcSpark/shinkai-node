// main.rs
use network::Node;
use shinkai_message::encryption::ephemeral_keys;
use shinkai_node::shinkai_message::encryption::{public_key_to_string, string_to_public_key};
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey};

use crate::network::node_api;
use crate::shinkai_message::encryption::{secret_key_to_string, string_to_static_key};

mod network;
mod shinkai_message;

mod shinkai_message_proto {
    include!(concat!(env!("OUT_DIR"), "/shinkai_message_proto.rs"));
}

fn main() {
    // Create Tokio runtime
    let mut rt = Runtime::new().unwrap();

    // Generate your keys here or load them from a file.
    let (secret_key, public_key) = match (env::var("SECRET_KEY")) {
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

    let secret_key_string = secret_key_to_string(secret_key.clone());
    let public_key_string = public_key_to_string(public_key.clone());
    // Log the address, port, and public_key
    println!(
        "Starting node with address: {}, port: {}, secret_key {} and public_key: {}",
        ip, port, secret_key_string, public_key_string
    );

    // Create a new node
    let node = Arc::new(Mutex::new(Node::new(
        listen_address,
        secret_key.clone(),
        public_key.clone(),
    )));

    // Clone the Arc<Mutex<Node>> for use in each task
    let api_node = Arc::clone(&node);
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
            node_api::serve(api_node, api_listen_address).await;
        });

        // Node task
        let node_task = if let Ok(connect_addr) = env::var("CONNECT_ADDR") {
            if let Ok(connect_pk_str) = env::var("CONNECT_PK") {
                let connect_pk: PublicKey = string_to_public_key(&connect_pk_str).unwrap();
                tokio::spawn(async move {
                    connect_node
                        .lock()
                        .await
                        .start_and_connect(&connect_addr, connect_pk)
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
