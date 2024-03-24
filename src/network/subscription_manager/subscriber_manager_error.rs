use std::fmt;

use shinkai_vector_resources::resource_errors::VRError;

use crate::vector_fs::vector_fs_error::VectorFSError;

// Define a custom error for SubscriberManager operations
#[derive(Debug)]
pub enum SubscriberManagerError {
    DatabaseError(String),
    FileSystemError(String),
    MessageProcessingError(String),
    VectorFSNotAvailable(String),
    InvalidRequest(String),
    NodeNotAvailable(String),
    DatabaseNotAvailable(String),
    VectorFSError(String),
    VRError(String),
    SharedFolderNotFound(String),
    IdentityManagerUnavailable,
    AddressUnavailable(String),
    PaymentNotValid(String),
    SubscriptionFailed(String),
}

impl fmt::Display for SubscriberManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SubscriberManagerError::DatabaseError(e) => write!(f, "Database error: {}", e),
            SubscriberManagerError::FileSystemError(e) => write!(f, "File system error: {}", e),
            SubscriberManagerError::MessageProcessingError(e) => write!(f, "Message processing error: {}", e),
            SubscriberManagerError::VectorFSNotAvailable(e) => write!(f, "VectorFS not available: {}", e),
            SubscriberManagerError::InvalidRequest(e) => write!(f, "Invalid request: {}", e),
            SubscriberManagerError::NodeNotAvailable(e) => write!(f, "Node not available: {}", e),
            SubscriberManagerError::DatabaseNotAvailable(e) => write!(f, "Database not available: {}", e),
            SubscriberManagerError::VectorFSError(e) => write!(f, "VectorFS error: {}", e),
            SubscriberManagerError::VRError(e) => write!(f, "VR error: {}", e),
            SubscriberManagerError::SharedFolderNotFound(e) => write!(f, "Shared folder not found: {}", e),
            SubscriberManagerError::IdentityManagerUnavailable => write!(f, "Identity manager unavailable"),
            SubscriberManagerError::AddressUnavailable(e) => write!(f, "Address unavailable: {}", e),
            SubscriberManagerError::PaymentNotValid(e) => write!(f, "Payment not valid: {}", e),
            SubscriberManagerError::SubscriptionFailed(e) => write!(f, "Subscription failed: {}", e),
        }
    }
}

impl std::error::Error for SubscriberManagerError {}

impl From<VectorFSError> for SubscriberManagerError {
    fn from(error: VectorFSError) -> Self {
        SubscriberManagerError::VectorFSNotAvailable(error.to_string())
    }
}

impl From<VRError> for SubscriberManagerError {
    fn from(error: VRError) -> Self {
        SubscriberManagerError::VRError(error.to_string())
    }
}

impl From<String> for SubscriberManagerError {
    fn from(error: String) -> Self {
        SubscriberManagerError::MessageProcessingError(error)
    }
}