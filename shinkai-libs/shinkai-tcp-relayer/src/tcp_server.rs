use clap::Parser;
use derivative::Derivative;
use ed25519_dalek::{SigningKey, Verifier, VerifyingKey};
use rand::distributions::Alphanumeric;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use shinkai_crypto_identities::ShinkaiRegistry;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use shinkai_message_primitives::shinkai_message::shinkai_message::{MessageBody, ShinkaiMessage};
use shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_static_key;
use shinkai_message_primitives::shinkai_utils::signatures::signature_public_key_to_string;
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
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::NetworkMessageError;

#[derive(Serialize, Deserialize, Debug)]
enum Message {
    Identity(String),
    Data { destination: String, payload: Vec<u8> },
}

pub type TCPProxyClients = Arc<Mutex<HashMap<String, Sender<Vec<u8>>>>>;

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
        eprintln!("\n\nread_from_socket> Reading message");
        let mut socket = socket.lock().await;
        let mut length_bytes = [0u8; 4];
        socket.read_exact(&mut length_bytes).await?;
        let total_length = u32::from_be_bytes(length_bytes) as usize;
        println!("read_from_socket> Read total length: {}", total_length);

        let mut identity_length_bytes = [0u8; 4];
        socket.read_exact(&mut identity_length_bytes).await?;
        let identity_length = u32::from_be_bytes(identity_length_bytes) as usize;
        println!("read_from_socket> Read identity length: {}", identity_length);

        let mut identity_bytes = vec![0u8; identity_length];
        socket.read_exact(&mut identity_bytes).await?;
        println!("read_from_socket> Read identity bytes length: {}", identity_bytes.len());
        let identity = String::from_utf8(identity_bytes)?;
        eprintln!("read_from_socket> Read identity: {}", identity);

        let mut header_byte = [0u8; 1];
        socket.read_exact(&mut header_byte).await?;
        let message_type = match header_byte[0] {
            0x01 => NetworkMessageType::ShinkaiMessage,
            0x02 => NetworkMessageType::VRKaiPathPair,
            _ => return Err(NetworkMessageError::UnknownMessageType(header_byte[0])),
        };
        println!("read_from_socket> Read message type: {}", header_byte[0]);

        let msg_length = total_length - 1 - 4 - identity_length;
        let mut buffer = vec![0u8; msg_length];
        println!("read_from_socket> Calculated payload length: {}", msg_length);

        socket.read_exact(&mut buffer).await?;
        println!("read_from_socket> Read payload length: {}", buffer.len());

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

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct TCPProxy {
    pub clients: TCPProxyClients,
    pub registry: ShinkaiRegistry,
    pub node_name: ShinkaiName,
    #[derivative(Debug = "ignore")]
    pub identity_secret_key: SigningKey,
    pub identity_public_key: VerifyingKey,
    #[derivative(Debug = "ignore")]
    pub encryption_secret_key: EncryptionStaticKey,
    pub encryption_public_key: EncryptionPublicKey,
}

impl TCPProxy {
    pub async fn new(
        identity_secret_key: Option<SigningKey>,
        encryption_secret_key: Option<EncryptionStaticKey>,
        node_name: Option<String>,
    ) -> Result<Self, NetworkMessageError> {
        let rpc_url = env::var("RPC_URL").unwrap_or("https://ethereum-sepolia-rpc.publicnode.com".to_string());
        let contract_address =
            env::var("CONTRACT_ADDRESS").unwrap_or("0xDCbBd3364a98E2078e8238508255dD4a2015DD3E".to_string());

        let registry = ShinkaiRegistry::new(&rpc_url, &contract_address, None).await.unwrap();

        let identity_secret_key = identity_secret_key
            .or_else(|| {
                let key = env::var("IDENTITY_SECRET_KEY").expect("IDENTITY_SECRET_KEY not found in ENV");
                let key_bytes: [u8; 32] = hex::decode(key)
                    .expect("Invalid IDENTITY_SECRET_KEY")
                    .try_into()
                    .expect("Invalid length for IDENTITY_SECRET_KEY");
                Some(SigningKey::from_bytes(&key_bytes))
            })
            .unwrap();

        let encryption_secret_key = encryption_secret_key
            .or_else(|| {
                let key = env::var("ENCRYPTION_SECRET_KEY").expect("ENCRYPTION_SECRET_KEY not found in ENV");
                Some(string_to_encryption_static_key(&key).expect("Invalid ENCRYPTION_SECRET_KEY"))
            })
            .unwrap();

        let node_name = node_name
            .or_else(|| Some(env::var("NODE_NAME").expect("NODE_NAME not found in ENV")))
            .unwrap();

        let identity_public_key = identity_secret_key.verifying_key();
        let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);
        let node_name = ShinkaiName::new(node_name).unwrap();

