// use lancedb::Error as LanceDbError;
// use shinkai_tools_primitives::tools::error::ToolError;
// use shinkai_vector_resources::resource_errors::VRError;
// use std::fmt;

// #[derive(Debug)]
// pub enum ShinkaiLanceDBError {
//     LanceDB(LanceDbError),
//     Schema(String),
//     Arrow(String),
//     ToolError(String),
//     InvalidPath(String),
//     ShinkaiDBError(String),
//     RocksDBError(String),
//     DatabaseError(String),
//     EmbeddingGenerationError(String),
// }

// impl fmt::Display for ShinkaiLanceDBError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             ShinkaiLanceDBError::LanceDB(err) => write!(f, "LanceDB error: {}", err),
//             ShinkaiLanceDBError::Schema(err) => write!(f, "Schema error: {}", err),
//             ShinkaiLanceDBError::Arrow(err) => write!(f, "Arrow error: {}", err),
//             ShinkaiLanceDBError::ToolError(err) => write!(f, "Tool error: {}", err),
//             ShinkaiLanceDBError::InvalidPath(err) => write!(f, "Invalid path error: {}", err),
//             ShinkaiLanceDBError::ShinkaiDBError(err) => write!(f, "ShinkaiDB error: {}", err),
//             ShinkaiLanceDBError::RocksDBError(err) => write!(f, "RocksDB error: {}", err),
//             ShinkaiLanceDBError::DatabaseError(err) => write!(f, "Database error: {}", err),
//             ShinkaiLanceDBError::EmbeddingGenerationError(err) => write!(f, "Embedding generation error: {}", err),
//         }
//     }
// }

// impl std::error::Error for ShinkaiLanceDBError {
//     fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
//         match self {
//             ShinkaiLanceDBError::LanceDB(err) => Some(err),
//             _ => None,
//         }
//     }
// }
    
// impl From<LanceDbError> for ShinkaiLanceDBError {
//     fn from(err: LanceDbError) -> Self {
//         ShinkaiLanceDBError::LanceDB(err)
//     }
// }

// impl From<ShinkaiLanceDBError> for ToolError {
//     fn from(error: ShinkaiLanceDBError) -> Self {
//         ToolError::DatabaseError(error.to_string())
//     }
// }

// // Add this implementation
// impl From<VRError> for ShinkaiLanceDBError {
//     fn from(err: VRError) -> Self {
//         ShinkaiLanceDBError::Schema(err.to_string())
//     }
// }
