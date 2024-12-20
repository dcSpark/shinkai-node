use shinkai_sqlite::errors::SqliteManagerError;
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShinkaiFsError {
    #[error("File not found")]
    FileNotFound,
    #[error("Invalid model architecture")]
    InvalidModelArchitecture,
    #[error("Unimplemented model dimensions")]
    UnimplementedModelDimensions(String),
    #[error("Request failed")]
    RequestFailed(String),
    #[error("Failed to generate embeddings")]
    FailedEmbeddingGeneration(String),
    #[error("IO error occurred: {0}")]
    Io(#[from] io::Error),
    #[error("Database error: {0}")]
    Database(#[from] SqliteManagerError),
    #[error("File not found in database")]
    FileNotFoundInDatabase,
    #[error("File not found on filesystem")]
    FileNotFoundOnFilesystem,
    #[error("Folder not found on filesystem")]
    FolderNotFoundOnFilesystem,
    #[error("Cannot move folder into itself")]
    InvalidFolderMove,
}

impl From<reqwest::Error> for ShinkaiFsError {
    fn from(error: reqwest::Error) -> Self {
        ShinkaiFsError::RequestFailed(error.to_string())
    }
}
