use shinkai_sqlite::errors::SqliteManagerError;

use super::subscription_file_uploader::{FileDestinationError, FileTransferError};
use std::fmt;

#[derive(Debug)]
pub enum HttpUploadError {
    #[allow(dead_code)]
    SubscriptionNotFound,
    FileSystemError(String),
    ErrorGettingFolderContents,
    NetworkError,
    IOError(std::io::Error),
    VectorFSNotAvailable(String),
    DatabaseError(String),
    InvalidRequest(String),
    TaskJoinError(String),
    InvalidSubscriptionRequirement(String),
    MissingSubscriptionRequirement(String),
    SerdeJsonError(serde_json::Error),
    ShinkaiDBError(SqliteManagerError),
}

impl std::error::Error for HttpUploadError {}

impl fmt::Display for HttpUploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            HttpUploadError::SubscriptionNotFound => write!(f, "Subscription not found"),
            HttpUploadError::FileSystemError(ref err) => write!(f, "File system error: {}", err),
            HttpUploadError::ErrorGettingFolderContents => write!(f, "Error getting folder contents"),
            HttpUploadError::NetworkError => write!(f, "Network operation failed"),
            HttpUploadError::IOError(ref err) => write!(f, "I/O error: {}", err),
            HttpUploadError::VectorFSNotAvailable(ref err) => write!(f, "VectorFS instance is not available: {}", err),
            HttpUploadError::DatabaseError(ref err) => write!(f, "Database error: {}", err),
            HttpUploadError::InvalidRequest(ref err) => write!(f, "Invalid request: {}", err),
            HttpUploadError::TaskJoinError(ref err) => write!(f, "Task join error: {}", err),
            HttpUploadError::InvalidSubscriptionRequirement(ref err) => {
                write!(f, "Invalid subscription requirement: {}", err)
            }
            HttpUploadError::MissingSubscriptionRequirement(ref err) => {
                write!(f, "Missing subscription requirement: {}", err)
            }
            HttpUploadError::SerdeJsonError(ref err) => write!(f, "Serde JSON error: {}", err),
            HttpUploadError::ShinkaiDBError(ref err) => write!(f, "ShinkaiDB error: {}", err),
        }
    }
}

impl From<serde_json::Error> for HttpUploadError {
    fn from(error: serde_json::Error) -> Self {
        HttpUploadError::SerdeJsonError(error)
    }
}

impl From<SqliteManagerError> for HttpUploadError {
    fn from(error: SqliteManagerError) -> Self {
        HttpUploadError::ShinkaiDBError(error)
    }
}

impl From<&str> for HttpUploadError {
    fn from(err: &str) -> Self {
        HttpUploadError::FileSystemError(err.to_string()) // Convert the &str error message to String
    }
}

impl From<FileTransferError> for HttpUploadError {
    fn from(err: FileTransferError) -> Self {
        match err {
            FileTransferError::NetworkError(_) => HttpUploadError::NetworkError,
            FileTransferError::InvalidHeaderValue => HttpUploadError::NetworkError,
            FileTransferError::Other(e) => HttpUploadError::FileSystemError(format!("File transfer error: {}", e)), // Provide a formatted error message
        }
    }
}

impl From<FileDestinationError> for HttpUploadError {
    fn from(err: FileDestinationError) -> Self {
        match err {
            FileDestinationError::JsonError(e) => {
                HttpUploadError::FileSystemError(format!("JSON parsing error: {}", e))
            }
            FileDestinationError::InvalidInput(e) => HttpUploadError::FileSystemError(format!("Invalid input: {}", e)),
            FileDestinationError::UnknownTypeField => {
                HttpUploadError::FileSystemError("Unknown type field in file destination".to_string())
            }
            FileDestinationError::FileSystemError(e) => {
                HttpUploadError::FileSystemError(format!("File system error: {}", e))
            } // Now correctly handles the new variant
        }
    }
}

impl From<std::io::Error> for HttpUploadError {
    fn from(err: std::io::Error) -> Self {
        HttpUploadError::IOError(err)
    }
}

impl From<tokio::task::JoinError> for HttpUploadError {
    fn from(err: tokio::task::JoinError) -> Self {
        HttpUploadError::TaskJoinError(format!("Task failed with JoinError: {}", err))
    }
}

impl From<shinkai_vector_resources::resource_errors::VRError> for HttpUploadError {
    fn from(err: shinkai_vector_resources::resource_errors::VRError) -> Self {
        HttpUploadError::FileSystemError(err.to_string())
    }
}
