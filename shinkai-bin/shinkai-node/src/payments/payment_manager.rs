use super::payment_methods::{CryptoWallet, CryptoToken, CryptoPayment, CryptoTokenAmount};
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
    execute_transaction_bitcoin: fn(CryptoWallet, CryptoWallet, CryptoToken, CryptoTokenAmount, String) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
    execute_transaction_evm: fn(CryptoWallet, CryptoWallet, CryptoToken, CryptoTokenAmount, String) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
    execute_transaction_solana: fn(CryptoWallet, CryptoWallet, CryptoToken, CryptoTokenAmount, String) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
    execute_transaction_cardano: fn(CryptoWallet, CryptoWallet, CryptoToken, CryptoTokenAmount, String) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
}

impl PaymentManager {
    pub fn new(
        execute_transaction_bitcoin: fn(CryptoWallet, CryptoWallet, CryptoToken, CryptoTokenAmount, String) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
        execute_transaction_evm: fn(CryptoWallet, CryptoWallet, CryptoToken, CryptoTokenAmount, String) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
        execute_transaction_solana: fn(CryptoWallet, CryptoWallet, CryptoToken, CryptoTokenAmount, String) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
        execute_transaction_cardano: fn(CryptoWallet, CryptoWallet, CryptoToken, CryptoTokenAmount, String) -> Pin<Box<dyn Future<Output = Result<(), PaymentManagerError>> + Send>>,
    ) -> Self {
        Self {
            execute_transaction_bitcoin,
            execute_transaction_evm,
            execute_transaction_solana,
            execute_transaction_cardano,
        }
    }

    pub async fn send_transaction(&self, from: &CryptoPayment, to: &CryptoWallet, token: &CryptoToken, send_token: &CryptoTokenAmount, provider_url: String) -> Result<(), PaymentManagerError> {
        match from {
            CryptoPayment::BitcoinVM(wallet) => (self.execute_transaction_bitcoin)(wallet.clone(), to.clone(), token.clone(), send_token.clone(), provider_url.clone()).await,
            CryptoPayment::EVM(wallet) => (self.execute_transaction_evm)(wallet.clone(), to.clone(), token.clone(), send_token.clone(), provider_url.clone()).await,
            CryptoPayment::SolanaVM(wallet) => (self.execute_transaction_solana)(wallet.clone(), to.clone(), token.clone(), send_token.clone(), provider_url.clone()).await,
            CryptoPayment::CardanoVM(wallet) => (self.execute_transaction_cardano)(wallet.clone(), to.clone(), token.clone(), send_token.clone(), provider_url.clone()).await,
        }
    }
}