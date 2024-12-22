use regex::Error as RegexError;
use serde_json::Error as SerdeError;
use shinkai_embedding::shinkai_embedding_errors::ShinkaiEmbeddingError;
use shinkai_sqlite::errors::SqliteManagerError;
use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShinkaiFsError {
    #[error("Failed to read file: {0}")]
    FailedIO(String),
    #[error("File not found")]
    FileNotFound,
    #[error("File not found: {0}")]
    FileNotFoundWithPath(String),
    #[error("Invalid model architecture")]
    InvalidModelArchitecture,
    #[error("Unimplemented model dimensions: {0}")]
    UnimplementedModelDimensions(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Failed to generate embeddings: {0}")]
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
    #[error("Invalid node id: {0}")]
    InvalidNodeId(String),
    #[error("VectorResource is empty")]
    VectorResourceEmpty,
    #[error("No matching node found")]
    NoNodeFound,
    #[error("Failed JSON parsing")]
    FailedJSONParsing,
    #[error("Failed CSV parsing")]
    FailedCSVParsing,
    #[error("Failed DOCX parsing")]
    FailedDOCXParsing,
    #[error("Failed PDF parsing")]
    FailedPDFParsing,
    #[error("Failed MD parsing")]
    FailedMDParsing,
    #[error("Failed TXT parsing")]
    FailedTXTParsing,
    #[error("Failed XLSX parsing")]
    FailedXLSXParsing,
    #[error("No embedding provided")]
    NoEmbeddingProvided,
    #[error("The resource type does not match any of the VRBaseTypes")]
    InvalidVRBaseType,
    #[error("Regex error: {0}")]
    RegexError(#[from] RegexError),
    #[error("Content inside of the Node is of a different type than requested")]
    ContentIsNonMatchingType,
    #[error("Failed to parse Unstructed API response json: {0}")]
    FailedParsingUnstructedAPIJSON(String),
    #[error("File type not supported: {0}")]
    FileTypeNotSupported(String),
    #[error("Vector Resource reference string is invalid: {0}")]
    InvalidReferenceString(String),
    #[error("Provided datetime string does not match RFC3339: {0}")]
    InvalidDateTimeString(String),
    #[error("Failed to acquire lock for: {0}")]
    LockAcquisitionFailed(String),
    #[error("Missing key not found in hashmap: {0}")]
    MissingKey(String),
    #[error("String is not formatted as a proper path string: {0}")]
    InvalidPathString(String),
    #[error(
        "Attempted to perform ordered operations on a resource that does not implement OrderedVectorResource: {0}"
    )]
    ResourceDoesNotSupportOrderedOperations(String),
    #[error("Unexpected/unsupported NodeContent type for Node with id: {0}")]
    InvalidNodeType(String),
    #[error("The provided merkle hash String is not a validly encoded Blake3 hash: {0}")]
    InvalidMerkleHashString(String),
    #[error("The Vector Resource does not contain a merkle root: {0}")]
    MerkleRootNotFound(String),
    #[error("The Node does not contain a merkle root: {0}")]
    MerkleHashNotFoundInNode(String),
    #[error("The Vector Resource is not merkelized, and thus cannot perform merkel-related functionality: {0}")]
    VectorResourceIsNotMerkelized(String),
    #[error("Failed to parse contents into VRKai struct: {0}")]
    VRKaiParsingError(String),
    #[error("Failed to parse contents into VRPack struct: {0}")]
    VRPackParsingError(String),
    #[error("Unsupported VRKai version: {0}")]
    UnsupportedVRKaiVersion(String),
    #[error("Unsupported VRPack version: {0}")]
    UnsupportedVRPackVersion(String),
    #[error("Failed to convert SimplifiedFSEntry at path: {0}")]
    InvalidSimplifiedFSEntryType(String),
    #[error("Embedding Model Error: {0}")]
    VRPackEmbeddingModelError(String),
    #[error("Unsupported file type: {0}")]
    UnsupportedFileType(String),
}

impl From<SerdeError> for ShinkaiFsError {
    fn from(_error: SerdeError) -> Self {
        ShinkaiFsError::FailedJSONParsing
    }
}

impl From<reqwest::Error> for ShinkaiFsError {
    fn from(error: reqwest::Error) -> Self {
        ShinkaiFsError::RequestFailed(error.to_string())
    }
}

impl From<ShinkaiEmbeddingError> for ShinkaiFsError {
    fn from(error: ShinkaiEmbeddingError) -> Self {
        ShinkaiFsError::FailedEmbeddingGeneration(error.to_string())
    }
}
