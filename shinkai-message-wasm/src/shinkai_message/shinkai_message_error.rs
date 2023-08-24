use std::fmt;

#[derive(Debug)]
pub enum ShinkaiMessageError {
    SigningError(String),
    DecryptionError(String),
    EncryptionError(String),
    InvalidMessageSchemaType(String),
    MissingMessageBody(String),
    DeserializationError(String),
}

impl fmt::Display for ShinkaiMessageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShinkaiMessageError::SigningError(msg) => write!(f, "SigningError: {}", msg),
            ShinkaiMessageError::DecryptionError(msg) => write!(f, "DecryptionError: {}", msg),
            ShinkaiMessageError::EncryptionError(msg) => write!(f, "EncryptionError: {}", msg),
            ShinkaiMessageError::InvalidMessageSchemaType(msg) => write!(f, "InvalidMessageSchemaType: {}", msg),
            ShinkaiMessageError::MissingMessageBody(msg) => write!(f, "MissingMessageBody: {}", msg),
            ShinkaiMessageError::DeserializationError(msg) => write!(f, "DeserializationError: {}", msg),
        }
    }
}

impl std::error::Error for ShinkaiMessageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Note: Update this if we wrap other error and we want to return the source (underlying cause).
        None
    }
}