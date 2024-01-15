#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::{DateTime, Utc};
    use ethers::types::U256;
    use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_tracing;
    use shinkai_node::crypto_identities::shinkai_registry::{OnchainIdentity, ShinkaiRegistry};
    use tokio::{runtime::Runtime, time::sleep};

    #[test]
    fn test_get_identity_record() {
        init_tracing(); 
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut registry = ShinkaiRegistry::new(
                "https://rpc.sepolia.org",
                "0x6964241D2458f0Fd300BB37535CF0145380810E0",
                "./src/crypto_identities/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json",
            )
            .await
            .unwrap();

            let identity = "nico.shinkai".to_string();

            let record = registry.get_identity_record(identity.clone()).await.unwrap();

            let expected_record = OnchainIdentity {
                shinkai_identity: "nico2.shinkai".to_string(),
                bound_nft: U256::from_dec_str("22").unwrap(),
                staked_tokens: U256::from_dec_str("40000000000000000000").unwrap(),
                encryption_key: "12bb5823b96886941da4261219735e10cac53783c9a23f5fa31bacc8a1e68019".to_string(),
                signature_key: "bdcd4569d4e01cafe0543babfbaf35766feead28ce81932a41dbde8d7da8d720".to_string(),
                routing: false,
                address_or_proxy_nodes: vec!["139.49.219.177:9550".to_string()],
                delegated_tokens: U256::from_dec_str("0").unwrap(),
                last_updated: DateTime::<Utc>::from(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1704927408)),
            };
            assert_eq!(record, expected_record);

            let initial_cache_time = registry.get_cache_time(&identity).unwrap();

            // Request the identity record again to trigger a cache update
            let _ = registry.get_identity_record(identity.clone()).await.unwrap();

            // Check every 500 ms for up to 5 seconds to see if the cache time has updated
            for _ in 0..10 {
                sleep(Duration::from_millis(500)).await;
                if let Some(cache_time) = registry.get_cache_time(&identity) {
                    if cache_time != initial_cache_time {
                        return;
                    }
                }
            }

            panic!("Cache time did not update within 5 seconds");
        });
    }
}
