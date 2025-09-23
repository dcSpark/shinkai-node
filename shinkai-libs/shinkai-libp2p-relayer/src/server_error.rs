use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LibP2PRelayError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Timeout")]
    Timeout,

    #[error("Unknown message type: {0}")]
    UnknownMessageType(u8),

    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("LibP2P error: {0}")]
    LibP2PError(String),

    #[error("Registry error: {0}")]
    RegistryError(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Message delivery failed: {0}")]
    MessageDeliveryFailed(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

impl From<String> for LibP2PRelayError {
    fn from(s: String) -> Self {
        LibP2PRelayError::InvalidData(s)
    }
}

impl From<&str> for LibP2PRelayError {
    fn from(s: &str) -> Self {
        LibP2PRelayError::InvalidData(s.to_string())
    }
}
