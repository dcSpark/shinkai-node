use shinkai_message_primitives::shinkai_utils::{
    signatures::{signature_public_key_to_string, string_to_signature_public_key},
};
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use crate::crypto_identities::shinkai_registry::{OnchainIdentity, ShinkaiRegistry};

pub struct IdentityNetworkManager {
    registry: Arc<Mutex<ShinkaiRegistry>>,
}

impl IdentityNetworkManager {
    pub async fn new() -> Self {
        // TODO: Update with mainnet values (eventually)
        let rpc_url = env::var("RPC_URL").unwrap_or("https://rpc.sepolia.org".to_string());
        let contract_address = env::var("CONTRACT_ADDRESS").unwrap_or("0x6964241D2458f0Fd300BB37535CF0145380810E0".to_string());
        let abi_path = env::var("ABI_PATH").unwrap_or("./src/crypto_identities/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json".to_string());
        eprintln!("ABI Path: {}", abi_path);

        let registry = ShinkaiRegistry::new(
            &rpc_url,
            &contract_address,
            &abi_path,
        )
        .await
        .unwrap();

        let registry = Arc::new(Mutex::new(registry));
        
        IdentityNetworkManager { registry }
    }

    pub async fn external_identity_to_profile_data(
        &self,
        global_identity: String,
    ) -> Result<OnchainIdentity, &'static str> {
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
