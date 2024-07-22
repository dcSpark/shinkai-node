use clap::{App, Arg};
use dotenv::dotenv;
use shinkai_message_primitives::shinkai_utils::{
    encryption::string_to_encryption_static_key, signatures::string_to_signature_secret_key,
};
use shinkai_tcp_relayer::{NetworkMessageError, TCPProxy};
use std::env;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), NetworkMessageError> {
    dotenv().ok();

    let matches = App::new("Shinkai TCP Relayer")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Nico Arqueros <nico@shinkai.com>")
        .about("Relays TCP connections for Shinkai")
        .arg(
            Arg::with_name("address")
                .short('a')
                .long("address")
                .value_name("ADDRESS")
                .help("Sets the address to bind the server")
                .takes_value(true)
                .default_value("0.0.0.0:8080")
                .env("ADDRESS"),
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
            Arg::with_name("open_to_all")
                .long("open-to-all")
                .value_name("OPEN_TO_ALL")
                .help("Open to all (true/false)")
                .takes_value(true)
                .default_value("true")
                .env("OPEN_TO_ALL"),
        )
        .get_matches();

    let address = matches.value_of("address").unwrap().to_string();
    let rpc_url = matches.value_of("rpc_url").map(String::from);
    let contract_address = matches.value_of("contract_address").map(String::from);
    let identity_secret_key = matches.value_of("identity_secret_key").unwrap().to_string();
    let encryption_secret_key = matches.value_of("encryption_secret_key").unwrap().to_string();
    let node_name = matches.value_of("node_name").unwrap().to_string();
    let open_to_all = matches.value_of("open_to_all").map(|v| v == "true").unwrap_or(true);

    let identity_secret_key =
        string_to_signature_secret_key(&identity_secret_key).expect("Invalid IDENTITY_SECRET_KEY");
    let encryption_secret_key =
        string_to_encryption_static_key(&encryption_secret_key).expect("Invalid ENCRYPTION_SECRET_KEY");

    let listener = TcpListener::bind(&address).await.unwrap();
    println!("Server listening on {}", address);

    let proxy = TCPProxy::new(
        Some(identity_secret_key),
        Some(encryption_secret_key),
        Some(node_name),
        rpc_url,
        contract_address,
    )
    .await?;

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let proxy = proxy.clone();
        tokio::spawn(async move {
            proxy.handle_client(socket).await;
        });
    }
}
