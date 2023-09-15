use std::fmt;

pub enum AgentError {
    UrlNotSet,
    ApiKeyNotSet,
    ReqwestError(reqwest::Error),
    MissingInitialStepInExecutionPlan,
    FailedExtractingJSONObjectFromResponse(String),
    FailedInferencingLocalLLM,
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
        }
    }
}

impl From<reqwest::Error> for AgentError {
    fn from(err: reqwest::Error) -> AgentError {
        AgentError::ReqwestError(err)
    }
}
