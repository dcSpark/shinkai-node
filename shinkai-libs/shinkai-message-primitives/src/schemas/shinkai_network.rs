use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum NetworkMessageType {
    ShinkaiMessage,
    VRKaiPathPair,
    ProxyMessage,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserNetworkNotification {
    pub message: String,
    pub datetime: DateTime<Utc>,
}
