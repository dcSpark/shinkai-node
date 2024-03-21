use std::fmt;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiNameError;

// Define your new error type
#[derive(Debug)]
pub enum NetworkJobQueueError {
    JobDequeueFailed(String),
    ContentParseFailed,
    AgentNotFound,
    NotAJobMessage,
    DatabaseError(String),
    Other(String), // For any other errors not covered above
    IOError(String),
    ShinkaDBUpgradeFailed,
}

// Implement std::fmt::Display for NetworkJobQueueError
impl fmt::Display for NetworkJobQueueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NetworkJobQueueError::JobDequeueFailed(ref job_id) => {
                write!(f, "Failed to dequeue job with ID: {}", job_id)
            }
            NetworkJobQueueError::ContentParseFailed => write!(f, "Failed to parse job content"),
            NetworkJobQueueError::AgentNotFound => write!(f, "Agent not found"),
            NetworkJobQueueError::NotAJobMessage => write!(f, "Not a job message"),
            NetworkJobQueueError::DatabaseError(ref err) => write!(f, "Database error: {}", err),
            NetworkJobQueueError::Other(ref err) => write!(f, "Error: {}", err),
            NetworkJobQueueError::ShinkaDBUpgradeFailed => write!(f, "ShinkaDB upgrade failed"),
            NetworkJobQueueError::IOError(ref err) => write!(f, "IO error: {}", err),
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