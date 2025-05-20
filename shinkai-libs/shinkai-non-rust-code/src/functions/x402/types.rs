use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FacilitatorConfig {
    pub url: String,
    pub network: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct X402ClientData {
    pub facilitator_url: String,
    pub network: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub max_amount_required: String,
    pub resource: String,
    pub description: String,
    pub mime_type: String,
    pub pay_to: String,
    pub max_timeout_seconds: u64,
    pub asset: String,
    pub output_schema: Option<serde_json::Value>,
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentPayload {
    pub scheme: String,
    pub x402_version: u32,
    pub network: String,
    pub payload: PaymentAuthorization,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentAuthorization {
    pub signature: String,
    pub authorization: PaymentAuthorizationDetails,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentAuthorizationDetails {
    pub from: String,
    pub to: String,
    pub value: String,
    pub valid_after: String,
    pub valid_before: String,
    pub nonce: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentVerification {
    pub valid: bool,
    pub error: Option<String>,
    pub payer: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentSettlement {
    pub success: bool,
    pub transaction: Option<String>,
    pub network: Option<String>,
    pub error_reason: Option<String>,
}
