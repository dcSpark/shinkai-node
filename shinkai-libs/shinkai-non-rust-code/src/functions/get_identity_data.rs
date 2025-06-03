use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

#[derive(Debug, Serialize)]
pub struct Configurations {
    rpc_urls: Vec<String>,
    contract_address: String,
    contract_abi: String,
    timeout_rpc_request_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct Input {
    #[serde(rename = "identityId")]
    identity_id: String,
}

#[derive(Debug, Deserialize)]
pub struct IdentityData {
    #[serde(rename = "boundNft")]
    pub bound_nft: String,
    #[serde(rename = "stakedTokens")]
    pub staked_tokens: String,
    #[serde(rename = "encryptionKey")]
    pub encryption_key: String,
    #[serde(rename = "signatureKey")]
    pub signature_key: String,
    #[serde(rename = "routing")]
    pub routing: bool,
    #[serde(rename = "addressOrProxyNodes")]
    pub address_or_proxy_nodes: Vec<String>,
    #[serde(rename = "delegatedTokens")]
    pub delegated_tokens: String,
    #[serde(rename = "lastUpdated")]
    pub last_updated: u64,
}

#[derive(Debug, Deserialize)]
pub struct Output {
    #[serde(rename = "identityData")]
    pub identity_data: Option<IdentityData>,
}

pub async fn get_identity_data(
    rpc_urls: Vec<String>,
    contract_address: String,
    contract_abi: String,
    identity_id: String,
) -> Result<Output, RunError> {
    let code = include_str!("getIdentityDataImpl.ts");

    let per_rpc_timeout = Duration::from_secs(5);
    let configurations = Configurations {
        rpc_urls: rpc_urls.clone(),
        contract_address,
        contract_abi,
        timeout_rpc_request_ms: per_rpc_timeout.as_millis() as u64,
    };

    // The JsonRpcProvider has some issues https://github.com/ethers-io/ethers.js/issues/4377
    // and the are some casses where even with a real timeout on the network layer the node/deno process remains opened
    // so we need to set a custom timeout on top of the process
    let execution_timeout = Some(per_rpc_timeout * rpc_urls.len() as u32);
    let runner = NonRustCodeRunnerFactory::new("get_identity_data", code, vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(configurations);
    runner
        .run::<Input, Output>(Input { identity_id }, execution_timeout)
        .await
}

#[cfg(test)]
mod tests {
    use crate::functions::get_identity_data::get_identity_data;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    #[tokio::test]
    async fn test_get_identity_data() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let output = get_identity_data(
            vec![
                "https://sepolia.base.org".to_string(),
                "https://base-sepolia-rpc.publicnode.com".to_string(),
                "https://base-sepolia.gateway.tenderly.co".to_string(),
            ],
            "0x425Fb20ba3874e887336aAa7f3fab32D08135BA9".to_string(),
            include_str!("../../../shinkai-crypto-identities/src/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json")
                .to_string(),
            "official.sep-shinkai".to_string(),
        )
        .await
        .unwrap();
        println!("output: {:?}", output);

        assert!(output.identity_data.is_some());

        let identity_data = output.identity_data.unwrap();
        assert_eq!(identity_data.bound_nft, "4n");
        assert_eq!(
            identity_data.encryption_key,
            "9d89af22de24fcc621ed47a08e98f1c52fada3e49b98462cb02c48237940c85b"
        );
        assert_eq!(
            identity_data.signature_key,
            "1ffbfa5d90e7b79b395d034f81ec07ea0c7eabd6c9a510014173c6e5081411d1"
        );
        assert_eq!(identity_data.staked_tokens, "165000000000000000000n");
        assert_eq!(identity_data.delegated_tokens, "0n");
        assert!(identity_data.last_updated > 1715000000);
    }

    #[tokio::test]
    async fn test_get_identity_data_with_timeout() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let output = get_identity_data(
            vec![
                "https://sepolia.base.org".to_string(),
                "https://base-sepolia.blockpi.network/v1/rpc/public".to_string(),
                "https://base-sepolia-rpc.publicnode.com".to_string(),
            ],
            "0x425Fb20ba3874e887336aAa7f3fab32D08135BA9".to_string(),
            include_str!("../../../shinkai-crypto-identities/src/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json")
                .to_string(),
            "official.sep-shinkai".to_string(),
        )
        .await;
        println!("output: {:?}", output);
        assert!(output.is_ok());
    }

    #[tokio::test]
    async fn test_get_identity_data_hanging_forever() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let output = get_identity_data(
            vec![
                "https://api.shinkai.com".to_string(),
                "https://base-sepolia.blockpi.network/v1/rpc/public".to_string(),
                "https://sepolia.base.org".to_string(),
                "https://base-sepolia-rpc.publicnode.com".to_string(),
            ],
            "0x425Fb20ba3874e887336aAa7f3fab32D08135BA9".to_string(),
            include_str!("../../../shinkai-crypto-identities/src/abi/ShinkaiRegistry.sol/ShinkaiRegistry.json")
                .to_string(),
            "official.sep-shinkai".to_string(),
        )
        .await;
        println!("output: {:?}", output);
        assert!(output.is_ok());
    }
}
