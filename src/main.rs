// main.rs
use crate::network::node::NodeCommand;
use crate::network::node_api;
use crate::shinkai_message::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, hash_encryption_public_key, string_to_encryption_public_key, unsafe_deterministic_encryption_keypair, EncryptionMethod
};
use crate::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use crate::shinkai_message::shinkai_message_extension::ShinkaiMessageWrapper;
use crate::shinkai_message::signatures::{
    clone_signature_secret_key, ephemeral_signature_keypair, hash_signature_public_key, string_to_signature_secret_key, unsafe_deterministic_signature_keypair
};
use crate::shinkai_message::signatures::{signature_public_key_to_string, signature_secret_key_to_string};
use crate::utils::args::parse_args;
use crate::utils::environment::fetch_node_environment;
use crate::utils::keys::generate_or_load_keys;
use anyhow::Error;
use async_channel::{bounded, Receiver, Sender};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use log::{info, warn};
use network::Node;
use shinkai_message::encryption::ephemeral_encryption_keys;
use shinkai_node::resources::local_ai::LocalAIProcess;
use shinkai_node::shinkai_message::encryption::string_to_encryption_static_key;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

mod db;
mod managers;
mod network;
mod shinkai_message;
mod utils;
mod managers;
mod schemas;

mod shinkai_message_proto {
    include!(concat!(env!("OUT_DIR"), "/shinkai_message_proto.rs"));
}

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

    if args.create_message {
        let node2_encryption_pk_str = args
            .receiver_encryption_pk
            .expect("receiver_encryption_pk argument is required for create_message");
        let recipient = args
            .recipient
            .expect("recipient argument is required for create_message");
        let sender_subidentity = args
            .sender_subidentity
            .unwrap_or("".to_string());
        let receiver_subidentity = args
            .receiver_subidentity
            .unwrap_or("".to_string());
        let inbox = args
            .inbox
            .unwrap_or("".to_string());
        let body_content = args
            .body_content
            .unwrap_or("body content".to_string());
        let other = args.other.unwrap_or("".to_string());
        let node2_encryption_pk = string_to_encryption_public_key(node2_encryption_pk_str.as_str()).unwrap();

        println!("Creating message for recipient: {}", recipient);
        println!("identity_secret_key: {}", identity_secret_key_string);
        println!("receiver_encryption_pk: {}", node2_encryption_pk_str);

        if let Some(code) = args.code_registration {
            // Call the `code_registration` function
            let message = ShinkaiMessageBuilder::code_registration(
                node_keys.encryption_secret_key,
                node_keys.identity_secret_key,
                node2_encryption_pk,
                code.to_string(),
                "device".to_string(),
                global_identity_name.to_string().clone(),
                recipient.to_string(),
            )
            .expect("Failed to create message with code registration");

            println!(
                "Message's signature: {}",
                message.clone().external_metadata.unwrap().signature
            );

            // Parse the message to JSON and print to stdout
            let message_wrapper = ShinkaiMessageWrapper::from(&message);

            // Serialize the wrapper into JSON and print to stdout
            let message_json = serde_json::to_string_pretty(&message_wrapper);

            match message_json {
                Ok(json) => println!("{}", json),
                Err(e) => println!("Error creating JSON: {}", e),
            }
            return;
        } else if args.create_message {
            // Use your key generation and ShinkaiMessageBuilder code here
            let message = ShinkaiMessageBuilder::new(
                node_keys.encryption_secret_key,
                node_keys.identity_secret_key,
                node2_encryption_pk,
            )
            .body(body_content.to_string())
            .body_encryption(EncryptionMethod::None)
            .message_schema_type("schema type".to_string())
            .internal_metadata(
                sender_subidentity.to_string(),
                receiver_subidentity.to_string(),
                inbox.to_string(),
                EncryptionMethod::None,
            )
            .external_metadata(
                recipient.to_string(),
                global_identity_name.to_string().clone(),
            )
            .build();

            println!(
                "Message's signature: {}",
                message.clone().unwrap().external_metadata.unwrap().signature
            );

            // Parse the message to JSON and print to stdout
            let message_wrapper = ShinkaiMessageWrapper::from(&message.unwrap());

            // Serialize the wrapper into JSON and print to stdout
            let message_json = serde_json::to_string_pretty(&message_wrapper);

            match message_json {
                Ok(json) => println!("{}", json),
                Err(e) => println!("Error creating JSON: {}", e),
            }
            return;
        }
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
        // API Server task
        let api_server = tokio::spawn(async move {
            node_api::run_api(node_commands_sender, node_env.api_listen_address).await;
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
