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
