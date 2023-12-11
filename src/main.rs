// main.rs
use crate::network::node::NodeCommand;
use crate::network::node_api;
use crate::utils::args::parse_args;
use crate::utils::cli::cli_handle_create_message;
use crate::utils::environment::{fetch_agent_env, fetch_node_environment};
use crate::utils::keys::generate_or_load_keys;
use crate::utils::qr_code_setup::generate_qr_codes;
use async_channel::{bounded, Receiver, Sender};
use ed25519_dalek::VerifyingKey;
use network::Node;
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, hash_signature_public_key, signature_public_key_to_string,
    signature_secret_key_to_string,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::{env, fs};
use tokio::runtime::Runtime;

mod agent;
mod cron_tasks;
mod crypto_identities;
mod db;
mod managers;
mod network;
mod planner;
mod resources;
mod schemas;
mod tools;
mod utils;

extern crate log;
extern crate syslog;

use log::LevelFilter;
use syslog::BasicLogger;

fn initialize_runtime() -> Runtime {
    Runtime::new().unwrap()
}

fn get_db_path(identity_public_key: &VerifyingKey) -> String {
    Path::new("db")
        .join(hash_signature_public_key(identity_public_key))
        .into_os_string()
        .into_string()
        .unwrap()
}

fn parse_secret_file() -> HashMap<String, String> {
    let contents = fs::read_to_string(Path::new("db").join(".secret")).unwrap_or_default();
    contents
        .lines()
        .map(|line| {
            let mut parts = line.splitn(2, '=');
            let key = parts.next().unwrap_or_default().to_string();
            let value = parts.next().unwrap_or_default().to_string();
            (key, value)
        })
        .collect()
}

fn main() {
    let udp_syslog_server = std::env::var("LOG_SYSLOG_SERVER").unwrap_or("".to_string());
    if !udp_syslog_server.is_empty() {
        let formatter = syslog::Formatter3164 {
            facility: syslog::Facility::LOG_USER,
            hostname: Some(std::env::var("LOG_SYSLOG_NODE_HOSTNAME").unwrap_or("localhost".to_string())),
            process: "shinkai_node".into(),
            pid: 0,
        };
        let logger = syslog::udp(formatter, "0.0.0.0:0", &udp_syslog_server).unwrap();
        log::set_boxed_logger(Box::new(BasicLogger::new(logger)))
            .map(|()| log::set_max_level(LevelFilter::Info))
            .unwrap();
    } else {
        env_logger::init();
    }

    // Placeholder for now. Maybe it should be a parameter that the user sets
    // and then it's checked with onchain data for matching with the keys provided
    let secrets = parse_secret_file();
    let global_identity_name = secrets
        .get("GLOBAL_IDENTITY_NAME")
        .cloned()
        .unwrap_or_else(|| env::var("GLOBAL_IDENTITY_NAME").unwrap_or("@@localhost.shinkai".to_string()));

    // Initialization
    let args = parse_args();

    // Create Tokio runtime
    let mut _rt = initialize_runtime();
    let node_keys = generate_or_load_keys();
    let node_env = fetch_node_environment();
    let db_path = get_db_path(&node_keys.identity_public_key);
    let initial_agents = fetch_agent_env(global_identity_name.clone());

    shinkai_log(
        ShinkaiLogOption::Node,
        ShinkaiLogLevel::Info,
        format!("Initial Agent: {:?}", initial_agents).as_str(),
    );

    let identity_secret_key_string =
        signature_secret_key_to_string(clone_signature_secret_key(&node_keys.identity_secret_key));
    let identity_public_key_string = signature_public_key_to_string(node_keys.identity_public_key.clone());

    let encryption_secret_key_string = encryption_secret_key_to_string(node_keys.encryption_secret_key.clone());
    let encryption_public_key_string = encryption_public_key_to_string(node_keys.encryption_public_key.clone());

    // Log the address, port, and public_key
    shinkai_log(
        ShinkaiLogOption::Node,
        ShinkaiLogLevel::Info,
        format!(
            "Starting node with address: {}, db path: {}",
            node_env.api_listen_address, db_path
        )
        .as_str(),
    );
    shinkai_log(
        ShinkaiLogOption::Node,
        ShinkaiLogLevel::Info,
        format!(
            "identity sk: {} pk: {} encryption sk: {} pk: {}",
            identity_secret_key_string,
            identity_public_key_string,
            encryption_secret_key_string,
            encryption_public_key_string,
        )
        .as_str(),
    );

    // CLI
    if args.create_message {
        cli_handle_create_message(args, &node_keys, &global_identity_name);
        return;
    }

    // Store to .secret file
    let identity_secret_key_string =
        signature_secret_key_to_string(clone_signature_secret_key(&node_keys.identity_secret_key));
    let encryption_secret_key_string = encryption_secret_key_to_string(node_keys.encryption_secret_key.clone());

    let secret_content = format!(
        "GLOBAL_IDENTITY_NAME={}\nIDENTITY_SECRET_KEY={}\nENCRYPTION_SECRET_KEY={}",
        global_identity_name, identity_secret_key_string, encryption_secret_key_string
    );
    if !node_env.no_secret_file {
        std::fs::write(Path::new("db").join(".secret"), secret_content).expect("Unable to write to .secret file");
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
                node_env.first_device_needs_registration_code,
                initial_agents,
                node_env.js_toolkit_executor_remote.clone(),
            )
            .await
        }),
    ));

    // Clone the Arc<Mutex<Node>> for use in each task
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
        let node_task = tokio::spawn(async move { start_node.lock().await.start().await.unwrap() });

        // Check if the node is ready
        if !node.lock().await.is_node_ready().await {
            log::warn!("Warning! (Expected for a new Node) The node doesn't have any profiles or devices initialized so it's waiting for that.");

            let _ = generate_qr_codes(&node_commands_sender, &node_env, &node_keys, global_identity_name.as_str(), identity_public_key_string.as_str()).await;
        }

        // API Server task
        let api_server = tokio::spawn(async move {
            node_api::run_api(node_commands_sender, node_env.api_listen_address).await;
        });

        let _ = tokio::try_join!(api_server, node_task);
    });
}
