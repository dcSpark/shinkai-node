use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

use super::types::{FacilitatorConfig, Network, PaymentPayload, PaymentRequirements, Price};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub price: Price,
    pub network: Network,
    // 0x... Address
    pub pay_to: String,
    pub payment: Option<String>,
    pub x402_version: u32,
    pub facilitator: FacilitatorConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidOutput {
    pub error: String,
    pub accepts: Vec<PaymentRequirements>,
    pub x402_version: u32,
    pub payer: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidOutput {
    pub decoded_payment: PaymentPayload,
    pub selected_payment_requirements: PaymentRequirements,
}

#[derive(Debug, Deserialize)]
pub struct Output {
    pub invalid: Option<InvalidOutput>,
    pub valid: Option<ValidOutput>,
}

pub async fn verify_payment(input: Input) -> Result<Output, RunError> {
    let code = include_str!("verifyPaymentDenoImpl.ts");
    let runner = NonRustCodeRunnerFactory::new("verify_payment", code, vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
    runner.run::<_, Output>(input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    #[tokio::test]
    async fn test_verify_payment() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let price_in_raw_usd = 0.001;
        let input = Input {
            price: Price::Money(price_in_raw_usd),
            network: Network::BaseSepolia,
            // This is Shinkai Faucet address
            pay_to: std::env::var("X402_PAY_TO").expect("X402_PAY_TO must be set"),
            x402_version: 1,
            facilitator: FacilitatorConfig::default(),
            payment: None,
        };

        let output = verify_payment(input).await.unwrap();
        println!("{:?}", output);
        assert!(output.valid.is_none());
        assert!(output.invalid.is_some());
        assert_eq!(
            output.invalid.unwrap().accepts.first().unwrap().max_amount_required,
            (price_in_raw_usd * 1000000.0).to_string()
        );
    }
}
