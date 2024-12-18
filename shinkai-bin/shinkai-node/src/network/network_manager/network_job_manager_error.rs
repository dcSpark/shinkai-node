use std::fmt;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiNameError;
use crate::network::agent_payments_manager::external_agent_offerings_manager::AgentOfferingManagerError;

// Define your new error type
#[derive(Debug)]
pub enum NetworkJobQueueError {
    JobDequeueFailed(String),
    ContentParseFailed,
    LLMProviderNotFound,
    NotAJobMessage,
    DatabaseError(String),
    Other(String), // For any other errors not covered above
    IOError(String),
    ShinkaDBUpgradeFailed,
    NonceParseFailed,
    DeserializationFailed(String),
    DecryptionFailed,
    SymmetricKeyNotFound(String),
    VectorFSUpgradeFailed,
    InvalidVRPath(String),
    ProxyConnectionInfoUpgradeFailed,
    ManagerUnavailable,
}

// Implement std::fmt::Display for NetworkJobQueueError
impl fmt::Display for NetworkJobQueueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NetworkJobQueueError::JobDequeueFailed(ref job_id) => {
                write!(f, "Failed to dequeue job with ID: {}", job_id)
            }
            NetworkJobQueueError::ContentParseFailed => write!(f, "Failed to parse job content"),
            NetworkJobQueueError::LLMProviderNotFound => write!(f, "LLMProvider not found"),
            NetworkJobQueueError::NotAJobMessage => write!(f, "Not a job message"),
            NetworkJobQueueError::DatabaseError(ref err) => write!(f, "Database error: {}", err),
            NetworkJobQueueError::Other(ref err) => write!(f, "Error: {}", err),
            NetworkJobQueueError::ShinkaDBUpgradeFailed => write!(f, "ShinkaDB upgrade failed"),
            NetworkJobQueueError::IOError(ref err) => write!(f, "IO error: {}", err),
            NetworkJobQueueError::NonceParseFailed => write!(f, "Failed to parse nonce"),
            NetworkJobQueueError::DeserializationFailed(ref err) => write!(f, "Deserialization failed: {}", err),
            NetworkJobQueueError::DecryptionFailed => write!(f, "Decryption failed"),
            NetworkJobQueueError::SymmetricKeyNotFound(ref err) => write!(f, "Symmetric key not found: {}", err),
            NetworkJobQueueError::VectorFSUpgradeFailed => write!(f, "VectorFS upgrade failed"),
            NetworkJobQueueError::InvalidVRPath(ref err) => write!(f, "Invalid VR path: {}", err),
            NetworkJobQueueError::ProxyConnectionInfoUpgradeFailed => write!(f, "Proxy Connection Info upgrade failed"),
            NetworkJobQueueError::ManagerUnavailable => write!(f, "Manager unavailable"),
        }
    }
}

// Implement std::error::Error for NetworkJobQueueError
impl std::error::Error for NetworkJobQueueError {}

impl From<std::io::Error> for NetworkJobQueueError {
    fn from(err: std::io::Error) -> NetworkJobQueueError {
        NetworkJobQueueError::Other(format!("{}", err))
    }
}

impl From<ShinkaiNameError> for NetworkJobQueueError {
    fn from(err: ShinkaiNameError) -> NetworkJobQueueError {
        NetworkJobQueueError::Other(format!("ShinkaiName error: {}", err))
    }
}

impl From<&str> for NetworkJobQueueError {
    fn from(err: &str) -> NetworkJobQueueError {
        NetworkJobQueueError::Other(err.to_string())
    }
}

impl From<AgentOfferingManagerError> for NetworkJobQueueError {
    fn from(err: AgentOfferingManagerError) -> NetworkJobQueueError {
        NetworkJobQueueError::Other(format!("AgentOfferingManager error: {}", err))
    }
}
