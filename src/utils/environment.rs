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

pub fn fetch_agent_env(global_identity: String) -> Option<SerializedAgent> {
    // Agent
    let initial_agent_name: String = env::var("INITIAL_AGENT_NAME")
        .unwrap_or_else(|_| "".to_string())
        .parse()
        .expect("Failed to parse agent name");

    let initial_agent_api_key: String = env::var("INITIAL_AGENT_API_KEY")
        .unwrap_or_else(|_| "".to_string())
        .parse()
        .expect("Failed to parse agent api key");

    let initial_agent_url: String = env::var("INITIAL_AGENT_URL")
        .unwrap_or_else(|_| "".to_string())
        .parse()
        .expect("Failed to parse agent url e.g. https://api.openai.com");

    let initial_agent_model: String = env::var("INITIAL_AGENT_MODEL")
        .unwrap_or_else(|_| "".to_string())
        .parse()
        .expect("Failed to parse agent model e.g. openai:gpt-3.5-turbo-1106");

    if initial_agent_name.is_empty()
        || initial_agent_api_key.is_empty()
        || initial_agent_url.is_empty()
        || initial_agent_model.is_empty()
    {
        return None;
    }

    let model: Result<AgentLLMInterface, _> = AgentLLMInterface::from_str(&initial_agent_model);
    let agent = SerializedAgent {
        id: initial_agent_name.clone(),
        full_identity_name: ShinkaiName::new(format!("{}/main/agent/{}", global_identity, initial_agent_name)).unwrap(),
        perform_locally: false,
        external_url: Some(initial_agent_url),
        api_key: Some(initial_agent_api_key),
        model: model.expect("Failed to parse agent model"),
        toolkit_permissions: vec![],
        storage_bucket_permissions: vec![],
        allowed_message_senders: vec![],
    };

    Some(agent)
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
