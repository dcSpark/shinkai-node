use super::db::ShinkaiDB;
use super::managers::identity_manager::IdentityManagerTrait;
use super::network::node::NEW_PROFILE_DEFAULT_EMBEDDING_MODEL;
use super::network::ws_manager::{WSUpdateHandler, WebSocketManager};
use super::network::ws_routes::run_ws_api;
use super::network::Node;
use super::utils::environment::{fetch_static_server_env, NodeEnvironment};
use super::utils::static_server::start_static_server;
use crate::network::node::NodeCommand;
use crate::network::node_api;
use crate::utils::args::parse_args;
use crate::utils::cli::cli_handle_create_message;
use crate::utils::environment::{fetch_llm_provider_env, fetch_node_environment};
use crate::utils::keys::generate_or_load_keys;
use crate::utils::qr_code_setup::generate_qr_codes;
use async_channel::{bounded, Receiver, Sender};
use ed25519_dalek::VerifyingKey;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{
    init_default_tracing, shinkai_log, ShinkaiLogLevel, ShinkaiLogOption,
};
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, hash_signature_public_key, signature_public_key_to_string,
    signature_secret_key_to_string,
};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::file_parser::unstructured_api::UnstructuredAPI;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Weak};
use std::{env, fs};

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct NodeRunnerError {
    pub source: Box<dyn StdError + Send + Sync>,
}

impl fmt::Display for NodeRunnerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl StdError for NodeRunnerError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.source.as_ref())
    }
}

impl From<Box<dyn StdError + Send + Sync>> for NodeRunnerError {
    fn from(err: Box<dyn StdError + Send + Sync>) -> Self {
        Self { source: err }
    }
}

