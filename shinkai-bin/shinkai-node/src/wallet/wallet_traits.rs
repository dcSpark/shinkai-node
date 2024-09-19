use std::future::Future;
use std::pin::Pin;

use super::{
    wallet_error::WalletError, wallet_manager::WalletEnum,
};

use downcast_rs::{impl_downcast, Downcast};
use shinkai_message_primitives::schemas::wallet_mixed::{Address, AddressBalanceList, Asset, Balance, Network, PublicAddress, Transaction};

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
    fn send_transaction(
        &self,
        to_wallet: PublicAddress,
        token: Option<Asset>,
        send_amount: String,
        invoice_id: String,
    ) -> Pin<Box<dyn Future<Output = Result<TransactionHash, WalletError>> + Send>>;

    fn sign_transaction(&self, tx: Transaction) -> Pin<Box<dyn Future<Output = Result<String, WalletError>> + Send>>;
}

/// Trait for common actions.
pub trait CommonActions {
    fn get_payment_address(&self) -> PublicAddress;
    fn get_address(&self) -> Address;
    fn get_balance(&self) -> Pin<Box<dyn Future<Output = Result<f64, WalletError>> + Send>>;

    fn check_balances(&self) -> Pin<Box<dyn Future<Output = Result<AddressBalanceList, WalletError>> + Send>>;

    fn check_asset_balance(
        &self,
        public_address: PublicAddress,
        asset: Asset,
    ) -> Pin<Box<dyn Future<Output = Result<Balance, WalletError>> + Send>>;

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

/// Trait for payment wallet.
pub trait PaymentWallet: SendActions + CommonActions + IsWallet + Send + Sync + Downcast {
    fn to_wallet_enum(&self) -> WalletEnum;
}

impl_downcast!(PaymentWallet);

/// Trait that combines `CommonActions` and `IsWallet`.
pub trait CommonIsWallet: CommonActions + IsWallet + Send + Sync {}
impl<T> CommonIsWallet for T where T: CommonActions + IsWallet + Send + Sync {}

/// Trait for receiving wallet.
pub trait ReceivingWallet: CommonActions + IsWallet + Send + Sync + Downcast {
    fn to_wallet_enum(&self) -> WalletEnum;
}

impl_downcast!(ReceivingWallet);
