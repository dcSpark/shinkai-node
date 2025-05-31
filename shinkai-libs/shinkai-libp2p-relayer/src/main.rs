use clap::{App, Arg};
use dotenv::dotenv;
use shinkai_message_primitives::shinkai_utils::{
    encryption::string_to_encryption_static_key, signatures::string_to_signature_secret_key,
};
use shinkai_libp2p_relayer::{LibP2PProxy, LibP2PRelayError};
use std::env;

#[tokio::main]
async fn main() -> Result<(), LibP2PRelayError> {
    dotenv().ok();

    let matches = App::new("Shinkai LibP2P Relayer")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Shinkai Team <team@shinkai.com>")
        .about("Relays LibP2P connections for Shinkai")
        .arg(
            Arg::with_name("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Sets the port to bind the server")
                .takes_value(true)
                .default_value("8080")
                .env("PORT"),
        )
        .arg(
            Arg::with_name("rpc_url")
                .long("rpc-url")
                .value_name("RPC_URL")
                .help("RPC URL for the registry")
                .takes_value(true)
                .env("RPC_URL"),
        )
        .arg(
            Arg::with_name("contract_address")
                .long("contract-address")
                .value_name("CONTRACT_ADDRESS")
                .help("Contract address for the registry")
                .takes_value(true)
                .env("CONTRACT_ADDRESS"),
        )
        .arg(
            Arg::with_name("identity_secret_key")
                .long("identity-secret-key")
                .value_name("IDENTITY_SECRET_KEY")
                .help("Identity secret key")
                .takes_value(true)
                .required(true)
                .env("IDENTITY_SECRET_KEY"),
        )
        .arg(
            Arg::with_name("encryption_secret_key")
                .long("encryption-secret-key")
                .value_name("ENCRYPTION_SECRET_KEY")
                .help("Encryption secret key")
                .takes_value(true)
                .required(true)
                .env("ENCRYPTION_SECRET_KEY"),
        )
        .arg(
            Arg::with_name("node_name")
                .long("node-name")
                .value_name("NODE_NAME")
                .help("Node name")
                .takes_value(true)
                .required(true)
                .env("NODE_NAME"),
        )
        .arg(
            Arg::with_name("max_connections")
                .long("max-connections")
                .value_name("MAX_CONNECTIONS")
                .help("Maximum number of concurrent connections")
                .takes_value(true)
                .env("MAX_CONNECTIONS"),
        )
        .get_matches();

    let port = matches
        .value_of("port")
        .unwrap()
        .parse::<u16>()
        .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid port: {}", e)))?;
    let rpc_url = matches.value_of("rpc_url").map(String::from);
    let contract_address = matches.value_of("contract_address").map(String::from);
    let identity_secret_key = matches.value_of("identity_secret_key").unwrap().to_string();
    let encryption_secret_key = matches.value_of("encryption_secret_key").unwrap().to_string();
    let node_name = matches.value_of("node_name").unwrap().to_string();
    let max_connections = matches.value_of("max_connections").map(|v| v.parse().unwrap_or(20));

    let identity_secret_key =
        string_to_signature_secret_key(&identity_secret_key)
            .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid IDENTITY_SECRET_KEY: {}", e)))?;
    let encryption_secret_key =
        string_to_encryption_static_key(&encryption_secret_key)
            .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid ENCRYPTION_SECRET_KEY: {}", e)))?;

    println!("Initializing LibP2P Relay Server on port {}", port);

    let proxy = LibP2PProxy::new(
        Some(identity_secret_key),
        Some(encryption_secret_key),
        Some(node_name),
        rpc_url,
        contract_address,
        max_connections,
        Some(port),
    )
    .await?;

    println!("LibP2P Relay Server initialized successfully");
    println!("Starting relay server...");

    // Start the relay server (this will run indefinitely)
    proxy.start().await?;

    Ok(())
} 