pub async fn initialize_node() -> Result<
    (
        Sender<NodeCommand>,
        JoinHandle<()>,
        JoinHandle<()>,
        JoinHandle<()>,
        Weak<Mutex<Node>>,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Check if TELEMETRY_ENDPOINT is defined
    if let Ok(_telemetry_endpoint) = std::env::var("TELEMETRY_ENDPOINT") {
        // If TELEMETRY_ENDPOINT is defined, initialize telemetry tracing
        #[cfg(feature = "telemetry")]
        {
            init_telemetry_tracing(&_telemetry_endpoint);
        }
    } else {
        // If TELEMETRY_ENDPOINT is not defined, initialize default tracing
        init_default_tracing();
    }

    let main_db: &str = "main_db";
    let vector_fs_db: &str = "vector_fs_db";
    let secrets_file: &str = ".secret";

    // Fetch Env vars/args
    let args = parse_args();
    let node_env = fetch_node_environment();

    let node_storage_path = node_env.node_storage_path.clone();

    let secrets_file_path = get_secrets_file_path(secrets_file, node_storage_path.clone());
    let node_keys = generate_or_load_keys(&secrets_file_path);

    // Storage db filesystem
    let main_db_path = get_main_db_path(main_db, &node_keys.identity_public_key, node_storage_path.clone());
    let vector_fs_db_path = get_vector_fs_db_path(vector_fs_db, &node_keys.identity_public_key, node_storage_path);

    // Acquire the Node's keys. TODO: Should check with on
    // and then it's with onchain data for matching with the keys provided
    let secrets = parse_secrets_file(&secrets_file_path);
    let global_identity_name = secrets
        .get("GLOBAL_IDENTITY_NAME")
        .cloned()
        .unwrap_or_else(|| env::var("GLOBAL_IDENTITY_NAME").unwrap_or("@@localhost.arb-sep-shinkai".to_string()));

    // Initialization, creating Tokio runtime and fetching needed startup data
    let initial_llm_providers = fetch_llm_provider_env(global_identity_name.clone());
    let identity_secret_key_string =
        signature_secret_key_to_string(clone_signature_secret_key(&node_keys.identity_secret_key));
    let identity_public_key_string = signature_public_key_to_string(node_keys.identity_public_key);
    let encryption_secret_key_string = encryption_secret_key_to_string(node_keys.encryption_secret_key.clone());
    let encryption_public_key_string = encryption_public_key_to_string(node_keys.encryption_public_key);

    // Initialize Embedding Generator & Unstructured API
    let embedding_generator = init_embedding_generator(&node_env);
    let unstructured_api = init_unstructured_api(&node_env);

    // Log the address, port, and public_key
    shinkai_log(
        ShinkaiLogOption::Node,
        ShinkaiLogLevel::Info,
        format!(
            "Starting node with address: {}, main db path: {}, vector fs db path: {}",
            node_env.api_listen_address, main_db_path, vector_fs_db_path
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
        format!("Initial LLM Provider: {:?}", initial_llm_providers).as_str(),
    );

    // CLI check
    if args.create_message {
        cli_handle_create_message(args, &node_keys, &global_identity_name);
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Node not started due to CLI message creation",
        )));
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
        std::fs::create_dir_all(Path::new(&secrets_file_path.clone()).parent().unwrap()).expect("Failed to create .secret dir");
        std::fs::write(secrets_file_path.clone(), secret_content).expect("Unable to write to .secret file");
    }

    // Now that all core init data acquired, start running the node itself
    let (node_commands_sender, node_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) = bounded(100);
    let node = Node::new(
        global_identity_name.clone().to_string(),
        node_env.listen_address,
        clone_signature_secret_key(&node_keys.identity_secret_key),
        node_keys.encryption_secret_key.clone(),
        node_env.ping_interval,
        node_commands_receiver,
        main_db_path.clone(),
        secrets_file_path.clone(),
        node_env.proxy_identity.clone(),
        node_env.first_device_needs_registration_code,
        initial_llm_providers,
        node_env.js_toolkit_executor_remote.clone(),
        vector_fs_db_path.clone(),
        Some(embedding_generator),
        Some(unstructured_api),
    )
    .await;

    // Put the Node in an Arc<Mutex<Node>> for use in a task
    let start_node = Arc::clone(&node);
    let node_copy = Arc::downgrade(&start_node.clone());

    // Run the API server and node in separate tasks
    // Get identity_manager before starting the node
    let identity_manager = {
        let node = start_node.lock().await;
        node.identity_manager.clone()
    };

    let shinkai_db = {
        let node = start_node.lock().await;
        node.db.clone()
    };

    // Node task
    let node_task = tokio::spawn(async move { start_node.lock().await.start().await.unwrap() });

    // Copy of node commands center
    let node_commands_sender_copy = node_commands_sender.clone();

    // Check if the node is ready
    if !node.lock().await.is_node_ready().await {
        println!("Warning! (Expected for a new Node) The node doesn't have any profiles or devices initialized so it's waiting for that.");
        let _ = generate_qr_codes(
            &node_commands_sender,
            &node_env.clone(),
            &node_keys,
            global_identity_name.as_str(),
            identity_public_key_string.as_str(),
        )
        .await;
    } else {
        print_node_info(
            &node_env,
            &encryption_public_key_string,
            &identity_public_key_string,
            &main_db_path,
            &vector_fs_db_path,
        );
    }

    // Fetch static server environment variables
    if let Some(static_server_env) = fetch_static_server_env() {
        // Start the static server if the environment variables are set
        let _static_server = start_static_server(
            static_server_env.ip,
            static_server_env.port,
            static_server_env.folder_path,
        )
        .await;
    }

    // Setup API Server task
    let api_listen_address = node_env.clone().api_listen_address;
    let api_server = tokio::spawn(async move {
        if let Err(e) = node_api::run_api(
            node_commands_sender,
            api_listen_address,
            global_identity_name.clone().to_string(),
        )
        .await
        {
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Error,
                &format!("API server failed to start: {}", e),
            );
            panic!("API server failed to start: {}", e);
        }
    });

    let shinkai_db_copy = Arc::downgrade(&shinkai_db.clone());
    let ws_server = tokio::spawn(async move {
        init_ws_server(&node_env, identity_manager, shinkai_db_copy).await;
    });

    // Return the node_commands_sender_copy and the tasks
    Ok((node_commands_sender_copy, api_server, node_task, ws_server, node_copy))
}

pub async fn run_node_tasks(
    api_server: JoinHandle<()>,
    node_task: JoinHandle<()>,
    ws_server: JoinHandle<()>,
    _: Weak<Mutex<Node>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let api_server_abort = api_server.abort_handle();
    let node_task_abort = node_task.abort_handle();
    let ws_server_abort = ws_server.abort_handle();

    match tokio::try_join!(api_server, node_task, ws_server) {
        Ok(_) => {
            shinkai_log(ShinkaiLogOption::Node, ShinkaiLogLevel::Info, "All tasks completed");
            Ok(())
        }
        Err(e) => {
            api_server_abort.abort();
            node_task_abort.abort();
            ws_server_abort.abort();

            Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))
        }
    }
}

