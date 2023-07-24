use rocksdb::Error as RocksError;
use serde_json::Error as SerdeError;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum ResourceError {
    InvalidChunkId,
    ResourceEmpty,
    FailedEmbeddingGeneration,
    NoChunkFound,
    InvalidModelArchitecture,
    FailedJSONParsing,
    FailedCSVParsing,
    FailedPDFParsing,
    RocksDBError(RocksError),
    RegexError(regex::Error),
    RequestFailed(String),
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ResourceError::InvalidChunkId => write!(f, "Invalid chunk id"),
            ResourceError::ResourceEmpty => write!(f, "Resource is empty"),
            ResourceError::FailedEmbeddingGeneration => write!(f, "Failed to generate embeddings"),
            ResourceError::NoChunkFound => write!(f, "No matching data chunk found"),
            ResourceError::InvalidModelArchitecture => write!(f, "An unsupported model architecture was specified."),
            ResourceError::FailedJSONParsing => write!(f, "Failed JSON parsing."),
            ResourceError::FailedCSVParsing => write!(f, "Failed CSV parsing."),
            ResourceError::FailedPDFParsing => write!(f, "Failed PDF parsing."),
            ResourceError::RegexError(ref e) => write!(f, "Regex error: {}", e),
            ResourceError::RequestFailed(ref e) => write!(f, "HTTP request failed: {}", e), // Add this line
            ResourceError::RocksDBError(ref e) => write!(f, "Rocks DB Error: {}", e),       // Add this line
        }
    }
}

impl Error for ResourceError {}

impl From<RocksError> for ResourceError {
    fn from(err: RocksError) -> ResourceError {
        ResourceError::RocksDBError(err)
    }
}

impl From<regex::Error> for ResourceError {
    fn from(err: regex::Error) -> ResourceError {
        ResourceError::RegexError(err)
    }
}

impl From<SerdeError> for ResourceError {
    fn from(error: SerdeError) -> Self {
        match error.classify() {
            serde_json::error::Category::Io => ResourceError::FailedJSONParsing,
            serde_json::error::Category::Syntax => ResourceError::FailedJSONParsing,
            serde_json::error::Category::Data => ResourceError::FailedJSONParsing,
            serde_json::error::Category::Eof => ResourceError::FailedJSONParsing,
        }
    }
}
