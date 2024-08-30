use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub enum UsageTypeInquiry {
    PerUse,
    Downloadable,
}

impl fmt::Display for UsageTypeInquiry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsageTypeInquiry::PerUse => write!(f, "PerUse"),
            UsageTypeInquiry::Downloadable => write!(f, "Downloadable"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct ShinkaiToolOffering {
    pub tool_key: String,
    pub usage_type: UsageType,
    pub meta_description: Option<String>,
}

// Updated enum to include aliases for prices
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum UsageType {
    PerUse(ToolPrice),
    Downloadable(ToolPrice),
    Both {
        per_use_price: ToolPrice,
        download_price: ToolPrice,
    },
}

impl UsageType {
    pub fn per_use_usd_price(&self) -> f32 {
        match self {
            UsageType::PerUse(price) => price.to_usd_float(),
            UsageType::Both { per_use_price, .. } => per_use_price.to_usd_float(),
            _ => 0.0,
        }
    }
}

impl ToolPrice {
    // TODO: expand to support the other assets correctly
    pub fn to_usd_float(&self) -> f32 {
        match self {
            ToolPrice::Free => 0.0,
            ToolPrice::DirectDelegation(_) => 0.0, // Handle this case as needed
            ToolPrice::Payment(payments) => {
                for payment in payments {
                    if payment.asset.asset_id == "USDC" {
                        return payment.amount.parse::<f32>().unwrap_or(999_999_999.0);
                    }
                }
                999_999_999.0 // Return 999_999_999 if USDC is not found
            }
        }
    }
}

type KAIAmount = String;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ToolPrice {
    Free,
    DirectDelegation(KAIAmount),
    Payment(Vec<AssetPayment>),
}

// New Code
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum AssetType {
    ETH,
    USDC,
    KAI,
}

/// Represents a payment with an asset and amount.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssetPayment {
    /// The asset to be paid.
    pub asset: Asset,
    /// The amount to be paid in atomic units of the asset.
    pub amount: String,
}

/// Represents an asset onchain scoped to a particular network.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    /// The ID of the blockchain network.
    pub network_id: String, // TODO: it needs to be an enum
    /// The ID for the asset on the network.
    pub asset_id: String,
    /// The number of decimals the asset supports. This is used to convert from atomic units to base units.
    pub decimals: Option<u32>,
    /// The optional contract address for the asset. This will be specified for smart contract-based assets, for example ERC20s.
    pub contract_address: Option<String>,
}

// TODO: add the rest of the code here from the playground project

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_shinkai_tool_offering_to_json() {
        let offering = ShinkaiToolOffering {
            tool_key: "test_tool".to_string(),
            usage_type: UsageType::Both {
                per_use_price: ToolPrice::Free,
                download_price: ToolPrice::DirectDelegation("1000".to_string()),
            },
            meta_description: Some("A tool for testing".to_string()),
        };

        let json = serde_json::to_string(&offering).expect("Failed to convert to JSON");
        println!("{}", json);
        assert!(json.contains("\"tool_key\":\"test_tool\""));
        assert!(json.contains("\"per_use_price\":\"Free\""));
        assert!(json.contains("\"download_price\":\"DirectDelegation\""));
    }

    #[test]
    fn test_shinkai_tool_offering_from_json() {
        let json = r#"
        {
            "tool_key": "test_tool",
            "usage_type": {
                "Both": {
                    "per_use_price": "Free",
                    "download_price": {
                        "DirectDelegation": "1000"
                    }
                }
            },
            "meta_description": "A tool for testing"
        }"#;

        let offering: ShinkaiToolOffering = serde_json::from_str(json).expect("Failed to convert from JSON");
        assert_eq!(offering.tool_key, "test_tool");
        assert_eq!(offering.meta_description, Some("A tool for testing".to_string()));
        if let UsageType::Both {
            per_use_price,
            download_price,
        } = offering.usage_type
        {
            assert_eq!(per_use_price, ToolPrice::Free);
            assert_eq!(download_price, ToolPrice::DirectDelegation("1000".to_string()));
        } else {
            panic!("UsageType did not match expected value");
        }
    }
}
