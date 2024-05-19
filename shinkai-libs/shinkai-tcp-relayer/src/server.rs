use clap::Parser;
use ed25519_dalek::Verifier;
use rand::distributions::Alphanumeric;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use shinkai_crypto_identities::ShinkaiRegistry;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use std::collections::HashMap;
use std::convert::TryInto;
use std::env;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

use crate::NetworkMessageError;

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
    fn validate_signature(
        public_key: &ed25519_dalek::VerifyingKey,
        message: &str,
        signature: &str,
    ) -> Result<bool, NetworkMessageError> {
        // Decode the hex signature to bytes
        let signature_bytes = hex::decode(signature).map_err(|_e| NetworkMessageError::InvalidData)?;

        // Convert the bytes to Signature
        let signature_bytes_slice = &signature_bytes[..];
        let signature_bytes_array: &[u8; 64] = signature_bytes_slice
            .try_into()
            .map_err(|_| NetworkMessageError::InvalidData)?;

        let signature = ed25519_dalek::Signature::from_bytes(signature_bytes_array);

        // Verify the signature against the message
        match public_key.verify(message.as_bytes(), &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

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

    if let Err(e) = validate_identity(socket.clone(), &identity).await {
        eprintln!("Identity validation failed: {}", e);
        return;
    }

    println!("Identity validated: {}", identity);

    let (tx, mut rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel(100);
    {
        let mut clients_lock = clients.lock().await;
        clients_lock.insert(identity.clone(), tx);
    }

    let clients_clone = clients.clone();
    let socket_clone = socket.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = NetworkMessage::read_from_socket(socket_clone.clone()) => {
                    match msg {
                        Ok(msg) => {
                            match msg.message_type {
                                NetworkMessageType::ShinkaiMessage | NetworkMessageType::VRKaiPathPair => {
                                    eprintln!("Received a new message: {:?}", msg);
                                    let destination = String::from_utf8(msg.payload.clone()).unwrap_or_default();
                                    eprintln!("with destination: {}", destination);
                                    if let Some(tx) = clients_clone.lock().await.get(&destination) {
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
                    eprintln!("rx.recv()");
                    let mut socket = socket_clone.lock().await;
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

        {
            let mut clients_lock = clients_clone.lock().await;
            clients_lock.remove(&identity);
        }
        println!("disconnected: {}", identity);
    });
}

async fn send_message_with_length(socket: &Arc<Mutex<TcpStream>>, message: String) -> Result<(), NetworkMessageError> {
    let message_len = message.len() as u32;
    let message_len_bytes = message_len.to_be_bytes();
    let message_bytes = message.as_bytes();

    let mut socket = socket.lock().await;
    socket.write_all(&message_len_bytes).await?;
    socket.write_all(message_bytes).await?;

    Ok(())
}

async fn validate_identity(socket: Arc<Mutex<TcpStream>>, identity: &str) -> Result<(), NetworkMessageError> {
    let identity = identity.trim_start_matches("@@");
    let validation_result = if !identity.starts_with("localhost") {
        let mut rng = StdRng::from_entropy();
        let random_string: String = (0..16).map(|_| rng.sample(Alphanumeric) as char).collect();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        let validation_data = format!("{}{}", random_string, timestamp);

        send_message_with_length(&socket, validation_data.clone()).await?;

        let mut len_buffer = [0u8; 4];
        {
            let mut socket = socket.lock().await;
            socket.read_exact(&mut len_buffer).await?;
        }

        let response_len = u32::from_be_bytes(len_buffer) as usize;
        let mut buffer = vec![0u8; response_len];
        {
            let mut socket = socket.lock().await;
            match socket.read_exact(&mut buffer).await {
                Ok(_) => eprintln!("server> Validation response read from socket"),
                Err(e) => eprintln!("server> Failed to read validation response: {}", e),
            }
        }

        let response = String::from_utf8(buffer).map_err(NetworkMessageError::Utf8Error)?;

        // Fetch the public key from ShinkaiRegistry
        let rpc_url = env::var("RPC_URL").unwrap_or("https://ethereum-sepolia-rpc.publicnode.com".to_string());
        let contract_address =
            env::var("CONTRACT_ADDRESS").unwrap_or("0xDCbBd3364a98E2078e8238508255dD4a2015DD3E".to_string());

        let mut registry = ShinkaiRegistry::new(&rpc_url, &contract_address, None).await.unwrap();
        let identity = identity.trim_start_matches("@@");
        let onchain_identity = registry.get_identity_record(identity.to_string()).await;
        let public_key = onchain_identity.unwrap().signature_verifying_key().unwrap();

        // Validate the signature
        if !NetworkMessage::validate_signature(&public_key, &validation_data, &response)? {
            Err(NetworkMessageError::InvalidData)
        } else {
            Ok(())
        }
    } else {
        Ok(())
    };

    // Send validation result back to the client
    let validation_message = match &validation_result {
        Ok(_) => "Validation successful".to_string(),
        Err(e) => format!("Validation failed: {}", e),
    };

    send_message_with_length(&socket, validation_message).await?;

    validation_result
}

#[derive(Parser, Debug)]
pub struct Args {
    #[clap(long, default_value = "0.0.0.0:8080")]
    pub address: String,
}
