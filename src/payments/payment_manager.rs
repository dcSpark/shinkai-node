use super::payment_methods::{CryptoWallet, CryptoToken};
use ethers::prelude::*;
use std::convert::TryFrom;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug)]
pub enum PaymentManagerError {
    UnsupportedNetwork,
    TransactionError(String),
    // Add other error variants as needed
}

impl std::fmt::Display for PaymentManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PaymentManagerError::UnsupportedNetwork => write!(f, "Unsupported network"),
            PaymentManagerError::TransactionError(err) => write!(f, "Transaction error: {}", err),
        }
    }
}

impl std::error::Error for PaymentManagerError {}

pub struct PaymentManager {
    execute_transaction_bitcoin: fn(CryptoWallet, CryptoWallet, CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
    execute_transaction_evm: fn(CryptoWallet, CryptoWallet, CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
    execute_transaction_solana: fn(CryptoWallet, CryptoWallet, CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
    execute_transaction_cardano: fn(CryptoWallet, CryptoWallet, CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
}

impl PaymentManager {
    pub fn new(
        execute_transaction_bitcoin: fn(CryptoWallet, CryptoWallet, CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
        execute_transaction_evm: fn(CryptoWallet, CryptoWallet, CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
        execute_transaction_solana: fn(CryptoWallet, CryptoWallet, CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
        execute_transaction_cardano: fn(CryptoWallet, CryptoWallet, CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
    ) -> Self {
        Self {
            execute_transaction_bitcoin,
            execute_transaction_evm,
            execute_transaction_solana,
            execute_transaction_cardano,
        }
    }

    pub async fn send_transaction(&self, from_wallet: &CryptoWallet, to_wallet: &CryptoWallet, token: &CryptoToken) -> Result<(), PaymentManagerError> {
        match from_wallet.network.as_str() {
            "Bitcoin" => (self.execute_transaction_bitcoin)(from_wallet.clone(), to_wallet.clone(), token.clone()).await,
            "EVM" => (self.execute_transaction_evm)(from_wallet.clone(), to_wallet.clone(), token.clone()).await,
            "Solana" => (self.execute_transaction_solana)(from_wallet.clone(), to_wallet.clone(), token.clone()).await,
            "Cardano" => (self.execute_transaction_cardano)(from_wallet.clone(), to_wallet.clone(), token.clone()).await,
            _ => Err(PaymentManagerError::UnsupportedNetwork),
        }
    }
}