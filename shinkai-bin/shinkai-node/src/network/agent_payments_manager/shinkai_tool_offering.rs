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
    pub human_readable_name: String,
    pub tool_key_name: String,
    pub tool_description: String,
    pub usage_type: UsageType,
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
    pub network_id: String,
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
            human_readable_name: "Test Tool".to_string(),
            tool_key_name: "test_tool".to_string(),
            tool_description: "A tool for testing".to_string(),
            usage_type: UsageType::Both {
                per_use_price: ToolPrice::Free,
                download_price: ToolPrice::DirectDelegation("1000".to_string()),
            },
        };

        let json = serde_json::to_string(&offering).expect("Failed to convert to JSON");
        println!("{}", json);
        assert!(json.contains("\"human_readable_name\":\"Test Tool\""));
        assert!(json.contains("\"per_use_price\":\"Free\""));
        assert!(json.contains("\"download_price\":\"DirectDelegation\""));
    }

    #[test]
    fn test_shinkai_tool_offering_from_json() {
        let json = r#"
        {
            "human_readable_name": "Test Tool",
            "tool_key_name": "test_tool",
            "tool_description": "A tool for testing",
            "usage_type": {
                "Both": {
                    "per_use_price": "Free",
                    "download_price": {
                        "DirectDelegation": "1000"
                    }
                }
            }
        }"#;

        let offering: ShinkaiToolOffering = serde_json::from_str(json).expect("Failed to convert from JSON");
        assert_eq!(offering.human_readable_name, "Test Tool");
        assert_eq!(offering.tool_key_name, "test_tool");
        assert_eq!(offering.tool_description, "A tool for testing");
        if let UsageType::Both { per_use_price, download_price } = offering.usage_type {
            assert_eq!(per_use_price, ToolPrice::Free);
            assert_eq!(download_price, ToolPrice::DirectDelegation("1000".to_string()));
        } else {
            panic!("UsageType did not match expected value");
        }
    }
}