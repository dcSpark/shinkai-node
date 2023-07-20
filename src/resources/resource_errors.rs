use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum ResourceError {
    InvalidChunkId,
    ResourceEmpty,
    FailedEmbeddingGeneration,
    NoChunkFound,
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ResourceError::InvalidChunkId => write!(f, "Invalid chunk id"),
            ResourceError::ResourceEmpty => write!(f, "Resource is empty"),
            ResourceError::FailedEmbeddingGeneration => write!(f, "Failed to generate embeddings"),
            ResourceError::NoChunkFound => write!(f, "No matching data chunk found"),
        }
    }
}

impl Error for ResourceError {}
