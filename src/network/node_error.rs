use shinkai_message_primitives::{shinkai_message::shinkai_message_error::ShinkaiMessageError, schemas::{inbox_name::InboxNameError, shinkai_name::ShinkaiNameError}};
use crate::{managers::job_manager::JobManagerError, db::db_errors::ShinkaiDBError};


#[derive(Debug)]
pub struct NodeError {
    pub message: String,
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for NodeError {}

impl From<Box<dyn std::error::Error + Send + Sync>> for NodeError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> NodeError {
        NodeError {
            message: format!("{}", err),
        }
    }
}

impl From<std::io::Error> for NodeError {
    fn from(err: std::io::Error) -> NodeError {
        NodeError {
            message: format!("{}", err),
        }
    }
}

impl From<ShinkaiMessageError> for NodeError {
    fn from(err: ShinkaiMessageError) -> NodeError {
        NodeError {
            message: format!("{}", err),
        }
    }
}

impl From<JobManagerError> for NodeError {
    fn from(error: JobManagerError) -> Self {
        NodeError {
            message: format!("JobManagerError occurred: {}", error),
        }
    }
}

impl From<ShinkaiDBError> for NodeError {
    fn from(error: ShinkaiDBError) -> Self {
        NodeError {
            message: format!("Database error: {}", error),
        }
    }
}

impl From<InboxNameError> for NodeError {
    fn from(err: InboxNameError) -> NodeError {
        NodeError {
            message: format!("InboxNameError: {}", err),
        }
    }
}

impl From<ShinkaiNameError> for NodeError {
    fn from(error: ShinkaiNameError) -> Self {
        NodeError {
            message: format!("ShinkaiNameError: {}", error.to_string()),
        }
    }
}