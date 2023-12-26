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
use network::node::DEFAULT_EMBEDDING_MODEL;
use network::Node;
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, hash_signature_public_key, signature_public_key_to_string,
    signature_secret_key_to_string,
};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::model_type::EmbeddingModelType;
use shinkai_vector_resources::model_type::TextEmbeddingsInference;
use shinkai_vector_resources::unstructured::unstructured_api::UnstructuredAPI;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::{env, fs};
use tokio::runtime::Runtime;
use utils::environment::NodeEnvironment;

mod agent;
mod cron_tasks;
mod crypto_identities;
mod db;
mod managers;
mod network;
mod payments;
mod planner;
mod resources;
mod schemas;
mod tools;
mod utils;
mod vector_fs;

fn main() {
    env_logger::init();

    // Fetch Env vars/args
    let args = parse_args();
    let node_env = fetch_node_environment();

    // Acquire the Node's keys. TODO: Should check with on
    // and then it's with onchain data for matching with the keys provided
    let secrets = parse_secrets_file(&node_env);
    let global_identity_name = secrets
        .get("GLOBAL_IDENTITY_NAME")
        .cloned()
        .unwrap_or_else(|| env::var("GLOBAL_IDENTITY_NAME").unwrap_or("@@localhost.shinkai".to_string()));

    // Initialization, creating Tokio runtime and fetching needed startup data
    let mut _rt = initialize_runtime();
    let node_keys = generate_or_load_keys();
    let db_path = get_db_path(&node_keys.identity_public_key, &node_env);
    let vector_fs_db_path = get_vector_fs_db_path(&node_keys.identity_public_key, &node_env);
    let initial_agents = fetch_agent_env(global_identity_name.clone());
    let identity_secret_key_string =
        signature_secret_key_to_string(clone_signature_secret_key(&node_keys.identity_secret_key));
    let identity_public_key_string = signature_public_key_to_string(node_keys.identity_public_key.clone());
    let encryption_secret_key_string = encryption_secret_key_to_string(node_keys.encryption_secret_key.clone());
    let encryption_public_key_string = encryption_public_key_to_string(node_keys.encryption_public_key.clone());

    // Initialize Embedding Generator & Unstructured API
    let embedding_generator = init_embedding_generator(&node_env);
    let unstructured_api = init_unstructured_api(&node_env);

    // Log the address, port, and public_key
    shinkai_log(
        ShinkaiLogOption::Node,
        ShinkaiLogLevel::Info,
        format!(
            "Starting node with address: {}, db path: {}, vector fs db path: {}",
            node_env.api_listen_address, db_path, vector_fs_db_path
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
    shinkai_log(
        ShinkaiLogOption::Node,
        ShinkaiLogLevel::Info,
        format!("Initial Agent: {:?}", initial_agents).as_str(),
    );

    // CLI check
    if args.create_message {
        cli_handle_create_message(args, &node_keys, &global_identity_name);
        return;
    }

    // Store secrets into machine filesystem `db.secret` file (needed if new secrets were generated)
    let identity_secret_key_string =
        signature_secret_key_to_string(clone_signature_secret_key(&node_keys.identity_secret_key));
    let encryption_secret_key_string = encryption_secret_key_to_string(node_keys.encryption_secret_key.clone());
    let secret_content = format!(
        "GLOBAL_IDENTITY_NAME={}\nIDENTITY_SECRET_KEY={}\nENCRYPTION_SECRET_KEY={}",
        global_identity_name, identity_secret_key_string, encryption_secret_key_string
    );
    if !node_env.no_secrets_file {
        std::fs::write(Path::new("db").join(".secret"), secret_content).expect("Unable to write to .secret file");
    }

    // Now that all core init data acquired, start running the node itself
    let (node_commands_sender, node_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) = bounded(100);
    let node = std::sync::Arc::new(tokio::sync::Mutex::new(
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            Node::new(
                global_identity_name.clone().to_string(),
                node_env.listen_address,
                clone_signature_secret_key(&node_keys.identity_secret_key),
                node_keys.encryption_secret_key.clone(),
                node_env.ping_interval,
                node_commands_receiver,
                db_path,
                node_env.first_device_needs_registration_code,
                initial_agents,
                node_env.js_toolkit_executor_remote.clone(),
                vector_fs_db_path,
                Some(embedding_generator),
                Some(unstructured_api),
            )
            .await
        }),
    ));
    // Put the Node in an Arc<Mutex<Node>> for use in a task
    let start_node = Arc::clone(&node);
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
            println!("Warning! (Expected for a new Node) The node doesn't have any profiles or devices initialized so it's waiting for that.");
            let _ = generate_qr_codes(&node_commands_sender, &node_env, &node_keys, global_identity_name.as_str(), identity_public_key_string.as_str()).await;
        }

        // Setup API Server task
        let api_server = tokio::spawn(async move {
            node_api::run_api(node_commands_sender, node_env.api_listen_address, global_identity_name.clone().to_string()).await;
        });
        let _ = tokio::try_join!(api_server, node_task);
    });
}

