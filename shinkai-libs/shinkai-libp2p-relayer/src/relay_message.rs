use serde::{Deserialize, Serialize};
use shinkai_message_primitives::{
    schemas::shinkai_network::NetworkMessageType,
    shinkai_message::shinkai_message::ShinkaiMessage,
};

use crate::LibP2PRelayError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayMessage {
    pub identity: String,
    pub message_type: NetworkMessageType,
    pub payload: Vec<u8>,
    pub target_peer: Option<String>, // Target peer identity for routing
}

impl RelayMessage {
    pub fn new_proxy_message(identity: String) -> Self {
        Self {
            identity,
            message_type: NetworkMessageType::ProxyMessage,
            payload: Vec::new(),
            target_peer: None,
        }
    }

    pub fn new_shinkai_message(
        identity: String,
        message: ShinkaiMessage,
        target_peer: Option<String>,
    ) -> Result<Self, LibP2PRelayError> {
        let payload = serde_json::to_vec(&message)?;
        Ok(Self {
            identity,
            message_type: NetworkMessageType::ShinkaiMessage,
            payload,
            target_peer,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, LibP2PRelayError> {
        serde_json::to_vec(self).map_err(LibP2PRelayError::from)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LibP2PRelayError> {
        serde_json::from_slice(bytes).map_err(LibP2PRelayError::from)
    }

    pub fn extract_shinkai_message(&self) -> Result<ShinkaiMessage, LibP2PRelayError> {
        if self.message_type != NetworkMessageType::ShinkaiMessage {
            return Err(LibP2PRelayError::InvalidData(
                "Message is not a ShinkaiMessage".to_string(),
            ));
        }
        serde_json::from_slice(&self.payload).map_err(LibP2PRelayError::from)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayRegistration {
    pub identity: String,
    pub peer_id: String,
    pub public_key_hex: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl RelayResponse {
    pub fn success(message: String) -> Self {
        Self {
            success: true,
            message,
            data: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            message,
            data: None,
        }
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }
} 