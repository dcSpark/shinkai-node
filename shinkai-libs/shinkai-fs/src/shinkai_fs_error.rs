use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
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
}

impl From<reqwest::Error> for ShinkaiFsError {
    fn from(error: reqwest::Error) -> Self {
        ShinkaiFsError::RequestFailed(error.to_string())
    }
}
