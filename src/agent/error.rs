use crate::db::db_errors::ShinkaiDBError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiNameError;
use shinkai_vector_resources::resource_errors::VectorResourceError;
use std::fmt;
use tokio::task::JoinError;

#[derive(Debug)]
pub enum AgentError {
    UrlNotSet,
    ApiKeyNotSet,
    ReqwestError(reqwest::Error),
    MissingInitialStepInExecutionPlan,
    FailedExtractingJSONObjectFromResponse(String),
    FailedInferencingLocalLLM,
    UserPromptMissingEBNFDefinition,
    NotAJobMessage,
    JobNotFound,
    JobCreationDeserializationFailed,
    JobMessageDeserializationFailed,
    JobPreMessageDeserializationFailed,
    MessageTypeParseFailed,
    IO(String),
    ShinkaiDB(ShinkaiDBError),
    ShinkaiNameError(ShinkaiNameError),
    AgentNotFound,
    ContentParseFailed,
    InferenceJSONResponseMissingField(String),
    JSONSerializationError(String),
    VectorResource(VectorResourceError),
    InvalidSubidentity(ShinkaiNameError),
    InvalidProfileSubidentity(String),
    SerdeError(serde_json::Error),
    TaskJoinError(String),
    InferenceRecursionLimitReached(String),
    TokenizationError(String),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AgentError::UrlNotSet => write!(f, "URL is not set"),
            AgentError::ApiKeyNotSet => write!(f, "API Key not set"),
            AgentError::ReqwestError(err) => write!(f, "Reqwest error: {}", err),
            AgentError::MissingInitialStepInExecutionPlan => write!(
                f,
                "The provided execution plan does not have an InitialExecutionStep as its first element."
            ),
            AgentError::FailedExtractingJSONObjectFromResponse(s) => {
                write!(f, "Could not find JSON Object in the LLM's response: {}", s)
            }
            AgentError::FailedInferencingLocalLLM => {
                write!(f, "Failed inferencing and getting a valid response from the local LLM")
            }
            AgentError::UserPromptMissingEBNFDefinition => {
                write!(f, "At least 1 EBNF subprompt must be defined for the user message.")
            }
            AgentError::NotAJobMessage => write!(f, "Message is not a job message"),
            AgentError::JobNotFound => write!(f, "Job not found"),
            AgentError::JobCreationDeserializationFailed => {
                write!(f, "Failed to deserialize JobCreationInfo message")
            }
            AgentError::JobMessageDeserializationFailed => write!(f, "Failed to deserialize JobMessage"),
            AgentError::JobPreMessageDeserializationFailed => write!(f, "Failed to deserialize JobPreMessage"),
            AgentError::MessageTypeParseFailed => write!(f, "Could not parse message type"),
            AgentError::IO(err) => write!(f, "IO error: {}", err),
            AgentError::ShinkaiDB(err) => write!(f, "Shinkai DB error: {}", err),
            AgentError::AgentNotFound => write!(f, "Agent not found"),
            AgentError::ContentParseFailed => write!(f, "Failed to parse content"),
            AgentError::ShinkaiNameError(err) => write!(f, "ShinkaiName error: {}", err),
            AgentError::InferenceJSONResponseMissingField(s) => {
                write!(f, "JSON Response from LLM does not include needed field: {}", s)
            }
            AgentError::JSONSerializationError(s) => write!(f, "JSON Serialization error: {}", s),
            AgentError::VectorResource(err) => write!(f, "VectorResource error: {}", err),
            AgentError::InvalidSubidentity(err) => write!(f, "Invalid subidentity: {}", err),
            AgentError::InvalidProfileSubidentity(s) => write!(f, "Invalid profile subidentity: {}", s),
            AgentError::SerdeError(err) => write!(f, "Serde error: {}", err),
            AgentError::TaskJoinError(s) => write!(f, "Task join error: {}", s),
            AgentError::InferenceRecursionLimitReached(s) => write!(f, "Inferencing the LLM has reached too many iterations of recursion with no progess, and thus has been stopped for this job_task: {}", s),
            AgentError::TokenizationError(s) => write!(f, "Tokenization error: {}", s),

        }
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AgentError::ReqwestError(err) => Some(err),
            AgentError::ShinkaiDB(err) => Some(err),
            AgentError::ShinkaiNameError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for AgentError {
    fn from(err: reqwest::Error) -> AgentError {
        AgentError::ReqwestError(err)
    }
}

impl From<ShinkaiDBError> for AgentError {
    fn from(err: ShinkaiDBError) -> AgentError {
        AgentError::ShinkaiDB(err)
    }
}

impl From<ShinkaiNameError> for AgentError {
    fn from(err: ShinkaiNameError) -> AgentError {
        AgentError::ShinkaiNameError(err)
    }
}

impl From<Box<dyn std::error::Error>> for AgentError {
    fn from(err: Box<dyn std::error::Error>) -> AgentError {
        AgentError::IO(err.to_string())
    }
}

impl From<serde_json::Error> for AgentError {
    fn from(err: serde_json::Error) -> AgentError {
        AgentError::JSONSerializationError(err.to_string())
    }
}

impl From<VectorResourceError> for AgentError {
    fn from(error: VectorResourceError) -> Self {
        AgentError::VectorResource(error)
    }
}

impl From<JoinError> for AgentError {
    fn from(err: JoinError) -> AgentError {
        AgentError::TaskJoinError(err.to_string())
    }
}
