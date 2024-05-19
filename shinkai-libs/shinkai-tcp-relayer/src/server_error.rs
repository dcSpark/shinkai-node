use std::fmt;

#[derive(Debug)]
pub enum NetworkMessageError {
    ReadError(std::io::Error),
    Utf8Error(std::string::FromUtf8Error),
    UnknownMessageType(u8),
    InvalidData,
}

impl fmt::Display for NetworkMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkMessageError::ReadError(err) => write!(f, "Failed to read exact bytes from socket: {}", err),
            NetworkMessageError::Utf8Error(err) => write!(f, "Invalid UTF-8 sequence: {}", err),
            NetworkMessageError::UnknownMessageType(t) => write!(f, "Unknown message type: {}", t),
            NetworkMessageError::InvalidData => write!(f, "Invalid data received"),
        }
    }
}

impl std::error::Error for NetworkMessageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NetworkMessageError::ReadError(err) => Some(err),
            NetworkMessageError::Utf8Error(err) => Some(err),
            NetworkMessageError::UnknownMessageType(_) => None,
            NetworkMessageError::InvalidData => None,
        }
    }
}

impl From<std::io::Error> for NetworkMessageError {
    fn from(err: std::io::Error) -> NetworkMessageError {
        NetworkMessageError::ReadError(err)
    }
}

impl From<std::string::FromUtf8Error> for NetworkMessageError {
    fn from(err: std::string::FromUtf8Error) -> NetworkMessageError {
        NetworkMessageError::Utf8Error(err)
    }
}