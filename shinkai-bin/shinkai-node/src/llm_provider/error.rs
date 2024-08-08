use crate::{db::db_errors::ShinkaiDBError, managers::model_capabilities_manager::ModelCapabilitiesManagerError, vector_fs::vector_fs_error::VectorFSError, workflows::sm_executor::WorkflowError};
use anyhow::Error as AnyhowError;
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxNameError, shinkai_name::ShinkaiNameError},
    shinkai_message::shinkai_message_error::ShinkaiMessageError,
};
use shinkai_vector_resources::resource_errors::VRError;
use std::fmt;
use tokio::task::JoinError;



#[derive(Debug)]
pub enum LLMProviderError {
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
    MessageTypeParseFailed,
    IO(String),
    ShinkaiDB(ShinkaiDBError),
    VectorFS(VectorFSError),
    ShinkaiNameError(ShinkaiNameError),
    LLMProviderNotFound,
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
    LLMProviderMissingCapabilities(String),
    UnexpectedPromptResult(String),
    LLMProviderCapabilitiesManagerError(ModelCapabilitiesManagerError),
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
    FunctionNotFound(String),
    FunctionExecutionError(String),
    InvalidFunctionArguments(String),
    InvalidFunctionResult(String),
    MaxIterationsReached(String),
    ToolRouterError(String),
    SerializationError(String),
    SheetManagerNotFound,
    CallbackManagerNotFound,
    SheetManagerError(String),
    InputProcessingError(String)
}

