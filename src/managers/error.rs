use crate::db::db_errors::ShinkaiDBError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiNameError;
use std::fmt;

#[derive(Debug)]
pub enum JobManagerError {
    NotAJobMessage,
    JobNotFound,
    JobCreationDeserializationFailed,
    JobMessageDeserializationFailed,
    JobPreMessageDeserializationFailed,
    MessageTypeParseFailed,
    IO(String),
    ShinkaiDB(ShinkaiDBError),
    ShinkaiNameError(ShinkaiNameError),
    AgentNotFound,
    ContentParseFailed,
}

impl fmt::Display for JobManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JobManagerError::NotAJobMessage => write!(f, "Message is not a job message"),
            JobManagerError::JobNotFound => write!(f, "Job not found"),
            JobManagerError::JobCreationDeserializationFailed => {
                write!(f, "Failed to deserialize JobCreationInfo message")
            }
            JobManagerError::JobMessageDeserializationFailed => write!(f, "Failed to deserialize JobMessage"),
            JobManagerError::JobPreMessageDeserializationFailed => write!(f, "Failed to deserialize JobPreMessage"),
            JobManagerError::MessageTypeParseFailed => write!(f, "Could not parse message type"),
            JobManagerError::IO(err) => write!(f, "IO error: {}", err),
            JobManagerError::ShinkaiDB(err) => write!(f, "Shinkai DB error: {}", err),
            JobManagerError::AgentNotFound => write!(f, "Agent not found"),
            JobManagerError::ContentParseFailed => write!(f, "Failed to parse content"),
            JobManagerError::ShinkaiNameError(err) => write!(f, "ShinkaiName error: {}", err),
        }
    }
}

impl std::error::Error for JobManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JobManagerError::ShinkaiDB(err) => Some(err),
            JobManagerError::ShinkaiNameError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<Box<dyn std::error::Error>> for JobManagerError {
    fn from(err: Box<dyn std::error::Error>) -> JobManagerError {
        JobManagerError::IO(err.to_string())
    }
}

impl From<ShinkaiDBError> for JobManagerError {
    fn from(err: ShinkaiDBError) -> JobManagerError {
        JobManagerError::ShinkaiDB(err)
    }
}

impl From<ShinkaiNameError> for JobManagerError {
    fn from(err: ShinkaiNameError) -> JobManagerError {
        JobManagerError::ShinkaiNameError(err)
    }
}
