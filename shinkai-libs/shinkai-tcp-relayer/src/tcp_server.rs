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
    encryption_public_key_to_string, string_to_encryption_public_key, string_to_encryption_static_key
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
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{NetworkMessage, NetworkMessageError};

pub type TCPProxyClients =
    Arc<Mutex<HashMap<String, (Arc<Mutex<ReadHalf<TcpStream>>>, Arc<Mutex<WriteHalf<TcpStream>>>)>>>; // e.g. @@nico.shinkai -> (Reader, Writer)
pub type TCPProxyPKtoIdentity = Arc<Mutex<HashMap<String, String>>>; // e.g. PK -> @@localhost.shinkai:::PK, PK -> @@nico.shinkai
pub type PublicKeyHex = String;

// Notes:
// TODO: Messages redirected to someone should be checked if the client is still connected if not send an error message
// back to the sender

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
    pub connection_semaphore: Arc<Semaphore>,
    pub max_connections: usize,
}

impl TCPProxy {
    pub async fn new(
        identity_secret_key: Option<SigningKey>,
        encryption_secret_key: Option<EncryptionStaticKey>,
        node_name: Option<String>,
        rpc_url: Option<String>,
        contract_address: Option<String>,
        max_connections: Option<usize>,
    ) -> Result<Self, NetworkMessageError> {
        let rpc_url = rpc_url
            .or_else(|| env::var("RPC_URL").ok())
            .unwrap_or("https://sepolia.base.org".to_string());
        let contract_address = contract_address
            .or_else(|| env::var("CONTRACT_ADDRESS").ok())
            .unwrap_or("0x425fb20ba3874e887336aaa7f3fab32d08135ba9".to_string());
        let max_connections = max_connections
            .or_else(|| env::var("MAX_CONNECTIONS").ok().and_then(|s| s.parse().ok()))
            .unwrap_or(20);

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
        let registry_identity = registry.get_identity_record(node_name.to_string(), None).await.unwrap();
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
            connection_semaphore: Arc::new(Semaphore::new(max_connections)),
            max_connections,
        })
    }

    /// Handle a new client connection which could be:
    /// - a Node that needs punch hole
    /// - a Node answering to a request that needs to get redirected to a Node using a punch hole
    pub async fn handle_client(&self, socket: TcpStream) {
        let session_id = Uuid::new_v4();
        println!("[{}] New incoming connection", session_id);

        let (reader, writer) = tokio::io::split(socket);
        let reader = Arc::new(Mutex::new(reader));
        let writer = Arc::new(Mutex::new(writer));

        let permit = self.connection_semaphore.clone().acquire_owned().await.unwrap();
        let max_connections = self.max_connections;
        println!(
            "Semaphore acquired. Available permits: {} out of {}",
            self.connection_semaphore.available_permits(),
            max_connections
        );

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
            "[{}] connecting: {} with message_type: {:?}",
            session_id, identity, network_msg.message_type
        );

        match network_msg.message_type {
            NetworkMessageType::ProxyMessage => {
                self.handle_proxy_message_type(reader, writer, identity, session_id)
                    .await;
                drop(permit);
                println!(
                    "[{}] Semaphore permit dropped. Available permits: {} out of {}",
                    session_id,
                    self.connection_semaphore.available_permits(),
                    max_connections
                );
            }
            NetworkMessageType::ShinkaiMessage => {
                let clients = self.clients.clone();
                let pk_to_clients = self.pk_to_clients.clone();
                let registry = self.registry.clone();
                let node_name = self.node_name.clone();
                let identity_secret_key = self.identity_secret_key.clone();
                let encryption_secret_key = self.encryption_secret_key.clone();
                let connection_semaphore = self.connection_semaphore.clone();

                tokio::spawn(async move {
                    // The permit will be dropped when the task completes, releasing the semaphore
                    let _permit = permit;
                    println!(
                        "[{}] Semaphore acquired in spawned task. Available permits: {} out of {}",
                        session_id,
                        connection_semaphore.available_permits(),
                        max_connections
                    );

                    TCPProxy::handle_shinkai_message(
                        reader,
                        writer,
                        network_msg,
                        &clients,
                        &pk_to_clients,
                        &registry,
                        &identity,
                        node_name,
                        identity_secret_key,
                        encryption_secret_key,
                        session_id,
                    )
                    .await;

                    drop(_permit);
                    println!(
                        "[{}] Semaphore permit dropped in spawned task. Available permits: {} out of {}",
                        session_id,
                        connection_semaphore.available_permits(),
                        max_connections
                    );
                });
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
        session_id: Uuid,
    ) {
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
                    session_id,
                )
                .await;
                match response {
                    Ok(_) => {
                        println!("[{}] Successfully handled ShinkaiMessage for: {}", session_id, identity);
                    }
                    Err(e) => {
                        eprintln!("[{}] Failed to handle ShinkaiMessage: {}", session_id, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("[{}] Failed to parse ShinkaiMessage: {}", session_id, e);
            }
        }
    }

    async fn handle_proxy_message_type(
        &self,
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
        identity: String,
        session_id: Uuid,
    ) {
        println!("[{}] Received a ProxyMessage from {}...", session_id, identity);
        let public_key_hex = match self.validate_identity(reader.clone(), writer.clone(), &identity).await {
            Ok(pk) => pk,
            Err(e) => {
                eprintln!("[{}] Identity validation failed: {}", session_id, e);
                return;
            }
        };

        println!("[{}] Identity validated: {}", session_id, identity);
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
                                if let Err(e) = Self::handle_incoming_message(Ok(msg), &clients_clone, &pk_to_clients_clone, reader.clone(), writer.clone(), &registry_clone, &identity, node_name.clone(), identity_sk.clone(), encryption_sk.clone(), session_id).await {
                                    eprintln!("[{}] Error handling incoming message: {}", session_id, e);
                                    break;
                                }
                            }
                            Err(NetworkMessageError::Timeout) => {
                                sleep(std::time::Duration::from_secs(1));
                            }
                            Err(e) => {
                                eprintln!("[{}] Connection lost for {} with error: {}", session_id, identity, e);
                                break;
                            }
                        }
                    }
                    else => {
                        eprintln!("[{}] Connection lost for {}", session_id, identity);
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
            eprintln!("[{}] disconnected: {}", session_id, identity);
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
        session_id: Uuid,
    ) -> Result<(), NetworkMessageError> {
        match msg {
            Ok(msg) => match msg.message_type {
                NetworkMessageType::ProxyMessage => {
                    println!("[{}] Received a ProxyMessage ...", session_id);
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
                                session_id,
                            )
                            .await
                        }
                        Err(e) => {
                            eprintln!("[{}] Failed to parse ShinkaiMessage: {}", session_id, e);
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
                        session_id,
                    )
                    .await;
                    Ok(())
                }
            },
            Err(e) => {
                eprintln!("[{}] Failed to read message: {}", session_id, e);
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
        tcp_node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
        session_id: Uuid,
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
        println!(
            "[{}] Received a ShinkaiMessage from {}...",
            session_id, parsed_message.external_metadata.sender
        );
        println!(
            "[{}] handle_proxy_message> ShinkaiMessage External Metadata: {:?}",
            session_id, parsed_message.external_metadata
        );

        // Check if the message needs to be proxied or if it's meant for one of the connected nodes behind NAT
        let msg_recipient = parsed_message
            .clone()
            .external_metadata
            .recipient
            .trim_start_matches("@@")
            .to_string();

        let msg_sender = parsed_message
            .clone()
            .external_metadata
            .sender
            .trim_start_matches("@@")
            .to_string();

        // Strip out @@ from node_name before comparing recipient and node_name
        let tcp_node_name_string = tcp_node_name.to_string().trim_start_matches("@@").to_string();

        // For Debugging
        println!("[{}] TCP Node Name: {}", session_id, tcp_node_name_string);
        println!("[{}] MSG Recipient: {}", session_id, msg_recipient);
        println!("[{}] MSG Sender: {}", session_id, msg_sender);

        // Case A
        // If Message Recipient is TCP Node Name
        // We proxied a message from a localhost identity and we are getting the response
        if msg_recipient == tcp_node_name_string {
            println!("[{}] Case A", session_id);
            println!(
                "[{}] Recipient is the same as the node name, handling message locally",
                session_id
            );
            Self::handle_ext_node_to_proxy_msg(
                parsed_message,
                clients,
                pk_to_clients,
                encryption_secret_key,
                identity_secret_key,
                registry,
                tcp_node_name,
                msg_sender,
                session_id,
            )
            .await?;
            return Ok(());
        }

        // Case B
        // If Message Recipient is a Shinkai Identity (exclude localhost ofc)
        // We check if we it's being proxied through us
        let recipient_matches = msg_recipient == tcp_node_name_string
            || msg_recipient.starts_with("localhost")
            || msg_recipient.starts_with("@@localhost");

        let recipient_addresses = if !recipient_matches {
            // Fetch the public keys from the registry
            let registry_identity = registry.get_identity_record(msg_recipient.clone(), None).await.unwrap();
            registry_identity.address_or_proxy_nodes
        } else {
            vec![]
        };

        // if the node is using the proxy as a relay then it will be in the recipient_addresses
        if recipient_addresses.iter().any(|addr| addr == &tcp_node_name_string) {
            println!("[{}] Case B", session_id);
            println!(
                "[{}] Recipient is {} and uses this node as proxy, handling message locally",
                session_id, msg_recipient
            );

            let connection = {
                let clients_guard = clients.lock().await;
                match clients_guard.get(&msg_recipient) {
                    Some(connection) => connection.clone(),
                    None => {
                        eprintln!(
                            "[{}] Error: Connection not found for recipient {}",
                            session_id, msg_recipient
                        );
                        return Ok(());
                    }
                }
            };

            if Self::send_shinkai_message_to_proxied_identity(connection.1, parsed_message)
                .await
                .is_err()
            {
                eprintln!("[{}] Failed to send message to client {}", session_id, msg_recipient);
            }

            return Ok(());
        }

        // Case C
        // Proxying message out. Sender is behind NAT & localhost
        if msg_sender.starts_with("localhost") || msg_sender.starts_with("@@localhost") {
            println!("[{}] Case C", session_id);
            println!(
                "[{}] Proxying message out. Sender is behind NAT & localhost",
                session_id
            );
            let client_identity_pk = client_identity.split(":::").last().unwrap_or("").to_string();
            let modified_message = Self::modify_shinkai_message_proxied_localhost_to_external(
                parsed_message,
                tcp_node_name,
                identity_secret_key,
                encryption_secret_key,
                client_identity_pk,
                session_id,
            )
            .await?;

            Self::handle_proxy_out_to_ext_node(registry, msg_recipient, modified_message, writer.clone(), session_id)
                .await?;

            return Ok(());
        }

        // Case D
        // Sending a message using the relayer public IP
        // This could proce a loop in the relayer so we discard the message
        let relayer_addresses = if !recipient_matches {
            // Fetch the public keys from the registry
            let registry_identity = registry
                .get_identity_record(tcp_node_name_string.clone(), None)
                .await
                .unwrap();
            registry_identity.address_or_proxy_nodes
        } else {
            vec![]
        };
        if relayer_addresses
            .iter()
            .any(|addr| recipient_addresses.iter().any(|recipient_addr| addr == recipient_addr))
        {
            println!("[{}] Case D", session_id);
            return Err(NetworkMessageError::RecipientLoopError(recipient_addresses.join(",")));
        }

        // Case E
        // Proxying message out. Sender is not localhost but a well defined identity
        // We need to proxy the message to the recipient
        println!("[{}] Case E", session_id);
        println!(
            "[{}] Proxying message out. Sender is not localhost but a well defined identity",
            session_id
        );
        Self::handle_proxy_out_to_ext_node(registry, msg_recipient, parsed_message, writer.clone(), session_id).await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_ext_node_to_proxy_msg(
        parsed_message: ShinkaiMessage,
        clients: &TCPProxyClients,
        pk_to_clients: &TCPProxyPKtoIdentity,
        encryption_secret_key: EncryptionStaticKey,
        identity_secret_key: SigningKey,
        registry: &ShinkaiRegistry,
        tcp_node_name: ShinkaiName,
        msg_sender: String,
        session_id: Uuid,
    ) -> Result<(), NetworkMessageError> {
        // Fetch the public keys from the registry
        let registry_identity = registry.get_identity_record(msg_sender.clone(), None).await.unwrap();
        let sender_encryption_pk = registry_identity.encryption_public_key()?;
        let sender_signature_pk = registry_identity.signature_verifying_key()?;

        // validate that's signed by the sender
        parsed_message.verify_outer_layer_signature(&sender_signature_pk)?;

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
            tcp_node_name,
            identity_secret_key,
            encryption_secret_key,
            sender_encryption_pk,
            "main".to_string(),
        )
        .await
        {
            Ok(message) => message,
            Err(e) => {
                eprintln!("[{}] Error modifying Shinkai message: {}", session_id, e);
                return Ok(());
            }
        };

        // TODO: check if we also need to decrypt inner layer
        let subidentity = match unencrypted_msg.get_recipient_subidentity() {
            Some(subidentity) => subidentity,
            None => {
                eprintln!(
                    "[{}] Error: No subidentity found for the recipient. Subidentity: {:?}",
                    session_id, updated_message.external_metadata.recipient
                );
                return Ok(());
            }
        };

        // Find the client that needs to receive the message
        let client_identity = {
            let pk_to_clients_guard = pk_to_clients.lock().await;
            match pk_to_clients_guard.get(&subidentity) {
                Some(client_identity) => client_identity.clone(),
                None => {
                    eprintln!("[{}] Error: Client not found for recipient {}", session_id, msg_sender);
                    return Ok(());
                }
            }
        };

        let connection = {
            let clients_guard = clients.lock().await;
            match clients_guard.get(&client_identity) {
                Some(connection) => connection.clone(),
                None => {
                    eprintln!(
                        "[{}] Error: Connection not found for recipient {}",
                        session_id, msg_sender
                    );
                    return Ok(());
                }
            }
        };

        // Send message to the client using connection
        if Self::send_shinkai_message_to_proxied_identity(connection.1, updated_message)
            .await
            .is_err()
        {
            eprintln!("[{}] Failed to send message to client {}", session_id, client_identity);
        }

        Ok(())
    }

    async fn handle_proxy_out_to_ext_node(
        registry: &ShinkaiRegistry,
        msg_recipient: String,
        modified_message: ShinkaiMessage,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
        session_id: Uuid,
    ) -> Result<(), NetworkMessageError> {
        match registry.get_identity_record(msg_recipient.clone(), None).await {
            Ok(onchain_identity) => {
                match onchain_identity.first_address().await {
                    Ok(first_address) => {
                        println!(
                            "[{}] Connecting to first address: {} for identity: {}",
                            session_id, first_address, onchain_identity.shinkai_identity
                        );
                        match TcpStream::connect(first_address).await {
                            Ok(mut stream) => {
                                println!(
                                    "[{}] Establish connection to {} successful. Streaming...",
                                    session_id, onchain_identity.shinkai_identity
                                );
                                let payload = modified_message.encode_message().unwrap();
                                // Add '@@' to message recipient if it doesn't start with that
                                let modified_msg_recipient = if !msg_recipient.starts_with("@@") {
                                    format!("@@{}", msg_recipient)
                                } else {
                                    msg_recipient.clone()
                                };
                                let identity_bytes = modified_msg_recipient.as_bytes();
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
                                println!("[{}] Sent message to {}", session_id, stream.peer_addr().unwrap());
                            }
                            Err(e) => {
                                eprintln!(
                                    "[{}] Failed to connect to first address for {}: {}",
                                    session_id, msg_recipient, e
                                );
                                let error_message = format!("Failed to connect to first address for {}", msg_recipient);
                                send_message_with_length(writer.clone(), error_message).await?;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "[{}] Failed to fetch first address for {}: {}",
                            session_id, msg_recipient, e
                        );
                        let error_message = format!(
                            "[{}] Recipient {} not connected and failed to fetch first address",
                            session_id, msg_recipient
                        );
                        send_message_with_length(writer.clone(), error_message).await?;
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "[{}] Failed to fetch onchain identity for {}: {}",
                    session_id, msg_recipient, e
                );
                let error_message = format!(
                    "[{}] Recipient {} not connected and failed to fetch onchain identity",
                    session_id, msg_recipient
                );
                send_message_with_length(writer.clone(), error_message).await?;
            }
        }
        Ok(())
    }

    async fn modify_shinkai_message_proxied_localhost_to_external(
        message: ShinkaiMessage,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
        subidentity: String,
        session_id: Uuid,
    ) -> Result<ShinkaiMessage, NetworkMessageError> {
        let mut modified_message = message;
        if modified_message.is_body_currently_encrypted() {
            if !modified_message.external_metadata.other.is_empty() {
                let intra_sender = modified_message.external_metadata.other.clone();
                let sender_encryption_pk = string_to_encryption_public_key(&intra_sender)?;

                // Attempt to decrypt the message
                modified_message = modified_message
                    .decrypt_outer_layer(&encryption_secret_key, &sender_encryption_pk)
                    .map_err(|e| NetworkMessageError::EncryptionError(format!("Failed to decrypt message: {}", e)))?;
            } else {
                eprintln!(
                    "[{}] Error: No intra_sender found for the recipient. Identity: {:?}",
                    session_id, subidentity
                );
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
        _encryption_secret_key: EncryptionStaticKey,
        _sender_encryption_pk: EncryptionPublicKey,
        _subidentity: String,
    ) -> Result<ShinkaiMessage, NetworkMessageError> {
        let mut modified_message = message;
        modified_message.external_metadata.recipient = "@@locahost.sep-shinkai".to_string();
        modified_message.external_metadata.intra_sender = "".to_string();
        modified_message.body = match modified_message.body {
            MessageBody::Unencrypted(mut body) => {
                let sender_identity = ShinkaiName::from_node_and_profile_names(
                    modified_message.external_metadata.sender.clone(),
                    body.internal_metadata.sender_subidentity,
                )?;
                modified_message.external_metadata.other = sender_identity.to_string();

                body.internal_metadata.recipient_subidentity = "main".to_string(); // TODO: eventually update this to be flexible
                body.internal_metadata.sender_subidentity = "".to_string();
                MessageBody::Unencrypted(body)
            }
            encrypted => encrypted,
        };
        modified_message.external_metadata.sender = node_name.to_string();

        // Re-sign the inner layer
        modified_message.sign_inner_layer(&identity_secret_key)?;

        // TODO: re-encrypt the outer layer using the target's pk

        // Re-sign the outer layer
        let signed_message = modified_message.sign_outer_layer(&identity_secret_key)?;

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
        let mut cursor = std::io::Cursor::new(buffer);

        let _public_key = Self::read_public_key_from_cursor(&mut cursor).await?;
        let signature = Self::read_signature_from_cursor(&mut cursor).await?;

        // eprintln!("Received response: {}", signature);
        let onchain_identity = self.registry.get_identity_record(identity.to_string(), None).await;
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
    let message_len = message.len() as u32;
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
