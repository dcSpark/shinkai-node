use thiserror::Error;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct McpError {
    pub message: String,
}
