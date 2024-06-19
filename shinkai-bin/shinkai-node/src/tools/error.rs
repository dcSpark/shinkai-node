use reqwest::Error as ReqwestError;
use rocksdb::Error as RocksError;
use serde_json::Error as SerdeError;
use shinkai_vector_resources::resource_errors::VRError;
use std::error::Error;
use std::fmt::{self};

#[derive(Debug)]
pub enum ToolError {
    RocksDBError(RocksError),
    RegexError(regex::Error),
    FailedJSONParsing,
    ParseError(String),
    ToolkitNotFound,
    ToolkitVersionAlreadyInstalled(String, String),
    JSToolkitExecutorNotAvailable,
    JSToolkitExecutorFailedStarting,
    RequestError(ReqwestError),
    ToolNotFound(String),
    VRError(VRError),
    ToolAlreadyInstalled(String),
    JSToolkitHeaderValidationFailed(String),
    ToolkitAlreadyActivated(String),
    ToolkitAlreadyDeactivated(String),
    SerializationError(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ToolError::RegexError(ref e) => write!(f, "Regex error: {}", e),
            ToolError::RocksDBError(ref e) => write!(f, "Rocks DB Error: {}", e),
            ToolError::FailedJSONParsing => write!(f, "Failed JSON parsing."),
            ToolError::ParseError(ref s) => write!(f, "Failed to parse {}", s),
            ToolError::ToolkitNotFound => write!(f, "Toolkit was not found."),
            ToolError::ToolkitVersionAlreadyInstalled(ref s, ref e) => {
                write!(f, "Toolkit with the same version is already installed: {} {}", s, e)
            }
            ToolError::JSToolkitExecutorNotAvailable => {
                write!(f, "Failed connecting to JS Toolkit Executor over HTTP.")
            }
            ToolError::JSToolkitExecutorFailedStarting => write!(f, "Failed starting local JS Toolkit Executor."),
            ToolError::RequestError(ref e) => write!(f, "Request error: {}", e),
            ToolError::ToolNotFound(ref t) => write!(f, "Tool not found: {}", t),
            ToolError::VRError(ref e) => write!(f, "{}", e),
            ToolError::ToolAlreadyInstalled(ref t) => write!(f, "Tool already installed: {}", t),
            ToolError::JSToolkitHeaderValidationFailed(ref e) => write!(f, "Toolkit header validation failed: {}", e),
            ToolError::ToolkitAlreadyActivated(ref t) => write!(f, "Toolkit is already activated: {}", t),
            ToolError::ToolkitAlreadyDeactivated(ref t) => write!(f, "Toolkit is already deactivated: {}", t),
            ToolError::SerializationError(ref e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl Error for ToolError {}

impl From<VRError> for ToolError {
    fn from(err: VRError) -> ToolError {
        ToolError::VRError(err)
    }
}

impl From<ReqwestError> for ToolError {
    fn from(err: ReqwestError) -> ToolError {
        ToolError::RequestError(err)
    }
}

impl From<RocksError> for ToolError {
    fn from(err: RocksError) -> ToolError {
        ToolError::RocksDBError(err)
    }
}

impl From<regex::Error> for ToolError {
    fn from(err: regex::Error) -> ToolError {
        ToolError::RegexError(err)
    }
}

impl From<SerdeError> for ToolError {
    fn from(error: SerdeError) -> Self {
        match error.classify() {
            serde_json::error::Category::Io => ToolError::ParseError(error.to_string()),
            serde_json::error::Category::Syntax => ToolError::ParseError(error.to_string()),
            serde_json::error::Category::Data => ToolError::ParseError(error.to_string()),
            serde_json::error::Category::Eof => ToolError::ParseError(error.to_string()),
        }
    }
}

impl From<anyhow::Error> for ToolError {
    fn from(err: anyhow::Error) -> ToolError {
        ToolError::ParseError(err.to_string())
    }
}
