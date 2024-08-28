use chrono::{DateTime, Utc};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use uuid::Uuid;

use crate::network::agent_payments_manager::{
    invoices::{Invoice, InvoiceStatusEnum, Payment, PaymentStatusEnum},
    shinkai_tool_offering::{AssetPayment, ShinkaiToolOffering},
};

use super::wallet_traits::{CommonActions, IsWallet, SendActions};

/// Manages multiple wallets and their roles.
pub struct WalletManager<T, U>
where
    T: SendActions + CommonActions + IsWallet,
    U: CommonActions + IsWallet,
{
    /// The wallet used for payments.
    payment_wallet: T,
    /// The wallet used for receiving payments.
    receiving_wallet: U,
}

impl<T, U> WalletManager<T, U>
where
    T: SendActions + CommonActions + IsWallet,
    U: CommonActions + IsWallet,
{
    fn new(payment_wallet: T, receiving_wallet: U) -> Self {
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
}
