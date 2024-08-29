use chrono::{DateTime, Utc};
use downcast_rs::Downcast;
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use uuid::Uuid;

use crate::network::agent_payments_manager::{
    invoices::{Invoice, InvoiceStatusEnum, Payment, PaymentStatusEnum},
    shinkai_tool_offering::ShinkaiToolOffering,
};

use super::{
    local_ether_wallet::{LocalEthersWallet, WalletSource},
    mixed::Network,
    wallet_error::WalletError,
    wallet_traits::{PaymentWallet, ReceivingWallet},
};

/// Enum to represent different wallet types.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum WalletEnum {
    LocalEthersWallet(LocalEthersWallet),
    // Add other wallet types here as needed
}

pub struct WalletManager {
    /// The wallet used for payments.
    payment_wallet: Box<dyn PaymentWallet>,
    /// The wallet used for receiving payments.
    receiving_wallet: Box<dyn ReceivingWallet>,
}

impl Serialize for WalletManager {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("WalletManager", 2)?;
        state.serialize_field(
            "payment_wallet",
            &self
                .payment_wallet
                .as_ref()
                .downcast_ref::<LocalEthersWallet>()
                .unwrap(),
        )?;
        state.serialize_field(
            "receiving_wallet",
            &self
                .receiving_wallet
                .as_ref()
                .downcast_ref::<LocalEthersWallet>()
                .unwrap(),
        )?;
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
            payment_wallet: LocalEthersWallet,
            receiving_wallet: LocalEthersWallet,
        }

        let helper = WalletManagerHelper::deserialize(deserializer)?;
        Ok(WalletManager {
            payment_wallet: Box::new(helper.payment_wallet),
            receiving_wallet: Box::new(helper.receiving_wallet),
        })
    }
}

impl WalletManager {
    fn new(payment_wallet: Box<dyn PaymentWallet>, receiving_wallet: Box<dyn ReceivingWallet>) -> Self {
        WalletManager {
            payment_wallet,
            receiving_wallet,
        }
    }

    fn create_invoice(
        &self,
        requester_name: ShinkaiName,
        shinkai_offering: ShinkaiToolOffering,
        expiration_time: DateTime<Utc>,
    ) -> Invoice {
        Invoice {
            invoice_id: Self::generate_unique_id(),
            requester_name,
            shinkai_offering,
            expiration_time,
            status: InvoiceStatusEnum::Pending,
            payment: None,
        }
    }

    fn pay_invoice(&self, invoice: &mut Invoice, transaction_hash: String) -> Payment {
        invoice.update_status(InvoiceStatusEnum::Paid);
        Payment::new(
            transaction_hash,
            invoice.invoice_id.clone(),
            Some(Self::get_current_date()),
            PaymentStatusEnum::Confirmed,
        )
    }

    fn generate_unique_id() -> String {
        Uuid::new_v4().to_string()
    }

    fn get_current_date() -> String {
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
}

#[cfg(test)]
mod tests {
    use crate::wallet::mixed::{Asset, NetworkIdentifier, NetworkProtocolFamilyEnum};

    use super::*;

    fn create_test_network() -> Network {
        Network {
            id: NetworkIdentifier::Anvil,
            display_name: "Anvil".to_string(),
            chain_id: 31337,
            protocol_family: NetworkProtocolFamilyEnum::Evm,
            is_testnet: true,
            native_asset: Asset {
                network_id: "Anvil".to_string(),
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
