use std::fmt::{self};

use shinkai_crypto_identities::ShinkaiRegistryError;
use shinkai_message_primitives::shinkai_message::shinkai_message_error::ShinkaiMessageError;

#[derive(Debug)]
pub enum NetworkMessageError {
    ReadError(std::io::Error),
    Utf8Error(std::string::FromUtf8Error),
    UnknownMessageType(u8),
    InvalidData(String),
    ShinkaiMessageError(ShinkaiMessageError),
    ShinkaiRegistryError(ShinkaiRegistryError),
    CustomError(String),
    SendError,
    ConnectionClosed,
    IoError(std::io::Error),
    Timeout,
    EncryptionError(String),
    RecipientLoopError(String),
}

impl fmt::Display for NetworkMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkMessageError::ReadError(err) => write!(f, "Failed to read exact bytes from socket: {}", err),
            NetworkMessageError::Utf8Error(err) => write!(f, "Invalid UTF-8 sequence: {}", err),
            NetworkMessageError::UnknownMessageType(t) => write!(f, "Unknown message type: {}", t),
            NetworkMessageError::InvalidData(msg) => write!(f, "Invalid data received: {}", msg),
            NetworkMessageError::ShinkaiMessageError(err) => write!(f, "Shinkai message error: {}", err),
            NetworkMessageError::ShinkaiRegistryError(err) => write!(f, "Shinkai registry error: {}", err),
            NetworkMessageError::CustomError(msg) => write!(f, "{}", msg),
            NetworkMessageError::SendError => write!(f, "Failed to send message"),
            NetworkMessageError::ConnectionClosed => write!(f, "Connection closed"),
            NetworkMessageError::IoError(err) => write!(f, "I/O error: {}", err),
            NetworkMessageError::Timeout => write!(f, "Operation timed out"),
            NetworkMessageError::EncryptionError(msg) => write!(f, "{}", msg),
            NetworkMessageError::RecipientLoopError(msg) => write!(f, "Trying to relay a message using the relayer public ip/dns {}", msg),
        }
    }
}

impl std::error::Error for NetworkMessageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NetworkMessageError::ReadError(err) => Some(err),
            NetworkMessageError::Utf8Error(err) => Some(err),
            NetworkMessageError::UnknownMessageType(_) => None,
            NetworkMessageError::InvalidData(_) => None,
            NetworkMessageError::ShinkaiMessageError(err) => Some(err),
            NetworkMessageError::ShinkaiRegistryError(err) => Some(err),
            NetworkMessageError::CustomError(_) => None,
            NetworkMessageError::SendError => None,
            NetworkMessageError::ConnectionClosed => None,
            NetworkMessageError::IoError(err) => Some(err),
            NetworkMessageError::Timeout => None,
            NetworkMessageError::EncryptionError(_) => None,
            NetworkMessageError::RecipientLoopError(_) => None,
        }
    }
}

impl From<std::io::Error> for NetworkMessageError {
    fn from(err: std::io::Error) -> NetworkMessageError {
        NetworkMessageError::ReadError(err)
    }
}

impl From<std::string::FromUtf8Error> for NetworkMessageError {
    fn from(err: std::string::FromUtf8Error) -> NetworkMessageError {
        NetworkMessageError::Utf8Error(err)
    }
}

impl From<ShinkaiMessageError> for NetworkMessageError {
    fn from(error: ShinkaiMessageError) -> Self {
        NetworkMessageError::ShinkaiMessageError(error)
    }
}

impl From<ShinkaiRegistryError> for NetworkMessageError {
    fn from(error: ShinkaiRegistryError) -> Self {
        NetworkMessageError::ShinkaiRegistryError(error)
    }
}

impl From<&str> for NetworkMessageError {
    fn from(error: &str) -> Self {
        NetworkMessageError::CustomError(error.to_string())
    }
}
