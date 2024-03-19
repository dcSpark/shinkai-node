use std::fmt;

// Define your new error type
#[derive(Debug)]
pub enum NetworkJobQueueError {
    JobDequeueFailed(String),
    ContentParseFailed,
    AgentNotFound,
    NotAJobMessage,
    DatabaseError(String),
    Other(String), // For any other errors not covered above
}

// Implement std::fmt::Display for NetworkJobQueueError
impl fmt::Display for NetworkJobQueueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NetworkJobQueueError::JobDequeueFailed(ref job_id) => write!(f, "Failed to dequeue job with ID: {}", job_id),
            NetworkJobQueueError::ContentParseFailed => write!(f, "Failed to parse job content"),
            NetworkJobQueueError::AgentNotFound => write!(f, "Agent not found"),
            NetworkJobQueueError::NotAJobMessage => write!(f, "Not a job message"),
            NetworkJobQueueError::DatabaseError(ref err) => write!(f, "Database error: {}", err),
            NetworkJobQueueError::Other(ref err) => write!(f, "Error: {}", err),
        }
    }
}

// Implement std::error::Error for NetworkJobQueueError
impl std::error::Error for NetworkJobQueueError {}
