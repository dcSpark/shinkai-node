use serde::{Serialize, Deserialize};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobQueueManagerError {
    UrlNotSet,
    ApiKeyNotSet,
    ReqwestError(String),
}

impl fmt::Display for JobQueueManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JobQueueManagerError::UrlNotSet => write!(f, "URL is not set"),
            JobQueueManagerError::ApiKeyNotSet => write!(f, "API Key not set"),
            JobQueueManagerError::ReqwestError(err) => write!(f, "Reqwest error: {}", err),
        }
    }
}

impl Error for JobQueueManagerError {}