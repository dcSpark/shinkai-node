use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ShinkaiEmbeddingError {
    #[error("Request failed")]
    RequestFailed(String),
    #[error("Invalid model architecture")]
    InvalidModelArchitecture,
    #[error("Unimplemented model dimensions")]
    UnimplementedModelDimensions(String),
    #[error("Failed embedding generation")]
    FailedEmbeddingGeneration(String),
}

impl From<reqwest::Error> for ShinkaiEmbeddingError {
    fn from(error: reqwest::Error) -> Self {
        ShinkaiEmbeddingError::RequestFailed(error.to_string())
    }
}