impl fmt::Display for LLMProviderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LLMProviderError::UrlNotSet => write!(f, "URL is not set"),
            LLMProviderError::ApiKeyNotSet => write!(f, "API Key not set"),
            LLMProviderError::ReqwestError(err) => write!(f, "Reqwest error: {}", err),
            LLMProviderError::MissingInitialStepInExecutionPlan => write!(
                f,
                "The provided execution plan does not have an InitialExecutionStep as its first element."
            ),
            LLMProviderError::FailedExtractingJSONObjectFromResponse(s) => {
                write!(f, "Could not find JSON Object in the LLM's response: {}", s)
            }
            LLMProviderError::InferenceFailed => {
                write!(f, "Failed inferencing and getting a valid response from the local LLM")
            }
            LLMProviderError::UserPromptMissingEBNFDefinition => {
                write!(f, "At least 1 EBNF subprompt must be defined for the user message.")
            }
            LLMProviderError::NotAJobMessage => write!(f, "Message is not a job message"),
            LLMProviderError::JobNotFound => write!(f, "Job not found"),
            LLMProviderError::JobCreationDeserializationFailed => {
                write!(f, "Failed to deserialize JobCreationInfo message")
            }
            LLMProviderError::JobMessageDeserializationFailed => write!(f, "Failed to deserialize JobMessage"),
            LLMProviderError::MessageTypeParseFailed => write!(f, "Could not parse message type"),
            LLMProviderError::IO(err) => write!(f, "IO error: {}", err),
            LLMProviderError::ShinkaiDB(err) => write!(f, "Shinkai DB error: {}", err),
            LLMProviderError::VectorFS(err) => write!(f, "VectorFS error: {}", err),
            LLMProviderError::LLMProviderNotFound => write!(f, "Agent not found"),
            LLMProviderError::ContentParseFailed => write!(f, "Failed to parse content"),
            LLMProviderError::ShinkaiNameError(err) => write!(f, "ShinkaiName error: {}", err),
            LLMProviderError::InferenceJSONResponseMissingField(s) => {
                write!(f, "Response from LLM does not include needed key/field: {}", s)
            }
            LLMProviderError::JSONSerializationError(s) => write!(f, "JSON Serialization error: {}", s),
            LLMProviderError::VectorResource(err) => write!(f, "VectorResource error: {}", err),
            LLMProviderError::InvalidSubidentity(err) => write!(f, "Invalid subidentity: {}", err),
            LLMProviderError::InvalidProfileSubidentity(s) => write!(f, "Invalid profile subidentity: {}", s),
            LLMProviderError::SerdeError(err) => write!(f, "Serde error: {}", err),
            LLMProviderError::TaskJoinError(s) => write!(f, "Task join error: {}", s),
            LLMProviderError::InferenceRecursionLimitReached(s) => write!(f, "Inferencing the LLM has reached too many iterations of recursion with no progess, and thus has been stopped for this user_message: {}", s),
            LLMProviderError::TokenizationError(s) => write!(f, "Tokenization error: {}", s),
            LLMProviderError::JobDequeueFailed(s) => write!(f, "Job dequeue failed: {}", s),
            LLMProviderError::ShinkaiMessage(err) => write!(f, "ShinkaiMessage error: {}", err),
            LLMProviderError::InboxNameError(err) => write!(f, "InboxName error: {}", err),
            LLMProviderError::InvalidCronCreationChainStage(s) => write!(f, "Invalid cron creation chain stage: {}", s),
            LLMProviderError::WebScrapingFailed(err) => write!(f, "Web scraping failed: {}", err),
            LLMProviderError::InvalidCronExecutionChainStage(s) => write!(f, "Invalid cron execution chain stage: {}", s),
            LLMProviderError::AnyhowError(err) => write!(f, "{}", err),
            LLMProviderError::LLMProviderMissingCapabilities(s) => write!(f, "LLMProvider is missing capabilities: {}", s),
            LLMProviderError::UnexpectedPromptResult(s) => write!(f, "Unexpected prompt result: {}", s),
            LLMProviderError::LLMProviderCapabilitiesManagerError(err) => write!(f, "LLMProviderCapabilitiesManager error: {}", err),
            LLMProviderError::UnexpectedPromptResultVariant(s) => write!(f, "Unexpected prompt result variant: {}", s),
            LLMProviderError::ImageContentNotFound(s) => write!(f, "Image content not found: {}", s),
            LLMProviderError::NoUserProfileFound => write!(f, "Cannot proceed as User Profile returned None."),
            LLMProviderError::NetworkError(s) => write!(f, "Network error: {}", s),
            LLMProviderError::InvalidModelType(s) => write!(f, "Invalid model type: {}", s),
            LLMProviderError::ShinkaiBackendInvalidAuthentication(s) => write!(f, "Shinkai Backend Invalid Authentication: {}", s),
            LLMProviderError::ShinkaiBackendInvalidConfiguration(s) => write!(f, "Shinkai Backend Invalid configuration: {}", s),
            LLMProviderError::ShinkaiBackendInferenceLimitReached(s) => write!(f, "Shinkai Backend Inference Limit Reached: {}", s),
            LLMProviderError::ShinkaiBackendAIProviderError(s) => write!(f, "Shinkai Backend AI Provider Error: {}", s),
            LLMProviderError::ShinkaiBackendUnexpectedStatusCode(code) => write!(f, "Shinkai Backend Unexpected Status Code: {}", code),
            LLMProviderError::ShinkaiBackendUnexpectedError(e) => write!(f, "Shinkai Backend Unexpected Error: {}", e),
            LLMProviderError::LLMServiceInferenceLimitReached(s) => write!(f, "LLM Provider Inference Limit Reached: {}", s),
            LLMProviderError::LLMServiceUnexpectedError(e) => write!(f, "LLM Provider Unexpected Error: {}", e),
            LLMProviderError::FailedSerdeParsingJSONString(s, err) => write!(f, "Failed parsing JSON string: `{}`. Fix the following Serde error: {}", s, err),
            LLMProviderError::FailedSerdeParsingXMLString(s, err) => write!(f, "Failed parsing XML string: `{}`. Fix the following Serde error: {}", s, err),
            LLMProviderError::ShinkaiMessageBuilderError(s) => write!(f, "{}", s),
            LLMProviderError::TokenLimit(s) => write!(f, "{}", s),
            LLMProviderError::WorkflowExecutionError(s) => write!(f, "{}", s),
            LLMProviderError::FunctionNotFound(s) => write!(f, "{}", s),
            LLMProviderError::FunctionExecutionError(s) => write!(f, "{}", s),
            LLMProviderError::InvalidFunctionArguments(s) => write!(f, "{}", s),
            LLMProviderError::InvalidFunctionResult(s) => write!(f, "{}", s),
            LLMProviderError::MaxIterationsReached(s) => write!(f, "{}", s),
            LLMProviderError::ToolRouterError(s) => write!(f, "{}", s),
            LLMProviderError::SerializationError(s) => write!(f, "{}", s),
            LLMProviderError::SheetManagerNotFound => write!(f, "Sheet Manager not found"),
            LLMProviderError::CallbackManagerNotFound => write!(f, "Callback Manager not found"),
            LLMProviderError::SheetManagerError(s) => write!(f, "{}", s),
            LLMProviderError::InputProcessingError(s) => write!(f, "{}", s),
        }
    }
}

