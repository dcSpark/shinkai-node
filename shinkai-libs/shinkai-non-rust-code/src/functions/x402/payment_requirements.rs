use serde::Deserialize;
use serde_json::json;

use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};

use super::types::PaymentRequirements;
use super::verify_payment::Input;

pub type PaymentRequirementsInput = Input;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirementsOutput {
    pub payment_requirements: Vec<PaymentRequirements>,
}

pub async fn get_payment_requirements(input: PaymentRequirementsInput) -> Result<PaymentRequirementsOutput, RunError> {
    let code = include_str!("paymentRequirementsDenoImpl.ts");
    let runner = NonRustCodeRunnerFactory::new("payment_requirements", code, vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(json!({}));
    runner.run::<_, PaymentRequirementsOutput>(input).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        functions::x402::types::{FacilitatorConfig, Network, Price}, test_utils::testing_create_tempdir_and_set_env_var
    };

    #[tokio::test]
    async fn test_payment_requirements() {
        let _dir = testing_create_tempdir_and_set_env_var();

        let price_in_raw_usd = 0.001;
        let input = PaymentRequirementsInput {
            price: Price::Money(price_in_raw_usd),
            network: Network::BaseSepolia,
            pay_to: std::env::var("X402_PAY_TO").expect("X402_PAY_TO must be set"),
            payment: None,
            x402_version: 1,
            facilitator: FacilitatorConfig::default(),
        };

        let output = get_payment_requirements(input).await.unwrap();
        assert!(!output.payment_requirements.is_empty());

        let requirements = &output.payment_requirements[0];
        assert_eq!(
            requirements.max_amount_required,
            (price_in_raw_usd * 1000000.0).to_string()
        );
        assert_eq!(requirements.scheme, "exact");
        assert_eq!(requirements.network, Network::BaseSepolia);
    }
}
