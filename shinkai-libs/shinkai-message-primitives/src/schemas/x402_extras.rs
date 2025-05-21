use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct X402JobFailedPayload {
    pub job_id: String, // The JTI of the payment
    pub error_message: String,
    pub error_code: Option<String>, // e.g., "SettlementFailed", "ToolExecutionFailed"
}
