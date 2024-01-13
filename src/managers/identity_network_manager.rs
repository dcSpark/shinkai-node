use shinkai_message_primitives::shinkai_utils::{
    signatures::{signature_public_key_to_string, string_to_signature_public_key},
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use x25519_dalek::PublicKey as EncryptionPublicKey;

use crate::crypto_identities::shinkai_registry::{OnchainIdentity, ShinkaiRegistry};

pub struct IdentityNetworkManager {
    registry: Arc<Mutex<ShinkaiRegistry>>,
    identities: Arc<Mutex<HashMap<String, OnchainIdentity>>>,
}

impl IdentityNetworkManager {
    pub async fn new() -> Self {
        // TODO: read from config
        let registry = ShinkaiRegistry::new(
            "https://rpc.sepolia.org",
            "0x6964241D2458f0Fd300BB37535CF0145380810E0",
            "./src/crypto_identities/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json",
        )
        .await
        .unwrap();

        let registry = Arc::new(Mutex::new(registry));
        let identities = Arc::new(Mutex::new(HashMap::new()));
        
        IdentityNetworkManager { registry, identities }
    }

    pub async fn external_identity_to_profile_data(
        &self,
        global_identity: String,
    ) -> Result<OnchainIdentity, &'static str> {
        eprintln!("Getting identity record for {}", global_identity);
        let record = {
            let identity = global_identity.trim_start_matches("@@");
            let mut registry = self.registry.lock().await;
            match registry.get_identity_record(identity.to_string()).await {
                Ok(record) => record,
                Err(_) => return Err("Unrecognized global identity"),
            }
        };
    
        Ok(record)
    }
}