/// Initialzied Tokio runtime
fn initialize_runtime() -> Runtime {
    Runtime::new().unwrap()
}

/// Machine filesystem path to the main ShinkaiDB database. Uses env var first, else pub key based.
fn get_db_path(identity_public_key: &VerifyingKey, node_env: &NodeEnvironment) -> String {
    if let Some(path) = node_env.main_db_path.clone() {
        Path::new(&path)
            .to_str()
            .expect("Invalid NODE_MAIN_DB_PATH")
            .to_string()
    } else {
        Path::new("db")
            .join(hash_signature_public_key(identity_public_key))
            .into_os_string()
            .into_string()
            .unwrap()
    }
}

/// Machine filesystem path to the main VectorFS database. Uses env var first, else pub key based.
fn get_vector_fs_db_path(identity_public_key: &VerifyingKey, node_env: &NodeEnvironment) -> String {
    if let Some(path) = node_env.vector_fs_db_path.clone() {
        Path::new(&path)
            .to_str()
            .expect("Invalid NODE_VEC_FS_DB_PATH")
            .to_string()
    } else {
        Path::new("vector_fs_db")
            .join(hash_signature_public_key(identity_public_key))
            .into_os_string()
            .into_string()
            .unwrap()
    }
}

/// Parses the secrets file ( `db.secret`) from the machine's filesystem
/// This file holds the user's keys.
fn parse_secrets_file(node_env: &NodeEnvironment) -> HashMap<String, String> {
    let path = if let Some(path) = node_env.secrets_file_path.clone() {
        Path::new(&path)
            .to_str()
            .expect("Invalid NODE_SECRET_FILE_PATH")
            .to_string()
    } else {
        Path::new("db")
            .join(".secret")
            .to_str()
            .expect("Invalid NODE_SECRET_FILE_PATH")
            .to_string()
    };

    let contents = fs::read_to_string(path).unwrap_or_default();
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

/// Initializes UnstructuredAPI struct using node environment
fn init_unstructured_api(node_env: &NodeEnvironment) -> UnstructuredAPI {
    let api_url = node_env
        .unstructured_server_url
        .clone()
        .expect("UNSTRUCTURED_SERVER_URL not found in node_env");
    let api_key = node_env.unstructured_server_api_key.clone();
    UnstructuredAPI::new(api_url, api_key)
}

/// Initializes RemoteEmbeddingGenerator struct using node environment/default embedding model for now
fn init_embedding_generator(node_env: &NodeEnvironment) -> RemoteEmbeddingGenerator {
    let api_url = node_env
        .embeddings_server_url
        .clone()
        .expect("EMBEDDINGS_SERVER_URL not found in node_env");
    let api_key = node_env.embeddings_server_api_key.clone();
    // TODO: Replace this hard-coded model to having the default being saved/read from the DB
    let model = DEFAULT_EMBEDDING_MODEL.clone();
    RemoteEmbeddingGenerator::new(model, &api_url, api_key)
}
