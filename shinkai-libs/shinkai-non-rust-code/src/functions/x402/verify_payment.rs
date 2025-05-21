use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

use super::types::{FacilitatorConfig, Network, PaymentPayload, PaymentRequirements, Price};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub payment: Option<String>, // Keep
    pub payment_requirements: Vec<PaymentRequirements>, // Keep & ensure it's used
    pub content_id: String, // Keep (this is the resource identifier)
    pub buyer_id: Option<String>, // Keep
    pub seller_id: String, // Keep
    pub expected_seller_id: Option<String>, // Keep
    pub facilitator_config: Option<FacilitatorConfig>, // Keep, ensure Deno side uses it
    // REMOVE: price: Price,
    // REMOVE: network: Network,
    // REMOVE: pay_to: String,
    // Keep: x402_version: u32 (already in PaymentPayload, but verify_payment might need it if payment is None)
                       // Let's assume x402_version is still needed at this level for when payment is None.
                       // The Deno script uses parameters.x402Version when payment is None.
    pub x402_version: u32,
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
        let test_payment_requirements = PaymentRequirements {
            id: "req_123".to_string(),
            prices: vec![Price::Money(price_in_raw_usd)],
            accepts_test_payments: Some(true),
            resource_data: None,
            asset: None, 
            extra: None,
        };

        let input = Input {
            payment: None,
            payment_requirements: vec![test_payment_requirements.clone()],
            content_id: "test_content_id".to_string(),
            buyer_id: Some("test_buyer_id".to_string()),
            seller_id: "test_seller_id".to_string(),
            expected_seller_id: Some("test_seller_id".to_string()),
            facilitator_config: Some(FacilitatorConfig::default()),
            x402_version: 1,
        };

        let output = verify_payment(input).await.unwrap();
        println!("{:?}", output);
        assert!(output.valid.is_none());
        assert!(output.invalid.is_some());
        let invalid_output = output.invalid.unwrap();
        assert_eq!(invalid_output.accepts.len(), 1);
        assert_eq!(invalid_output.accepts.first().unwrap().id, test_payment_requirements.id);
        // Further assertions can be added based on how Deno script processes Money price
        // For example, if it converts Money to a specific token/amount, that can be checked.
        // The current Deno script seems to primarily focus on EVM prices for the `max_amount_required` field.
        // If Price::Money is passed, it might not populate `max_amount_required` in the same way.
        // Let's check if the price is passed through:
        assert_eq!(invalid_output.accepts.first().unwrap().prices.first().unwrap(), &Price::Money(price_in_raw_usd));
    }
}
