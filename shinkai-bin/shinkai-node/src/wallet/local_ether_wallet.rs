use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::wallet_complementary::WalletSource;
use shinkai_message_primitives::schemas::wallet_mixed::{
    Address, AddressBalanceList, Asset, Balance, PublicAddress, Transaction
};
use shinkai_message_primitives::schemas::x402_types::{Network, PaymentRequirements};
use shinkai_non_rust_code::functions::x402;
use std::future::Future;
use std::pin::Pin;

use crate::wallet::wallet_error::WalletError;

use super::wallet_manager::WalletEnum;
use super::wallet_traits::{CommonActions, IsWallet, PaymentWallet, ReceivingWallet, SendActions, TransactionHash};
use shinkai_non_rust_code::functions::ethers_wallet::create_wallet::{
    self as ethers_create_wallet, Input as EthersWalletInput
};
use shinkai_non_rust_code::functions::ethers_wallet::get_balance;
use shinkai_non_rust_code::functions::ethers_wallet::recover_wallet::{
    self as ethers_recover_wallet, Input as EthersRecoverInput, RecoverySource
};
use shinkai_non_rust_code::functions::x402::create_payment::{self, Input};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalEthersWallet {
    pub id: String,
    pub network: Network,
    pub address: Address,
    pub private_key: String,
    pub public_key: String,
}

impl LocalEthersWallet {
    pub async fn create_wallet_async(network: Network) -> Result<Self, WalletError> {
        // Call the Deno-based wallet creation
        let input = EthersWalletInput {};
        let result = ethers_create_wallet::create_wallet(input)
            .await
            .map_err(|e| WalletError::FunctionExecutionError(e.to_string()))?;
        let wallet = result.wallet;

        // Map the result to LocalEthersWallet
        let address = Address {
            wallet_id: wallet.public_key.clone(),
            network_id: network.clone(),
            public_key: Some(wallet.public_key.clone()),
            address_id: wallet.address.clone(),
        };

        Ok(LocalEthersWallet {
            id: wallet.public_key.clone(),
            network,
            address,
            private_key: wallet.private_key,
            public_key: wallet.public_key,
        })
    }

    pub async fn recover_wallet(network: Network, source: WalletSource) -> Result<Self, WalletError> {
        // Map WalletSource to RecoverySource
        let recovery_source = match source {
            WalletSource::Mnemonic(mnemonic) => RecoverySource::Mnemonic(mnemonic),
            WalletSource::PrivateKey(private_key) => RecoverySource::PrivateKey(private_key),
        };

        let input = EthersRecoverInput {
            source: recovery_source,
        };

        let result = ethers_recover_wallet::recover_wallet(input)
            .await
            .map_err(|e| WalletError::FunctionExecutionError(e.to_string()))?;
        let wallet = result.wallet;

        let address = Address {
            wallet_id: wallet.public_key.clone().unwrap_or_else(|| wallet.address.clone()),
            network_id: network.clone(),
            public_key: wallet.public_key.clone(),
            address_id: wallet.address.clone(),
        };

        Ok(LocalEthersWallet {
            id: wallet.public_key.clone().unwrap_or_else(|| wallet.address.clone()),
            network,
            address,
            private_key: wallet.private_key,
            public_key: wallet.public_key.unwrap_or_default(),
        })
    }

    pub async fn prepare_transaction_request(
        _from_wallet: &LocalEthersWallet,
        _to_wallet: PublicAddress,
        _token: Option<Asset>,
        _send_amount: u64,
        _provider_url: String,
        _invoice_id: String,
    ) -> Result<(), WalletError> {
        unimplemented!()
    }

    pub async fn internal_sign_transaction(&self, _tx: ()) -> Result<(), WalletError> {
        unimplemented!()
    }

    pub async fn internal_send_transaction(
        &self,
        _to_wallet: PublicAddress,
        _token: Option<Asset>,
        _send_amount: u64,
        _invoice_id: String,
    ) -> Result<(), WalletError> {
        unimplemented!()
    }

    pub fn convert_to_typed_transaction(_tx: ()) -> () {
        unimplemented!()
    }

