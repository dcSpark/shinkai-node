use std::env;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, SerializedLLMProvider,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

#[derive(Debug, Clone)]
pub struct NodeEnvironment {
    pub global_identity_name: String,
    pub listen_address: SocketAddr,
    pub api_listen_address: SocketAddr,
    pub ws_address: Option<SocketAddr>,
    pub ping_interval: u64,
    pub starting_num_qr_profiles: u32,
    pub starting_num_qr_devices: u32,
    pub first_device_needs_registration_code: bool,
    pub js_toolkit_executor_remote: Option<String>,
    pub no_secrets_file: bool,
    pub node_storage_path: Option<String>,
    pub unstructured_server_url: Option<String>,
    pub unstructured_server_api_key: Option<String>,
    pub embeddings_server_url: Option<String>,
    pub embeddings_server_api_key: Option<String>,
    pub auto_detect_local_llms: bool,
    pub proxy_identity: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StaticServerEnvironment {
    pub ip: IpAddr,
    pub port: u16,
    pub folder_path: String,
}

pub fn fetch_llm_provider_env(global_identity: String) -> Vec<SerializedLLMProvider> {
    let initial_agent_names: Vec<String> = env::var("INITIAL_AGENT_NAMES")
        .or_else(|_| env::var("INITIAL_LLM_PROVIDER_NAMES"))
        .unwrap_or_else(|_| "".to_string())
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let initial_agent_api_keys: Vec<String> = env::var("INITIAL_AGENT_API_KEYS")
        .or_else(|_| env::var("INITIAL_LLM_PROVIDER_API_KEYS"))
        .unwrap_or_else(|_| "".to_string())
        .split(',')
        .map(|s| s.to_string())
        .collect();

    let initial_agent_urls: Vec<String> = env::var("INITIAL_AGENT_URLS")
        .or_else(|_| env::var("INITIAL_LLM_PROVIDER_URLS"))
        .unwrap_or_else(|_| "".to_string())
        .split(',')
        .map(|s| s.to_string())
        .collect();

    let initial_agent_models: Vec<String> = env::var("INITIAL_AGENT_MODELS")
        .or_else(|_| env::var("INITIAL_LLM_PROVIDER_MODELS"))
        .unwrap_or_else(|_| "".to_string())
        .split(',')
        .map(|s| s.to_string())
        .collect();

    let mut llm_providers = Vec::new();

    for i in 0..initial_agent_names.len() {
        let model: Result<LLMProviderInterface, _> = LLMProviderInterface::from_str(&initial_agent_models[i]);

        let agent = SerializedLLMProvider {
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

        llm_providers.push(agent);
    }

    llm_providers
}

pub fn fetch_node_environment() -> NodeEnvironment {
    let global_identity_name = env::var("GLOBAL_IDENTITY_NAME").unwrap_or("@@localhost.arb-sep-shinkai".to_string());

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

    let ws_port: Option<u16> = env::var("NODE_WS_PORT").ok().and_then(|p| p.parse().ok());

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

    let no_secrets_file: bool = env::var("NO_SECRET_FILE")
        .unwrap_or_else(|_| "false".to_string())
        .parse()
        .expect("Failed to parse NO_SECRET_FILE");

    // Define the address and port where your node will listen
    let listen_address = SocketAddr::new(ip, port);
    let api_listen_address = SocketAddr::new(api_ip, api_port);

    // DB Path Env Vars
    let node_storage_path: Option<String> = match env::var("NODE_STORAGE_PATH").ok() {
        Some(val) => Some(val),
        None => Some("storage".to_string()),
    };

    // Inside the fetch_node_environment function, add the following line to initialize auto_detect_local_llms
    let auto_detect_local_llms: bool = env::var("AUTO_DETECT_LOCAL_LLMS")
        .unwrap_or_else(|_| "true".to_string())
        .parse()
        .expect("Failed to parse AUTO_DETECT_LOCAL_LLMS");

    // External server env vars
    let unstructured_server_url: Option<String> = env::var("UNSTRUCTURED_SERVER_URL").ok();
    let unstructured_server_api_key: Option<String> = env::var("UNSTRUCTURED_SERVER_API_KEY").ok();
    let embeddings_server_url: Option<String> = env::var("EMBEDDINGS_SERVER_URL").ok();
    let embeddings_server_api_key: Option<String> = env::var("EMBEDDINGS_SERVER_API_KEY").ok();

    // Fetch the PROXY_IDENTITY environment variable
    let proxy_identity: Option<String> = env::var("PROXY_IDENTITY").ok().and_then(|addr| addr.parse().ok());

    // WebSocket address
    let ws_address = ws_port.map(|port| SocketAddr::new(ip, port));

    // Check if NODE_API_IP:NODE_API_PORT is the same as NODE_IP:NODE_PORT
    if ip == api_ip && port == api_port {
        panic!("NODE_API_IP:NODE_API_PORT cannot be the same as NODE_IP:NODE_PORT");
    }

    NodeEnvironment {
        global_identity_name,
        listen_address,
        api_listen_address,
        ws_address,
        ping_interval,
        starting_num_qr_profiles,
        starting_num_qr_devices,
        first_device_needs_registration_code,
        js_toolkit_executor_remote,
        no_secrets_file,
        node_storage_path,
        unstructured_server_url,
        unstructured_server_api_key,
        embeddings_server_url,
        embeddings_server_api_key,
        auto_detect_local_llms,
        proxy_identity,
    }
}

pub fn fetch_static_server_env() -> Option<StaticServerEnvironment> {
    let port = env::var("STATIC_SERVER_PORT").ok().and_then(|p| p.parse::<u16>().ok());
    let ip = env::var("STATIC_SERVER_IP")
        .ok()
        .and_then(|ip| ip.parse::<IpAddr>().ok());
    let folder_path = env::var("STATIC_SERVER_FOLDER").ok();

    eprintln!("Static server env: {:?} {:?} {:?}", ip, port, folder_path);

    match (ip, port, folder_path) {
        (Some(ip), Some(port), Some(folder_path)) => Some(StaticServerEnvironment { ip, port, folder_path }),
        _ => None,
    }
}
