use std::{collections::VecDeque, fmt, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_message_primitives::{schemas::shinkai_tool_offering::UsageType, shinkai_message::shinkai_message_schemas::WSTopic};
use shinkai_sheet::sheet::CellUpdateInfo;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    ShinkaiMessage,
    Stream,
    Sheet,
    Widget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSMessagePayload {
    pub message_type: MessageType,
    pub inbox: String,
    pub message: Option<String>,
    pub error_message: Option<String>,
    pub metadata: Option<WSMetadata>,
    pub widget: Option<Value>,
    pub is_stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WSMetadata {
    pub id: Option<String>,
    pub is_done: bool,
    pub done_reason: Option<String>,
    pub total_duration: Option<u64>,
    pub eval_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMetadata {
    pub tool_key: String,
    pub description: String,
    pub usage_type: UsageType,
    pub invoice_id: String,
    pub invoice: Value,
    pub function_args: Value,
    pub wallet_balances: Value,
}

#[derive(Debug)]
pub enum WebSocketManagerError {
    UserValidationFailed(String),
    AccessDenied(String),
    InvalidSharedKey(String),
}

impl fmt::Display for WebSocketManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WebSocketManagerError::UserValidationFailed(msg) => write!(f, "User validation failed: {}", msg),
            WebSocketManagerError::AccessDenied(msg) => write!(f, "Access denied: {}", msg),
            WebSocketManagerError::InvalidSharedKey(msg) => write!(f, "Invalid shared key: {}", msg),
        }
    }
}

#[async_trait]
pub trait WSUpdateHandler {
    async fn queue_message(
        &self,
        topic: WSTopic,
        subtopic: String,
        update: String,
        metadata: WSMessageType,
        is_stream: bool,
    );
}

#[derive(Debug, Clone)]
pub enum WSMessageType {
    Metadata(WSMetadata),
    Sheet(CellUpdateInfo),
    Widget(WidgetMetadata),
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WidgetMetadata {
    PaymentRequest(PaymentMetadata),
}

pub type MessageQueue = Arc<Mutex<VecDeque<(WSTopic, String, String, WSMessageType, bool)>>>;