    // TODO: move this to a config file or merge it to Network struct
    fn rpc_url_for_network(network: &Network) -> String {
        match network {
            Network::BaseSepolia => "https://sepolia.base.org".to_string(),
            Network::Base => "https://mainnet.base.org".to_string(),
            Network::AvalancheFuji => "https://api.avax-test.network/ext/bc/C/rpc".to_string(),
            Network::Avalanche => "https://api.avax.network/ext/bc/C/rpc".to_string(),
        }
    }
}

impl IsWallet for LocalEthersWallet {}

impl PaymentWallet for LocalEthersWallet {
    fn to_wallet_enum(&self) -> WalletEnum {
        WalletEnum::LocalEthersWallet(self.clone())
    }
}

impl ReceivingWallet for LocalEthersWallet {
    fn to_wallet_enum(&self) -> WalletEnum {
        WalletEnum::LocalEthersWallet(self.clone())
    }
}

impl SendActions for LocalEthersWallet {
    fn send_transaction(
        &self,
        _to_wallet: PublicAddress,
        _token: Option<Asset>,
        _send_amount: String,
        _invoice_id: String,
        _node_name: ShinkaiName,
    ) -> Pin<Box<dyn Future<Output = Result<TransactionHash, WalletError>> + Send + 'static>> {
        unimplemented!()
    }

    fn sign_transaction(
        &self,
        _tx: Transaction,
    ) -> Pin<Box<dyn Future<Output = Result<String, WalletError>> + Send + 'static>> {
        unimplemented!()
    }

    fn create_payment_request(
        &self,
        payment_requirements: PaymentRequirements,
    ) -> Pin<Box<dyn Future<Output = Result<x402::create_payment::Output, WalletError>> + Send>> {
        let input = Input {
            accepts: vec![payment_requirements],
            x402_version: 1,
            private_key: self.private_key.clone(),
        };

        Box::pin(async move {
            let input_cloned = input.clone();
            match create_payment::create_payment(input_cloned).await {
                Ok(output) => Ok(output),
                Err(e) => Err(WalletError::FunctionExecutionError(e.to_string())),
            }
        })
    }
}

impl CommonActions for LocalEthersWallet {
    /// Returns the full internal Address struct, including wallet_id and public_key.
    /// This is useful for internal logic, auditing, or advanced wallet operations.
    fn get_address(&self) -> Address {
        self.address.clone()
    }

    /// Returns the public payment address (network and address_id only).
    /// This is the address to share for receiving payments and is safe to expose publicly.
    fn get_payment_address(&self) -> PublicAddress {
        PublicAddress {
            network_id: self.address.network_id.clone(),
            address_id: self.address.address_id.clone(),
        }
    }

    fn get_balance(
        &self,
        _node_name: ShinkaiName,
    ) -> Pin<Box<dyn Future<Output = Result<f64, WalletError>> + Send + 'static>> {
        unimplemented!()
    }

    fn check_balances(
        &self,
        _node_name: ShinkaiName,
    ) -> Pin<Box<dyn Future<Output = Result<AddressBalanceList, WalletError>> + Send + 'static>> {
        unimplemented!()
    }

    fn check_asset_balance(
        &self,
        public_address: PublicAddress,
        asset: Asset,
        _node_name: ShinkaiName,
    ) -> Pin<Box<dyn Future<Output = Result<Balance, WalletError>> + Send + 'static>> {
        Box::pin(async move {
            let rpc_url = Self::rpc_url_for_network(&asset.network_id);
            let token_address = asset.contract_address.clone().unwrap_or_else(|| asset.asset_id.clone());
            let input = get_balance::Input {
                token_address: Some(token_address),
                wallet_address: public_address.address_id.clone(),
                rpc_url,
            };
            let result = get_balance::get_balance(input)
                .await
                .map_err(|e| WalletError::FunctionExecutionError(e.to_string()))?;

            println!(
                "balance for {:?} with token {:?} is {:?}",
                public_address, asset, result
            );

            Ok(Balance {
                amount: result.balance,
                decimals: Some(result.token_info.decimals as u32),
                asset,
            })
        })
    }
}
