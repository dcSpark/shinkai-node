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
use tokio::sync::{ Mutex, Semaphore};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{LibP2PRelayError, RelayManager};

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
        let status_endpoint_url = env::var("STATUS_ENDPOINT_URL").ok();
        let relay_manager = RelayManager::new(listen_port, node_name.to_string(), identity_secret_key.clone(), encryption_secret_key.clone(), registry.clone(), status_endpoint_url).await?;
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
} 