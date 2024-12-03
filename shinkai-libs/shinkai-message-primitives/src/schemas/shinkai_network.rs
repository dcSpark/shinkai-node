use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum NetworkMessageType {
    ShinkaiMessage,
    ProxyMessage,
}