use reqwest::Error as ReqwestError;
use serde_json::Error as SerdeError;
use std::error::Error;
use std::fmt::{self};

#[derive(Debug)]
pub enum ToolError {
    RegexError(regex::Error),
    FailedJSONParsing,
    ParseError(String),
    ToolkitNotFound,
    ToolkitVersionAlreadyInstalled(String, String),
    RequestError(ReqwestError),
    ToolNotFound(String),
    ToolAlreadyInstalled(String),
    ToolkitAlreadyActivated(String),
    ToolkitAlreadyDeactivated(String),
    SerializationError(String),
    InvalidProfile(String),
    AlreadyStarted,
    NotStarted,
    ToolNotRunnable(String),
    ExecutionError(String),
    DatabaseError(String),
    MissingEmbedding,
    EmbeddingGenerationError(String),
    MissingConfigError(String),
    MissingParameterError(String),
    InvalidFunctionArguments(String),
    InvalidToolRouterKey(String),
    OAuthError(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ToolError::RegexError(ref e) => write!(f, "Regex error: {}", e),
            ToolError::FailedJSONParsing => write!(f, "Failed JSON parsing."),
            ToolError::ParseError(ref s) => write!(f, "Failed to parse {}", s),
            ToolError::ToolkitNotFound => write!(f, "Toolkit was not found."),
            ToolError::ToolkitVersionAlreadyInstalled(ref s, ref e) => {
                write!(f, "Toolkit with the same version is already installed: {} {}", s, e)
            }
            ToolError::RequestError(ref e) => write!(f, "Request error: {}", e),
            ToolError::ToolNotFound(ref t) => write!(f, "Tool not found: {}", t),
            ToolError::ToolAlreadyInstalled(ref t) => write!(f, "Tool already installed: {}", t),
            ToolError::ToolkitAlreadyActivated(ref t) => write!(f, "Toolkit is already activated: {}", t),
            ToolError::ToolkitAlreadyDeactivated(ref t) => write!(f, "Toolkit is already deactivated: {}", t),
            ToolError::SerializationError(ref e) => write!(f, "Serialization error: {}", e),
            ToolError::InvalidProfile(ref e) => write!(f, "Invalid profile: {}", e),
            ToolError::AlreadyStarted => write!(f, "Tool is already started."),
            ToolError::NotStarted => write!(f, "Tool is not started."),
            ToolError::ToolNotRunnable(ref t) => write!(f, "Tool is not runnable: {}", t),
            ToolError::ExecutionError(ref e) => write!(f, "Execution error: {}", e),
            ToolError::DatabaseError(ref e) => write!(f, "Database error: {}", e),
            ToolError::MissingEmbedding => write!(f, "Missing embedding."),
            ToolError::MissingParameterError(ref e) => write!(f, "Missing parameter error: {}", e),
            ToolError::EmbeddingGenerationError(ref e) => write!(f, "Embedding generation error: {}", e),
            ToolError::MissingConfigError(ref e) => write!(f, "Missing config error: {}", e),
            ToolError::InvalidFunctionArguments(ref e) => write!(f, "Invalid function arguments: {}", e),
            ToolError::InvalidToolRouterKey(ref e) => write!(f, "Invalid tool router key: {}", e),
            ToolError::OAuthError(ref e) => write!(f, "OAuth not setup: {}", e),
        }
    }
}

impl Error for ToolError {}

impl From<ReqwestError> for ToolError {
    fn from(err: ReqwestError) -> ToolError {
        ToolError::RequestError(err)
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

impl From<String> for ToolError {
    fn from(err: String) -> ToolError {
        ToolError::ParseError(err)
    }
}
