use bytes::{Buf, BytesMut};
use clap::Parser;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use std::fmt;

#[derive(Debug)]
pub enum NetworkMessageError {
    ReadError(std::io::Error),
    Utf8Error(std::string::FromUtf8Error),
    UnknownMessageType(u8),
}

impl fmt::Display for NetworkMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkMessageError::ReadError(err) => write!(f, "Failed to read exact bytes from socket: {}", err),
            NetworkMessageError::Utf8Error(err) => write!(f, "Invalid UTF-8 sequence: {}", err),
            NetworkMessageError::UnknownMessageType(t) => write!(f, "Unknown message type: {}", t),
        }
    }
}

impl std::error::Error for NetworkMessageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NetworkMessageError::ReadError(err) => Some(err),
            NetworkMessageError::Utf8Error(err) => Some(err),
            NetworkMessageError::UnknownMessageType(_) => None,
        }
    }
}

impl From<std::io::Error> for NetworkMessageError {
    fn from(err: std::io::Error) -> NetworkMessageError {
        NetworkMessageError::ReadError(err)
    }
}

impl From<std::string::FromUtf8Error> for NetworkMessageError {
    fn from(err: std::string::FromUtf8Error) -> NetworkMessageError {
        NetworkMessageError::Utf8Error(err)
    }
}
// end error code

#[derive(Serialize, Deserialize, Debug)]
enum Message {
    Identity(String),
    Data { destination: String, payload: Vec<u8> },
}

pub type Clients = Arc<Mutex<HashMap<String, Sender<Vec<u8>>>>>;

#[derive(Debug)]
pub struct NetworkMessage {
    pub identity: String,
    pub message_type: NetworkMessageType,
    pub payload: Vec<u8>,
}

impl NetworkMessage {
    pub async fn read_from_socket(socket: Arc<Mutex<TcpStream>>) -> Result<Self, NetworkMessageError> {
        eprintln!("\n\nReading message");
        let mut socket = socket.lock().await;
        let mut length_bytes = [0u8; 4];
        socket.read_exact(&mut length_bytes).await?;
        let total_length = u32::from_be_bytes(length_bytes) as usize;
        println!("Read total length: {}", total_length);

        let mut identity_length_bytes = [0u8; 4];
        socket.read_exact(&mut identity_length_bytes).await?;
        let identity_length = u32::from_be_bytes(identity_length_bytes) as usize;
        println!("Read identity length: {}", identity_length);

        let mut identity_bytes = vec![0u8; identity_length];
        socket.read_exact(&mut identity_bytes).await?;
        println!("Read identity bytes length: {}", identity_bytes.len());
        let identity = String::from_utf8(identity_bytes)?;

        let mut header_byte = [0u8; 1];
        socket.read_exact(&mut header_byte).await?;
        let message_type = match header_byte[0] {
            0x01 => NetworkMessageType::ShinkaiMessage,
            0x02 => NetworkMessageType::VRKaiPathPair,
            _ => return Err(NetworkMessageError::UnknownMessageType(header_byte[0])),
        };
        println!("Read message type: {}", header_byte[0]);

        let msg_length = total_length - 1 - 4 - identity_length;
        let mut buffer = vec![0u8; msg_length];
        println!("Calculated payload length: {}", msg_length);

        socket.read_exact(&mut buffer).await?;
        println!("Read payload length: {}", buffer.len());

        Ok(NetworkMessage {
            identity,
            message_type,
            payload: buffer,
        })
    }
}

// TODO:
// identify the client (only if they are not localhost)
// otherwise give them a random id on top of localhost (per session)
// store the client id in a dashmap

// Questions:
// What's the format of the identification?
// Generate a random hash + timestamp for the client that needs to sign and send back
// (do we care if the client is localhost? probably not so we can bypass the identification process for localhost)

// Notes:
// Messages redirected to someone should be checked if the client is still connected if not send an error message back to the sender

// TODO:
// Messages are ShinkaiMessage / Encrypted Messages
//
// Check current implementation of the TCP protocol

pub async fn handle_client(socket: TcpStream, clients: Clients) {
    eprintln!("New connection");
    let socket = Arc::new(Mutex::new(socket));

    // Read identity
    let identity_msg = match NetworkMessage::read_from_socket(socket.clone()).await {
        Ok(msg) => msg,
        Err(e) => {
            eprintln!("Failed to read identity: {}", e);
            return;
        }
    };

    let identity = identity_msg.identity;
    println!("connected: {} ", identity);

    // Old
    // let (mut reader, mut writer) = socket.split();
    // let mut buffer = BytesMut::with_capacity(1024);

    // // Read identity
    // let identity = match read_message(&mut reader, &mut buffer).await {
    //     Ok(msg) => msg,
    //     Err(e) => {
    //         eprintln!("Failed to read identity: {}", e);
    //         return;
    //     }
    // };
    // let identity_msg: Message = match serde_json::from_slice(&identity) {
    //     Ok(msg) => msg,
    //     Err(e) => {
    //         eprintln!("Failed to parse identity message: {}", e);
    //         return;
    //     }
    // };
    // let identity = if let Message::Identity(id) = identity_msg {
    //     id
    // } else {
    //     eprintln!("Expected identity message");
    //     return;
    // };
    // println!("connected: {} ", identity);

    let (tx, mut rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel(100);
    clients.lock().await.insert(identity.clone(), tx);

    loop {
        tokio::select! {
            msg = NetworkMessage::read_from_socket(socket.clone()) => {
                match msg {
                    Ok(msg) => {
                        match msg.message_type {
                            NetworkMessageType::ShinkaiMessage | NetworkMessageType::VRKaiPathPair => {
                                let destination = String::from_utf8(msg.payload.clone()).unwrap_or_default();
                                if let Some(tx) = clients.lock().await.get(&destination) {
                                    println!("sending: {} -> {}", identity, &destination);
                                    if tx.send(msg.payload).await.is_err() {
                                        eprintln!("Failed to send data to {}", destination);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read message: {}", e);
                        break;
                    }
                }
            }
            Some(data) = rx.recv() => {
                let mut socket = socket.lock().await;
                if socket.write_all(&data).await.is_err() {
                    eprintln!("Failed to write message");
                    break;
                }
            }
            else => {
                eprintln!("Connection lost for {}", identity);
                break;
            }
        }
    }

    clients.lock().await.remove(&identity);
    println!("disconnected: {}", identity);
}

pub async fn read_message(
    reader: &mut (impl AsyncReadExt + Unpin),
    buffer: &mut BytesMut,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // Read the length prefix
    while buffer.len() < 4 {
        if reader.read_buf(buffer).await? == 0 {
            return Err("Connection closed".into());
        }
    }
    let len = (&buffer[..4]).get_u32() as usize;
    buffer.advance(4);

    // Read the message
    while buffer.len() < len {
        if reader.read_buf(buffer).await? == 0 {
            return Err("Connection closed".into());
        }
    }
    let msg = buffer.split_to(len).to_vec();
    Ok(msg)
}

pub async fn write_message(
    writer: &mut (impl AsyncWriteExt + Unpin),
    msg: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let len = msg.len() as u32;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(msg).await?;
    Ok(())
}

#[derive(Parser, Debug)]
pub struct Args {
    #[clap(long, default_value = "0.0.0.0:8080")]
    pub address: String,
}
