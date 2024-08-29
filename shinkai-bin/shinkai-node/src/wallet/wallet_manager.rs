use chrono::{DateTime, Utc};
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
    wallet_traits::{CommonActions, IsWallet, PaymentWallet, ReceivingWallet},
};

/// Manages multiple wallets and their roles.
pub struct WalletManager {
    /// The wallet used for payments.
    payment_wallet: Box<dyn PaymentWallet>,
    /// The wallet used for receiving payments.
    receiving_wallet: Box<dyn ReceivingWallet>,
}

impl WalletManager {
    fn new(
        payment_wallet: Box<dyn PaymentWallet>,
        receiving_wallet: Box<dyn ReceivingWallet>,
    ) -> Self {
        let h = "hello hello";
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

    pub fn create_local_ethers_wallet_manager(
        network: Network,
    ) -> Result<WalletManager, WalletError> {
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
        let payment_wallet: Box<dyn PaymentWallet> = Box::new(LocalEthersWallet::recover_wallet(network.clone(), source.clone())?);
        let receiving_wallet: Box<dyn ReceivingWallet> = Box::new(LocalEthersWallet::recover_wallet(network, source)?);

        Ok(WalletManager {
            payment_wallet,
            receiving_wallet,
        })
    }
}
