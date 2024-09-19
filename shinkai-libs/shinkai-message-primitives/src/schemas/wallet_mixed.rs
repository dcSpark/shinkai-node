// Heavily inspired by the Coinbase SDK so we can easily connect to it
// Add more about this ^

use std::fmt;

use serde::{Deserialize, Serialize};

/// Represents an address in a wallet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    /// The ID of the wallet that owns the address.
    pub wallet_id: String,
    /// The ID of the blockchain network.
    pub network_id: NetworkIdentifier,
    /// The public key from which the address is derived.
    pub public_key: Option<String>,
    /// The onchain address derived on the server-side.
    pub address_id: String,
}

impl From<Address> for PublicAddress {
    fn from(address: Address) -> Self {
        PublicAddress {
            network_id: address.network_id,
            address_id: address.address_id,
        }
    }
}

/// Represents an address in a wallet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicAddress {
    /// The ID of the blockchain network.
    pub network_id: NetworkIdentifier,
    /// The onchain address derived on the server-side.
    pub address_id: String,
}

/// Represents a list of balances for an address.
/// For now we'll just track ETH, USDC and KAI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressBalanceList {
    /// The list of balances.
    pub data: Vec<Balance>,
    /// True if this list has another page of items after this one that can be fetched.
    pub has_more: bool,
    /// The page token to be used to fetch the next page.
    pub next_page: String,
    /// The total number of balances for the wallet.
    pub total_count: u32,
}

/// Represents a list of addresses in a wallet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressList {
    /// The list of addresses.
    data: Vec<Address>,
    /// True if this list has another page of items after this one that can be fetched.
    has_more: bool,
    /// The page token to be used to fetch the next page.
    next_page: String,
    /// The total number of addresses for the wallet.
    total_count: u32,
}

/// Represents an asset onchain scoped to a particular network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    /// The ID of the blockchain network.
    pub network_id: NetworkIdentifier,
    /// The ID for the asset on the network.
    pub asset_id: String,
    /// The number of decimals the asset supports. This is used to convert from atomic units to base units.
    pub decimals: Option<u32>,
    /// The optional contract address for the asset. This will be specified for smart contract-based assets, for example ERC20s.
    pub contract_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetType {
    ETH,
    USDC,
    KAI,
}

impl Asset {
    pub fn new(asset_type: AssetType, network: &NetworkIdentifier) -> Option<Self> {
        match (asset_type, network) {
            (AssetType::ETH, _) => Some(Asset {
                network_id: network.clone(),
                asset_id: "ETH".to_string(),
                decimals: Some(18),
                contract_address: None,
            }),
            (AssetType::USDC, NetworkIdentifier::BaseSepolia) => Some(Asset {
                network_id: NetworkIdentifier::BaseSepolia,
                asset_id: "USDC".to_string(),
                decimals: Some(6),
                contract_address: Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string()),
            }),
            (AssetType::USDC, NetworkIdentifier::BaseMainnet) => Some(Asset {
                network_id: NetworkIdentifier::BaseMainnet,
                asset_id: "USDC".to_string(),
                decimals: Some(6),
                contract_address: Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()),
            }),
            (AssetType::USDC, NetworkIdentifier::EthereumSepolia) => Some(Asset {
                network_id: NetworkIdentifier::EthereumSepolia,
                asset_id: "USDC".to_string(),
                decimals: Some(6),
                contract_address: Some("0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238".to_string()),
            }),
            (AssetType::USDC, NetworkIdentifier::EthereumMainnet) => Some(Asset {
                network_id: NetworkIdentifier::EthereumMainnet,
                asset_id: "USDC".to_string(),
                decimals: Some(6),
                contract_address: Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string()),
            }),
            (AssetType::USDC, NetworkIdentifier::ArbitrumSepolia) => Some(Asset {
                network_id: NetworkIdentifier::ArbitrumSepolia,
                asset_id: "USDC".to_string(),
                decimals: Some(6),
                contract_address: Some("0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238".to_string()),
            }),
            (AssetType::USDC, NetworkIdentifier::ArbitrumMainnet) => Some(Asset {
                network_id: NetworkIdentifier::ArbitrumMainnet,
                asset_id: "USDC".to_string(),
                decimals: Some(6),
                contract_address: Some("0xaf88d065e77c8cC2239327C5EDb3A432268e5831".to_string()),
            }),
            (AssetType::KAI, _) => None, // KAI is not specified for any network yet
            (_, NetworkIdentifier::Anvil) => None, // No tokens specified for Anvil network
        }
    }

    // TODO: add equivalent wherever it belongs
    // pub fn get_address(&self) -> Option<ethers::types::Address> {
    //     self.contract_address
    //         .as_ref()
    //         .and_then(|addr| ethers::types::Address::from_str(addr).ok())
    // }
}

