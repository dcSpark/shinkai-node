use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShinkaiSubscriptionReq {
    pub minimum_token_delegation: Option<u64>,
    pub minimum_time_delegated_hours: Option<u64>,
    pub monthly_payment: Option<PaymentOption>,
    pub is_free: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaymentOption {
    USD(f64),
    KAITokens(u64),
}