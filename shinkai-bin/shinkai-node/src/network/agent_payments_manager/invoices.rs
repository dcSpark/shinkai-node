use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use super::shinkai_tool_offering::{ShinkaiToolOffering, UsageTypeInquiry};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Invoice {
    pub invoice_id: String,
    pub requester_name: ShinkaiName,
    pub shinkai_offering: ShinkaiToolOffering,
    pub expiration_time: DateTime<Utc>,
    pub status: InvoiceStatusEnum,
    pub payment: Option<InvoicePayment>,
    // Note: Maybe add something related to current estimated response times
    // average response time / congestion level or something like that
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
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct InvoicePayment {
    pub invoice_id: String,
    pub date_time: DateTime<Utc>,
    pub signed_invoice: String, // necessary? it acts like a written contract
    pub payment_id: String,
    pub payment_amount: String,
    pub payment_time: DateTime<Utc>,
    pub requester_node_name: ShinkaiName,
    // TODO: add payload and other stuff to be able to perform the job
    // This is sent by the requester by verified by us before getting added
}

impl Ord for InvoicePayment {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date_time.cmp(&other.date_time)
    }
}

impl PartialOrd for InvoicePayment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// TODO: Maybe create a trait that's shared between the two structs?
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct InvoiceRequest {
    pub requester_name: ShinkaiName,
    pub tool_key_name: String,
    pub usage_type_inquiry: UsageTypeInquiry,
    pub date_time: DateTime<Utc>,
    pub unique_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct InternalInvoiceRequest {
    pub requester_name: ShinkaiName,
    pub tool_key_name: String,
    pub usage_type_inquiry: UsageTypeInquiry,
    pub date_time: DateTime<Utc>,
    pub unique_id: String,
    pub secret_prehash: String,
}

impl InternalInvoiceRequest {
    pub fn new(requester_name: ShinkaiName, tool_key_name: String, usage_type_inquiry: UsageTypeInquiry) -> Self {
        // Generate a random number
        let random_number: u64 = rand::thread_rng().gen();

        // Encode the random number in base64
        let random_number_base64 = base64::encode(&random_number.to_be_bytes());

        // Use only the first half of the base64 encoded string
        let short_random_number = &random_number_base64[..random_number_base64.len() / 2];

        // Combine the short random number and timestamp to create a unique ID
        let unique_id = format!("{}", short_random_number);

        // Generate a secret prehash value (example: using the tool_key_name and random number)
        let secret_prehash = format!("{}{}", tool_key_name, random_number);

        Self {
            requester_name,
            tool_key_name,
            usage_type_inquiry,
            date_time: Utc::now(),
            unique_id,
            secret_prehash,
        }
    }

    pub fn to_invoice_request(&self) -> InvoiceRequest {
        InvoiceRequest {
            requester_name: self.requester_name.clone(),
            tool_key_name: self.tool_key_name.clone(),
            usage_type_inquiry: self.usage_type_inquiry.clone(),
            date_time: self.date_time,
            unique_id: self.unique_id.clone(),
        }
    }
}

// Note: do we need this? or can we just use the payment struct from the wallet manager?
// or merge it with something else like invoice_payment?

/// Enum representing the status of the payment.
pub enum PaymentStatusEnum {
    Pending,
    Confirmed,
    Failed,
}

/// Represents a payment.
pub struct Payment {
    /// The transaction hash of the payment.
    transaction_hash: String,
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
            transaction_hash,
            invoice_id,
            date_paid,
            status,
        }
    }
}
