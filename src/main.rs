// main.rs
use crate::network::node::NodeCommand;
use crate::network::node_api;
use crate::utils::args::parse_args;
use crate::utils::cli::cli_handle_create_message;
use crate::utils::environment::fetch_node_environment;
use crate::utils::keys::generate_or_load_keys;
use crate::utils::qr_code_setup::generate_qr_codes;
use async_channel::{bounded, Receiver, Sender};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use network::Node;
use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::{IdentityPermissions, RegistrationCodeType};
use shinkai_message_wasm::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string,
};
use shinkai_message_wasm::shinkai_utils::signatures::{
    clone_signature_secret_key, hash_signature_public_key, signature_public_key_to_string,
    signature_secret_key_to_string,
};
use std::env;
use std::sync::Arc;
use tokio::runtime::Runtime;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

mod db;
mod managers;
mod network;
mod resources;
mod schemas;
mod utils;

fn initialize_runtime() -> Runtime {
    Runtime::new().unwrap()
}

fn get_db_path(identity_public_key: &SignaturePublicKey) -> String {
    format!("db/{}", hash_signature_public_key(identity_public_key))
}

fn main() {
    env_logger::init();

    // Placeholder for now. Maybe it should be a parameter that the user sets
    // and then it's checked with onchain data for matching with the keys provided
    let global_identity_name = env::var("GLOBAL_IDENTITY_NAME").unwrap_or("@@node1.shinkai".to_string());

    // Initialization
    let args = parse_args();

    // Create Tokio runtime
    let mut rt = initialize_runtime();
    let node_keys = generate_or_load_keys();
    let node_env = fetch_node_environment();
    let db_path = get_db_path(&node_keys.identity_public_key);

    // old code from here:

    let identity_secret_key_string =
        signature_secret_key_to_string(clone_signature_secret_key(&node_keys.identity_secret_key));
    let identity_public_key_string = signature_public_key_to_string(node_keys.identity_public_key.clone());

    // Log the address, port, and public_key
    println!(
        "Starting node with address: {}, db path: {}",
        node_env.api_listen_address, db_path
    );
    println!(
        "identity sk: {} pk: {} encryption sk: {} pk: {}",
        identity_secret_key_string,
        identity_public_key_string,
        encryption_secret_key_to_string(node_keys.encryption_secret_key.clone()),
        encryption_public_key_to_string(node_keys.encryption_public_key.clone())
    );

    // CLI
    if args.create_message {
        cli_handle_create_message(args, &node_keys, &global_identity_name);
        return;
    }

    let (node_commands_sender, node_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) = bounded(100);

    // Create a new node
    let node = std::sync::Arc::new(tokio::sync::Mutex::new(
        // This is the async block where you can use `.await`
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            Node::new(
                global_identity_name.to_string(),
                node_env.listen_address,
                clone_signature_secret_key(&node_keys.identity_secret_key),
                node_keys.encryption_secret_key.clone(),
                node_env.ping_interval,
                node_commands_receiver,
                db_path,
            )
            .await
        }),
    ));

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
        // Node task
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

        // Check if the node is ready
        if !node.lock().await.is_node_ready().await {
            println!("Warning! (Expected for a new Node) The node doesn't have any profiles or devices initialized so it's waiting for that.");

            let _ = generate_qr_codes(&node_commands_sender, &node_env, &node_keys, global_identity_name.as_str(), identity_public_key_string.as_str()).await;
        }

        // API Server task
        let api_server = tokio::spawn(async move {
            node_api::run_api(node_commands_sender, node_env.api_listen_address).await;
        });

        let _ = tokio::try_join!(api_server, node_task);
    });
}