/// Represents the balance of an asset onchain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    /// The amount in the atomic units of the asset.
    pub amount: String,
    /// The number of decimals the asset supports. This is used to convert from atomic units to base units.
    pub decimals: Option<u32>,
    /// The asset associated with the balance.
    pub asset: Asset,
}

/// Represents a request to create a transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateTransferRequest {
    /// The amount to transfer.
    pub amount: String,
    /// The ID of the blockchain network.
    pub network_id: String,
    /// The ID of the asset to transfer.
    pub asset_id: String,
    /// The destination address.
    pub destination: String,
    /// Whether the transfer uses sponsored gas.
    pub gasless: Option<bool>,
}

/// Represents an error response from the Coinbase Developer Platform API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelError {
    /// A short string representing the reported error. Can be used to handle errors programmatically.
    code: String,
    /// A human-readable message providing more details about the error.
    message: String,
}

/// Represents a blockchain network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Network {
    /// The ID of the blockchain network.
    pub id: NetworkIdentifier,
    /// The human-readable name of the blockchain network.
    pub display_name: String,
    /// The chain ID of the blockchain network.
    pub chain_id: u32,
    /// The protocol family of the blockchain network.
    pub protocol_family: NetworkProtocolFamilyEnum,
    /// Whether the network is a testnet or not.
    pub is_testnet: bool,
    /// The native asset of the blockchain network.
    pub native_asset: Asset,
    // /// The feature set of the blockchain network.
    // feature_set: FeatureSet,
}

impl Network {
    pub fn new(id: NetworkIdentifier) -> Self {
        let (display_name, chain_id, protocol_family, is_testnet, native_asset) = match id {
            NetworkIdentifier::BaseSepolia => (
                "Base Sepolia".to_string(),
                84532,
                NetworkProtocolFamilyEnum::Evm,
                true,
                Asset {
                    network_id: NetworkIdentifier::BaseSepolia,
                    asset_id: "ETH".to_string(),
                    decimals: Some(18),
                    contract_address: None,
                },
            ),
            NetworkIdentifier::BaseMainnet => (
                "Base Mainnet".to_string(),
                1,
                NetworkProtocolFamilyEnum::Evm,
                false,
                Asset {
                    network_id: NetworkIdentifier::BaseMainnet,
                    asset_id: "ETH".to_string(),
                    decimals: Some(18),
                    contract_address: None,
                },
            ),
            NetworkIdentifier::EthereumSepolia => (
                "Ethereum Sepolia".to_string(),
                11155111,
                NetworkProtocolFamilyEnum::Evm,
                true,
                Asset {
                    network_id: NetworkIdentifier::EthereumSepolia,
                    asset_id: "ETH".to_string(),
                    decimals: Some(18),
                    contract_address: None,
                },
            ),
            NetworkIdentifier::EthereumMainnet => (
                "Ethereum Mainnet".to_string(),
                1,
                NetworkProtocolFamilyEnum::Evm,
                false,
                Asset {
                    network_id: NetworkIdentifier::EthereumMainnet,
                    asset_id: "ETH".to_string(),
                    decimals: Some(18),
                    contract_address: None,
                },
            ),
            NetworkIdentifier::ArbitrumSepolia => (
                "Arbitrum Sepolia".to_string(),
                421611,
                NetworkProtocolFamilyEnum::Evm,
                true,
                Asset {
                    network_id: NetworkIdentifier::ArbitrumSepolia,
                    asset_id: "ETH".to_string(),
                    decimals: Some(18),
                    contract_address: None,
                },
            ),
            NetworkIdentifier::ArbitrumMainnet => (
                "Arbitrum Mainnet".to_string(),
                42161,
                NetworkProtocolFamilyEnum::Evm,
                false,
                Asset {
                    network_id: NetworkIdentifier::ArbitrumMainnet,
                    asset_id: "ETH".to_string(),
                    decimals: Some(18),
                    contract_address: None,
                },
            ),
            NetworkIdentifier::Anvil => (
                "Anvil".to_string(),
                31337,
                NetworkProtocolFamilyEnum::Evm,
                true,
                Asset {
                    network_id: NetworkIdentifier::Anvil,
                    asset_id: "ETH".to_string(),
                    decimals: Some(18),
                    contract_address: None,
                },
            ),
        };

        Network {
            id,
            display_name,
            chain_id,
            protocol_family,
            is_testnet,
            native_asset,
        }
    }

