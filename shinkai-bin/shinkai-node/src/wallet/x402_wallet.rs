use serde::{Serialize, Deserialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use rand;
use hex;
use uuid::Uuid;

use shinkai_message_primitives::schemas::{
    shinkai_name::ShinkaiName,
    wallet_mixed::{Address, AddressBalanceList, Asset, Balance, Network, PublicAddress},
};
use shinkai_sqlite::SqliteManager;

use super::wallet_error::WalletError;
use super::wallet_manager::WalletEnum;
use super::wallet_traits::{CommonActions, IsWallet, PaymentWallet, ReceivingWallet, SendActions, TransactionHash};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402Wallet {
    pub id: String,
    pub network: Network,
    pub address: Address,
    pub private_key: String,
    #[serde(skip, default)]
    pub sqlite_manager: Option<Weak<SqliteManager>>,
}

impl X402Wallet {
    pub async fn create_wallet(network: Network, sqlite_manager: Weak<SqliteManager>) -> Result<Self, WalletError> {
        let wallet_id = Uuid::new_v4().to_string();
        let address_id = format!("0x{}", hex::encode(rand::random::<[u8;20]>()));
        Ok(X402Wallet {
            id: wallet_id.clone(),
            network: network.clone(),
            address: Address {
                wallet_id,
                network_id: network.id,
                public_key: None,
                address_id,
            },
            private_key: hex::encode(rand::random::<[u8;32]>()),
            sqlite_manager: Some(sqlite_manager),
        })
    }

    pub async fn restore_wallet(network: Network, sqlite_manager: Weak<SqliteManager>, wallet_id: String) -> Result<Self, WalletError> {
        let address_id = format!("0x{}", hex::encode(rand::random::<[u8;20]>()));
        Ok(X402Wallet {
            id: wallet_id.clone(),
            network: network.clone(),
            address: Address {
                wallet_id,
                network_id: network.id,
                public_key: None,
                address_id,
            },
            private_key: hex::encode(rand::random::<[u8;32]>()),
            sqlite_manager: Some(sqlite_manager),
        })
    }
}

impl IsWallet for X402Wallet {}

impl PaymentWallet for X402Wallet {
    fn to_wallet_enum(&self) -> WalletEnum { WalletEnum::X402Wallet(self.clone()) }
}

impl ReceivingWallet for X402Wallet {
    fn to_wallet_enum(&self) -> WalletEnum { WalletEnum::X402Wallet(self.clone()) }
}

impl SendActions for X402Wallet {
    fn send_transaction(
        &self,
        to_wallet: PublicAddress,
        token: Option<Asset>,
        send_amount: String,
        invoice_id: String,
        _node_name: ShinkaiName,
    ) -> Pin<Box<dyn Future<Output = Result<TransactionHash, WalletError>> + Send>> {
        let private_key = self.private_key.clone();
        let network = self.network.id.to_string();
        Box::pin(async move {
            let token_address = token.map(|t| t.asset_id);
            let result = shinkai_non_rust_code::functions::x402_send_payment::send_payment(
                private_key,
                network,
                to_wallet.address_id,
                send_amount,
                token_address,
                invoice_id,
            )
            .await
            .map_err(|e| WalletError::FunctionExecutionError(e.to_string()))?;
            Ok(result.tx_hash)
        })
    }

    fn sign_transaction(&self, _tx: shinkai_message_primitives::schemas::wallet_mixed::Transaction) -> Pin<Box<dyn Future<Output = Result<String, WalletError>> + Send>> {
        Box::pin(async move { Err(WalletError::FunctionNotFound("sign_transaction".to_string())) })
    }
}

impl CommonActions for X402Wallet {
    fn get_payment_address(&self) -> PublicAddress { self.address.clone().into() }

    fn get_address(&self) -> Address { self.address.clone() }

    fn get_balance(&self, _node_name: ShinkaiName) -> Pin<Box<dyn Future<Output = Result<f64, WalletError>> + Send>> {
        Box::pin(async move { Ok(0.0) })
    }

    fn check_balances(&self, _node_name: ShinkaiName) -> Pin<Box<dyn Future<Output = Result<AddressBalanceList, WalletError>> + Send>> {
        let list = AddressBalanceList { data: vec![], has_more: false, next_page: String::new(), total_count: 0 };
        Box::pin(async move { Ok(list) })
    }

    fn check_asset_balance(&self, _public_address: PublicAddress, asset: Asset, _node_name: ShinkaiName) -> Pin<Box<dyn Future<Output = Result<Balance, WalletError>> + Send>> {
        let bal = Balance { amount: "0".to_string(), decimals: asset.decimals, asset };
        Box::pin(async move { Ok(bal) })
    }
}

