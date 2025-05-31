use std::{cmp::Ordering, fmt};

use rand::RngCore;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Generates a random 32-byte nonce encoded as a hex string prefixed with `0x`.
/// This mimics the nonce used by x402 payment requests and is used as the
/// unique identifier for invoices.
pub fn generate_x402_nonce() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("0x{}", hex::encode(bytes))
}

use super::{
    shinkai_name::ShinkaiName, shinkai_tool_offering::{ShinkaiToolOffering, UsageTypeInquiry}, tool_router_key::ToolRouterKey, wallet_mixed::PublicAddress
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Invoice {
    pub invoice_id: String,
    pub provider_name: ShinkaiName,
    pub requester_name: ShinkaiName,
    pub usage_type_inquiry: UsageTypeInquiry,
    pub shinkai_offering: ShinkaiToolOffering,
    pub request_date_time: DateTime<Utc>,
    pub invoice_date_time: DateTime<Utc>,
    pub expiration_time: DateTime<Utc>,
    pub status: InvoiceStatusEnum,
    pub payment: Option<Payment>,
    pub address: PublicAddress,
    pub tool_data: Option<Value>, // expected to have all of the required input_args: Vec<ToolArgument>,
    pub response_date_time: Option<DateTime<Utc>>, // when the response was sent back to the requester
    pub result_str: Option<String>, /* depending on the tool, the result varies
                                   * Note: Maybe add something related to current estimated response times
                                   * average response time / congestion level or something like that */
}

impl PartialOrd for Invoice {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.invoice_date_time.cmp(&other.invoice_date_time))
    }
}

impl Invoice {
    /// Updates the status of the invoice.
    pub fn update_status(&mut self, new_status: InvoiceStatusEnum) {
        self.status = new_status;
    }
}

/// Enum representing the status of the invoice.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum InvoiceStatusEnum {
    Pending,
    Paid,
    Failed,
    Processed,
}

#[derive(Debug)]
pub enum InvoiceError {
    InvalidToolKeyFormat,
    NodeNameMismatch { expected: String, found: String },
    OperationFailed(String),
}

impl fmt::Display for InvoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvoiceError::InvalidToolKeyFormat => write!(f, "Invalid tool_key_name format"),
            InvoiceError::NodeNameMismatch { expected, found } => {
                write!(f, "Node name mismatch: expected {}, found {}", expected, found)
            }
            InvoiceError::OperationFailed(msg) => write!(f, "Operation failed: {}", msg),
        }
    }
}

impl std::error::Error for InvoiceError {}

// TODO: Maybe create a trait that's shared between the two structs?
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct InvoiceRequest {
    pub requester_name: ShinkaiName,
    pub provider_name: ShinkaiName,
    pub tool_key_name: String,
    pub usage_type_inquiry: UsageTypeInquiry,
    pub request_date_time: DateTime<Utc>,
    pub unique_id: String,
}

impl InvoiceRequest {
    pub fn validate_and_convert_tool_key(&self, node_name: &ShinkaiName) -> Result<String, InvoiceError> {
        // Extract the node name from the tool_key_name
        let parts: Vec<&str> = self.tool_key_name.split(":::").collect();
        if parts.len() < 3 {
            return Err(InvoiceError::OperationFailed(
                "Invalid tool_key_name format".to_string(),
            ));
        }

        let node_name_part = parts[0];
        let author = parts[1];
        let tool_name = parts[2];

        // Normalize both node_name_part and node_name for comparison
        let normalized_node_name_part = ToolRouterKey::sanitize(node_name_part);
        let normalized_node_name = ToolRouterKey::sanitize(&node_name.to_string());

        // Validate that the normalized node name part matches our normalized node_name
        if normalized_node_name_part != normalized_node_name {
            return Err(InvoiceError::OperationFailed(format!(
                "Node name in tool_key_name does not match our node_name (expected: {}, found: {})",
                normalized_node_name, normalized_node_name_part
            )));
        }

        // Convert the tool_key_name to the actual tool_key_name
        let actual_tool_key_name = format!("local:::{}:::{}", author, tool_name);

        Ok(actual_tool_key_name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct InternalInvoiceRequest {
    pub provider_name: ShinkaiName,
    pub requester_name: ShinkaiName,
    pub tool_key_name: String,
    pub usage_type_inquiry: UsageTypeInquiry,
    pub date_time: DateTime<Utc>,
    pub unique_id: String,
}

impl InternalInvoiceRequest {
    pub fn new(
        provider: ShinkaiName,
        requester_name: ShinkaiName,
        tool_key_name: String,
        usage_type_inquiry: UsageTypeInquiry,
    ) -> Self {
        // Generate the unique invoice identifier using an x402-style nonce
        let unique_id = generate_x402_nonce();

        Self {
            provider_name: provider,
            requester_name,
            tool_key_name,
            usage_type_inquiry,
            date_time: Utc::now(),
            unique_id,
        }
    }

    pub fn to_invoice_request(&self) -> InvoiceRequest {
        InvoiceRequest {
            provider_name: self.provider_name.clone(),
            requester_name: self.requester_name.clone(),
            tool_key_name: self.tool_key_name.clone(),
            usage_type_inquiry: self.usage_type_inquiry.clone(),
            request_date_time: self.date_time,
            unique_id: self.unique_id.clone(),
        }
    }
}

// Note: do we need this? or can we just use the payment struct from the wallet manager?
// or merge it with something else like invoice_payment?

/// Enum representing the status of the payment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub enum PaymentStatusEnum {
    Pending,
    Signed,
    Failed,
}

/// Represents a payment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct Payment {
    /// The transaction hash of the payment.
    transaction_signed: String,
    /// The unique ID of the invoice associated with the payment.
    invoice_id: String,
    /// The date the payment was made (ISO8601 format).
    date_paid: Option<String>,
    /// The status of the payment.
    status: PaymentStatusEnum,
}

impl Payment {
    /// Creates a new payment.
    pub fn new(
        transaction_hash: String,
        invoice_id: String,
        date_paid: Option<String>,
        status: PaymentStatusEnum,
    ) -> Self {
        Payment {
            transaction_signed: transaction_hash,
            invoice_id,
            date_paid,
            status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct InvoiceRequestNetworkError {
    pub invoice_id: String,
    pub provider_name: ShinkaiName,
    pub requester_name: ShinkaiName,
    pub request_date_time: DateTime<Utc>,
    pub response_date_time: DateTime<Utc>,
    pub user_error_message: Option<String>,
    pub error_message: String,
}