impl LLMProviderError {
    /// Encodes the error as a JSON string that is easily parsable by frontends
    pub fn to_error_json(&self) -> String {
        let error_name = match self {
            LLMProviderError::UrlNotSet => "UrlNotSet",
            LLMProviderError::ApiKeyNotSet => "ApiKeyNotSet",
            LLMProviderError::ReqwestError(_) => "ReqwestError",
            LLMProviderError::MissingInitialStepInExecutionPlan => "MissingInitialStepInExecutionPlan",
            LLMProviderError::FailedExtractingJSONObjectFromResponse(_) => "FailedExtractingJSONObjectFromResponse",
            LLMProviderError::InferenceFailed => "InferenceFailed",
            LLMProviderError::UserPromptMissingEBNFDefinition => "UserPromptMissingEBNFDefinition",
            LLMProviderError::NotAJobMessage => "NotAJobMessage",
            LLMProviderError::JobNotFound => "JobNotFound",
            LLMProviderError::JobCreationDeserializationFailed => "JobCreationDeserializationFailed",
            LLMProviderError::JobMessageDeserializationFailed => "JobMessageDeserializationFailed",
            LLMProviderError::MessageTypeParseFailed => "MessageTypeParseFailed",
            LLMProviderError::IO(_) => "IO",
            LLMProviderError::ShinkaiDB(_) => "ShinkaiDB",
            LLMProviderError::VectorFS(_) => "VectorFS",
            LLMProviderError::ShinkaiNameError(_) => "ShinkaiNameError",
            LLMProviderError::LLMProviderNotFound => "LLMProviderNotFound",
            LLMProviderError::ContentParseFailed => "ContentParseFailed",
            LLMProviderError::InferenceJSONResponseMissingField(_) => "InferenceJSONResponseMissingField",
            LLMProviderError::JSONSerializationError(_) => "JSONSerializationError",
            LLMProviderError::VectorResource(_) => "VectorResource",
            LLMProviderError::InvalidSubidentity(_) => "InvalidSubidentity",
            LLMProviderError::InvalidProfileSubidentity(_) => "InvalidProfileSubidentity",
            LLMProviderError::SerdeError(_) => "SerdeError",
            LLMProviderError::TaskJoinError(_) => "TaskJoinError",
            LLMProviderError::InferenceRecursionLimitReached(_) => "InferenceRecursionLimitReached",
            LLMProviderError::TokenizationError(_) => "TokenizationError",
            LLMProviderError::JobDequeueFailed(_) => "JobDequeueFailed",
            LLMProviderError::ShinkaiMessage(_) => "ShinkaiMessage",
            LLMProviderError::InboxNameError(_) => "InboxNameError",
            LLMProviderError::InvalidCronCreationChainStage(_) => "InvalidCronCreationChainStage",
            LLMProviderError::WebScrapingFailed(_) => "WebScrapingFailed",
            LLMProviderError::InvalidCronExecutionChainStage(_) => "InvalidCronExecutionChainStage",
            LLMProviderError::AnyhowError(_) => "AnyhowError",
            LLMProviderError::LLMProviderMissingCapabilities(_) => "LLMProviderMissingCapabilities",
            LLMProviderError::UnexpectedPromptResult(_) => "UnexpectedPromptResult",
            LLMProviderError::LLMProviderCapabilitiesManagerError(_) => "LLMProviderCapabilitiesManagerError",
            LLMProviderError::UnexpectedPromptResultVariant(_) => "UnexpectedPromptResultVariant",
            LLMProviderError::ImageContentNotFound(_) => "ImageContentNotFound",
            LLMProviderError::NetworkError(_) => "NetworkError",
            LLMProviderError::NoUserProfileFound => "NoUserProfileFound",
            LLMProviderError::InvalidModelType(_) => "InvalidModelType",
            LLMProviderError::ShinkaiBackendInvalidAuthentication(_) => "ShinkaiBackendInvalidAuthentication",
            LLMProviderError::ShinkaiBackendInvalidConfiguration(_) => "ShinkaiBackendInvalidConfiguration",
            LLMProviderError::ShinkaiBackendInferenceLimitReached(_) => "ShinkaiBackendInferenceLimitReached",
            LLMProviderError::ShinkaiBackendAIProviderError(_) => "ShinkaiBackendAIProviderError",
            LLMProviderError::ShinkaiBackendUnexpectedStatusCode(_) => "ShinkaiBackendUnexpectedStatusCode",
            LLMProviderError::ShinkaiBackendUnexpectedError(_) => "ShinkaiBackendUnexpectedError",
            LLMProviderError::LLMServiceInferenceLimitReached(_) => "LLMServiceInferenceLimitReached",
            LLMProviderError::LLMServiceUnexpectedError(_) => "LLMServiceUnexpectedError",
            LLMProviderError::FailedSerdeParsingJSONString(_, _) => "FailedSerdeParsingJSONString",
            LLMProviderError::FailedSerdeParsingXMLString(_, _) => "FailedSerdeParsingXMLString",
            LLMProviderError::ShinkaiMessageBuilderError(_) => "ShinkaiMessageBuilderError",
            LLMProviderError::TokenLimit(_) => "TokenLimit",
            LLMProviderError::WorkflowExecutionError(_) => "WorkflowExecutionError",
            LLMProviderError::FunctionNotFound(_) => "FunctionNotFound",
            LLMProviderError::FunctionExecutionError(_) => "FunctionExecutionError",
            LLMProviderError::InvalidFunctionArguments(_) => "InvalidFunctionArguments",
            LLMProviderError::InvalidFunctionResult(_) => "InvalidFunctionResult",
            LLMProviderError::MaxIterationsReached(_) => "MaxIterationsReached",
            LLMProviderError::ToolRouterError(_) => "ToolRouterError",
            LLMProviderError::SerializationError(_) => "SerializationError",
            LLMProviderError::SheetManagerNotFound => "SheetManagerNotFound",
            LLMProviderError::CallbackManagerNotFound => "CallbackManagerNotFound",
            LLMProviderError::SheetManagerError(_) => "SheetManagerError",
            LLMProviderError::InputProcessingError(_) => "InputProcessingError",
        };

        let error_message = format!("{}", self);

        serde_json::json!({
            "error": error_name,
            "error_message": error_message
        }).to_string()
    }
}





