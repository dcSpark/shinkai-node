use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::shinkai_message::shinkai_message::ShinkaiMessage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryMessage {
    pub retry_count: u32,
    pub message: ShinkaiMessage,
    pub save_to_db_flag: bool,
    pub peer: (SocketAddr, String),
}
