use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, ToSchema)]
pub struct FolderSubscription {
    pub minimum_token_delegation: Option<u64>,
    pub minimum_time_delegated_hours: Option<u64>,
    pub monthly_payment: Option<PaymentOption>,
    pub is_free: bool,
    pub has_web_alternative: Option<bool>,
    pub folder_description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, ToSchema)]
pub enum PaymentOption {
    #[schema(value_type = f64)]
    USD(Decimal),
    #[schema(value_type = f64)]
    KAITokens(Decimal),
}

#[derive(Debug, Eq, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum SubscriptionPayment {
    Free,
    DirectDelegation,
    Payment(String),
}
