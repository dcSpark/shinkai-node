use serde_json::Error as SerdeError;
use std::error::Error;
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum VectorResourceError {
    InvalidChunkId,
    VectorResourceEmpty,
    FailedEmbeddingGeneration(String),
    NoChunkFound,
    InvalidModelArchitecture,
    FailedJSONParsing,
    FailedCSVParsing,
    FailedPDFParsing,
    InvalidVectorResourceBaseType,
    RegexError(regex::Error),
    RequestFailed(String),
    NoEmbeddingProvided,
    DataIsNonMatchingType,
}

impl fmt::Display for VectorResourceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            VectorResourceError::InvalidChunkId => write!(f, "Invalid chunk id"),
            VectorResourceError::VectorResourceEmpty => write!(f, "VectorResource is empty"),
            VectorResourceError::FailedEmbeddingGeneration(ref s) => write!(f, "Failed to generate embeddings: {}", s),
            VectorResourceError::NoChunkFound => write!(f, "No matching data chunk found"),
            VectorResourceError::InvalidModelArchitecture => {
                write!(f, "An unsupported model architecture was specified.")
            }
            VectorResourceError::FailedJSONParsing => write!(f, "Failed JSON parsing."),
            VectorResourceError::FailedCSVParsing => write!(f, "Failed CSV parsing."),
            VectorResourceError::FailedPDFParsing => write!(f, "Failed PDF parsing."),
            VectorResourceError::NoEmbeddingProvided => write!(f, "No embedding provided."),
            VectorResourceError::InvalidVectorResourceBaseType => write!(
                f,
                "The resource type does not match any of the VectorResourceBaseTypes."
            ),
            VectorResourceError::RegexError(ref e) => write!(f, "Regex error: {}", e),
            VectorResourceError::RequestFailed(ref e) => write!(f, "HTTP request failed: {}", e),
            VectorResourceError::DataIsNonMatchingType => {
                write!(f, "Data inside of the DataChunk is of a different type than requested.")
            }
        }
    }
}

impl Error for VectorResourceError {}

impl From<regex::Error> for VectorResourceError {
    fn from(err: regex::Error) -> VectorResourceError {
        VectorResourceError::RegexError(err)
    }
}

impl From<SerdeError> for VectorResourceError {
    fn from(error: SerdeError) -> Self {
        match error.classify() {
            serde_json::error::Category::Io => VectorResourceError::FailedJSONParsing,
            serde_json::error::Category::Syntax => VectorResourceError::FailedJSONParsing,
            serde_json::error::Category::Data => VectorResourceError::FailedJSONParsing,
            serde_json::error::Category::Eof => VectorResourceError::FailedJSONParsing,
        }
    }
}