impl From<AnyhowError> for LLMProviderError {
    fn from(error: AnyhowError) -> Self {
        LLMProviderError::AnyhowError(error)
    }
}

impl std::error::Error for LLMProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LLMProviderError::ReqwestError(err) => Some(err),
            LLMProviderError::ShinkaiDB(err) => Some(err),
            LLMProviderError::ShinkaiNameError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for LLMProviderError {
    fn from(err: reqwest::Error) -> LLMProviderError {
        LLMProviderError::ReqwestError(err)
    }
}

impl From<ShinkaiDBError> for LLMProviderError {
    fn from(err: ShinkaiDBError) -> LLMProviderError {
        LLMProviderError::ShinkaiDB(err)
    }
}

impl From<ShinkaiNameError> for LLMProviderError {
    fn from(err: ShinkaiNameError) -> LLMProviderError {
        LLMProviderError::ShinkaiNameError(err)
    }
}

impl From<Box<dyn std::error::Error>> for LLMProviderError {
    fn from(err: Box<dyn std::error::Error>) -> LLMProviderError {
        LLMProviderError::IO(err.to_string())
    }
}

impl From<serde_json::Error> for LLMProviderError {
    fn from(err: serde_json::Error) -> LLMProviderError {
        LLMProviderError::JSONSerializationError(err.to_string())
    }
}

impl From<VRError> for LLMProviderError {
    fn from(error: VRError) -> Self {
        LLMProviderError::VectorResource(error)
    }
}

impl From<JoinError> for LLMProviderError {
    fn from(err: JoinError) -> LLMProviderError {
        LLMProviderError::TaskJoinError(err.to_string())
    }
}

impl From<ShinkaiMessageError> for LLMProviderError {
    fn from(error: ShinkaiMessageError) -> Self {
        LLMProviderError::ShinkaiMessage(error)
    }
}

impl From<InboxNameError> for LLMProviderError {
    fn from(error: InboxNameError) -> Self {
        LLMProviderError::InboxNameError(error)
    }
}

impl From<ModelCapabilitiesManagerError> for LLMProviderError {
    fn from(error: ModelCapabilitiesManagerError) -> Self {
        LLMProviderError::LLMProviderCapabilitiesManagerError(error)
    }
}

impl From<VectorFSError> for LLMProviderError {
    fn from(err: VectorFSError) -> LLMProviderError {
        LLMProviderError::VectorFS(err)
    }
}

impl From<String> for LLMProviderError {
    fn from(err: String) -> LLMProviderError {
        LLMProviderError::WorkflowExecutionError(err)
    }
}

impl From<WorkflowError> for LLMProviderError {
    fn from(err: WorkflowError) -> LLMProviderError {
        LLMProviderError::WorkflowExecutionError(err.to_string())
    }
}