use std::sync::Arc;

use chrono::Utc;
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use shinkai_message_primitives::schemas::{
    coinbase_mpc_config::CoinbaseMPCWalletConfig,
    invoices::{Invoice, Payment, PaymentStatusEnum},
    shinkai_name::ShinkaiName,
    shinkai_tool_offering::ToolPrice,
    wallet_complementary::{WalletRole, WalletSource},
    wallet_mixed::{Asset, Balance, PublicAddress},
    x402_types::Network,
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
    CoinbaseMPCWallet(CoinbaseMPCWallet),
    LocalEthersWallet(LocalEthersWallet),
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
                WalletEnum::CoinbaseMPCWallet(wallet) => Box::new(wallet),
                WalletEnum::LocalEthersWallet(wallet) => Box::new(wallet),
            },
            receiving_wallet: match helper.receiving_wallet {
                WalletEnum::CoinbaseMPCWallet(wallet) => Box::new(wallet),
                WalletEnum::LocalEthersWallet(wallet) => Box::new(wallet),
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

    pub async fn pay_invoice(&self, invoice: Invoice, node_name: ShinkaiName) -> Result<Payment, WalletError> {
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

        println!("Sending transaction with amount: {}", asset_payment.max_amount_required);
        println!("Sending transaction to address: {:?}", invoice.address);
        println!("Sending transaction with asset: {:?}", asset_payment.asset);

        println!("\n\n asset_payment: {:?}", asset_payment);

        let transaction_encoded = self
            .payment_wallet
            .create_payment_request(asset_payment.clone())
            .await?;

        Ok(Payment::new(
            transaction_encoded.payment,
            invoice.invoice_id.clone(),
            Some(Self::get_current_date()),
            PaymentStatusEnum::Signed,
        ))
    }

    pub async fn check_balance_payment_wallet(
        &self,
        public_address: PublicAddress,
        asset: Asset,
        node_name: ShinkaiName,
    ) -> Result<Balance, WalletError> {
        self.payment_wallet
            .check_asset_balance(public_address, asset, node_name)
            .await
    }

    pub async fn check_balances_payment_wallet(
        &self,
        node_name: ShinkaiName,
    ) -> Result<shinkai_message_primitives::schemas::wallet_mixed::AddressBalanceList, WalletError> {
        self.payment_wallet.check_balances(node_name).await
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

    pub async fn create_coinbase_mpc_wallet_manager(
        network: Network,
        sqlite_manager: Arc<SqliteManager>,
        config: Option<CoinbaseMPCWalletConfig>,
        node_name: ShinkaiName,
    ) -> Result<WalletManager, WalletError> {
        let payment_wallet: Box<dyn PaymentWallet> = Box::new(
            CoinbaseMPCWallet::create_wallet(
                network.clone(),
                Arc::downgrade(&sqlite_manager),
                config.clone(),
                node_name.clone(),
            )
            .await?,
        );
        let receiving_wallet: Box<dyn ReceivingWallet> = Box::new(
            CoinbaseMPCWallet::create_wallet(network, Arc::downgrade(&sqlite_manager), config, node_name).await?,
        );

        Ok(WalletManager {
            payment_wallet,
            receiving_wallet,
        })
    }

    pub async fn recover_coinbase_mpc_wallet_manager(
        network: Network,
        sqlite_manager: Arc<SqliteManager>,
        config: Option<CoinbaseMPCWalletConfig>,
        wallet_id: String,
        node_name: ShinkaiName,
    ) -> Result<WalletManager, WalletError> {
        let payment_wallet: Box<dyn PaymentWallet> = Box::new(
            CoinbaseMPCWallet::restore_wallet(
                network.clone(),
                Arc::downgrade(&sqlite_manager),
                config.clone(),
                wallet_id.clone(),
                node_name.clone(),
            )
            .await?,
        );
        let receiving_wallet: Box<dyn ReceivingWallet> = Box::new(
            CoinbaseMPCWallet::restore_wallet(network, Arc::downgrade(&sqlite_manager), config, wallet_id, node_name)
                .await?,
        );

        Ok(WalletManager {
            payment_wallet,
            receiving_wallet,
        })
    }

    pub async fn create_local_ethers_wallet_manager(
        network: Network,
        _db: Arc<SqliteManager>,
        _role: WalletRole,
    ) -> Result<WalletManager, WalletError> {
        // Create a single wallet instance and use it for both payment and receiving
        let wallet = LocalEthersWallet::create_wallet_async(network).await?;

        let payment_wallet: Box<dyn PaymentWallet> = Box::new(wallet.clone());
        let receiving_wallet: Box<dyn ReceivingWallet> = Box::new(wallet);
        Ok(WalletManager {
            payment_wallet,
            receiving_wallet,
        })
    }

    pub async fn recover_local_ethers_wallet_manager(
        network: Network,
        _db: Arc<SqliteManager>,
        source: WalletSource,
        _role: WalletRole,
    ) -> Result<WalletManager, WalletError> {
        // Recover a single wallet instance and use it for both payment and receiving
        let wallet = LocalEthersWallet::recover_wallet(network, source).await?;

        let payment_wallet: Box<dyn PaymentWallet> = Box::new(wallet.clone());
        let receiving_wallet: Box<dyn ReceivingWallet> = Box::new(wallet);
        Ok(WalletManager {
            payment_wallet,
            receiving_wallet,
        })
    }
}
