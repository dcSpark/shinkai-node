use crate::vector_resource::{VRKaiVersion, VRPath};
use lz4_flex::block::DecompressError;
use serde_json::Error as SerdeError;
use std::error::Error;
use std::fmt;
use std::string::FromUtf8Error;

#[derive(Debug, PartialEq)]
pub enum VRError {
    InvalidNodeId(String),
    VectorResourceEmpty,
    FailedEmbeddingGeneration(String),
    NoNodeFound,
    InvalidModelArchitecture,
    FailedJSONParsing(String),
    FailedCSVParsing,
    FailedPDFParsing,
    InvalidVRBaseType,
    RegexError(regex::Error),
    RequestFailed(String),
    NoEmbeddingProvided,
    ContentIsNonMatchingType,
    InvalidVRPath(VRPath),
    FailedParsingUnstructedAPIJSON(String),
    CouldNotDetectFileType(String),
    InvalidReferenceString(String),
    InvalidDateTimeString(String),
    LockAcquisitionFailed(String),
    MissingKey(String),
    InvalidPathString(String),
    ResourceDoesNotSupportOrderedOperations(String),
    InvalidNodeType(String),
    InvalidMerkleHashString(String),
    MerkleRootNotFound(String),
    MerkleHashNotFoundInNode(String),
    VectorResourceIsNotMerkelized(String),
    VRKaiParsingError(String),
    VRPackParsingError(String),
    UnsupportedVRKaiVersion(String),
    UnsupportedVRPackVersion(String),
    InvalidSimplifiedFSEntryType(String),
    VRPackEmbeddingModelError(String),
}

impl fmt::Display for VRError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            VRError::InvalidNodeId(ref s) => write!(f, "Invalid node id: {}", s),
            VRError::VectorResourceEmpty => write!(f, "VectorResource is empty"),
            VRError::FailedEmbeddingGeneration(ref s) => write!(f, "Failed to generate embeddings: {}", s),
            VRError::NoNodeFound => write!(f, "No matching node found"),
            VRError::InvalidModelArchitecture => {
                write!(f, "An unsupported model architecture was specified.")
            }
            VRError::FailedJSONParsing(ref e) => write!(f, "Failed JSON parsing: {}", e),
            VRError::FailedCSVParsing => write!(f, "Failed CSV parsing."),
            VRError::FailedPDFParsing => write!(f, "Failed PDF parsing."),
            VRError::NoEmbeddingProvided => write!(f, "No embedding provided."),
            VRError::InvalidVRBaseType => {
                write!(f, "The resource type does not match any of the VRBaseTypes.")
            }
            VRError::RegexError(ref e) => write!(f, "Regex error: {}", e),
            VRError::RequestFailed(ref e) => write!(f, "HTTP request failed: {}", e),
            VRError::ContentIsNonMatchingType => {
                write!(f, "Content inside of the Node is of a different type than requested.")
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
            VRError::InvalidDateTimeString(ref s) => {
                write!(f, "Provided datetime string does not match RFC3339: {}", s)
            }
            VRError::LockAcquisitionFailed(ref s) => write!(f, "Failed to acquire lock for: {}", s),
            VRError::MissingKey(ref s) => write!(f, "Missing key not found in hashmap: {}", s),
            VRError::InvalidPathString(ref s) => write!(f, "String is not formatted as a proper path string: {}", s),
            VRError::ResourceDoesNotSupportOrderedOperations(ref s) => write!(f, "Attempted to perform ordered operations on a resource that does not implement OrderedVectorResource: {}", s),
            VRError::InvalidNodeType(ref s) => write!(f, "Unexpected/unsupported NodeContent type for Node with id: {}", s),
            VRError::InvalidMerkleHashString(ref s) => write!(f, "The provided merkle hash String is not a validly encoded Blake3 hash: {}", s),
            VRError::MerkleRootNotFound(ref s) => write!(f, "The Vector Resource does not contain a merkle root: {}", s),
            VRError::MerkleHashNotFoundInNode(ref s) => write!(f, "The Node does not contain a merkle root: {}", s),
            VRError::VectorResourceIsNotMerkelized(ref s) => write!(f, "The Vector Resource is not merkelized, and thus cannot perform merkel-related functionality: {}", s),
            VRError::VRKaiParsingError(ref s) => write!(f, "Failed to parse contents into VRKai struct: {}", s),
            VRError::VRPackParsingError(ref s) => write!(f, "Failed to parse contents into VRKai struct: {}", s),
            VRError::UnsupportedVRKaiVersion(ref s) => write!(f, "Unsupported VRKai version: {}", s),
            VRError::UnsupportedVRPackVersion(ref s) => write!(f, "Unsupported VRPack version: {}", s),
            VRError::InvalidSimplifiedFSEntryType(ref s) => write!(f, "Failed to convert SimplifiedFSEntry at path: {}", s),
            VRError::VRPackEmbeddingModelError(ref s) => write!(f, "Embedding Model Error: {}", s),
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
            serde_json::error::Category::Io => VRError::FailedJSONParsing("IO error during JSON parsing".to_string()),
            serde_json::error::Category::Syntax => VRError::FailedJSONParsing("Syntax error during JSON parsing".to_string()),
            serde_json::error::Category::Data => VRError::FailedJSONParsing("Data error during JSON parsing".to_string()),
            serde_json::error::Category::Eof => VRError::FailedJSONParsing("IEof error during JSON parsing".to_string())
        }
    }
}

impl From<reqwest::Error> for VRError {
    fn from(error: reqwest::Error) -> Self {
        VRError::RequestFailed(error.to_string())
    }
}
