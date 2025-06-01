use thiserror::Error;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct McpError {
    pub message: String,
}

impl From<Box<dyn std::error::Error + Send + Sync>> for McpError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        McpError {
            message: format!("{}", err),
        }
    }
}

impl From<Box<dyn std::error::Error + Send>> for McpError {
    fn from(err: Box<dyn std::error::Error + Send>) -> Self {
        McpError {
            message: format!("{}", err),
        }
    }
}

impl From<std::io::Error> for McpError {
    fn from(err: std::io::Error) -> Self {
        McpError {
            message: format!("{}", err),
        }
    }
}

impl From<rmcp::Error> for McpError {
    fn from(err: rmcp::Error) -> Self {
        McpError {
            message: format!("{}", err),
        }
    }
}

impl From<serde_json::Error> for McpError {
    fn from(err: serde_json::Error) -> Self {
        McpError {
            message: format!("{}", err),
        }
    }
}

impl From<rmcp::transport::sse::SseTransportError> for McpError {
    fn from(err: rmcp::transport::sse::SseTransportError) -> Self {
        McpError {
            message: format!("{}", err),
        }
    }
}

impl From<rmcp::service::ServiceError> for McpError {
    fn from(err: rmcp::service::ServiceError) -> Self {
        McpError {
            message: format!("{}", err),
        }
    }
}

impl From<String> for McpError {
    fn from(err: String) -> Self {
        McpError { message: err }
    }
}

