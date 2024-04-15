use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderSubscription {
    pub minimum_token_delegation: Option<u64>,
    pub minimum_time_delegated_hours: Option<u64>,
    pub monthly_payment: Option<PaymentOption>,
    pub is_free: bool,
    pub folder_description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaymentOption {
    USD(f64),
    KAITokens(u64),
}

#[derive(Debug, Eq, Clone, PartialEq, Serialize, Deserialize)]
pub enum SubscriptionPayment {
    Free,
    DirectDelegation,
    Payment(String),
}