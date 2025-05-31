use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

use shinkai_message_primitives::schemas::x402_types::{FacilitatorConfig, PaymentPayload, PaymentRequirements};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub payment: PaymentPayload,
    pub accepts: Vec<PaymentRequirements>,
    pub facilitator: FacilitatorConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidOutput {
    pub error: String,
    pub accepts: Vec<PaymentRequirements>,
    pub x402_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidOutput {
    pub payment_response: String,
}

#[derive(Debug, Deserialize)]
pub struct Output {
    pub invalid: Option<InvalidOutput>,
    pub valid: Option<ValidOutput>,
}

pub async fn settle_payment(input: Input) -> Result<Output, RunError> {
    let code = include_str!("settlePaymentDenoImpl.ts");
    let runner = NonRustCodeRunnerFactory::new("settle_payment", code, vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
    runner.run::<_, Output>(input, None).await
}

#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::x402_types::{Network, Price};

    use super::*;
    use crate::{
        functions::x402::{create_payment, verify_payment}, test_utils::testing_create_tempdir_and_set_env_var
    };

    #[tokio::test]
    async fn test_settle_payment() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let price_in_raw_usd = 0.1;

        let pay_to = std::env::var("X402_PAY_TO").expect("X402_PAY_TO must be set");
        let private_key = std::env::var("X402_PRIVATE_KEY").expect("X402_PRIVATE_KEY must be set");

        // First verify with no payment to get accepts
        let verify_input = verify_payment::Input {
            price: Price::Money(price_in_raw_usd),
            network: Network::BaseSepolia,
            pay_to: pay_to.clone(),
            payment: None,
            x402_version: 1,
            facilitator: FacilitatorConfig::default(),
        };

        let verify_output = verify_payment::verify_payment(verify_input.clone()).await.unwrap();
        assert!(verify_output.invalid.is_some());
        let invalid_verify = verify_output.invalid.unwrap();

        // Create payment using the accepts from verify
        let create_input = create_payment::Input {
            accepts: invalid_verify.accepts.clone(),
            x402_version: invalid_verify.x402_version,
            private_key,
        };

        let payment = create_payment::create_payment(create_input).await.unwrap().payment;
        assert!(!payment.is_empty());

        // Verify the created payment
        let mut verify_input = verify_input.clone();
        verify_input.payment = Some(payment.clone());

        let verify_output = verify_payment::verify_payment(verify_input).await.unwrap();

        // Check for insufficient funds error
        if let Some(invalid) = &verify_output.invalid {
            if invalid.error == "Invalid payment - insufficient_funds" {
                let payment_req = &invalid.accepts[0];
                panic!(
                    "Insufficient funds error detected: Required {} {} on {:?} network",
                    payment_req.max_amount_required, payment_req.asset, payment_req.network
                );
            }
        }

        assert!(verify_output.valid.is_some());
        let decoded_payment = verify_output.valid.unwrap().decoded_payment;

        // Finally settle the payment
        let settle_input = Input {
            payment: decoded_payment,
            accepts: invalid_verify.accepts,
            facilitator: FacilitatorConfig::default(),
        };

        let settle_output = settle_payment(settle_input).await.unwrap();
        assert!(settle_output.valid.is_some());
        assert!(!settle_output.valid.unwrap().payment_response.is_empty());
    }
}
