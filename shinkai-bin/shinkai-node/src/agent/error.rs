use crate::{db::db_errors::ShinkaiDBError, managers::model_capabilities_manager::ModelCapabilitiesManagerError, vector_fs::vector_fs_error::VectorFSError};
use anyhow::Error as AnyhowError;
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxNameError, shinkai_name::ShinkaiNameError},
    shinkai_message::shinkai_message_error::ShinkaiMessageError,
};
use shinkai_vector_resources::resource_errors::VRError;
use std::fmt;
use tokio::task::JoinError;



#[derive(Debug)]
pub enum AgentError {
    UrlNotSet,
    ApiKeyNotSet,
    ReqwestError(reqwest::Error),
    MissingInitialStepInExecutionPlan,
    FailedExtractingJSONObjectFromResponse(String),
    InferenceFailed,
    UserPromptMissingEBNFDefinition,
    NotAJobMessage,
    JobNotFound,
    JobCreationDeserializationFailed,
    JobMessageDeserializationFailed,
    JobPreMessageDeserializationFailed,
    MessageTypeParseFailed,
    IO(String),
    ShinkaiDB(ShinkaiDBError),
    VectorFS(VectorFSError),
    ShinkaiNameError(ShinkaiNameError),
    AgentNotFound,
    ContentParseFailed,
    InferenceJSONResponseMissingField(String),
    JSONSerializationError(String),
    VectorResource(VRError),
    InvalidSubidentity(ShinkaiNameError),
    InvalidProfileSubidentity(String),
    SerdeError(serde_json::Error),
    TaskJoinError(String),
    InferenceRecursionLimitReached(String),
    TokenizationError(String),
    JobDequeueFailed(String),
    ShinkaiMessage(ShinkaiMessageError),
    InboxNameError(InboxNameError),
    InvalidCronCreationChainStage(String),
    WebScrapingFailed(String),
    InvalidCronExecutionChainStage(String),
    AnyhowError(AnyhowError),
    AgentMissingCapabilities(String),
    UnexpectedPromptResult(String),
    AgentsCapabilitiesManagerError(ModelCapabilitiesManagerError),
    UnexpectedPromptResultVariant(String),
    ImageContentNotFound(String),
    NetworkError(String),
    NoUserProfileFound,
    InvalidModelType(String),
    ShinkaiBackendInvalidAuthentication(String),
    ShinkaiBackendInvalidConfiguration(String),
    ShinkaiBackendInferenceLimitReached(String),
    ShinkaiBackendAIProviderError(String),
    ShinkaiBackendUnexpectedStatusCode(u64),
    ShinkaiBackendUnexpectedError(String),
    LLMServiceInferenceLimitReached(String),
    LLMServiceUnexpectedError(String),
    FailedSerdeParsingJSONString(String, serde_json::Error),
    FailedSerdeParsingXMLString(String, minidom::Error),
    ShinkaiMessageBuilderError(String),
    TokenLimit(String),
    WorkflowExecutionError(String),
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
            AgentError::InferenceFailed => {
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
            AgentError::VectorFS(err) => write!(f, "VectorFS error: {}", err),
            AgentError::AgentNotFound => write!(f, "Agent not found"),
            AgentError::ContentParseFailed => write!(f, "Failed to parse content"),
            AgentError::ShinkaiNameError(err) => write!(f, "ShinkaiName error: {}", err),
            AgentError::InferenceJSONResponseMissingField(s) => {
                write!(f, "Response from LLM does not include needed key/field: {}", s)
            }
            AgentError::JSONSerializationError(s) => write!(f, "JSON Serialization error: {}", s),
            AgentError::VectorResource(err) => write!(f, "VectorResource error: {}", err),
            AgentError::InvalidSubidentity(err) => write!(f, "Invalid subidentity: {}", err),
            AgentError::InvalidProfileSubidentity(s) => write!(f, "Invalid profile subidentity: {}", s),
            AgentError::SerdeError(err) => write!(f, "Serde error: {}", err),
            AgentError::TaskJoinError(s) => write!(f, "Task join error: {}", s),
            AgentError::InferenceRecursionLimitReached(s) => write!(f, "Inferencing the LLM has reached too many iterations of recursion with no progess, and thus has been stopped for this user_message: {}", s),
            AgentError::TokenizationError(s) => write!(f, "Tokenization error: {}", s),
            AgentError::JobDequeueFailed(s) => write!(f, "Job dequeue failed: {}", s),
            AgentError::ShinkaiMessage(err) => write!(f, "ShinkaiMessage error: {}", err),
            AgentError::InboxNameError(err) => write!(f, "InboxName error: {}", err),
            AgentError::InvalidCronCreationChainStage(s) => write!(f, "Invalid cron creation chain stage: {}", s),
            AgentError::WebScrapingFailed(err) => write!(f, "Web scraping failed: {}", err),
            AgentError::InvalidCronExecutionChainStage(s) => write!(f, "Invalid cron execution chain stage: {}", s),
            AgentError::AnyhowError(err) => write!(f, "{}", err),
            AgentError::AgentMissingCapabilities(s) => write!(f, "Agent is missing capabilities: {}", s),
            AgentError::UnexpectedPromptResult(s) => write!(f, "Unexpected prompt result: {}", s),
            AgentError::AgentsCapabilitiesManagerError(err) => write!(f, "AgentsCapabilitiesManager error: {}", err),
            AgentError::UnexpectedPromptResultVariant(s) => write!(f, "Unexpected prompt result variant: {}", s),
            AgentError::ImageContentNotFound(s) => write!(f, "Image content not found: {}", s),
            AgentError::NoUserProfileFound => write!(f, "Cannot proceed as User Profile returned None."),
            AgentError::NetworkError(s) => write!(f, "Network error: {}", s),
            AgentError::InvalidModelType(s) => write!(f, "Invalid model type: {}", s),
            AgentError::ShinkaiBackendInvalidAuthentication(s) => write!(f, "Shinkai Backend Invalid Authentication: {}", s),
            AgentError::ShinkaiBackendInvalidConfiguration(s) => write!(f, "Shinkai Backend Invalid configuration: {}", s),
            AgentError::ShinkaiBackendInferenceLimitReached(s) => write!(f, "Shinkai Backend Inference Limit Reached: {}", s),
            AgentError::ShinkaiBackendAIProviderError(s) => write!(f, "Shinkai Backend AI Provider Error: {}", s),
            AgentError::ShinkaiBackendUnexpectedStatusCode(code) => write!(f, "Shinkai Backend Unexpected Status Code: {}", code),
            AgentError::ShinkaiBackendUnexpectedError(e) => write!(f, "Shinkai Backend Unexpected Error: {}", e),
            AgentError::LLMServiceInferenceLimitReached(s) => write!(f, "LLM Provider Inference Limit Reached: {}", s),
            AgentError::LLMServiceUnexpectedError(e) => write!(f, "LLM Provider Unexpected Error: {}", e),
            AgentError::FailedSerdeParsingJSONString(s, err) => write!(f, "Failed parsing JSON string: `{}`. Fix the following Serde error: {}", s, err),
            AgentError::FailedSerdeParsingXMLString(s, err) => write!(f, "Failed parsing XML string: `{}`. Fix the following Serde error: {}", s, err),
            AgentError::ShinkaiMessageBuilderError(s) => write!(f, "{}", s),
            AgentError::TokenLimit(s) => write!(f, "{}", s),
            AgentError::WorkflowExecutionError(s) => write!(f, "{}", s),
        }
    }
}

