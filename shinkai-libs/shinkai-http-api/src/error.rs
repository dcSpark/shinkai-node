use std::fmt;

#[derive(Debug)]
pub struct APIError {
    pub message: String,
}

impl fmt::Display for APIError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for APIError {}

impl From<String> for APIError {
    fn from(message: String) -> Self {
        APIError { message }
    }
}

impl From<&str> for APIError {
    fn from(message: &str) -> Self {
        APIError {
            message: message.to_string(),
        }
    }
}

impl From<crate::websocket::ws_manager::WebSocketManagerError> for APIError {
    fn from(error: crate::websocket::ws_manager::WebSocketManagerError) -> Self {
        APIError {
            message: error.to_string(),
        }
    }
}

impl APIError {
    pub const InvalidMessageContent: APIError = APIError {
        message: "Invalid message content".to_string(),
    };
}
