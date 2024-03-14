use std::fmt;

// Define a custom error for SubscriberManager operations
#[derive(Debug)]
pub enum SubscriberManagerError {
    DatabaseError(String),
    FileSystemError(String),
    MessageProcessingError(String),
    VectorFSNotAvailable(String),
    InvalidRequest(String),
    NodeNotAvailable(String),
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
        }
    }
}

impl std::error::Error for SubscriberManagerError {}
