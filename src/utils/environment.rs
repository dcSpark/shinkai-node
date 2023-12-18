use std::env;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, SerializedAgent};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

#[derive(Debug, Clone)]
pub struct NodeEnvironment {
    pub global_identity_name: String,
    pub listen_address: SocketAddr,
    pub api_listen_address: SocketAddr,
    pub ws_address: SocketAddr,
    pub ping_interval: u64,
    pub starting_num_qr_profiles: u32,
    pub starting_num_qr_devices: u32,
    pub first_device_needs_registration_code: bool,
    pub js_toolkit_executor_remote: Option<String>,
    pub no_secret_file: bool,
    pub secret_file_path: Option<String>,
    pub main_db_path: Option<String>,
    pub vector_fs_db_path: Option<String>,
}

pub fn fetch_agent_env(global_identity: String) -> Vec<SerializedAgent> {
    let initial_agent_names: Vec<String> = env::var("INITIAL_AGENT_NAMES")
        .unwrap_or_else(|_| "".to_string())
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let initial_agent_api_keys: Vec<String> = env::var("INITIAL_AGENT_API_KEYS")
        .unwrap_or_else(|_| "".to_string())
        .split(',')
        .map(|s| s.to_string())
        .collect();

    let initial_agent_urls: Vec<String> = env::var("INITIAL_AGENT_URLS")
        .unwrap_or_else(|_| "".to_string())
        .split(',')
        .map(|s| s.to_string())
        .collect();

    let initial_agent_models: Vec<String> = env::var("INITIAL_AGENT_MODELS")
        .unwrap_or_else(|_| "".to_string())
        .split(',')
        .map(|s| s.to_string())
        .collect();

    let mut agents = Vec::new();

    for i in 0..initial_agent_names.len() {
        let model: Result<AgentLLMInterface, _> = AgentLLMInterface::from_str(&initial_agent_models[i]);

        let agent = SerializedAgent {
            id: initial_agent_names[i].clone(),
            full_identity_name: ShinkaiName::new(format!("{}/main/agent/{}", global_identity, initial_agent_names[i]))
                .unwrap(),
            perform_locally: false,
            external_url: Some(initial_agent_urls[i].clone()),
            api_key: Some(initial_agent_api_keys[i].clone()),
            model: model.expect("Failed to parse agent model"),
            toolkit_permissions: vec![],
            storage_bucket_permissions: vec![],
            allowed_message_senders: vec![],
        };

        agents.push(agent);
    }

    agents
}

pub fn fetch_node_environment() -> NodeEnvironment {
    let global_identity_name = env::var("GLOBAL_IDENTITY_NAME").unwrap_or("@@node1.shinkai".to_string());

    // Fetch the environment variables for the IP and port, or use default values
    let ip: IpAddr = env::var("NODE_IP")
        .unwrap_or_else(|_| "0.0.0.0".to_string())
        .parse()
        .expect("Failed to parse IP address");
    let port: u16 = env::var("NODE_PORT")
        .unwrap_or_else(|_| "9552".to_string())
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
        .unwrap_or_else(|_| "9550".to_string())
        .parse()
        .expect("Failed to parse port number");

    let ws_port: u16 = env::var("NODE_WS_PORT")
        .unwrap_or_else(|_| "9551".to_string())
        .parse()
        .expect("Failed to parse ws port number");

    // TODO: remove this and just assume one device per profile
    let starting_num_qr_profiles: u32 = env::var("STARTING_NUM_QR_PROFILES")
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .expect("Failed to parse starting number of QR profiles");

    let starting_num_qr_devices: u32 = env::var("STARTING_NUM_QR_DEVICES")
        .unwrap_or_else(|_| "1".to_string())
        .parse()
        .expect("Failed to parse starting number of QR devices");

    let first_device_needs_registration_code: bool = env::var("FIRST_DEVICE_NEEDS_REGISTRATION_CODE")
        .unwrap_or_else(|_| "true".to_string())
        .parse()
        .expect("Failed to parse needs registration code");

    let js_toolkit_executor_remote: Option<String> = env::var("JS_TOOLKIT_ADDRESS").ok().filter(|s| !s.is_empty());

    // Secret file env vars
    let no_secret_file: bool = env::var("NO_SECRET_FILE")
        .unwrap_or_else(|_| "false".to_string())
        .parse()
        .expect("Failed to parse NO_SECRET_FILE");
    let secret_file_path: Option<String> = env::var("NODE_SECRET_FILE_PATH").ok();

    // DB Path Env Vars
    let main_db_path: Option<String> = env::var("NODE_MAIN_DB_PATH").ok();
    let vector_fs_db_path: Option<String> = env::var("NODE_VEC_FS_DB_PATH").ok();

    // Define the address and port where your node will listen
    let listen_address = SocketAddr::new(ip, port);
    let api_listen_address = SocketAddr::new(api_ip, api_port);

    NodeEnvironment {
        global_identity_name,
        listen_address,
        api_listen_address,
        ws_address: SocketAddr::new(ip, ws_port),
        ping_interval,
        starting_num_qr_profiles,
        starting_num_qr_devices,
        first_device_needs_registration_code,
        js_toolkit_executor_remote,
        no_secret_file,
        main_db_path,
        vector_fs_db_path,
        secret_file_path,
    }
}
