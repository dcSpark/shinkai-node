use ethers::{core::k256::elliptic_curve, utils::hex::FromHexError};
use std::{error::Error, fmt};

#[derive(Debug)]
pub enum WalletError {
    UuidError(uuid::Error),
    InvalidRpcUrl(String),
    Bip39Error(String),
    EllipticCurveError(elliptic_curve::Error),
    HexError(FromHexError),
    ProviderError(String),
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
    // Add other error types as needed
}

impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WalletError::UuidError(e) => write!(f, "UuidError: {}", e),
            WalletError::InvalidRpcUrl(e) => write!(f, "InvalidRpcUrl: {}", e),
            WalletError::Bip39Error(e) => write!(f, "Bip39Error: {}", e),
            WalletError::EllipticCurveError(e) => write!(f, "EllipticCurveError: {}", e),
            WalletError::HexError(e) => write!(f, "HexError: {}", e),
            WalletError::ProviderError(e) => write!(f, "ProviderError: {}", e),
            WalletError::NetworkMismatch => write!(f, "NetworkMismatch"),
            WalletError::InvalidAmount(e) => write!(f, "InvalidAmount: {}", e),
            WalletError::InvalidAddress(e) => write!(f, "InvalidAddress: {}", e),
            WalletError::UnsupportedAsset(e) => write!(f, "UnsupportedAsset: {}", e),
            WalletError::UnsupportedAssetForNetwork(e, n) => {
                write!(f, "UnsupportedAssetForNetwork: {} for network {}", e, n)
            },
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
        }
    }
}

impl Error for WalletError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            WalletError::UuidError(e) => Some(e),
            WalletError::InvalidRpcUrl(_) => None,
            WalletError::Bip39Error(e) => None,
            WalletError::EllipticCurveError(e) => Some(e),
            WalletError::HexError(e) => Some(e),
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
        }
    }
}

impl From<uuid::Error> for WalletError {
    fn from(error: uuid::Error) -> Self {
        WalletError::UuidError(error)
    }
}

impl From<elliptic_curve::Error> for WalletError {
    fn from(error: elliptic_curve::Error) -> Self {
        WalletError::EllipticCurveError(error)
    }
}

impl From<FromHexError> for WalletError {
    fn from(error: FromHexError) -> Self {
        WalletError::HexError(error)
    }
}