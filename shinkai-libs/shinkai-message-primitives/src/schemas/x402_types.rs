use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
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

impl PaymentRequirements {
    pub fn new(network: Network, max_amount_required: String, pay_to: String, asset: String, resource: String) -> Self {
        Self {
            scheme: "exact".to_string(),
            description: String::new(),
            network,
            max_amount_required,
            resource,
            mime_type: String::new(),
            pay_to,
            max_timeout_seconds: 300,
            asset,
            output_schema: Some(serde_json::json!({})),
            extra: None,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub fn with_mime_type(mut self, mime_type: String) -> Self {
        self.mime_type = mime_type;
        self
    }

    pub fn with_timeout(mut self, timeout_seconds: u64) -> Self {
        self.max_timeout_seconds = timeout_seconds;
        self
    }

    pub fn with_extra(mut self, extra: serde_json::Value) -> Self {
        self.extra = Some(extra);
        self
    }

    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }
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
