#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::{DateTime, Utc};
    use ethers::types::U256;
    use shinkai_crypto_identities::{OnchainIdentity, ShinkaiRegistry};
    use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
    use tokio::{runtime::Runtime, time::sleep};

    #[test]
    fn test_get_identity_record() {
        init_default_tracing();
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut registry = ShinkaiRegistry::new(
                "https://rpc.sepolia.org",
                "0xDCbBd3364a98E2078e8238508255dD4a2015DD3E",
                None, // "./src/crypto_identities/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json",
            )
            .await
            .unwrap();

            let identity = "node1_test.sepolia-shinkai".to_string();

            let record = registry.get_identity_record(identity.clone()).await.unwrap();

            let expected_record = OnchainIdentity {
                shinkai_identity: "node1_test.sepolia-shinkai".to_string(),
                bound_nft: U256::from_dec_str("6").unwrap(),
                staked_tokens: U256::from_dec_str("50000000000000000000").unwrap(),
                encryption_key: "60045bdb15c24b161625cf05558078208698272bfe113f792ea740dbd79f4708".to_string(),
                signature_key: "69fa099bdce516bfeb46d5fc6e908f6cf8ffac0aba76ca0346a7b1a751a2712e".to_string(),
                routing: false,
                address_or_proxy_nodes: vec!["127.0.0.1:8080".to_string()],
                delegated_tokens: U256::from_dec_str("2000000000000000000").unwrap(),
                last_updated: DateTime::<Utc>::from(
                    std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1708468692),
                ),
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

        rt.shutdown_background();
    }
}
