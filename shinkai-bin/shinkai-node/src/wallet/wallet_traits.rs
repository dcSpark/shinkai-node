
// Heavily inspured by the Coinbase SDK so we can easily connect to it
// Add more about this ^

use chrono::Utc;
use ethers::types::U256;
use uuid::Uuid;

use super::{
    mixed::{
        Address, AddressBalanceList, Asset, Balance, CreateTransferRequest, Network, Transaction,
        Transfer,
    },
    wallet_error::WalletError,
};

pub trait IsWallet {}

impl IsWallet for Wallet {}

pub type TransactionHash = String;

/// Represents a wallet.
pub struct Wallet {
    /// The assigned ID for the wallet.
    pub id: String,
    /// The blockchain network associated with the wallet.
    pub network: Network,
}

/// Trait for sending actions.
pub trait SendActions {
    async fn send_transaction(
        &self,
        to_wallet: Address,
        token: Option<Asset>,
        send_amount: String,
        invoice_id: String,
    ) -> Result<TransactionHash, WalletError>;
    async fn sign_transaction(&self, tx: Transaction) -> Result<String, WalletError>;
}

/// Trait for common actions.
#[async_trait::async_trait]
pub trait CommonActions {
    fn get_address(&self) -> Address;
    async fn get_balance(&self) -> Result<f64, WalletError>;
    async fn check_balances(&self) -> Result<AddressBalanceList, WalletError>;

    // fn get_main_balance(&self) -> Result<Balance, WalletError>;
    // fn get_transaction(&self, tx_hash: String) -> Result<Transaction, WalletError>;
    // fn get_transaction_confirmations(&self, tx_hash: String) -> Result<u64, WalletError>;
    // fn get_transaction_receipt(&self, tx_hash: String) -> Result<Transaction, WalletError>;
    // fn wait_for_transaction_receipt(&self, tx_hash: String) -> Result<Transaction, WalletError>;
    // fn watch_pending_transactions(&self) -> Result<Vec<Transaction>, WalletError>;
    // fn send_raw_transaction(&self, raw_tx: String) -> Result<(), WalletError>;
    // fn prepare_transaction_request(
    //     &self,
    //     request: CreateTransferRequest,
    // ) -> Result<Transfer, WalletError>;
}
