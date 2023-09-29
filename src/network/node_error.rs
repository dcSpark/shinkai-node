use crate::{agent::error::AgentError, db::db_errors::ShinkaiDBError};
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxNameError, shinkai_name::ShinkaiNameError},
    shinkai_message::shinkai_message_error::ShinkaiMessageError,
};

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

impl From<AgentError> for NodeError {
    fn from(error: AgentError) -> Self {
        NodeError {
            message: format!("AgentError occurred: {}", error),
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
