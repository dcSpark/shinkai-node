use serde::{Deserialize, Serialize};
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct FolderSubscription {
    pub minimum_token_delegation: Option<u64>,
    pub minimum_time_delegated_hours: Option<u64>,
    pub monthly_payment: Option<PaymentOption>,
    pub is_free: bool,
    pub has_web_alternative: Option<bool>,
    pub folder_description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub enum PaymentOption {
    USD(Decimal),
    KAITokens(Decimal),
}

#[derive(Debug, Eq, Clone, PartialEq, Serialize, Deserialize)]
pub enum SubscriptionPayment {
    Free,
    DirectDelegation,
    Payment(String),
}