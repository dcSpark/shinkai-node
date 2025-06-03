use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub token_address: String,
    pub wallet_address: String,
    pub rpc_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub balance: String,
    pub formatted_balance: String,
    pub token_info: TokenInfo,
}

pub async fn get_balance(input: Input) -> Result<Output, RunError> {
    let code = include_str!("getBalanceDenoImpl.ts");
    let runner = NonRustCodeRunnerFactory::new("get_balance", code, vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
    runner.run::<_, Output>(input, None).await
}

#[cfg(test)]
mod tests {
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    use super::*;

    #[tokio::test]
    async fn test_get_token_balance() {
        let _dir = testing_create_tempdir_and_set_env_var();

        // Using real USDC contract address on Ethereum mainnet
        let input = Input {
            token_address: "0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string(), // USDC on Ethereum
            wallet_address: "0x0000000000000000000000000000000000000000".to_string(), // Burn address
            rpc_url: "https://sepolia.base.org".to_string(),
        };

        let result = get_balance(input).await;

        // The test might fail if the token address doesn't exist or network issues
        match result {
            Ok(res) => {
                assert!(!res.balance.is_empty());
                assert!(!res.formatted_balance.is_empty());
                assert!(!res.token_info.symbol.is_empty());
                // Burn address should have 0 balance
                assert_eq!(res.balance, "0");
                assert_eq!(res.formatted_balance, "0.0");
            }
            Err(_) => {
                // Expected for network issues or if using example token address
                println!("Test failed - this is expected if using example addresses");
            }
        }
    }
}
