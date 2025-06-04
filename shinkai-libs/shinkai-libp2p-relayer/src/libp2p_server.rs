use derivative::Derivative;
use ed25519_dalek::{SigningKey, VerifyingKey};
use libp2p::PeerId;
use shinkai_crypto_identities::ShinkaiRegistry;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_utils::{
        encryption::encryption_public_key_to_string,
        signatures::signature_public_key_to_string,
    },
};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Semaphore};
use uuid::Uuid;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{LibP2PRelayError, RelayManager, RelayMessage};

pub type LibP2PClients = Arc<Mutex<HashMap<String, PeerId>>>; // identity -> peer_id
pub type LibP2PPKtoIdentity = Arc<Mutex<HashMap<String, String>>>; // public_key_hex -> identity

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct LibP2PProxy {
    pub clients: LibP2PClients,
    pub pk_to_clients: LibP2PPKtoIdentity,
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
    #[derivative(Debug = "ignore")]
    pub relay_manager: Arc<Mutex<RelayManager>>,
    pub message_sender: mpsc::UnboundedSender<RelayMessage>,
}

impl LibP2PProxy {
    pub async fn new(
        identity_secret_key: Option<SigningKey>,
        encryption_secret_key: Option<EncryptionStaticKey>,
        node_name: Option<String>,
        rpc_url: Option<String>,
        contract_address: Option<String>,
        max_connections: Option<usize>,
        listen_port: Option<u16>,
    ) -> Result<Self, LibP2PRelayError> {
        let rpc_url = rpc_url
            .or_else(|| env::var("RPC_URL").ok())
            .unwrap_or("https://sepolia.base.org".to_string());
        let contract_address = contract_address
            .or_else(|| env::var("CONTRACT_ADDRESS").ok())
            .unwrap_or("0x425fb20ba3874e887336aaa7f3fab32d08135ba9".to_string());
        let max_connections = max_connections
            .or_else(|| env::var("MAX_CONNECTIONS").ok().and_then(|s| s.parse().ok()))
            .unwrap_or(20);
        let listen_port = listen_port.unwrap_or(8080);

        let registry = ShinkaiRegistry::new(&rpc_url, &contract_address, None)
            .await
            .map_err(|e| LibP2PRelayError::RegistryError(format!("Failed to initialize registry: {}", e)))?;

        let identity_secret_key = identity_secret_key
            .or_else(|| {
                let key = env::var("IDENTITY_SECRET_KEY").ok()?;
                let key_bytes: [u8; 32] = hex::decode(key).ok()?.try_into().ok()?;
                Some(SigningKey::from_bytes(&key_bytes))
            })
            .ok_or_else(|| LibP2PRelayError::ConfigurationError("IDENTITY_SECRET_KEY required".to_string()))?;

        let encryption_secret_key = encryption_secret_key
            .or_else(|| {
                let key = env::var("ENCRYPTION_SECRET_KEY").ok()?;
                shinkai_message_primitives::shinkai_utils::encryption::string_to_encryption_static_key(&key).ok()
            })
            .ok_or_else(|| LibP2PRelayError::ConfigurationError("ENCRYPTION_SECRET_KEY required".to_string()))?;

        let node_name = node_name
            .or_else(|| env::var("NODE_NAME").ok())
            .ok_or_else(|| LibP2PRelayError::ConfigurationError("NODE_NAME required".to_string()))?;

        let identity_public_key = identity_secret_key.verifying_key();
        let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);
        let node_name = ShinkaiName::new(node_name)
            .map_err(|e| LibP2PRelayError::ConfigurationError(format!("Invalid node name: {}", e)))?;

        // Print the public keys
        println!(
            "LibP2P Relay Encryption Public Key: {:?}",
            encryption_public_key_to_string(encryption_public_key)
        );
        println!(
            "LibP2P Relay Signature Public Key: {:?}",
            signature_public_key_to_string(identity_public_key)
        );

        // Fetch the public keys from the registry and validate
        let registry_identity = registry
            .get_identity_record(node_name.to_string(), None)
            .await
            .map_err(|e| LibP2PRelayError::RegistryError(format!("Failed to get identity from registry: {}", e)))?;