/// Machine filesystem path to the main ShinkaiDB database, pub key based.
fn get_main_db_path(main_db: &str, identity_public_key: &VerifyingKey, node_storage_path: Option<String>) -> String {
    if let Some(path) = node_storage_path {
        Path::new(&path)
            .join(main_db)
            .join(hash_signature_public_key(identity_public_key))
            .to_str()
            .expect("Invalid NODE_STORAGE_PATH")
            .to_string()
    } else {
        Path::new(main_db)
            .join(hash_signature_public_key(identity_public_key))
            .into_os_string()
            .into_string()
            .unwrap()
    }
}

/// Machine filesystem path to the main VectorFS database, pub key based.
fn get_vector_fs_db_path(
    vector_fs_db: &str,
    identity_public_key: &VerifyingKey,
    node_storage_path: Option<String>,
) -> String {
    if let Some(path) = node_storage_path {
        Path::new(&path)
            .join(vector_fs_db)
            .join(hash_signature_public_key(identity_public_key))
            .to_str()
            .expect("Invalid NODE_STORAGE_PATH")
            .to_string()
    } else {
        Path::new(vector_fs_db)
            .join(hash_signature_public_key(identity_public_key))
            .into_os_string()
            .into_string()
            .unwrap()
    }
}

/// Machine filesystem path for .secret.
fn get_secrets_file_path(secrets_file: &str, node_storage_path: Option<String>) -> String {
    if let Some(path) = node_storage_path {
        Path::new(&path)
            .join(secrets_file)
            .to_str()
            .expect("Invalid NODE_STORAGE_PATH")
            .to_string()
    } else {
        Path::new(secrets_file).to_str().unwrap().to_string()
    }
}

/// Parses the secrets file ( `.secret`) from the machine's filesystem
/// This file holds the user's keys.
fn parse_secrets_file(secrets_file_path: &str) -> HashMap<String, String> {
    let contents = fs::read_to_string(secrets_file_path).unwrap_or_default();
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
    let model = NEW_PROFILE_DEFAULT_EMBEDDING_MODEL.clone();
    RemoteEmbeddingGenerator::new(model, &api_url, api_key)
}

async fn init_ws_server(
    node_env: &NodeEnvironment,
    identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send + 'static>>,
    shinkai_db: Weak<ShinkaiDB>,
) {
    let new_identity_manager: Arc<Mutex<Box<dyn IdentityManagerTrait + Send + 'static>>> = {
        let identity_manager_inner = identity_manager.lock().await;
        let boxed_identity_manager = identity_manager_inner.clone_box();
        Arc::new(Mutex::new(boxed_identity_manager))
    };

    let shinkai_name = ShinkaiName::new(node_env.global_identity_name.clone()).expect("Invalid global identity name");
    // Start the WebSocket server
    let manager = WebSocketManager::new(shinkai_db.clone(), shinkai_name, new_identity_manager).await;

    // Update ShinkaiDB with manager so it can trigger updates
    {
        let db = shinkai_db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
        db.set_ws_manager(Arc::clone(&manager) as Arc<Mutex<dyn WSUpdateHandler + Send + 'static>>);
    }
    run_ws_api(node_env.ws_address, Arc::clone(&manager)).await;
}

/// Prints Useful Node information at startup
pub fn print_node_info(
    node_env: &NodeEnvironment,
    encryption_pk: &str,
    signature_pk: &str,
    main_db_path: &str,
    vector_fs_db_path: &str,
) {
    println!("---------------------------------------------------------------");
    println!("Node API address: {}", node_env.api_listen_address);
    println!("Node TCP address: {}", node_env.listen_address);
    println!("Node WS address: {}", node_env.ws_address);
    println!("Node Shinkai identity: {}", node_env.global_identity_name);
    println!("Node Main Profile: main (assumption)"); // Assuming "main" as the main profile
    println!("Node encryption pk: {}", encryption_pk);
    println!("Node signature pk: {}", signature_pk);
    println!("Main DB path: {}", main_db_path);
    println!("Vector FS DB path: {}", vector_fs_db_path);
    println!("---------------------------------------------------------------");
}
