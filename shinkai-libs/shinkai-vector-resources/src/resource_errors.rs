use crate::vector_resource::VRPath;
use serde_json::Error as SerdeError;
use std::error::Error;
use std::fmt;

#[derive(Debug, PartialEq)]
pub enum VRError {
    InvalidNodeId,
    VectorResourceEmpty,
    FailedEmbeddingGeneration(String),
    NoNodeFound,
    InvalidModelArchitecture,
    FailedJSONParsing,
    FailedCSVParsing,
    FailedPDFParsing,
    InvalidVRBaseType,
    RegexError(regex::Error),
    RequestFailed(String),
    NoEmbeddingProvided,
    DataIsNonMatchingType,
    InvalidVRPath(VRPath),
    FailedParsingUnstructedAPIJSON(String),
    CouldNotDetectFileType(String),
    InvalidReferenceString(String),
}

impl fmt::Display for VRError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            VRError::InvalidNodeId => write!(f, "Invalid node id"),
            VRError::VectorResourceEmpty => write!(f, "VectorResource is empty"),
            VRError::FailedEmbeddingGeneration(ref s) => write!(f, "Failed to generate embeddings: {}", s),
            VRError::NoNodeFound => write!(f, "No matching node found"),
            VRError::InvalidModelArchitecture => {
                write!(f, "An unsupported model architecture was specified.")
            }
            VRError::FailedJSONParsing => write!(f, "Failed JSON parsing."),
            VRError::FailedCSVParsing => write!(f, "Failed CSV parsing."),
            VRError::FailedPDFParsing => write!(f, "Failed PDF parsing."),
            VRError::NoEmbeddingProvided => write!(f, "No embedding provided."),
            VRError::InvalidVRBaseType => {
                write!(f, "The resource type does not match any of the VRBaseTypes.")
            }
            VRError::RegexError(ref e) => write!(f, "Regex error: {}", e),
            VRError::RequestFailed(ref e) => write!(f, "HTTP request failed: {}", e),
            VRError::DataIsNonMatchingType => {
                write!(f, "Data inside of the Node is of a different type than requested.")
            }
            VRError::InvalidVRPath(ref p) => write!(f, "Vector Resource Path is invalid: {}", p),
            VRError::FailedParsingUnstructedAPIJSON(ref s) => {
                write!(f, "Failed to parse Unstructed API response json: {}", s)
            }
            VRError::CouldNotDetectFileType(ref s) => {
                write!(f, "Could not detect file type from file name: {}", s)
            }
            VRError::InvalidReferenceString(ref s) => {
                write!(f, "Vector Resource reference string is invalid: {}", s)
            }
        }
    }
}

impl Error for VRError {}

impl From<regex::Error> for VRError {
    fn from(err: regex::Error) -> VRError {
        VRError::RegexError(err)
    }
}

impl From<SerdeError> for VRError {
    fn from(error: SerdeError) -> Self {
        match error.classify() {
            serde_json::error::Category::Io => VRError::FailedJSONParsing,
            serde_json::error::Category::Syntax => VRError::FailedJSONParsing,
            serde_json::error::Category::Data => VRError::FailedJSONParsing,
            serde_json::error::Category::Eof => VRError::FailedJSONParsing,
        }
    }
}

impl From<reqwest::Error> for VRError {
    fn from(error: reqwest::Error) -> Self {
        VRError::RequestFailed(error.to_string())
    }
}
