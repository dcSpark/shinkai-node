use shinkai_tcp_relayer::{Args, NetworkMessageError, TCPProxy};
use tokio::net::TcpListener ;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), NetworkMessageError> {
    let args = Args::parse();
    let address = args.address;

    let listener = TcpListener::bind(&address).await.unwrap();
    println!("Server listening on {}", address);
    let proxy = TCPProxy::new().await?;

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let proxy = proxy.clone();
        tokio::spawn(async move {
            proxy.handle_client(socket).await;
        });
    }
}
