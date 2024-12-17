use std::fmt;
use warp::reject::Reject;
use shinkai_message_primitives::schemas::ws_types::WebSocketManagerError;

#[derive(Debug)]
pub struct APIError {
    pub message: String,
}

impl fmt::Display for APIError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Reject for APIError {}

impl From<&str> for APIError {
    fn from(message: &str) -> Self {
        APIError {
            message: message.to_string(),
        }
    }
}

impl From<String> for APIError {
    fn from(message: String) -> Self {
        APIError { message }
    }
}

impl From<WebSocketManagerError> for APIError {
    fn from(error: WebSocketManagerError) -> Self {
        APIError {
            message: error.to_string(),
        }
    }
}

impl APIError {
    pub fn invalid_message_content() -> Self {
        APIError {
            message: "Invalid message content".to_string(),
        }
    }
}
