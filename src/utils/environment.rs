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
    pub cron_devops_api_token: String,
    pub cron_devops_api_enabled: bool,
}

pub fn fetch_agent_env(global_identity: String) -> Vec<SerializedAgent> {
    let initial_agent_names: Vec<String> = env::var("INITIAL_AGENT_NAMES")
        .unwrap_or_else(|_| "".to_string())
        .split(',')
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

    let cron_devops_api_enabled: bool = env::var("CRON_DEVOPS_API_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .parse()
        .expect("Failed to parse CRON_DEVOPS_API_ENABLED");

    let cron_devops_api_token: String = env::var("CRON_DEVOPS_API_TOKEN")
        .unwrap_or_else(|_| "".to_string())
        .parse()
        .expect("Failed to parse CRON_DEVOPS_API_TOKEN");

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
        cron_devops_api_token,
        cron_devops_api_enabled,
    }
}
