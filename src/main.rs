use crate::db::db_identity_registration::RegistrationCodeType;
// main.rs
use crate::network::node::NodeCommand;
use crate::network::node_api;
use crate::schemas::identity::IdentityPermissions;
use crate::utils::args::parse_args;
use crate::utils::cli::cli_handle_create_message;
use crate::utils::environment::fetch_node_environment;
use crate::utils::keys::generate_or_load_keys;
use crate::utils::qr_code_setup::{QRSetupData, display_qr, print_qr_data_to_console, save_qr_data_to_local_image};
use anyhow::Error;
use async_channel::{bounded, Receiver, Sender};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use log::{info, warn};
use network::Node;
use qrcode::QrCode;
use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_wasm::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, string_to_encryption_public_key, EncryptionMethod,
};
use shinkai_message_wasm::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_wasm::shinkai_utils::signatures::{
    clone_signature_secret_key, hash_signature_public_key, signature_public_key_to_string,
    signature_secret_key_to_string,
};
use std::env;
use std::fs::File;
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
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

        // Check if the node is ready
        if !node.lock().await.is_node_ready().await {
            println!("Warning! (Expected for a new Node) The node doesn't have any profiles or devices initialized so it's waiting for that.");

            // Generate the device code
            let (res1_registration_sender, res1_registraton_receiver) = async_channel::bounded(1);
            node_commands_sender
                .send(NodeCommand::CreateRegistrationCode {
                    permissions: IdentityPermissions::Admin,
                    code_type: RegistrationCodeType::Device("main".to_string()),
                    res: res1_registration_sender,
                })
                .await
                .unwrap();
            let node_registration_code = res1_registraton_receiver.recv().await.unwrap();

            let qr_data = QRSetupData {
                registration_code: node_registration_code.clone(),
                profile: "main".to_string(),
                registration_type: "device".to_string(),
                node_address: node_env.api_listen_address.to_string(), // You need to extract the IP from node_env.api_listen_address
                shinkai_identity: global_identity_name.clone(),
                node_encryption_pk: encryption_public_key_to_string(node_keys.encryption_public_key.clone()),
                node_signature_pk: identity_public_key_string.clone(),
            };

            save_qr_data_to_local_image(qr_data.clone());
            print_qr_data_to_console(qr_data.clone());
            display_qr(&qr_data); 
        }

        // API Server task
        let api_server = tokio::spawn(async move {
            node_api::run_api(node_commands_sender, node_env.api_listen_address).await;
        });

        let _ = tokio::try_join!(api_server, node_task);
    });
}
