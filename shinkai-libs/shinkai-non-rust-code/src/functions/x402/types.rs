use serde::{Deserialize, Serialize};

pub type Money = f64;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EIP712 {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ERC20Asset {
    pub address: String,
    pub decimals: u32,
    pub eip712: EIP712,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ERC20TokenAmount {
    pub amount: String,
    pub asset: ERC20Asset,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Price {
    Money(Money),
    ERC20TokenAmount(ERC20TokenAmount),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Network {
    #[serde(rename = "base-sepolia")]
    BaseSepolia,
    #[serde(rename = "base")]
    Base,
    #[serde(rename = "avalanche-fuji")]
    AvalancheFuji,
    #[serde(rename = "avalanche")]
    Avalanche,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FacilitatorConfig {
    pub url: String,
}

impl Default for FacilitatorConfig {
    fn default() -> Self {
        Self {
            url: "https://x402.org/facilitator".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaymentRequirements {
    pub scheme: String,
    pub description: String,
    pub network: Network,
    #[serde(rename = "maxAmountRequired")]
    pub max_amount_required: String,
    pub resource: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "payTo")]
    pub pay_to: String,
    #[serde(rename = "maxTimeoutSeconds")]
    pub max_timeout_seconds: u64,
    pub asset: String,
    #[serde(rename = "outputSchema")]
    pub output_schema: Option<serde_json::Value>,
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaymentPayload {
    pub scheme: String,
    pub network: Network,
    #[serde(rename = "x402Version")]
    pub x402_version: u32,
    pub payload: PaymentPayloadData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaymentPayloadData {
    pub signature: String,
    pub authorization: PaymentAuthorization,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaymentAuthorization {
    pub from: String,
    pub to: String,
    pub value: String,
    #[serde(rename = "validAfter")]
    pub valid_after: String,
    #[serde(rename = "validBefore")]
    pub valid_before: String,
    pub nonce: String,
}
