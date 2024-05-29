use clap::Parser;
use derivative::Derivative;
use ed25519_dalek::{SigningKey, Verifier, VerifyingKey};
use rand::distributions::Alphanumeric;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use shinkai_crypto_identities::ShinkaiRegistry;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use shinkai_message_primitives::shinkai_message::shinkai_message::{MessageBody, ShinkaiMessage};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, string_to_encryption_public_key,
    string_to_encryption_static_key,
};
use shinkai_message_primitives::shinkai_utils::signatures::signature_public_key_to_string;
use std::collections::HashMap;
use std::convert::TryInto;
use std::env;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{NetworkMessage, NetworkMessageError};

pub type TCPProxyClients =
    Arc<Mutex<HashMap<String, (Arc<Mutex<ReadHalf<TcpStream>>>, Arc<Mutex<WriteHalf<TcpStream>>>)>>>; // e.g. @@nico.shinkai -> (Reader, Writer)
pub type TCPProxyPKtoIdentity = Arc<Mutex<HashMap<String, String>>>; // e.g. PK -> @@localhost.shinkai:::PK, PK -> @@nico.shinkai
pub type PublicKeyHex = String;

// Notes:
// TODO: Messages redirected to someone should be checked if the client is still connected if not send an error message back to the sender

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct TCPProxy {
    pub clients: TCPProxyClients,
    pub pk_to_clients: TCPProxyPKtoIdentity,
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
        rpc_url: Option<String>,
        contract_address: Option<String>,
    ) -> Result<Self, NetworkMessageError> {
        let rpc_url = rpc_url
            .or_else(|| env::var("RPC_URL").ok())
            .unwrap_or("https://sepolia.infura.io/v3/0153fa7ada9046f9acee3842cdb28082".to_string());
        let contract_address = contract_address
            .or_else(|| env::var("CONTRACT_ADDRESS").ok())
            .unwrap_or("0xDCbBd3364a98E2078e8238508255dD4a2015DD3E".to_string());

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

        // Print the public keys
        println!(
            "TCP Relay Encryption Public Key: {:?}",
            encryption_public_key_to_string(encryption_public_key)
        );
        println!(
            "TCP Relay Signature Public Key: {:?}",
            signature_public_key_to_string(identity_public_key)
        );

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
            return Err(NetworkMessageError::InvalidData(
                "Identity public key does not match the registry".to_string(),
            ));
        }

        if encryption_public_key != registry_encryption_public_key {
            return Err(NetworkMessageError::InvalidData(
                "Encryption public key does not match the registry".to_string(),
            ));
        }

        Ok(TCPProxy {
            clients: Arc::new(Mutex::new(HashMap::new())),
            pk_to_clients: Arc::new(Mutex::new(HashMap::new())),
            registry,
            node_name,
            identity_secret_key,
            identity_public_key,
            encryption_secret_key,
            encryption_public_key,
        })
    }

    /// Handle a new client connection which could be:
    /// - a Node that needs punch hole
    /// - a Node answering to a request that needs to get redirected to a Node using a punch hole
    pub async fn handle_client(&self, socket: TcpStream) {
        eprintln!("New incoming connection");
        let (reader, writer) = tokio::io::split(socket);
        let reader = Arc::new(Mutex::new(reader));
        let writer = Arc::new(Mutex::new(writer));

        // Read identity
        let network_msg = match NetworkMessage::read_from_socket(reader.clone(), None).await {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("Failed to read_from_socket: {}", e);
                return;
            }
        };
        let identity = network_msg.identity.clone();
        println!(
            "connecting: {} with message_type: {:?}",
            identity, network_msg.message_type
        );

        match network_msg.message_type {
            NetworkMessageType::ProxyMessage => {
                self.handle_proxy_message_type(reader, writer, identity).await;
            }
            NetworkMessageType::ShinkaiMessage => {
                Self::handle_shinkai_message(
                    reader,
                    writer,
                    network_msg,
                    &self.clients,
                    &self.pk_to_clients,
                    &self.registry,
                    &identity,
                    self.node_name.clone(),
                    self.identity_secret_key.clone(),
                    self.encryption_secret_key.clone(),
                )
                .await;
            }
            NetworkMessageType::VRKaiPathPair => {
                eprintln!("VRKaiPathPair message not supported yet");
            }
        };
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_shinkai_message(
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
        network_msg: NetworkMessage,
        clients: &TCPProxyClients,
        pk_to_clients: &TCPProxyPKtoIdentity,
        registry: &ShinkaiRegistry,
        identity: &str,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
    ) {
        eprintln!("Received a ShinkaiMessage from {}...", identity);
        let shinkai_message: Result<ShinkaiMessage, _> = serde_json::from_slice(&network_msg.payload);
        match shinkai_message {
            Ok(parsed_message) => {
                let response = Self::handle_proxy_message(
                    parsed_message.clone(),
                    clients,
                    pk_to_clients,
                    reader,
                    writer,
                    registry,
                    identity.to_string(),
                    node_name,
                    identity_secret_key,
                    encryption_secret_key,
                )
                .await;
                match response {
                    Ok(_) => {
                        eprintln!("Successfully handled ShinkaiMessage");
                    }
                    Err(e) => {
                        eprintln!("Failed to handle ShinkaiMessage: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to parse ShinkaiMessage: {}", e);
            }
        }
    }

    async fn handle_proxy_message_type(
        &self,
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
        identity: String,
    ) {
        eprintln!("Received a ProxyMessage from {}...", identity);
        let public_key_hex = match self.validate_identity(reader.clone(), writer.clone(), &identity).await {
            Ok(pk) => pk,
            Err(e) => {
                eprintln!("Identity validation failed: {}", e);
                return;
            }
        };

        println!("Identity validated: {}", identity);
        // Transform identity for localhost clients
        let mut identity = identity.trim_start_matches("@@").to_string();
        if identity.starts_with("localhost") {
            identity = format!("{}:::{}", identity, public_key_hex);
        }

        {
            let mut clients_lock = self.clients.lock().await;
            clients_lock.insert(identity.clone(), (reader.clone(), writer.clone()));
        }
        {
            let mut pk_to_clients_lock = self.pk_to_clients.lock().await;
            pk_to_clients_lock.insert(public_key_hex, identity.clone());
        }

        let clients_clone = self.clients.clone();
        let pk_to_clients_clone = self.pk_to_clients.clone();
        let reader = reader.clone();
        let writer = writer.clone();
        let registry_clone = self.registry.clone();
        let node_name = self.node_name.clone();
        let identity_sk = self.identity_secret_key.clone();
        let encryption_sk = self.encryption_secret_key.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = NetworkMessage::read_from_socket(reader.clone(), Some(identity.clone())) => {
                        match msg {
                            Ok(msg) => {
                                if let Err(e) = Self::handle_incoming_message(Ok(msg), &clients_clone, &pk_to_clients_clone, reader.clone(), writer.clone(), &registry_clone, &identity, node_name.clone(), identity_sk.clone(), encryption_sk.clone()).await {
                                    eprintln!("Error handling incoming message: {}", e);
                                    break;
                                }
                            }
                            Err(NetworkMessageError::Timeout) => {
                                sleep(std::time::Duration::from_secs(1));
                            }
                            Err(e) => {
                                eprintln!("Connection lost for {} with error: {}", identity, e);
                                break;
                            }
                        }
                    }
                    else => {
                        eprintln!("Connection lost for {}", identity);
                        break;
                    }
                }
            }

            {
                let mut pk_to_clients_lock = pk_to_clients_clone.lock().await;
                if let Some(public_key_hex) =
                    pk_to_clients_lock
                        .iter()
                        .find_map(|(k, v)| if v == &identity { Some(k.clone()) } else { None })
                {
                    pk_to_clients_lock.remove(&public_key_hex);
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
        pk_to_clients: &TCPProxyPKtoIdentity,
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
        registry: &ShinkaiRegistry,
        identity: &str,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
    ) -> Result<(), NetworkMessageError> {
        match msg {
            Ok(msg) => match msg.message_type {
                NetworkMessageType::ProxyMessage => {
                    eprintln!("Received a ProxyMessage ...");
                    let shinkai_message: Result<ShinkaiMessage, _> = serde_json::from_slice(&msg.payload);
                    match shinkai_message {
                        Ok(parsed_message) => {
                            Self::handle_proxy_message(
                                parsed_message,
                                clients,
                                pk_to_clients,
                                reader.clone(),
                                writer.clone(),
                                registry,
                                identity.to_string(),
                                node_name,
                                identity_secret_key,
                                encryption_secret_key,
                            )
                            .await
                        }
                        Err(e) => {
                            eprintln!("Failed to parse ShinkaiMessage: {}", e);
                            let error_message = format!("Failed to parse ShinkaiMessage: {}", e);
                            send_message_with_length(writer.clone(), error_message).await
                        }
                    }
                }
                NetworkMessageType::ShinkaiMessage => {
                    Self::handle_shinkai_message(
                        reader.clone(),
                        writer.clone(),
                        msg,
                        clients,
                        pk_to_clients,
                        registry,
                        identity,
                        node_name,
                        identity_secret_key,
                        encryption_secret_key,
                    )
                    .await;
                    Ok(())
                }
                NetworkMessageType::VRKaiPathPair => {
                    eprintln!("VRKaiPathPair not supported yet");
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
    async fn handle_proxy_message(
        parsed_message: ShinkaiMessage,
        clients: &TCPProxyClients,
        pk_to_clients: &TCPProxyPKtoIdentity,
        _reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
        registry: &ShinkaiRegistry,
        client_identity: String,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
    ) -> Result<(), NetworkMessageError> {
        /*
         For Proxy Message we have multiple cases

         1) From external node
            1.a) If the recipient is the same as the node_name, then we handle the message locally (means that the node is localhost)
            1.b) If the recipient is not the same as the node_name but the recipient's address is this node's address, then we handle the message locally

         2) From proxied node
            2.a) If the recipient is the same as the node_name, then we handle the message locally (means that the node is localhost)
            2.b) If the recipient is not the same as the node_name, then we need to proxy the message to the recipient
        */
        eprintln!("Parsed ShinkaiMessage: {:?}", parsed_message);

        // Check if the message needs to be proxied or if it's meant for one of the connected nodes behind NAT
        let recipient = parsed_message
            .clone()
            .external_metadata
            .recipient
            .trim_start_matches("@@")
            .to_string();

        // Strip out @@ from node_name before comparing recipient and node_name
        let stripped_node_name = node_name.to_string().trim_start_matches("@@").to_string();

        eprintln!("Recipient: {}", recipient);
        eprintln!("Node Name: {}", node_name);

        let recipient_matches = recipient == stripped_node_name
            || recipient.starts_with("localhost")
            || recipient.starts_with("@@localhost");

        let recipient_addresses = if !recipient_matches {
            // Fetch the public keys from the registry
            let registry_identity = registry.get_identity_record(recipient.clone()).await.unwrap();
            eprintln!("Registry Identity (recipient_addresses): {:?}", registry_identity);
            registry_identity.address_or_proxy_nodes
        } else {
            vec![]
        };

        // check if it matches the tcp relayer's node name
        if recipient == stripped_node_name {
            eprintln!("> Outside Node Message to Proxy");
            eprintln!("Recipient is the same as the node name, handling message locally");

            // Fetch the public keys from the registry
            let msg_sender = parsed_message.clone().external_metadata.sender;
            let registry_identity = registry.get_identity_record(msg_sender).await.unwrap();
            eprintln!("Registry Identity: {:?}", registry_identity);
            let sender_encryption_pk = registry_identity.encryption_public_key()?;
            let sender_signature_pk = registry_identity.signature_verifying_key()?;

            // validate that's signed by the sender
            parsed_message.verify_outer_layer_signature(&sender_signature_pk)?;
            eprintln!("Signature verified");

            let mut unencrypted_msg = parsed_message.clone();
            // Decrypt message if encrypted
            if parsed_message.is_body_currently_encrypted() {
                unencrypted_msg = Self::decrypt_message_if_needed(
                    unencrypted_msg.clone(),
                    encryption_secret_key.clone(),
                    sender_encryption_pk,
                )
                .await?;
            }

            let updated_message = match Self::modify_shinkai_message_external_to_proxied_localhost(
                unencrypted_msg.clone(),
                node_name,
                identity_secret_key,
                encryption_secret_key,
                sender_encryption_pk,
                "main".to_string(),
            )
            .await
            {
                Ok(message) => message,
                Err(e) => {
                    eprintln!("Error modifying Shinkai message: {}", e);
                    return Ok(());
                }
            };
            eprintln!("Updated ShinkaiMessage: {:?}", updated_message);

            // TODO: check if we also need to decrypt inner layer
            let subidentity = match unencrypted_msg.get_recipient_subidentity() {
                Some(subidentity) => subidentity,
                None => {
                    eprintln!(
                        "Error: No subidentity found for the recipient. Subidentity: {:?}",
                        updated_message.external_metadata.recipient
                    );
                    return Ok(());
                }
            };
            eprintln!("Recipient Subidentity: {}", subidentity);

            // Find the client that needs to receive the message
            let client_identity = {
                let pk_to_clients_guard = pk_to_clients.lock().await;
                match pk_to_clients_guard.get(&subidentity) {
                    Some(client_identity) => client_identity.clone(),
                    None => {
                        eprintln!("Error: Client not found for recipient {}", recipient);
                        return Ok(());
                    }
                }
            };
            eprintln!("Client Identity: {}", client_identity);

            let connection = {
                let clients_guard = clients.lock().await;
                match clients_guard.get(&client_identity) {
                    Some(connection) => connection.clone(),
                    None => {
                        eprintln!("Error: Connection not found for recipient {}", recipient);
                        return Ok(());
                    }
                }
            };

            // Send message to the client using connection
            eprintln!("Sending message to client {}", client_identity);
            if Self::send_shinkai_message_to_proxied_identity(connection.1, updated_message)
                .await
                .is_err()
            {
                eprintln!("Failed to send message to client {}", client_identity);
            }

            Ok(())
        } else if recipient_addresses.iter().any(|addr| addr == &stripped_node_name) {
            eprintln!(
                "Recipient is {} and uses this node as proxy, handling message locally",
                recipient
            );

            let connection = {
                let clients_guard = clients.lock().await;
                match clients_guard.get(&recipient) {
                    Some(connection) => connection.clone(),
                    None => {
                        eprintln!("Error: Connection not found for recipient {}", recipient);
                        return Ok(());
                    }
                }
            };

            // Send message to the client using connection
            eprintln!("Sending message to client {}", recipient);
            if Self::send_shinkai_message_to_proxied_identity(connection.1, parsed_message)
                .await
                .is_err()
            {
                eprintln!("Failed to send message to client {}", recipient);
            }

            Ok(())
        } else {
            eprintln!("Proxying message out. Sender is behind NAT & localhost");
            // it's meant to be proxied out of the NAT
            // Sender identity looks like: localhost.shinkai:::69fa099bdce516bfeb46d5fc6e908f6cf8ffac0aba76ca0346a7b1a751a2712e
            // we can't use that as a subidentity because it contains "." and ":::" which are not valid characters
            let msg_sender = parsed_message.external_metadata.clone().sender.to_string();
            eprintln!("Sender Identity {:?}", msg_sender);

            let stripped_sender_name = msg_sender.to_string().trim_start_matches("@@").to_string();
            let modified_message = if stripped_sender_name.starts_with("localhost") {
                eprintln!(
                    "Sender is localhost, modifying ShinkaiMessage for sender: {}",
                    client_identity
                );
                let client_identity_pk = client_identity.split(":::").last().unwrap_or("").to_string();
                Self::modify_shinkai_message_proxied_localhost_to_external(
                    parsed_message,
                    node_name,
                    identity_secret_key,
                    encryption_secret_key,
                    client_identity_pk,
                )
                .await?
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
                                    eprintln!("Connected successfully. Streaming...");
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
                                    send_message_with_length(writer.clone(), error_message).await?;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to fetch first address for {}: {}", recipient, e);
                            let error_message = format!(
                                "Recipient {} not connected and failed to fetch first address",
                                recipient
                            );
                            send_message_with_length(writer.clone(), error_message).await?;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Failed to fetch onchain identity for {}: {}", recipient, e);
                    let error_message = format!(
                        "Recipient {} not connected and failed to fetch onchain identity",
                        recipient
                    );
                    send_message_with_length(writer.clone(), error_message).await?;
                }
            }
            Ok(())
        }
    }

    async fn modify_shinkai_message_proxied_localhost_to_external(
        message: ShinkaiMessage,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
        subidentity: String,
    ) -> Result<ShinkaiMessage, NetworkMessageError> {
        eprintln!(
            "modify_shinkai_message_proxied_to_external> Modifying ShinkaiMessage from subidentity: {}",
            subidentity
        );

        let mut modified_message = message;
        if modified_message.is_body_currently_encrypted() {
            if !modified_message.external_metadata.other.is_empty() {
                let intra_sender = modified_message.external_metadata.other.clone();
                eprintln!("Intra Sender: {:?}", intra_sender);
                let sender_encryption_pk = string_to_encryption_public_key(&intra_sender)?;
                eprintln!("Sender Encryption Public Key: {:?}", subidentity);

                let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);
                let enc_pk_string = encryption_public_key_to_string(encryption_public_key);
                eprintln!("My Encryption Public Key: {:?}", enc_pk_string);
                // Attempt to decrypt the message
                modified_message = modified_message
                    .decrypt_outer_layer(&encryption_secret_key, &sender_encryption_pk)
                    .map_err(|e| NetworkMessageError::EncryptionError(format!("Failed to decrypt message: {}", e)))?;
            } else {
                eprintln!(
                    "Error: No intra_sender found for the recipient. Identity: {:?}",
                    subidentity
                );
                // print error
            }
        }

        modified_message.external_metadata.sender = node_name.to_string();
        modified_message.external_metadata.intra_sender = subidentity.to_string();
        modified_message.body = match modified_message.body {
            MessageBody::Unencrypted(mut body) => {
                body.internal_metadata.sender_subidentity = subidentity;
                MessageBody::Unencrypted(body)
            }
            encrypted => encrypted,
        };

        // Re-sign the inner layer
        modified_message.sign_inner_layer(&identity_secret_key)?;

        // TODO: re-encrypt the outer layer using the target's pk

        // Re-sign the outer layer
        let signed_message = modified_message.sign_outer_layer(&identity_secret_key)?;

        Ok(signed_message)
    }

    async fn modify_shinkai_message_external_to_proxied_localhost(
        message: ShinkaiMessage,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
        sender_encryption_pk: EncryptionPublicKey,
        subidentity: String,
    ) -> Result<ShinkaiMessage, NetworkMessageError> {
        eprintln!(
            "modify_shinkai_message_external_to_proxied> Modifying ShinkaiMessage from subidentity: {}",
            subidentity
        );

        let mut modified_message = message;
        modified_message.external_metadata.sender = node_name.to_string();
        modified_message.external_metadata.recipient = "@@locahost.sepolia-shinkai".to_string();
        modified_message.external_metadata.intra_sender = "".to_string();
        modified_message.body = match modified_message.body {
            MessageBody::Unencrypted(mut body) => {
                body.internal_metadata.recipient_subidentity = "main".to_string(); // TODO: eventually update this to be flexible
                MessageBody::Unencrypted(body)
            }
            encrypted => encrypted,
        };

        // Re-sign the inner layer
        modified_message.sign_inner_layer(&identity_secret_key)?;

        // TODO: re-encrypt the outer layer using the target's pk

        // Re-sign the outer layer
        let signed_message = modified_message.sign_outer_layer(&identity_secret_key)?;

        eprintln!(
            "modify_shinkai_message_external_to_proxied_localhost> Modified ShinkaiMessage: {:?}",
            signed_message
        );

        Ok(signed_message)
    }

    async fn validate_identity(
        &self,
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
        identity: &str,
    ) -> Result<PublicKeyHex, NetworkMessageError> {
        let identity = identity.trim_start_matches("@@");
        let validation_data = Self::generate_validation_data();

        // Send validation data to the client
        send_message_with_length(writer.clone(), validation_data.clone()).await?;

        eprintln!("Validating identity: {}", identity);
        let validation_result = if !identity.starts_with("localhost") {
            self.validate_non_localhost_identity(reader.clone(), identity, &validation_data)
                .await
        } else {
            self.validate_localhost_identity(reader.clone(), &validation_data).await
        };

        // Send validation result back to the client
        let validation_message = match &validation_result {
            Ok(_) => "Validation successful".to_string(),
            Err(e) => format!("Validation failed: {}", e),
        };

        send_message_with_length(writer, validation_message).await?;

        validation_result
    }

    fn generate_validation_data() -> String {
        let mut rng = StdRng::from_entropy();
        let random_string: String = (0..16).map(|_| rng.sample(Alphanumeric) as char).collect();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        format!("{}{}", random_string, timestamp)
    }

    async fn validate_non_localhost_identity(
        &self,
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        identity: &str,
        validation_data: &str,
    ) -> Result<PublicKeyHex, NetworkMessageError> {
        eprintln!("Validating non-localhost identity: {}", identity);
        // The client is expected to send back a message containing:
        // 1. The length of the signed validation data (4 bytes, big-endian).
        // 2. The signed validation data (hex-encoded string).
        let buffer = Self::read_buffer_from_socket(reader.clone()).await?;
        eprintln!("Received buffer: {:?}", buffer);
        let mut cursor = std::io::Cursor::new(buffer);

        let _public_key = Self::read_public_key_from_cursor(&mut cursor).await?;
        eprintln!("Received public key: {:?}", _public_key);
        let signature = Self::read_signature_from_cursor(&mut cursor).await?;

        // eprintln!("Received response: {}", signature);
        eprintln!("Obtaining onchain identity for {}...", identity);
        let onchain_identity = self.registry.get_identity_record(identity.to_string()).await;
        eprintln!("Onchain identity: {:?}", onchain_identity);
        let public_key = onchain_identity.unwrap().signature_verifying_key().unwrap();

        if public_key.verify(validation_data.as_bytes(), &signature).is_err() {
            Err(NetworkMessageError::InvalidData(
                "Signature verification failed".to_string(),
            ))
        } else {
            Ok(hex::encode(public_key.to_bytes()))
        }

        // TODO: validate that the timestamp is recent enough
        // TODO: and it hasn't been used before

        // if !Self::validate_signature(&public_key, validation_data, &signature)? {
        //     Err(NetworkMessageError::InvalidData(
        //         "Signature verification failed".to_string(),
        //     ))
        // } else {
        //     Ok(hex::encode(public_key.to_bytes()))
        // }
    }

    async fn validate_localhost_identity(
        &self,
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        validation_data: &str,
    ) -> Result<PublicKeyHex, NetworkMessageError> {
        // The client is expected to send back a message containing:
        // 1. The length of the public key (4 bytes, big-endian).
        // 2. The public key itself (hex-encoded string).
        // 3. The length of the signed validation data (4 bytes, big-endian).
        // 4. The signed validation data (hex-encoded string).
        let buffer = Self::read_buffer_from_socket(reader.clone()).await?;
        let mut cursor = std::io::Cursor::new(buffer);

        let public_key = Self::read_public_key_from_cursor(&mut cursor).await?;
        let signature = Self::read_signature_from_cursor(&mut cursor).await?;

        if public_key.verify(validation_data.as_bytes(), &signature).is_err() {
            Err(NetworkMessageError::InvalidData(
                "Signature verification failed".to_string(),
            ))
        } else {
            Ok(hex::encode(public_key.to_bytes()))
        }
    }

    async fn read_response_from_socket(reader: Arc<Mutex<ReadHalf<TcpStream>>>) -> Result<String, NetworkMessageError> {
        let mut len_buffer = [0u8; 4];
        {
            let mut reader = reader.lock().await;
            reader.read_exact(&mut len_buffer).await?;
        }

        let response_len = u32::from_be_bytes(len_buffer) as usize;
        let mut buffer = vec![0u8; response_len];
        {
            let mut reader = reader.lock().await;
            reader.read_exact(&mut buffer).await?;
        }

        String::from_utf8(buffer).map_err(NetworkMessageError::Utf8Error)
    }

    async fn read_buffer_from_socket(reader: Arc<Mutex<ReadHalf<TcpStream>>>) -> Result<Vec<u8>, NetworkMessageError> {
        let mut len_buffer = [0u8; 4];
        {
            let mut reader = reader.lock().await;
            reader.read_exact(&mut len_buffer).await?;
        }

        let total_len = u32::from_be_bytes(len_buffer) as usize;
        let mut buffer = vec![0u8; total_len];
        {
            let mut reader = reader.lock().await;
            reader.read_exact(&mut buffer).await?;
        }

        Ok(buffer)
    }

    async fn read_public_key_from_cursor(
        cursor: &mut std::io::Cursor<Vec<u8>>,
    ) -> Result<ed25519_dalek::VerifyingKey, NetworkMessageError> {
        let mut len_buffer = [0u8; 4];
        cursor.read_exact(&mut len_buffer).await?;
        let public_key_len = u32::from_be_bytes(len_buffer) as usize;

        let mut public_key_buffer = vec![0u8; public_key_len];
        cursor.read_exact(&mut public_key_buffer).await?;
        let public_key_hex = String::from_utf8(public_key_buffer).map_err(|_| {
            NetworkMessageError::InvalidData("Failed to convert public key buffer to string".to_string())
        })?;
        let public_key_bytes = hex::decode(public_key_hex)
            .map_err(|_| NetworkMessageError::InvalidData("Failed to decode public key hex".to_string()))?;
        let public_key_array: [u8; 32] = public_key_bytes
            .try_into()
            .map_err(|_| NetworkMessageError::InvalidData("Invalid length for public key array".to_string()))?;
        ed25519_dalek::VerifyingKey::from_bytes(&public_key_array)
            .map_err(|_| NetworkMessageError::InvalidData("Failed to create VerifyingKey from bytes".to_string()))
    }

    async fn read_signature_from_cursor(
        cursor: &mut std::io::Cursor<Vec<u8>>,
    ) -> Result<ed25519_dalek::Signature, NetworkMessageError> {
        let mut len_buffer = [0u8; 4];
        cursor.read_exact(&mut len_buffer).await?;
        let signature_len = u32::from_be_bytes(len_buffer) as usize;

        let mut signature_buffer = vec![0u8; signature_len];
        cursor.read_exact(&mut signature_buffer).await?;
        let signature_hex = String::from_utf8(signature_buffer).map_err(|_| {
            NetworkMessageError::InvalidData("Failed to convert signature buffer to string".to_string())
        })?;
        let signature_bytes = hex::decode(signature_hex)
            .map_err(|_| NetworkMessageError::InvalidData("Failed to decode signature hex".to_string()))?;
        let signature_array: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| NetworkMessageError::InvalidData("Invalid length for signature array".to_string()))?;
        Ok(ed25519_dalek::Signature::from_bytes(&signature_array))
    }

    fn validate_signature(
        public_key: &ed25519_dalek::VerifyingKey,
        message: &str,
        signature: &str,
    ) -> Result<bool, NetworkMessageError> {
        // Decode the hex signature to bytes
        let signature_bytes = hex::decode(signature)
            .map_err(|_| NetworkMessageError::InvalidData("Failed to decode signature hex".to_string()))?;

        // Convert the bytes to Signature
        let signature_bytes_slice = &signature_bytes[..];
        let signature_bytes_array: &[u8; 64] = signature_bytes_slice
            .try_into()
            .map_err(|_| NetworkMessageError::InvalidData("Invalid length for signature array".to_string()))?;

        let signature = ed25519_dalek::Signature::from_bytes(signature_bytes_array);

        // Verify the signature against the message
        match public_key.verify(message.as_bytes(), &signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn decrypt_message_if_needed(
        potentially_encrypted_msg: ShinkaiMessage,
        encryption_secret_key: EncryptionStaticKey,
        registry_sender_encryption_pk: EncryptionPublicKey,
    ) -> Result<ShinkaiMessage, NetworkMessageError> {
        let msg: ShinkaiMessage;

        // Convert the encryption_secret_key to a public key
        let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);

        // Print the public keys
        println!(
            "Encryption Secret Key Public Key: {:?}",
            encryption_public_key_to_string(encryption_public_key)
        );
        println!(
            "Registry Sender Encryption Public Key: {:?}",
            encryption_public_key_to_string(registry_sender_encryption_pk)
        );

        // check if the message is encrypted
        let is_body_encrypted = potentially_encrypted_msg.clone().is_body_currently_encrypted();
        if is_body_encrypted {
            /*
            When someone sends an encrypted message, we need to compute the shared key using Diffie-Hellman,
            but what if they are using a subidentity? We don't know which one because it's encrypted.
            So the only way to get the pk is if they send it to us in the external_metadata.other field or
            if they are using intra_sender (which needs to be deleted afterwards).
            For other cases, we can find it in the identity manager.
            */
            let sender_encryption_pk_string = potentially_encrypted_msg.external_metadata.clone().other;
            let sender_encryption_pk = string_to_encryption_public_key(sender_encryption_pk_string.as_str()).ok();

            if sender_encryption_pk.is_some() {
                msg = match potentially_encrypted_msg
                    .clone()
                    .decrypt_outer_layer(&encryption_secret_key, &sender_encryption_pk.unwrap())
                {
                    Ok(msg) => msg,
                    Err(e) => {
                        return Err(NetworkMessageError::InvalidData(format!(
                            "Failed to decrypt message: {}",
                            e
                        )));
                    }
                };
            } else {
                msg = match potentially_encrypted_msg
                    .clone()
                    .decrypt_outer_layer(&encryption_secret_key, &registry_sender_encryption_pk)
                {
                    Ok(msg) => msg,
                    Err(e) => {
                        return Err(NetworkMessageError::InvalidData(format!(
                            "Failed to decrypt message: {}",
                            e
                        )));
                    }
                };
            }
        } else {
            msg = potentially_encrypted_msg.clone();
        }
        Ok(msg)
    }

    async fn send_shinkai_message_to_proxied_identity(
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
        message: ShinkaiMessage,
    ) -> Result<(), NetworkMessageError> {
        let encoded_msg = message.encode_message().unwrap();
        let identity = &message.external_metadata.recipient;
        let identity_bytes = identity.as_bytes();
        let identity_length = (identity_bytes.len() as u32).to_be_bytes();

        // Prepare the message with a length prefix and identity length
        let total_length = (encoded_msg.len() as u32 + 1 + identity_bytes.len() as u32 + 4).to_be_bytes(); // Convert the total length to bytes, adding 1 for the header and 4 for the identity length

        let mut data_to_send = Vec::new();
        let header_data_to_send = vec![0x01]; // Message type identifier for ShinkaiMessage
        data_to_send.extend_from_slice(&total_length);
        data_to_send.extend_from_slice(&identity_length);
        data_to_send.extend(identity_bytes);
        data_to_send.extend(header_data_to_send);
        data_to_send.extend_from_slice(&encoded_msg);

        println!("Sending message to client: beep beep boop");
        let mut writer_lock = writer.lock().await;
        writer_lock
            .write_all(&data_to_send)
            .await
            .map_err(|_| NetworkMessageError::SendError)?;
        writer_lock.flush().await.map_err(|_| NetworkMessageError::SendError)?;

        println!("Message sent to client");
        Ok(())
    }
}

async fn send_message_with_length(
    writer: Arc<Mutex<WriteHalf<TcpStream>>>,
    message: String,
) -> Result<(), NetworkMessageError> {
    eprintln!("send_message_with_length> Sending message: {}", message);
    let message_len = message.len() as u32;
    eprintln!("send_message_with_length> Message length: {}", message_len);
    let message_len_bytes = message_len.to_be_bytes(); // This will always be 4 bytes big-endian
    let message_bytes = message.as_bytes();

    let mut writer = writer.lock().await;
    writer.write_all(&message_len_bytes).await?;
    writer.write_all(message_bytes).await?;

    Ok(())
}

#[derive(Parser, Debug)]
pub struct Args {
    #[clap(long, default_value = "0.0.0.0:8080")]
    pub address: String,
}