    pub fn default_rpc(&self) -> &str {
        match self.id {
            NetworkIdentifier::BaseSepolia => "https://base-sepolia.blockpi.network/v1/rpc/public",
            NetworkIdentifier::BaseMainnet => "https://base-mainnet.rpc.url",
            NetworkIdentifier::EthereumSepolia => "https://ethereum-sepolia.rpc.url",
            NetworkIdentifier::EthereumMainnet => "https://ethereum-mainnet.rpc.url",
            NetworkIdentifier::ArbitrumSepolia => "https://arbitrum-sepolia.rpc.url",
            NetworkIdentifier::ArbitrumMainnet => "https://arbitrum-mainnet.rpc.url",
            NetworkIdentifier::Anvil => "http://localhost:62582",
        }
    }
}

/// Enum representing the protocol family of the blockchain network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkProtocolFamilyEnum {
    Evm,
}

/// Enum representing the ID of the blockchain network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkIdentifier {
    BaseSepolia,
    BaseMainnet,
    EthereumSepolia,
    EthereumMainnet,
    ArbitrumSepolia,
    ArbitrumMainnet,
    Anvil,
}

impl fmt::Display for NetworkIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let display_str = match self {
            NetworkIdentifier::BaseSepolia => "Base Sepolia",
            NetworkIdentifier::BaseMainnet => "Base Mainnet",
            NetworkIdentifier::EthereumSepolia => "Ethereum Sepolia",
            NetworkIdentifier::EthereumMainnet => "Ethereum Mainnet",
            NetworkIdentifier::ArbitrumSepolia => "Arbitrum Sepolia",
            NetworkIdentifier::ArbitrumMainnet => "Arbitrum Mainnet",
            NetworkIdentifier::Anvil => "Anvil",
        };
        write!(f, "{}", display_str)
    }
}

/// Represents an onchain transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    /// The ID of the blockchain network.
    pub network_id: String,
    /// The onchain address of the sender.
    pub from_address_id: String,
    /// The onchain address of the recipient.
    pub to_address_id: Option<String>,
    /// The unsigned payload of the transaction. This is the payload that needs to be signed by the sender.
    pub unsigned_payload: String,
    /// The signed payload of the transaction. This is the payload that has been signed by the sender.
    pub signed_payload: Option<String>,
    /// The hash of the transaction.
    pub transaction_hash: Option<String>,
    /// The link to view the transaction on a block explorer. This is optional and may not be present for all transactions.
    pub transaction_link: Option<String>,
    /// The status of the transaction.
    pub status: TransactionStatusEnum,
}

/// Enum representing the status of the transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionStatusEnum {
    Pending,
    Signed,
    Broadcast,
    Complete,
    Failed,
}

/// Enum representing the type of transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionType {
    Transfer,
}

/// Represents a transfer of an asset from one address to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transfer {
    /// The ID of the blockchain network.
    network_id: String,
    /// The ID of the wallet that owns the from address.
    wallet_id: String,
    /// The onchain address of the sender.
    address_id: String,
    /// The onchain address of the recipient.
    destination: String,
    /// The amount in the atomic units of the asset.
    amount: String,
    /// The ID of the asset being transferred.
    asset_id: String,
    /// The asset associated with the transfer.
    asset: Asset,
    /// The ID of the transfer.
    transfer_id: String,
    /// The transaction associated with the transfer.
    transaction: Option<Transaction>,
    /// The sponsored send associated with the transfer.
    // sponsored_send: Option<SponsoredSend>,
    /// The unsigned payload of the transfer. This is the payload that needs to be signed by the sender.
    unsigned_payload: Option<String>,
    /// The signed payload of the transfer. This is the payload that has been signed by the sender.
    signed_payload: Option<String>,
    /// The hash of the transfer transaction.
    transaction_hash: Option<String>,
    /// The status of the transfer.
    status: Option<TransferStatusEnum>,
    /// Whether the transfer uses sponsored gas.
    gasless: bool,
}

/// Enum representing the status of the transfer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransferStatusEnum {
    Pending,
    Broadcast,
    Complete,
    Failed,
}

/// Represents a list of transfers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferList {
    /// The list of transfers.
    data: Vec<Transfer>,
    /// True if this list has another page of items after this one that can be fetched.
    has_more: bool,
    /// The page token to be used to fetch the next page.
    next_page: String,
    /// The total number of transfers for the address in the wallet.
    total_count: u32,
}