        let registry_identity_public_key = registry_identity
            .signature_verifying_key()
            .map_err(|e| LibP2PRelayError::RegistryError(format!("Failed to get signature key from registry: {}", e)))?;
        let registry_encryption_public_key = registry_identity
            .encryption_public_key()
            .map_err(|e| LibP2PRelayError::RegistryError(format!("Failed to get encryption key from registry: {}", e)))?;

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
            return Err(LibP2PRelayError::AuthenticationFailed(
                "Identity public key does not match the registry".to_string(),
            ));
        }

        if encryption_public_key != registry_encryption_public_key {
            return Err(LibP2PRelayError::AuthenticationFailed(
                "Encryption public key does not match the registry".to_string(),
            ));
        }

        // Initialize the relay manager
        let relay_manager = RelayManager::new(listen_port, node_name.to_string(), identity_secret_key.clone()).await?;
        let message_sender = relay_manager.get_message_sender();
        let relay_manager = Arc::new(Mutex::new(relay_manager));

        Ok(LibP2PProxy {
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
            relay_manager,
            message_sender,
        })
    }

    /// Start the libp2p relay server
    pub async fn start(&self) -> Result<(), LibP2PRelayError> {
        println!("Starting LibP2P Relay Server...");
        
        // Clone the relay manager for the background task
        let relay_manager = self.relay_manager.clone();
        
        // Spawn the relay manager task
        let relay_task = tokio::spawn(async move {
            let mut manager = relay_manager.lock().await;
            if let Err(e) = manager.run().await {
                eprintln!("Relay manager error: {}", e);
            }
        });

        // Wait for the relay task to complete (it runs indefinitely)
        if let Err(e) = relay_task.await {
            eprintln!("Relay task failed: {}", e);
            return Err(LibP2PRelayError::LibP2PError(format!("Relay task failed: {}", e)));
        }

        Ok(())
    }

    /// Handle peer registration
    pub async fn register_peer(&self, identity: String, peer_id: PeerId, public_key_hex: String) -> Result<(), LibP2PRelayError> {
        let session_id = Uuid::new_v4();
        println!("[{}] Registering peer: {} with PeerId: {}", session_id, identity, peer_id);

        // Validate the peer identity if needed
        // For now, we'll implement basic validation

        // Store the peer mapping
        {
            let mut clients = self.clients.lock().await;
            clients.insert(identity.clone(), peer_id);
        }
        {
            let mut pk_to_clients = self.pk_to_clients.lock().await;
            pk_to_clients.insert(public_key_hex, identity.clone());
        }

        // Register with the relay manager
        {
            let mut relay_manager = self.relay_manager.lock().await;
            relay_manager.register_peer(identity.clone(), peer_id);
        }

        println!("[{}] Successfully registered peer: {}", session_id, identity);
        Ok(())
    }

    /// Handle peer unregistration
    pub async fn unregister_peer(&self, peer_id: &PeerId) -> Result<(), LibP2PRelayError> {
        // Find and remove the peer from our mappings
        let identity = {
            let clients = self.clients.lock().await;
            clients.iter()
                .find_map(|(identity, id)| if id == peer_id { Some(identity.clone()) } else { None })
        };

        if let Some(identity) = identity {
            // Remove from clients mapping
            {
                let mut clients = self.clients.lock().await;
                clients.remove(&identity);
            }

            // Remove from pk_to_clients mapping
            {
                let mut pk_to_clients = self.pk_to_clients.lock().await;
                if let Some(public_key_hex) = pk_to_clients.iter()
                    .find_map(|(k, v)| if v == &identity { Some(k.clone()) } else { None }) {
                    pk_to_clients.remove(&public_key_hex);
                }
            }

            // Unregister from relay manager
            {
                let mut relay_manager = self.relay_manager.lock().await;
                relay_manager.unregister_peer(peer_id);
            }

            println!("Unregistered peer: {} with PeerId: {}", identity, peer_id);
        }

        Ok(())
    }

    /// Send a message through the relay
    pub async fn send_message(&self, message: RelayMessage) -> Result<(), LibP2PRelayError> {
        self.message_sender.send(message)
            .map_err(|e| LibP2PRelayError::MessageDeliveryFailed(format!("Failed to send message: {}", e)))?;
        Ok(())
    }

    /// Get relay statistics
    pub async fn get_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        
        let clients_count = {
            let clients = self.clients.lock().await;
            clients.len()
        };

        let peer_id = {
            let relay_manager = self.relay_manager.lock().await;
            relay_manager.local_peer_id().to_string()
        };

        stats.insert("connected_peers".to_string(), serde_json::Value::Number(clients_count.into()));
        stats.insert("max_connections".to_string(), serde_json::Value::Number(self.max_connections.into()));
        stats.insert("available_permits".to_string(), serde_json::Value::Number(self.connection_semaphore.available_permits().into()));
        stats.insert("relay_peer_id".to_string(), serde_json::Value::String(peer_id));
        stats.insert("node_name".to_string(), serde_json::Value::String(self.node_name.to_string()));

        stats
    }

    /// List connected peers
    pub async fn list_peers(&self) -> Vec<String> {
        let clients = self.clients.lock().await;
        clients.keys().cloned().collect()
    }
} 