impl AgentError {
    /// Encodes the error as a JSON string that is easily parsable by frontends
    pub fn to_error_json(&self) -> String {
        let error_name = match self {
            AgentError::UrlNotSet => "UrlNotSet",
            AgentError::ApiKeyNotSet => "ApiKeyNotSet",
            AgentError::ReqwestError(_) => "ReqwestError",
            AgentError::MissingInitialStepInExecutionPlan => "MissingInitialStepInExecutionPlan",
            AgentError::FailedExtractingJSONObjectFromResponse(_) => "FailedExtractingJSONObjectFromResponse",
            AgentError::InferenceFailed => "InferenceFailed",
            AgentError::UserPromptMissingEBNFDefinition => "UserPromptMissingEBNFDefinition",
            AgentError::NotAJobMessage => "NotAJobMessage",
            AgentError::JobNotFound => "JobNotFound",
            AgentError::JobCreationDeserializationFailed => "JobCreationDeserializationFailed",
            AgentError::JobMessageDeserializationFailed => "JobMessageDeserializationFailed",
            AgentError::JobPreMessageDeserializationFailed => "JobPreMessageDeserializationFailed",
            AgentError::MessageTypeParseFailed => "MessageTypeParseFailed",
            AgentError::IO(_) => "IO",
            AgentError::ShinkaiDB(_) => "ShinkaiDB",
            AgentError::VectorFS(_) => "VectorFS",
            AgentError::ShinkaiNameError(_) => "ShinkaiNameError",
            AgentError::AgentNotFound => "AgentNotFound",
            AgentError::ContentParseFailed => "ContentParseFailed",
            AgentError::InferenceJSONResponseMissingField(_) => "InferenceJSONResponseMissingField",
            AgentError::JSONSerializationError(_) => "JSONSerializationError",
            AgentError::VectorResource(_) => "VectorResource",
            AgentError::InvalidSubidentity(_) => "InvalidSubidentity",
            AgentError::InvalidProfileSubidentity(_) => "InvalidProfileSubidentity",
            AgentError::SerdeError(_) => "SerdeError",
            AgentError::TaskJoinError(_) => "TaskJoinError",
            AgentError::InferenceRecursionLimitReached(_) => "InferenceRecursionLimitReached",
            AgentError::TokenizationError(_) => "TokenizationError",
            AgentError::JobDequeueFailed(_) => "JobDequeueFailed",
            AgentError::ShinkaiMessage(_) => "ShinkaiMessage",
            AgentError::InboxNameError(_) => "InboxNameError",
            AgentError::InvalidCronCreationChainStage(_) => "InvalidCronCreationChainStage",
            AgentError::WebScrapingFailed(_) => "WebScrapingFailed",
            AgentError::InvalidCronExecutionChainStage(_) => "InvalidCronExecutionChainStage",
            AgentError::AnyhowError(_) => "AnyhowError",
            AgentError::AgentMissingCapabilities(_) => "AgentMissingCapabilities",
            AgentError::UnexpectedPromptResult(_) => "UnexpectedPromptResult",
            AgentError::AgentsCapabilitiesManagerError(_) => "AgentsCapabilitiesManagerError",
            AgentError::UnexpectedPromptResultVariant(_) => "UnexpectedPromptResultVariant",
            AgentError::ImageContentNotFound(_) => "ImageContentNotFound",
            AgentError::NetworkError(_) => "NetworkError",
            AgentError::NoUserProfileFound => "NoUserProfileFound",
            AgentError::InvalidModelType(_) => "InvalidModelType",
            AgentError::ShinkaiBackendInvalidAuthentication(_) => "ShinkaiBackendInvalidAuthentication",
            AgentError::ShinkaiBackendInvalidConfiguration(_) => "ShinkaiBackendInvalidConfiguration",
            AgentError::ShinkaiBackendInferenceLimitReached(_) => "ShinkaiBackendInferenceLimitReached",
            AgentError::ShinkaiBackendAIProviderError(_) => "ShinkaiBackendAIProviderError",
            AgentError::ShinkaiBackendUnexpectedStatusCode(_) => "ShinkaiBackendUnexpectedStatusCode",
            AgentError::ShinkaiBackendUnexpectedError(_) => "ShinkaiBackendUnexpectedError",
            AgentError::LLMServiceInferenceLimitReached(_) => "LLMServiceInferenceLimitReached",
            AgentError::LLMServiceUnexpectedError(_) => "LLMServiceUnexpectedError",
            AgentError::FailedSerdeParsingJSONString(_, _) => "FailedSerdeParsingJSONString",
            AgentError::FailedSerdeParsingXMLString(_, _) => "FailedSerdeParsingXMLString",
            AgentError::ShinkaiMessageBuilderError(_) => "ShinkaiMessageBuilderError",
            AgentError::TokenLimit(_) => "TokenLimit",
            AgentError::WorkflowExecutionError(_) => "WorkflowExecutionError",
        };

        let error_message = format!("{}", self);

        serde_json::json!({
            "error": error_name,
            "error_message": error_message
        }).to_string()
    }
}





impl From<AnyhowError> for AgentError {
    fn from(error: AnyhowError) -> Self {
        AgentError::AnyhowError(error)
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

impl From<VRError> for AgentError {
    fn from(error: VRError) -> Self {
        AgentError::VectorResource(error)
    }
}

impl From<JoinError> for AgentError {
    fn from(err: JoinError) -> AgentError {
        AgentError::TaskJoinError(err.to_string())
    }
}

impl From<ShinkaiMessageError> for AgentError {
    fn from(error: ShinkaiMessageError) -> Self {
        AgentError::ShinkaiMessage(error)
    }
}

impl From<InboxNameError> for AgentError {
    fn from(error: InboxNameError) -> Self {
        AgentError::InboxNameError(error)
    }
}

impl From<ModelCapabilitiesManagerError> for AgentError {
    fn from(error: ModelCapabilitiesManagerError) -> Self {
        AgentError::AgentsCapabilitiesManagerError(error)
    }
}

impl From<VectorFSError> for AgentError {
    fn from(err: VectorFSError) -> AgentError {
        AgentError::VectorFS(err)
    }
}

impl From<String> for AgentError {
    fn from(err: String) -> AgentError {
        AgentError::WorkflowExecutionError(err)
    }
}
