
#[cfg(test)]
mod tests {
    use ethers::types::U256;
    use shinkai_node::crypto_identities::crypto_identity_manager::{ShinkaiRegistry, OnchainIdentity};
    use tokio::runtime::Runtime;

    #[test]
    fn test_get_identity_record() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let registry = ShinkaiRegistry::new(
                "https://rpc.sepolia.org",
                "0xb2945D0CDa4C119DE184380955aA4FbfAFb6B8cC",
                "./src/crypto_identities/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json",
            ).await.unwrap();

            let record = registry.get_identity_record("nico.shinkai".to_string()).await.unwrap();

            let expected_record = OnchainIdentity {
                bound_nft: U256::from_dec_str("11").unwrap(),
                staked_tokens: U256::from_dec_str("62000000000000000000").unwrap(),
                encryption_key: "858bef3bb7839329e28e569288f441f8fa86af00d9f41a9845ef50dd3b6cd15f".to_string(),
                signature_key: "7aa221ec6761fdfdb478616babad8fad5330587392ad7e7dc9002af269909882".to_string(),
                routing: false,
                address_or_proxy_nodes: vec![],
                delegated_tokens: U256::from_dec_str("0").unwrap(),
            };
            assert_eq!(record, expected_record);
        });
    }
}