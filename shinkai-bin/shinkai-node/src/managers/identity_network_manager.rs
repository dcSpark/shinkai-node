use shinkai_crypto_identities::{OnchainIdentity, ShinkaiRegistry};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::{collections::HashMap, env, sync::Arc};
use tokio::sync::Mutex;

pub struct IdentityNetworkManager {
    registry: Option<Arc<Mutex<ShinkaiRegistry>>>,
    records: HashMap<String, OnchainIdentity>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn injected_records_are_authoritative_even_when_refresh_is_requested() {
        let record = OnchainIdentity {
            shinkai_identity: "local-fixture.sep-shinkai".into(),
            bound_nft: "0".into(),
            staked_tokens: "0".into(),
            encryption_key: String::new(),
            signature_key: String::new(),
            routing: false,
            address_or_proxy_nodes: vec!["127.0.0.1:1234".into()],
            delegated_tokens: "0".into(),
            last_updated: chrono::Utc::now(),
        };
        let manager = IdentityNetworkManager::from_records(vec![record.clone()]);
        assert_eq!(
            manager
                .external_identity_to_profile_data("@@local-fixture.sep-shinkai".into(), Some(true))
                .await
                .unwrap(),
            record
        );
        assert!(manager
            .external_identity_to_profile_data("@@not-in-fixture.sep-shinkai".into(), Some(true))
            .await
            .is_err());
    }
}

impl IdentityNetworkManager {
    /// Use authoritative records without an RPC fallback, including forced refreshes.
    pub fn from_records(records: Vec<OnchainIdentity>) -> Self {
        Self {
            registry: None,
            records: records
                .into_iter()
                .map(|record| (record.shinkai_identity.trim_start_matches("@@").to_string(), record))
                .collect(),
        }
    }

    async fn get_record(&self, identity: &str, force_refresh: Option<bool>) -> Result<OnchainIdentity, &'static str> {
        let identity = identity.trim_start_matches("@@");
        match &self.registry {
            Some(registry) => registry
                .lock()
                .await
                .get_identity_record(identity.to_string(), force_refresh)
                .await
                .map_err(|_| "Unrecognized global identity"),
            None => self
                .records
                .get(identity)
                .cloned()
                .ok_or("Unrecognized global identity"),
        }
    }

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

        IdentityNetworkManager {
            registry: Some(registry),
            records: HashMap::new(),
        }
    }

    pub async fn external_identity_to_profile_data(
        &self,
        global_identity: String,
        force_refresh: Option<bool>,
    ) -> Result<OnchainIdentity, &'static str> {
        let record = self.get_record(&global_identity, force_refresh).await?;

        // Check if any of the address_or_proxy_nodes ends with .sepolia-shinkai
        if record.address_or_proxy_nodes.iter().any(|node| {
            let node_base = node.split(':').next().unwrap_or(node);
            node_base.ends_with(".sepolia-shinkai")
                || node_base.ends_with(".shinkai")
                || node_base.ends_with(".sep-shinkai")
        }) {
            // Call the proxy node to get the actual data
            let proxy_identity = record.address_or_proxy_nodes.clone();
            let proxy_record = self
                .get_record(&proxy_identity.join(","), force_refresh)
                .await
                .map_err(|_| "Failed to fetch proxy node data")?;

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
