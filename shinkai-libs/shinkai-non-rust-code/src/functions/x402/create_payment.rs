use serde::{Deserialize, Serialize};
use serde_json::json;
use shinkai_message_primitives::schemas::x402_types::PaymentRequirements;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub accepts: Vec<PaymentRequirements>,
    pub x402_version: u32,
    // Atm it just support signer wallet
    pub private_key: String,
}

#[derive(Debug, Deserialize)]
pub struct Output {
    pub payment: String,
}

pub async fn create_payment(input: Input) -> Result<Output, RunError> {
    let code = include_str!("createPaymentDenoImpl.ts");
    let runner = NonRustCodeRunnerFactory::new("create_payment", code, vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
    runner.run::<_, Output>(input).await
}

#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::x402_types::Network;

    use super::*;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    #[tokio::test]
    async fn test_create_payment() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let price_in_raw_usd = 0.001;
        let input = Input {
            accepts: vec![PaymentRequirements {
                scheme: "exact".to_string(),
                description: "Test payment".to_string(),
                network: Network::BaseSepolia,
                max_amount_required: (price_in_raw_usd * 1000000.0).to_string(),
                resource: "https://shinkai.com".to_string(),
                mime_type: "".to_string(),
                pay_to: std::env::var("X402_PAY_TO").expect("X402_PAY_TO must be set"),
                max_timeout_seconds: 300,
                asset: "0x6e7907fbcEe166bd4000a22e0eBaA63B2c977534".to_string(),
                output_schema: Some(json!({})),
                extra: Some(json!({})),
            }],
            x402_version: 1,
            private_key: std::env::var("X402_PRIVATE_KEY").expect("X402_PRIVATE_KEY must be set"),
        };

        let output = create_payment(input).await.unwrap();
        assert!(!output.payment.is_empty());
    }
}
