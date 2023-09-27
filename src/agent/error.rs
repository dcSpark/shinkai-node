use crate::db::db_errors::ShinkaiDBError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiNameError;
use std::fmt;

/// TODO: Merge into AgentError
#[derive(Debug)]
pub enum JobManagerError {
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
}

impl fmt::Display for JobManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JobManagerError::NotAJobMessage => write!(f, "Message is not a job message"),
            JobManagerError::JobNotFound => write!(f, "Job not found"),
            JobManagerError::JobCreationDeserializationFailed => {
                write!(f, "Failed to deserialize JobCreationInfo message")
            }
            JobManagerError::JobMessageDeserializationFailed => write!(f, "Failed to deserialize JobMessage"),
            JobManagerError::JobPreMessageDeserializationFailed => write!(f, "Failed to deserialize JobPreMessage"),
            JobManagerError::MessageTypeParseFailed => write!(f, "Could not parse message type"),
            JobManagerError::IO(err) => write!(f, "IO error: {}", err),
            JobManagerError::ShinkaiDB(err) => write!(f, "Shinkai DB error: {}", err),
            JobManagerError::AgentNotFound => write!(f, "Agent not found"),
            JobManagerError::ContentParseFailed => write!(f, "Failed to parse content"),
            JobManagerError::ShinkaiNameError(err) => write!(f, "ShinkaiName error: {}", err),
        }
    }
}

impl std::error::Error for JobManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JobManagerError::ShinkaiDB(err) => Some(err),
            JobManagerError::ShinkaiNameError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<Box<dyn std::error::Error>> for JobManagerError {
    fn from(err: Box<dyn std::error::Error>) -> JobManagerError {
        JobManagerError::IO(err.to_string())
    }
}

impl From<ShinkaiDBError> for JobManagerError {
    fn from(err: ShinkaiDBError) -> JobManagerError {
        JobManagerError::ShinkaiDB(err)
    }
}

impl From<ShinkaiNameError> for JobManagerError {
    fn from(err: ShinkaiNameError) -> JobManagerError {
        JobManagerError::ShinkaiNameError(err)
    }
}

pub enum AgentError {
    UrlNotSet,
    ApiKeyNotSet,
    ReqwestError(reqwest::Error),
    MissingInitialStepInExecutionPlan,
    FailedExtractingJSONObjectFromResponse(String),
    FailedInferencingLocalLLM,
    UserPromptMissingEBNFDefinition,
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AgentError::UrlNotSet => write!(f, "URL is not set"),
            AgentError::ApiKeyNotSet => write!(f, "API Key not set"),
            AgentError::MissingInitialStepInExecutionPlan => write!(
                f,
                "The provided execution plan does not have an InitialExecutionStep as its first element."
            ),
            AgentError::FailedExtractingJSONObjectFromResponse(s) => {
                write!(f, "Could not find JSON Object in the LLM's response: {}", s)
            }
            AgentError::ReqwestError(err) => write!(f, "Reqwest error: {}", err),
            AgentError::FailedInferencingLocalLLM => {
                write!(f, "Failed inferencing and getting a valid response from the local LLM")
            }
            AgentError::UserPromptMissingEBNFDefinition => {
                write!(f, "At least 1 EBNF subprompt must be defined for the user message.")
            }
        }
    }
}

impl fmt::Debug for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::UrlNotSet => f.debug_tuple("UrlNotSet").finish(),
            AgentError::ApiKeyNotSet => f.debug_tuple("ApiKeyNotSet").finish(),
            AgentError::ReqwestError(err) => f.debug_tuple("ReqwestError").field(err).finish(),
            AgentError::MissingInitialStepInExecutionPlan => {
                f.debug_tuple("MissingInitialStepInExecutionPlan").finish()
            }
            AgentError::FailedExtractingJSONObjectFromResponse(err) => f
                .debug_tuple("FailedExtractingJSONObjectFromResponse")
                .field(err)
                .finish(),

            AgentError::FailedInferencingLocalLLM => f.debug_tuple("FailedInferencingLocalLLM").finish(),
            AgentError::UserPromptMissingEBNFDefinition => f.debug_tuple("UserPromptMissingEBNFDefinition").finish(),
        }
    }
}

impl From<reqwest::Error> for AgentError {
    fn from(err: reqwest::Error) -> AgentError {
        AgentError::ReqwestError(err)
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AgentError::ReqwestError(err) => Some(err),
            _ => None,
        }
    }
}
