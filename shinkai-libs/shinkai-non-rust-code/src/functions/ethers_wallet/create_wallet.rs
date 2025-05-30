use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedWallet {
    pub private_key: String,
    pub public_key: String,
    pub address: String,
    pub mnemonic: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub wallet: CreatedWallet,
}

pub async fn create_wallet(input: Input) -> Result<Output, RunError> {
    let code = include_str!("createWalletDenoImpl.ts");
    let runner = NonRustCodeRunnerFactory::new("create_wallet", code, vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
    runner.run::<_, Output>(input).await
}

#[cfg(test)]
mod tests {
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    use super::*;

    #[tokio::test]
    async fn test_create_wallet() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let input = Input {};
        let result = create_wallet(input).await.unwrap();

        // Check that wallet fields are present and in expected format
        assert!(!result.wallet.private_key.is_empty());
        assert!(!result.wallet.public_key.is_empty());
        assert!(!result.wallet.address.is_empty());

        // Check that address starts with "0x" and has correct length (42 chars = 0x + 40 hex chars)
        assert!(result.wallet.address.starts_with("0x"));
        assert_eq!(result.wallet.address.len(), 42);

        // Check that private key starts with "0x" and has correct length (66 chars = 0x + 64 hex chars)
        assert!(result.wallet.private_key.starts_with("0x"));
        assert_eq!(result.wallet.private_key.len(), 66);

        // Check that mnemonic is present and is a string
        assert!(result.wallet.mnemonic.is_some());
        assert!(!result.wallet.mnemonic.unwrap().is_empty());
    }
}
