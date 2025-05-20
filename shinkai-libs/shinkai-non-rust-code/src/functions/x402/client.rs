use crate::{NonRustCodeRunnerFactory, NonRustRuntime, RunError};
use serde::{Deserialize, Serialize};

use super::types::{FacilitatorConfig, PaymentPayload, PaymentRequirements, PaymentVerification, X402ClientData};

#[derive(Debug, Serialize)]
pub struct ClientConfig {
    pub facilitator_url: String,
    pub network: String,
}

#[derive(Debug, Serialize)]
pub struct VerifyPaymentInput {
    pub payment_header: String,
    pub request_path: String,
    pub request_method: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyPaymentOutput {
    pub verification: PaymentVerification,
}

#[derive(Debug, Deserialize)]
pub struct CreateClientOutput {
    pub client: X402ClientData,
}

pub async fn create_x402_client(config: FacilitatorConfig) -> Result<X402Client, RunError> {
    let code = r#"
            import { ethers } from 'npm:ethers';
            import { X402Client } from 'npm:@coinbase/x402/client';

            async function run(configurations, parameters) {
                try {
                    const x402Client = new X402Client({
                        facilitatorUrl: configurations.facilitator_url,
                        network: configurations.network,
                    });

                    return {
                        client: {
                            facilitator_url: configurations.facilitator_url,
                            network: configurations.network,
                            version: x402Client.version,
                        },
                    };
                } catch (error) {
                    throw new Error(`Failed to create X402 client: ${error.message}`);
                }
            }
            "#
    .to_string();

    let client_config = ClientConfig {
        facilitator_url: config.url.clone(),
        network: config.network.clone(),
    };

    let runner = NonRustCodeRunnerFactory::new("x402_client", code, vec![])
        .with_runtime(NonRustRuntime::Deno)
        .create_runner(client_config);

    let result = runner.run::<(), CreateClientOutput>(()).await?;

    Ok(X402Client {
        config,
        client_data: result.client,
    })
}

pub struct X402Client {
    config: FacilitatorConfig,
    client_data: X402ClientData,
}

impl X402Client {
    pub fn version(&self) -> &str {
        &self.client_data.version
    }

    pub fn network(&self) -> &str {
        &self.client_data.network
    }

    pub fn facilitator_url(&self) -> &str {
        &self.client_data.facilitator_url
    }

    pub async fn verify_payment(
        &self,
        payment_header: String,
        request_path: String,
        request_method: String,
    ) -> Result<PaymentVerification, RunError> {
        let code = r#"
            import { ethers } from 'npm:ethers';
            import { X402Client } from 'npm:@coinbase/x402/client';

            async function run(configurations, parameters) {
                try {
                    const x402Client = new X402Client({
                        facilitatorUrl: configurations.facilitator_url,
                        network: configurations.network,
                    });

                    const verification = await x402Client.verifyPayment({
                        paymentHeader: parameters.paymentHeader,
                        requestPath: parameters.requestPath,
                        requestMethod: parameters.requestMethod,
                    });

                    return {
                        verification: {
                            valid: verification.valid,
                            error: verification.error || null,
                            payer: verification.payer || null,
                        },
                    };
                } catch (error) {
                    return {
                        verification: {
                            valid: false,
                            error: error.message,
                            payer: null,
                        },
                    };
                }
            }
            "#
        .to_string();

        let runner = NonRustCodeRunnerFactory::new("x402_verify_payment", code, vec![])
            .with_runtime(NonRustRuntime::Deno)
            .create_runner(ClientConfig {
                facilitator_url: self.config.url.clone(),
                network: self.config.network.clone(),
            });

        let result = runner
            .run::<VerifyPaymentInput, VerifyPaymentOutput>(VerifyPaymentInput {
                payment_header,
                request_path,
                request_method,
            })
            .await?;

        Ok(result.verification)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::testing_create_tempdir_and_set_env_var;

    const TEST_FACILITATOR_URL: &str = "https://facilitator.example.com";
    const TEST_NETWORK: &str = "base-sepolia";
    const TEST_VERSION: &str = "1.0.0";

    fn create_test_config() -> FacilitatorConfig {
        FacilitatorConfig {
            url: TEST_FACILITATOR_URL.to_string(),
            network: TEST_NETWORK.to_string(),
        }
    }

    #[tokio::test]
    async fn test_create_x402_client() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let config = create_test_config();

        let client = create_x402_client(config.clone()).await.unwrap();

        assert_eq!(client.version(), TEST_VERSION);
        assert_eq!(client.network(), TEST_NETWORK);
        assert_eq!(client.facilitator_url(), TEST_FACILITATOR_URL);
    }

    #[tokio::test]
    async fn test_verify_payment_success() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let config = create_test_config();
        let client = create_x402_client(config).await.unwrap();

        let payment_header = "valid-payment-header";
        let request_path = "/api/resource";
        let request_method = "GET";

        let verification = client
            .verify_payment(
                payment_header.to_string(),
                request_path.to_string(),
                request_method.to_string(),
            )
            .await
            .unwrap();

        assert!(verification.valid);
        assert!(verification.error.is_none());
        assert!(verification.payer.is_some());
    }

    #[tokio::test]
    async fn test_verify_payment_failure() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let config = create_test_config();
        let client = create_x402_client(config).await.unwrap();

        let invalid_payment_header = "invalid-payment-header";
        let request_path = "/api/resource";
        let request_method = "GET";

        let verification = client
            .verify_payment(
                invalid_payment_header.to_string(),
                request_path.to_string(),
                request_method.to_string(),
            )
            .await
            .unwrap();

        assert!(!verification.valid);
        assert!(verification.error.is_some());
        assert!(verification.payer.is_none());
    }

    #[tokio::test]
    async fn test_verify_payment_network_mismatch() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let config = FacilitatorConfig {
            url: TEST_FACILITATOR_URL.to_string(),
            network: "ethereum-mainnet".to_string(), // Different network
        };
        let client = create_x402_client(config).await.unwrap();

        let payment_header = "valid-payment-header";
        let request_path = "/api/resource";
        let request_method = "GET";

        let verification = client
            .verify_payment(
                payment_header.to_string(),
                request_path.to_string(),
                request_method.to_string(),
            )
            .await
            .unwrap();

        assert!(!verification.valid);
        assert!(verification.error.is_some());
        assert!(verification.payer.is_none());
    }

    #[tokio::test]
    async fn test_verify_payment_invalid_facilitator() {
        let _dir = testing_create_tempdir_and_set_env_var();
        let config = FacilitatorConfig {
            url: "https://invalid-facilitator.example.com".to_string(),
            network: TEST_NETWORK.to_string(),
        };
        let client = create_x402_client(config).await.unwrap();

        let payment_header = "valid-payment-header";
        let request_path = "/api/resource";
        let request_method = "GET";

        let verification = client
            .verify_payment(
                payment_header.to_string(),
                request_path.to_string(),
                request_method.to_string(),
            )
            .await
            .unwrap();

        assert!(!verification.valid);
        assert!(verification.error.is_some());
        assert!(verification.payer.is_none());
    }
}
