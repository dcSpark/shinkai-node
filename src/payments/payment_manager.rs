use super::payment_methods::{Crypto, Payment, CryptoWallet, CryptoToken};
use ethers::prelude::*;
use std::convert::TryFrom;
use std::future::Future;
use std::pin::Pin;

pub struct PaymentManager {
    execute_transaction_bitcoin: fn(&CryptoWallet, &CryptoWallet, &CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>,
    execute_transaction_evm: fn(&CryptoWallet, &CryptoWallet, &CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>,
    execute_transaction_solana: fn(&CryptoWallet, &CryptoWallet, &CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>,
    execute_transaction_cardano: fn(&CryptoWallet, &CryptoWallet, &CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>,
}

impl PaymentManager {
    pub fn new(
        execute_transaction_bitcoin: fn(&CryptoWallet, &CryptoWallet, &CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>,
        execute_transaction_evm: fn(&CryptoWallet, &CryptoWallet, &CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>,
        execute_transaction_solana: fn(&CryptoWallet, &CryptoWallet, &CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>,
        execute_transaction_cardano: fn(&CryptoWallet, &CryptoWallet, &CryptoToken) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>,
    ) -> Self {
        Self {
            execute_transaction_bitcoin,
            execute_transaction_evm,
            execute_transaction_solana,
            execute_transaction_cardano,
        }
    }

    pub async fn send_transaction(&self, from_wallet: &CryptoWallet, to_wallet: &CryptoWallet, token: &CryptoToken) -> Result<(), &'static str> {
        match from_wallet.network.as_str() {
            "Bitcoin" => (self.execute_transaction_bitcoin)(from_wallet, to_wallet, token).await,
            "EVM" => (self.execute_transaction_evm)(from_wallet, to_wallet, token).await,
            "Solana" => (self.execute_transaction_solana)(from_wallet, to_wallet, token).await,
            "Cardano" => (self.execute_transaction_cardano)(from_wallet, to_wallet, token).await,
            _ => Err("Unsupported network"),
        }
    }
}