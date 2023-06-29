use crate::shinkai_message::encryption::{decrypt_body_content, string_to_public_key};
use crate::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use crate::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use std::env;
use structopt::StructOpt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use x25519_dalek::{PublicKey, StaticSecret};

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

pub async fn start_server(
    server_secret_key: StaticSecret,
    server_public_key: PublicKey,
) -> tokio::io::Result<()> {
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
            }
        };

        // If the read returned 0, it means the connection was closed
        if n == 0 {
            println!("Connection closed by the client.");
            break;
        }

        let msg = buf[..n].to_vec();

        let shinkai_message = match ShinkaiMessageHandler::decode_message(msg) {
            Ok(msg) => msg,
            Err(e) => {
                println!("Server> Failed to decode message, error: {}", e);
                continue;
            }
        };

        let message_content_string = shinkai_message.body.unwrap().content;
        let message_content = message_content_string.as_str();
        let message_encryption = shinkai_message.encryption.as_str();

        match (message_content, message_encryption) {
            ("Ping", _) => {
                let pong = ShinkaiMessageBuilder::ping_pong_message(
                    "Pong".to_owned(),
                    server_secret_key.clone(),
                    server_public_key,
                )
                .unwrap();
                let encoded_msg = ShinkaiMessageHandler::encode_shinkai_message(pong);
                socket.write_all(&encoded_msg).await?;
            }
            ("terminate", _) => {
                println!("Termination request received, closing connection.");
                let terminate =
                    ShinkaiMessageBuilder::terminate_message(server_secret_key.clone(), server_public_key)
                        .unwrap();
                let encoded_msg = ShinkaiMessageHandler::encode_shinkai_message(terminate);
                socket.write_all(&encoded_msg).await?;
                std::mem::drop(socket);
                break;
            }
            (_, "default") => {
                let sender_pk_string = shinkai_message.external_metadata.unwrap().sender;
                let sender_pk = string_to_public_key(sender_pk_string.as_str()).unwrap();
                let decrypted_content = decrypt_body_content(
                    message_content.as_bytes(),
                    &server_secret_key.clone(),
                    &sender_pk,
                    Some(message_encryption),
                );

                match decrypted_content {
                    Some(_) => {
                        let ack =
                            ShinkaiMessageBuilder::ack_message(server_secret_key.clone(), server_public_key)
                                .unwrap();
                        let encoded_msg = ShinkaiMessageHandler::encode_shinkai_message(ack);
                        socket.write_all(&encoded_msg).await?;
                    }
                    None => {
                        println!("Server> Failed to decrypt message.");
                        continue;
                    }
                }
            }
            (_, _) => {
                let ack =
                    ShinkaiMessageBuilder::ack_message(server_secret_key.clone(), server_public_key).unwrap();
                let encoded_msg = ShinkaiMessageHandler::encode_shinkai_message(ack);
                socket.write_all(&encoded_msg).await?;
            }
        }
    }

    Ok(())
}

async fn read_from_server(stream: &mut TcpStream) -> tokio::io::Result<String> {
    let mut buffer = [0u8; 1024];
    stream.read(&mut buffer[..]).await?;
    let msg = String::from_utf8(buffer.to_vec()).unwrap();

    Ok(msg)
}
