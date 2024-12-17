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
