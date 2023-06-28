use std::env;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
pub struct Opt {
    /// Number of retries
    #[structopt(short = "r", long = "retries", default_value = "5")]
    pub retries: u64,

    /// Delay between retries
    #[structopt(short = "d", long = "delay", default_value = "5")]
    pub delay: u64,
}

pub async fn start_server() -> tokio::io::Result<()> {
    let address = env::var("SERVER_ADDRESS").unwrap_or_else(|_| String::from("127.0.0.1"));
    let port = env::var("SERVER_PORT").unwrap_or_else(|_| String::from("8080"));
    let listener = TcpListener::bind(format!("{}:{}", address, port)).await?;
    let (mut socket, _) = listener.accept().await?;

    loop {
        let mut buf = vec![0; 1024];
        let n = match socket.read(&mut buf).await {
            Ok(n) => n,
            Err(e) => {
                println!("Connection closed, error: {}", e);
                return Err(e);
            },
        };

        // If the read returned 0, it means the connection was closed
        if n == 0 {
            println!("Connection closed by the client.");
            break;
        }

        let msg = String::from_utf8(buf[..n].to_vec()).unwrap();

        match msg.as_str() {
            "Hello, server!" => {
                socket.write_all(b"Hello, client!").await?;
            },
            "How are you?" => {
                socket.write_all(b"I'm good, thank you.").await?;
            },
            "Ping" => {
                socket.write_all(b"Pong").await?;
            },
            _ => {
                socket.write_all(b"Sorry, I didn't understand that.").await?;
            },
        }
    }

    Ok(())
}

pub async fn start_client(opt: Opt) -> tokio::io::Result<String> {
    for _ in 0..opt.retries {
        match TcpStream::connect("127.0.0.1:8080").await {
            Ok(mut stream) => {
                // Send a Ping message and expect a Pong response
                stream.write_all(b"Ping").await?;
                let response = read_from_server(&mut stream).await?;

                return Ok(response.trim_end().to_string());
            },
            Err(e) => {
                println!("Failed to connect, error: {}, retrying...", e);
                time::sleep(Duration::from_secs(opt.delay)).await;
            },
        }
    }

    Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Failed to connect after retries"))
}

async fn read_from_server(stream: &mut TcpStream) -> tokio::io::Result<String> {
    let mut buffer = [0u8; 1024];
    stream.read(&mut buffer[..]).await?;
    let msg = String::from_utf8(buffer.to_vec()).unwrap();

    Ok(msg)
}
