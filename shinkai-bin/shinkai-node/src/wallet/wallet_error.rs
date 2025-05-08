use std::{error::Error, fmt};

#[derive(Debug)]
pub enum WalletError {
    UuidError(uuid::Error),
    InvalidRpcUrl(String),
    Bip39Error(String),
    ProviderError(String),
    DetailedJsonRpcError {
        code: i32,
        message: String,
        data: Option<String>,
    },
    NetworkMismatch,
    InvalidAmount(String),
    InvalidAddress(String),
    UnsupportedAsset(String),
    UnsupportedAssetForNetwork(String, String),
    MissingContractAddress(String),
    AbiError(String),
    AbiEncodingError(String),
    InvalidPrivateKey(String),
    ContractError(String),
    SigningError(String),
    MissingTransactionReceipt,
    ConversionError(String),
    InvalidPayment(String),
    InvalidUsageType(String),
    TransactionFailed(String),
    ConfigNotFound,
    FunctionExecutionError(String),
    FunctionNotFound(String),
    ToolNotFound(String),
    SqliteManagerError(String),
    ParsingError(String),
    MissingToAddress,
    InsufficientBalance(String),
    // Add other error types as needed
}

impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WalletError::UuidError(e) => write!(f, "UuidError: {}", e),
            WalletError::InvalidRpcUrl(e) => write!(f, "InvalidRpcUrl: {}", e),
            WalletError::Bip39Error(e) => write!(f, "Bip39Error: {}", e),
            WalletError::ProviderError(e) => write!(f, "ProviderError: {}", e),
            WalletError::DetailedJsonRpcError { code, message, data } => {
                write!(
                    f,
                    "JSON-RPC error: code {}, message: {}, data: {:?}",
                    code, message, data
                )
            }
            WalletError::NetworkMismatch => write!(f, "NetworkMismatch"),
            WalletError::InvalidAmount(e) => write!(f, "InvalidAmount: {}", e),
            WalletError::InvalidAddress(e) => write!(f, "InvalidAddress: {}", e),
            WalletError::UnsupportedAsset(e) => write!(f, "UnsupportedAsset: {}", e),
            WalletError::UnsupportedAssetForNetwork(e, n) => {
                write!(f, "UnsupportedAssetForNetwork: {} for network {}", e, n)
            }
            WalletError::MissingContractAddress(e) => write!(f, "MissingContractAddress: {}", e),
            WalletError::AbiError(e) => write!(f, "AbiError: {}", e),
            WalletError::AbiEncodingError(e) => write!(f, "AbiEncodingError: {}", e),
            WalletError::InvalidPrivateKey(e) => write!(f, "InvalidPrivateKey: {}", e),
            WalletError::ContractError(e) => write!(f, "ContractError: {}", e),
            WalletError::SigningError(e) => write!(f, "SigningError: {}", e),
            WalletError::MissingTransactionReceipt => write!(f, "MissingTransactionReceipt"),
            WalletError::ConversionError(e) => write!(f, "ConversionError: {}", e),
            WalletError::InvalidPayment(e) => write!(f, "InvalidPayment: {}", e),
            WalletError::InvalidUsageType(e) => write!(f, "InvalidUsageType: {}", e),
            WalletError::TransactionFailed(e) => write!(f, "TransactionFailed: {}", e),
            WalletError::ConfigNotFound => write!(f, "ConfigNotFound"),
            WalletError::FunctionExecutionError(e) => write!(f, "FunctionExecutionError: {}", e),
            WalletError::FunctionNotFound(e) => write!(f, "FunctionNotFound: {}", e),
            WalletError::ToolNotFound(e) => write!(f, "ToolNotFound: {}", e),
            WalletError::SqliteManagerError(e) => write!(f, "SqliteManagerError: {}", e),
            WalletError::ParsingError(e) => write!(f, "ParsingError: {}", e),
            WalletError::MissingToAddress => write!(f, "MissingToAddress"),
            WalletError::InsufficientBalance(e) => write!(f, "InsufficientBalance: {}", e),
        }
    }
}

impl Error for WalletError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WalletError::UuidError(e) => Some(e),
            WalletError::InvalidRpcUrl(_) => None,
            WalletError::Bip39Error(_) => None,
            WalletError::ProviderError(_) => None,
            WalletError::NetworkMismatch => None,
            WalletError::InvalidAmount(_) => None,
            WalletError::InvalidAddress(_) => None,
            WalletError::UnsupportedAsset(_) => None,
            WalletError::UnsupportedAssetForNetwork(_, _) => None,
            WalletError::MissingContractAddress(_) => None,
            WalletError::AbiError(_) => None,
            WalletError::AbiEncodingError(_) => None,
            WalletError::InvalidPrivateKey(_) => None,
            WalletError::ContractError(_) => None,
            WalletError::SigningError(_) => None,
            WalletError::MissingTransactionReceipt => None,
            WalletError::ConversionError(_) => None,
            WalletError::InvalidUsageType(_) => None,
            WalletError::InvalidPayment(_) => None,
            WalletError::TransactionFailed(_) => None,
            WalletError::ConfigNotFound => None,
            WalletError::FunctionExecutionError(_) => None,
            WalletError::FunctionNotFound(_) => None,
            WalletError::ToolNotFound(_) => None,
            WalletError::SqliteManagerError(_) => None,
            WalletError::ParsingError(_) => None,
            WalletError::MissingToAddress => None,
            WalletError::InsufficientBalance(_) => None,
            WalletError::DetailedJsonRpcError { .. } => None,
        }
    }
}

impl From<uuid::Error> for WalletError {
    fn from(error: uuid::Error) -> Self {
        WalletError::UuidError(error)
    }
}