        // Fetch the public keys from the registry
        let registry_identity = registry.get_identity_record(node_name.to_string()).await.unwrap();
        eprintln!("Registry Identity: {:?}", registry_identity);
        let registry_identity_public_key = registry_identity.signature_verifying_key().unwrap();
        let registry_encryption_public_key = registry_identity.encryption_public_key().unwrap();

        // Check if the provided keys match the ones from the registry
        if identity_public_key != registry_identity_public_key {
            eprintln!(
                "Identity Public Key ENV: {:?}",
                signature_public_key_to_string(identity_public_key)
            );
            eprintln!(
                "Identity Public Key Registry: {:?}",
                signature_public_key_to_string(registry_identity_public_key)
            );
            return Err(NetworkMessageError::InvalidData);
        }

        if encryption_public_key != registry_encryption_public_key {
            return Err(NetworkMessageError::InvalidData);
        }

        Ok(TCPProxy {
            clients: Arc::new(Mutex::new(HashMap::new())),
            registry,
            node_name,
            identity_secret_key,
            identity_public_key,
            encryption_secret_key,
            encryption_public_key,
        })
    }

    pub async fn handle_client(&self, socket: TcpStream) {
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

        if let Err(e) = self.validate_identity(socket.clone(), &identity).await {
            eprintln!("Identity validation failed: {}", e);
            return;
        }

        println!("Identity validated: {}", identity);

        let (tx, mut rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = mpsc::channel(100);
        {
            let mut clients_lock = self.clients.lock().await;
            clients_lock.insert(identity.clone(), tx);
        }

        let clients_clone = self.clients.clone();
        let socket_clone = socket.clone();
        let registry_clone = self.registry.clone();
        let node_name = self.node_name.clone();
        let identity_sk = self.identity_secret_key.clone();
        let encryption_sk = self.encryption_secret_key.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = NetworkMessage::read_from_socket(socket_clone.clone()) => {
                        if let Err(e) = Self::handle_incoming_message(msg, &clients_clone, &socket_clone, &registry_clone, &identity, node_name.clone(), identity_sk.clone(), encryption_sk.clone()).await {
                            eprintln!("Error handling incoming message: {}", e);
                            break;
                        }
                    }
                    Some(data) = rx.recv() => {
                        if let Err(e) = Self::handle_outgoing_message(data, &socket_clone).await {
                            eprintln!("Error handling outgoing message: {}", e);
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

    #[allow(clippy::too_many_arguments)]
    async fn handle_incoming_message(
        msg: Result<NetworkMessage, NetworkMessageError>,
        clients: &TCPProxyClients,
        socket: &Arc<Mutex<TcpStream>>,
        registry: &ShinkaiRegistry,
        identity: &str,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
    ) -> Result<(), NetworkMessageError> {
        match msg {
            Ok(msg) => match msg.message_type {
                NetworkMessageType::ShinkaiMessage => {
                    eprintln!("Received a ShinkaiMessage ...");
                    let shinkai_message: Result<ShinkaiMessage, _> = serde_json::from_slice(&msg.payload);
                    match shinkai_message {
                        Ok(parsed_message) => {
                            Self::handle_shinkai_message(
                                parsed_message,
                                clients,
                                socket,
                                registry,
                                identity,
                                node_name,
                                identity_secret_key,
                                encryption_secret_key,
                            )
                            .await
                        }
                        Err(e) => {
                            eprintln!("Failed to parse ShinkaiMessage: {}", e);
                            let error_message = format!("Failed to parse ShinkaiMessage: {}", e);
                            send_message_with_length(socket, error_message).await
                        }
                    }
                }
                NetworkMessageType::VRKaiPathPair => {
                    eprintln!("Received a VRKaiPathPair message: {:?}", msg);
                    let destination = String::from_utf8(msg.payload.clone()).unwrap_or_default();
                    eprintln!("with destination: {}", destination);
                    if let Some(tx) = clients.lock().await.get(&destination) {
                        println!("sending: {} -> {}", identity, &destination);
                        if tx.send(msg.payload).await.is_err() {
                            eprintln!("Failed to send data to {}", destination);
                        }
                    }
                    Ok(())
                }
            },
            Err(e) => {
                eprintln!("Failed to read message: {}", e);
                Err(e)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_shinkai_message(
        parsed_message: ShinkaiMessage,
        _clients: &TCPProxyClients,
        socket: &Arc<Mutex<TcpStream>>,
        registry: &ShinkaiRegistry,
        sender_identity: &str,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
    ) -> Result<(), NetworkMessageError> {
        eprintln!("Parsed ShinkaiMessage: {:?}", parsed_message);

        let recipient = parsed_message
            .external_metadata
            .recipient
            .trim_start_matches("@@")
            .to_string();
        eprintln!("Recipient: {}", recipient);

        // Check if the sender is @@localhost.*
        eprintln!("Sender Identity {:?}", sender_identity);
        let modified_message = if sender_identity.starts_with("localhost") {
            eprintln!("Sender is localhost, modifying ShinkaiMessage");
            Self::modify_shinkai_message(parsed_message, node_name, identity_secret_key, encryption_secret_key).await?
        } else {
            parsed_message
        };
        eprintln!("\n\nModified ShinkaiMessage: {:?}", modified_message);

        match registry.get_identity_record(recipient.clone()).await {
            Ok(onchain_identity) => {
                match onchain_identity.first_address().await {
                    Ok(first_address) => {
                        eprintln!("Connecting to first address: {}", first_address);
                        match TcpStream::connect(first_address).await {
                            Ok(mut stream) => {
                                let payload = modified_message.encode_message().unwrap();

                                let identity_bytes = recipient.as_bytes();
                                let identity_length = (identity_bytes.len() as u32).to_be_bytes();
                                let total_length =
                                    (payload.len() as u32 + 1 + identity_bytes.len() as u32 + 4).to_be_bytes();

                                let mut data_to_send = Vec::new();
                                data_to_send.extend_from_slice(&total_length);
                                data_to_send.extend_from_slice(&identity_length);
                                data_to_send.extend(identity_bytes);
                                data_to_send.push(0x01); // Message type identifier for ShinkaiMessage
                                data_to_send.extend_from_slice(&payload);

                                stream.write_all(&data_to_send).await?;
                                stream.flush().await?;
                                eprintln!("Sent message to {}", stream.peer_addr().unwrap());
                            }
                            Err(e) => {
                                eprintln!("Failed to connect to first address: {}", e);
                                let error_message = format!("Failed to connect to first address for {}", recipient);
                                send_message_with_length(socket, error_message).await?;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch first address for {}: {}", recipient, e);
                        let error_message = format!(
                            "Recipient {} not connected and failed to fetch first address",
                            recipient
                        );
                        send_message_with_length(socket, error_message).await?;
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to fetch onchain identity for {}: {}", recipient, e);
                let error_message = format!(
                    "Recipient {} not connected and failed to fetch onchain identity",
                    recipient
                );
                send_message_with_length(socket, error_message).await?;
            }
        }
        Ok(())
    }

    async fn modify_shinkai_message(
        message: ShinkaiMessage,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
    ) -> Result<ShinkaiMessage, NetworkMessageError> {
        eprintln!("Modifying ShinkaiMessage");

        let mut modified_message = message;
        modified_message.external_metadata.sender = node_name.to_string();
        modified_message.external_metadata.intra_sender = node_name.to_string();
        modified_message.body = match modified_message.body {
            MessageBody::Unencrypted(mut body) => {
                body.internal_metadata.sender_subidentity = node_name.to_string();
                MessageBody::Unencrypted(body)
            }
            encrypted => encrypted,
        };

        // Re-sign the inner layer
        modified_message.sign_inner_layer(&identity_secret_key)?;

        // Re-sign the outer layer
        let signed_message = modified_message.sign_outer_layer(&identity_secret_key)?;

        Ok(signed_message)
    }

    async fn validate_identity(
        &self,
        socket: Arc<Mutex<TcpStream>>,
        identity: &str,
    ) -> Result<(), NetworkMessageError> {
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
            let identity = identity.trim_start_matches("@@");
            let onchain_identity = self.registry.get_identity_record(identity.to_string()).await;
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

    async fn handle_outgoing_message(data: Vec<u8>, socket: &Arc<Mutex<TcpStream>>) -> Result<(), NetworkMessageError> {
        let mut socket = socket.lock().await;
        socket.write_all(&data).await?;
        Ok(())
    }
}

async fn send_message_with_length(socket: &Arc<Mutex<TcpStream>>, message: String) -> Result<(), NetworkMessageError> {
    let message_len = message.len() as u32;
    let message_len_bytes = message_len.to_be_bytes(); // This will always be 4 bytes big-endian
    let message_bytes = message.as_bytes();

    let mut socket = socket.lock().await;
    socket.write_all(&message_len_bytes).await?;
    socket.write_all(message_bytes).await?;

    Ok(())
}

#[derive(Parser, Debug)]
pub struct Args {
    #[clap(long, default_value = "0.0.0.0:8080")]
    pub address: String,
}
