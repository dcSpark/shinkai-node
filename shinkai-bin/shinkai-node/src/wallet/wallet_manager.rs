use std::sync::Arc;

use chrono::Utc;
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use shinkai_message_primitives::schemas::{
    coinbase_mpc_config::CoinbaseMPCWalletConfig,
    invoices::{Invoice, Payment, PaymentStatusEnum},
    shinkai_tool_offering::ToolPrice,
    wallet_complementary::WalletSource,
    wallet_mixed::{Asset, Balance, Network, PublicAddress},
};
use shinkai_sqlite::SqliteManager;
use uuid::Uuid;

use super::{
    coinbase_mpc_wallet::CoinbaseMPCWallet,
    local_ether_wallet::LocalEthersWallet,
    wallet_error::WalletError,
    wallet_traits::{PaymentWallet, ReceivingWallet},
};

/// Enum to represent different wallet types.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WalletEnum {
    LocalEthersWallet(LocalEthersWallet),
    CoinbaseMPCWallet(CoinbaseMPCWallet),
}

pub struct WalletManager {
    /// The wallet used for payments.
    pub payment_wallet: Box<dyn PaymentWallet>,
    /// The wallet used for receiving payments.
    pub receiving_wallet: Box<dyn ReceivingWallet>,
}

impl Serialize for WalletManager {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("WalletManager", 2)?;
        state.serialize_field("payment_wallet", &self.payment_wallet.to_wallet_enum())?;
        state.serialize_field("receiving_wallet", &self.receiving_wallet.to_wallet_enum())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for WalletManager {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WalletManagerHelper {
            payment_wallet: WalletEnum,
            receiving_wallet: WalletEnum,
        }

        let helper = WalletManagerHelper::deserialize(deserializer)?;
        Ok(WalletManager {
            payment_wallet: match helper.payment_wallet {
                WalletEnum::LocalEthersWallet(wallet) => Box::new(wallet),
                WalletEnum::CoinbaseMPCWallet(wallet) => Box::new(wallet),
            },
            receiving_wallet: match helper.receiving_wallet {
                WalletEnum::LocalEthersWallet(wallet) => Box::new(wallet),
                WalletEnum::CoinbaseMPCWallet(wallet) => Box::new(wallet),
            },
        })
    }
}

impl WalletManager {
    pub fn new(payment_wallet: Box<dyn PaymentWallet>, receiving_wallet: Box<dyn ReceivingWallet>) -> Self {
        WalletManager {
            payment_wallet,
            receiving_wallet,
        }
    }

    pub async fn pay_invoice(&self, invoice: Invoice) -> Result<Payment, WalletError> {
        // Check if the invoice network matches the wallet network
        let public_address = self.payment_wallet.get_payment_address();
        if invoice.address.network_id != public_address.network_id {
            return Err(WalletError::NetworkMismatch);
        }

        // Extract the asset information from shinkai_offering
        let price = invoice
            .shinkai_offering
            .get_price_for_usage(&invoice.usage_type_inquiry)
            .ok_or_else(|| WalletError::InvalidUsageType("Invalid usage type".to_string()))?;

        let asset_payment = match price {
            ToolPrice::Payment(payments) => payments
                .first()
                .ok_or_else(|| WalletError::InvalidPayment("No payments available".to_string()))?,
            _ => return Err(WalletError::InvalidPayment("Invalid payment type".to_string())),
        };

        println!("Sending transaction with amount: {}", asset_payment.amount);
        println!("Sending transaction to address: {:?}", invoice.address);
        println!("Sending transaction with asset: {:?}", asset_payment.asset);

        let transaction_hash = self
            .payment_wallet
            .send_transaction(
                invoice.address,
                Some(asset_payment.asset.clone()),
                asset_payment.amount.clone(),
                invoice.invoice_id.clone(),
            )
            .await?;

        Ok(Payment::new(
            transaction_hash,
            invoice.invoice_id.clone(),
            Some(Self::get_current_date()),
            PaymentStatusEnum::Confirmed,
        ))
    }

    pub async fn check_balance_payment_wallet(
        &self,
        public_address: PublicAddress,
        asset: Asset,
    ) -> Result<Balance, WalletError> {
        self.payment_wallet.check_asset_balance(public_address, asset).await
    }

    pub fn update_payment_wallet(&mut self, new_payment_wallet: Box<dyn PaymentWallet>) {
        self.payment_wallet = new_payment_wallet;
    }

    pub fn update_receiving_wallet(&mut self, new_receiving_wallet: Box<dyn ReceivingWallet>) {
        self.receiving_wallet = new_receiving_wallet;
    }

