use shinkai_crypto_identities::{OnchainIdentity, ShinkaiRegistry};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::{env, sync::Arc};
use tokio::sync::Mutex;

pub struct IdentityNetworkManager {
    registry: Arc<Mutex<ShinkaiRegistry>>,
}

impl IdentityNetworkManager {
    pub async fn new() -> Self {
        // TODO: Update with mainnet values (eventually)
        let rpc_url = env::var("RPC_URL").unwrap_or("https://sepolia.base.org".to_string());
        let contract_address =
            env::var("CONTRACT_ADDRESS").unwrap_or("0x425fb20ba3874e887336aaa7f3fab32d08135ba9".to_string());
        let abi_path = env::var("ABI_PATH").ok();
        shinkai_log(
            ShinkaiLogOption::IdentityNetwork,
            ShinkaiLogLevel::Info,
            &format!("Identity Network Manager initialized with ABI path: {:?}", abi_path),
        );

        let registry = ShinkaiRegistry::new(&rpc_url, &contract_address, abi_path)
            .await
            .unwrap();

        let registry = Arc::new(Mutex::new(registry));

        IdentityNetworkManager { registry }
    }

    pub async fn external_identity_to_profile_data(
        &self,
        global_identity: String,
        force_refresh: Option<bool>,
    ) -> Result<OnchainIdentity, &'static str> {
        let record = {
            let identity = global_identity.trim_start_matches("@@");
            let registry = self.registry.lock().await;
            match registry.get_identity_record(identity.to_string(), force_refresh).await {
                Ok(record) => record,
                Err(_) => return Err("Unrecognized global identity"),
            }
        };

        // Check if any of the address_or_proxy_nodes ends with .sepolia-shinkai
        if record.address_or_proxy_nodes.iter().any(|node| {
            let node_base = node.split(':').next().unwrap_or(node);
            node_base.ends_with(".sepolia-shinkai")
                || node_base.ends_with(".shinkai")
                || node_base.ends_with(".sep-shinkai")
        }) {
            // Call the proxy node to get the actual data
            let proxy_identity = record.address_or_proxy_nodes.clone();
            let proxy_record = {
                let registry = self.registry.lock().await;
                match registry
                    .get_identity_record(proxy_identity.join(","), force_refresh)
                    .await
                {
                    Ok(record) => record,
                    Err(_) => return Err("Failed to fetch proxy node data"),
                }
            };

            // Return the same record but with the updated address_or_proxy_nodes field
            let updated_record = OnchainIdentity {
                address_or_proxy_nodes: proxy_record.address_or_proxy_nodes,
                ..record
            };
            eprintln!(
                "external_identity_to_profile_data> Found record with proxy: {:?}",
                updated_record
            );

            return Ok(updated_record);
        }

        eprintln!("external_identity_to_profile_data> Found record: {:?}", record);
        Ok(record)
    }
}
