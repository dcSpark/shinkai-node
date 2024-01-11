#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ethers::types::U256;
    use shinkai_node::crypto_identities::shinkai_registry::{OnchainIdentity, ShinkaiRegistry};
    use tokio::{runtime::Runtime, time::sleep};

    #[test]
    fn test_get_identity_record() {
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
                shinkai_identity: "nico.shinkai".to_string(),
                bound_nft: U256::from_dec_str("11").unwrap(),
                staked_tokens: U256::from_dec_str("62000000000000000000").unwrap(),
                encryption_key: "858bef3bb7839329e28e569288f441f8fa86af00d9f41a9845ef50dd3b6cd15f".to_string(),
                signature_key: "7aa221ec6761fdfdb478616babad8fad5330587392ad7e7dc9002af269909882".to_string(),
                routing: false,
                address_or_proxy_nodes: vec![],
                delegated_tokens: U256::from_dec_str("0").unwrap(),
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
