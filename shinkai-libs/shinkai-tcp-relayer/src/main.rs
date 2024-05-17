use std::sync::Arc;
use shinkai_tcp_relayer::{handle_client, Clients, Args};
use std::collections::HashMap;
use tokio::net::TcpListener ;
use tokio::sync::Mutex;
use clap::Parser;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let address = args.address;

    let listener = TcpListener::bind(&address).await.unwrap();
    println!("Server listening on {}", address);
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let clients = clients.clone();
        tokio::spawn(async move {
            handle_client(socket, clients).await;
        });
    }
}