    pub fn generate_unique_id() -> String {
        Uuid::new_v4().to_string()
    }

    pub fn get_current_date() -> String {
        Utc::now().to_rfc3339()
    }

    pub fn create_local_ethers_wallet_manager(network: Network) -> Result<WalletManager, WalletError> {
        let payment_wallet: Box<dyn PaymentWallet> = Box::new(LocalEthersWallet::create_wallet(network.clone())?);
        let receiving_wallet: Box<dyn ReceivingWallet> = Box::new(LocalEthersWallet::create_wallet(network)?);

        Ok(WalletManager {
            payment_wallet,
            receiving_wallet,
        })
    }

    pub fn recover_local_ethers_wallet_manager(
        network: Network,
        source: WalletSource,
    ) -> Result<WalletManager, WalletError> {
        let payment_wallet: Box<dyn PaymentWallet> =
            Box::new(LocalEthersWallet::recover_wallet(network.clone(), source.clone())?);
        let receiving_wallet: Box<dyn ReceivingWallet> = Box::new(LocalEthersWallet::recover_wallet(network, source)?);

        Ok(WalletManager {
            payment_wallet,
            receiving_wallet,
        })
    }

    pub async fn create_coinbase_mpc_wallet_manager(
        network: Network,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        config: Option<CoinbaseMPCWalletConfig>,
    ) -> Result<WalletManager, WalletError> {
        let payment_wallet: Box<dyn PaymentWallet> = Box::new(
            CoinbaseMPCWallet::create_wallet(network.clone(), Arc::downgrade(&sqlite_manager), config.clone()).await?,
        );
        let receiving_wallet: Box<dyn ReceivingWallet> =
            Box::new(CoinbaseMPCWallet::create_wallet(network, Arc::downgrade(&sqlite_manager), config).await?);

        Ok(WalletManager {
            payment_wallet,
            receiving_wallet,
        })
    }

    pub async fn recover_coinbase_mpc_wallet_manager(
        network: Network,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        config: Option<CoinbaseMPCWalletConfig>,
        wallet_id: String,
    ) -> Result<WalletManager, WalletError> {
        let payment_wallet: Box<dyn PaymentWallet> = Box::new(
            CoinbaseMPCWallet::restore_wallet(
                network.clone(),
                Arc::downgrade(&sqlite_manager),
                config.clone(),
                wallet_id.clone(),
            )
            .await?,
        );
        let receiving_wallet: Box<dyn ReceivingWallet> = Box::new(
            CoinbaseMPCWallet::restore_wallet(network, Arc::downgrade(&sqlite_manager), config, wallet_id).await?,
        );

        Ok(WalletManager {
            payment_wallet,
            receiving_wallet,
        })
    }
}

#[cfg(test)]
mod tests {
    use shinkai_message_primitives::schemas::wallet_mixed::{Network, NetworkIdentifier, NetworkProtocolFamilyEnum};

    use super::*;

    fn create_test_network() -> Network {
        Network {
            id: NetworkIdentifier::Anvil,
            display_name: "Anvil".to_string(),
            chain_id: 31337,
            protocol_family: NetworkProtocolFamilyEnum::Evm,
            is_testnet: true,
            native_asset: Asset {
                network_id: NetworkIdentifier::Anvil,
                asset_id: "ETH".to_string(),
                decimals: Some(18),
                contract_address: None,
            },
        }
    }

    #[test]
    fn test_wallet_manager_serialization() {
        let network = create_test_network();
        let wallet_manager = WalletManager::create_local_ethers_wallet_manager(network).unwrap();

        // Serialize the wallet manager
        let serialized_wallet_manager = serde_json::to_string(&wallet_manager).unwrap();

        // Deserialize the wallet manager
        let deserialized_wallet_manager: WalletManager = serde_json::from_str(&serialized_wallet_manager).unwrap();

        // Compare the original and deserialized wallet managers
        assert_eq!(
            wallet_manager
                .payment_wallet
                .as_ref()
                .downcast_ref::<LocalEthersWallet>()
                .unwrap()
                .id,
            deserialized_wallet_manager
                .payment_wallet
                .as_ref()
                .downcast_ref::<LocalEthersWallet>()
                .unwrap()
                .id
        );
        assert_eq!(
            wallet_manager
                .receiving_wallet
                .as_ref()
                .downcast_ref::<LocalEthersWallet>()
                .unwrap()
                .id,
            deserialized_wallet_manager
                .receiving_wallet
                .as_ref()
                .downcast_ref::<LocalEthersWallet>()
                .unwrap()
                .id
        );
    }
}
