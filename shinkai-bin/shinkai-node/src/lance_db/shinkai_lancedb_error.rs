use std::fmt;
use lancedb::Error as LanceDbError;
use crate::tools::error::ToolError;

#[derive(Debug)]
pub enum ShinkaiLanceDBError {
    LanceDB(LanceDbError),
    Schema(String),
    Arrow(String),
}

impl fmt::Display for ShinkaiLanceDBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShinkaiLanceDBError::LanceDB(err) => write!(f, "LanceDB error: {}", err),
            ShinkaiLanceDBError::Schema(err) => write!(f, "Schema error: {}", err),
            ShinkaiLanceDBError::Arrow(err) => write!(f, "Arrow error: {}", err),
        }
    }
}

impl std::error::Error for ShinkaiLanceDBError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShinkaiLanceDBError::LanceDB(err) => Some(err),
            _ => None,
        }
    }
}

impl From<LanceDbError> for ShinkaiLanceDBError {
    fn from(err: LanceDbError) -> Self {
        ShinkaiLanceDBError::LanceDB(err)
    }
}

impl From<ShinkaiLanceDBError> for ToolError {
    fn from(error: ShinkaiLanceDBError) -> Self {
        ToolError::DatabaseError(error.to_string())
    }
}