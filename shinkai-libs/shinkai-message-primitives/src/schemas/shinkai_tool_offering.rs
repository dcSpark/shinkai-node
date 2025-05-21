use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::wallet_mixed::Asset;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub enum UsageTypeInquiry {
    PerUse,
}

impl fmt::Display for UsageTypeInquiry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsageTypeInquiry::PerUse => write!(f, "PerUse"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, ToSchema)]
pub struct ShinkaiToolOffering {
    pub tool_key: String,
    pub usage_type: UsageType,
    pub meta_description: Option<String>,
}

impl ShinkaiToolOffering {
    pub fn get_price_for_usage(&self, usage_type_inquiry: &UsageTypeInquiry) -> Option<&ToolPrice> {
        match (usage_type_inquiry, &self.usage_type) {
            (UsageTypeInquiry::PerUse, UsageType::PerUse(price)) => Some(price),
            (UsageTypeInquiry::PerUse, UsageType::Both { per_use_price, .. }) => Some(per_use_price),
            _ => None,
        }
    }

    pub fn convert_tool_to_local(&self) -> Result<String, String> {
        let parts: Vec<&str> = self.tool_key.split(":::").collect();
        if parts.len() < 3 {
            return Err("Invalid tool_key format".to_string());
        }

        let toolkit_name = parts[1];
        let tool_name = parts[2];

        let local_tool_key = format!("local:::{}:::{}", toolkit_name, tool_name);
        Ok(local_tool_key)
    }
}

// Updated enum to include aliases for prices
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum ToolPrice {
    Free,
    #[schema(value_type = String)]
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
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct AssetPayment {
    /// The asset to be paid.
    pub asset: Asset,
    /// The amount to be paid in atomic units of the asset.
    pub amount: String,
}

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
        assert!(json.contains("\"download_price\":{\"DirectDelegation\":\"1000\"}"));
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

    #[test]
    fn test_convert_tool_to_local() {
        let offering = ShinkaiToolOffering {
            tool_key: "node1:::toolkit1:::tool1".to_string(),
            usage_type: UsageType::PerUse(ToolPrice::Free),
            meta_description: None,
        };

        let result = offering.convert_tool_to_local();
        assert_eq!(result.unwrap(), "local:::toolkit1:::tool1");
    }
}
