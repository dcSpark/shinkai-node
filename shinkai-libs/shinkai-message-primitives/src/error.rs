use std::fmt;

#[derive(Debug)]
pub enum WebSocketManagerError {
    ValidationError(String),
    AccessDenied(String),
    EncryptionError(String),
    IdentityNotFound(String),
    IdentityManagerError(String),
    InvalidSharedKey(String),
    DatabaseError(String),
    SerializationError(String),
}

impl fmt::Display for WebSocketManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebSocketManagerError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            WebSocketManagerError::AccessDenied(msg) => write!(f, "Access denied: {}", msg),
            WebSocketManagerError::EncryptionError(msg) => write!(f, "Encryption error: {}", msg),
            WebSocketManagerError::IdentityNotFound(msg) => write!(f, "Identity not found: {}", msg),
            WebSocketManagerError::IdentityManagerError(msg) => write!(f, "Identity manager error: {}", msg),
            WebSocketManagerError::InvalidSharedKey(msg) => write!(f, "Invalid shared key: {}", msg),
            WebSocketManagerError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            WebSocketManagerError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for WebSocketManagerError {}

impl From<serde_json::Error> for WebSocketManagerError {
    fn from(err: serde_json::Error) -> Self {
        WebSocketManagerError::SerializationError(err.to_string())
    }
}

impl From<APIError> for WebSocketManagerError {
    fn from(err: APIError) -> Self {
        WebSocketManagerError::ValidationError(err.to_string())
    }
}
