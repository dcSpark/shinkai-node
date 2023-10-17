use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use csv::ReaderBuilder;
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, SerializedAgent};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use crate::network::node_proxy::{NodeProxyMode, ProxyIdentity, IsProxyConf};

#[derive(Debug, Clone)]
pub struct NodeEnvironment {
    pub global_identity_name: String,
    pub listen_address: SocketAddr,
    pub api_listen_address: SocketAddr,
    pub ping_interval: u64,
    pub starting_num_qr_profiles: u32,
    pub starting_num_qr_devices: u32,
    pub first_device_needs_registration_code: bool,
}

pub fn fetch_node_proxy_mode() -> NodeProxyMode {
    let proxy_mode: String = env::var("NODE_PROXY_MODE").unwrap_or_else(|_| "NoProxy".to_string());

    match proxy_mode.as_str() {
        "IsProxy" => {
            let allow_new_identities: bool = env::var("ALLOW_NEW_IDENTITIES")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .expect("Failed to parse allow new identities flag");

                let mut proxy_node_identities: HashMap<String, ProxyIdentity> = HashMap::new();

                // Get the path of the CSV file from the environment variable
                let csv_file_path: String = env::var("PROXY_IDENTITIES_CSV_PATH")
                    .unwrap_or_else(|_| "proxy_identities.csv".to_string());
    
                // Read the CSV file
                let file = File::open(&csv_file_path)
                    .expect(&format!("Could not open {} with content: identity, address_tcp, address_api", csv_file_path));
                let mut reader = ReaderBuilder::new().has_headers(false).from_reader(file);
    

            // Parse the identities from the CSV file
            for result in reader.records() {
                let record = result.expect("Failed to read record");
                let identity = record[0].to_string();
                let tcp_peer: SocketAddr = record[1].parse().expect("Failed to parse TCP peer address");
                let api_peer: SocketAddr = record[2].parse().expect("Failed to parse API peer address");
                let shinkai_name = ShinkaiName::new(identity.clone()).expect("Failed to parse Shinkai name");

                proxy_node_identities.insert(
                    identity,
                    ProxyIdentity {
                        api_peer,
                        tcp_peer,
                        shinkai_name,
                    },
                );
            }

            NodeProxyMode::IsProxy(IsProxyConf {
                allow_new_identities,
                proxy_node_identities,
            })
        }
        "IsProxied" => {
            let api_peer: SocketAddr = env::var("PROXY_API_PEER")
                .unwrap_or_else(|_| "0.0.0.0:9550".to_string())
                .parse()
                .expect("Failed to parse API peer address");

            let tcp_peer: SocketAddr = env::var("PROXY_TCP_PEER")
                .unwrap_or_else(|_| "0.0.0.0:9552".to_string())
                .parse()
                .expect("Failed to parse TCP peer address");

            let shinkai_name: ShinkaiName =
                ShinkaiName::new(env::var("GLOBAL_IDENTITY_NAME").unwrap()).expect("Failed to parse Shinkai name");

            NodeProxyMode::IsProxied(ProxyIdentity {
                api_peer,
                tcp_peer,
                shinkai_name,
            })
        }
        _ => NodeProxyMode::NoProxy,
    }
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
        .expect("Failed to parse agent model e.g. openai:gpt-3.5-turbo");

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

    // Define the address and port where your node will listen
    let listen_address = SocketAddr::new(ip, port);
    let api_listen_address = SocketAddr::new(api_ip, api_port);

    NodeEnvironment {
        global_identity_name,
        listen_address,
        api_listen_address,
        ping_interval,
        starting_num_qr_profiles,
        starting_num_qr_devices,
        first_device_needs_registration_code,
    }
}
