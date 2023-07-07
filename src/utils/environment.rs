use std::env;
use std::net::{IpAddr, SocketAddr};

pub struct NodeEnvironment {
    pub global_identity_name: String,
    pub listen_address: SocketAddr,
    pub api_listen_address: SocketAddr,
    pub ping_interval: u64,
}

pub fn fetch_node_environment() -> NodeEnvironment {
    // TODO: import necessary dependencies and modules
    // You should refactor the corresponding functions to here, too.
    let global_identity_name =
        env::var("GLOBAL_IDENTITY_NAME").unwrap_or("@@node1.shinkai".to_string());

    // Fetch the environment variables for the IP and port, or use default values
    let ip: IpAddr = env::var("NODE_IP")
        .unwrap_or_else(|_| "0.0.0.0".to_string())
        .parse()
        .expect("Failed to parse IP address");
    let port: u16 = env::var("NODE_PORT")
        .unwrap_or_else(|_| "8000".to_string())
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
        .unwrap_or_else(|_| "3030".to_string())
        .parse()
        .expect("Failed to parse port number");

    // Define the address and port where your node will listen
    let listen_address = SocketAddr::new(ip, port);
    let api_listen_address = SocketAddr::new(api_ip, api_port);

    NodeEnvironment {
        global_identity_name,
        listen_address,
        api_listen_address,
        ping_interval,
    }
}
