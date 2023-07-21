use core::fmt;
use std::io;

#[derive(Debug)]
pub enum ShinkaiMessageDBError {
    RocksDBError(rocksdb::Error),
    DecodeError(prost::DecodeError),
    IOError(io::Error),
    PermissionDenied(String),
    InvalidIdentityType,
    Utf8ConversionError,
    PermissionNotFound,
    MissingExternalMetadata,
    MissingBody,
    MissingInternalMetadata,
    MessageNotFound,
    CodeAlreadyUsed,
    CodeNonExistent,
    ProfileNameAlreadyExists,
    SomeError,
    ProfileNameNonExistent,
    EncryptionKeyNonExistent,
    PublicKeyParseError,
    InboxNotFound,
    IdentityNotFound,
    InvalidData,
    InvalidInboxName,
}

impl From<rocksdb::Error> for ShinkaiMessageDBError {
    fn from(error: rocksdb::Error) -> Self {
        ShinkaiMessageDBError::RocksDBError(error)
    }
}

impl From<prost::DecodeError> for ShinkaiMessageDBError {
    fn from(error: prost::DecodeError) -> Self {
        ShinkaiMessageDBError::DecodeError(error)
    }
}

impl From<io::Error> for ShinkaiMessageDBError {
    fn from(error: io::Error) -> Self {
        ShinkaiMessageDBError::IOError(error)
    }
}

impl From<&str> for ShinkaiMessageDBError {
    fn from(_: &str) -> Self {
        ShinkaiMessageDBError::PublicKeyParseError
    }
}

impl fmt::Display for ShinkaiMessageDBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShinkaiMessageDBError::RocksDBError(e) => write!(f, "RocksDB error: {}", e),
            ShinkaiMessageDBError::CodeAlreadyUsed => {
                write!(f, "Registration code has already been used")
            }
            ShinkaiMessageDBError::CodeNonExistent => write!(f, "Registration code does not exist"),
            ShinkaiMessageDBError::ProfileNameAlreadyExists => {
                write!(f, "Profile name already exists")
            }
            ShinkaiMessageDBError::DecodeError(e) => write!(f, "Decoding Error: {}", e),
            ShinkaiMessageDBError::MessageNotFound => write!(f, "Message not found"),
            ShinkaiMessageDBError::SomeError => write!(f, "Some mysterious error..."),
            ShinkaiMessageDBError::ProfileNameNonExistent => {
                write!(f, "Profile name does not exist")
            }
            ShinkaiMessageDBError::EncryptionKeyNonExistent => {
                write!(f, "Encryption key does not exist")
            }
            ShinkaiMessageDBError::PublicKeyParseError => write!(f, "Error parsing public key"),
            ShinkaiMessageDBError::InboxNotFound => write!(f, "Inbox not found"),
            ShinkaiMessageDBError::IOError(e) => write!(f, "IO Error: {}", e),
            ShinkaiMessageDBError::MissingExternalMetadata => {
                write!(f, "Missing external metadata")
            }
            ShinkaiMessageDBError::MissingBody => write!(f, "Missing body"),
            ShinkaiMessageDBError::MissingInternalMetadata => {
                write!(f, "Missing internal metadata")
            }
            ShinkaiMessageDBError::IdentityNotFound => write!(f, "Identity not found"),
            ShinkaiMessageDBError::InvalidData => write!(f, "Invalid data"),
            ShinkaiMessageDBError::PermissionDenied(e) => write!(f, "Permission denied: {}", e),
            ShinkaiMessageDBError::PermissionNotFound => write!(f, "Permission not found"),
            ShinkaiMessageDBError::InvalidIdentityType => write!(f, "Invalid permission type"),
            ShinkaiMessageDBError::InvalidInboxName => write!(f, "Invalid inbox name"),
            ShinkaiMessageDBError::Utf8ConversionError => write!(f, "UTF8 conversion error"),
        }
    }
}

impl std::error::Error for ShinkaiMessageDBError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShinkaiMessageDBError::RocksDBError(e) => Some(e),
            ShinkaiMessageDBError::DecodeError(e) => Some(e),
            _ => None,
        }
    }
}

impl PartialEq for ShinkaiMessageDBError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ShinkaiMessageDBError::InboxNotFound, ShinkaiMessageDBError::InboxNotFound) => true,
            (ShinkaiMessageDBError::MessageNotFound, ShinkaiMessageDBError::MessageNotFound) => {
                true
            }
            (ShinkaiMessageDBError::CodeAlreadyUsed, ShinkaiMessageDBError::CodeAlreadyUsed) => {
                true
            }
            (ShinkaiMessageDBError::CodeNonExistent, ShinkaiMessageDBError::CodeNonExistent) => {
                true
            }
            (
                ShinkaiMessageDBError::ProfileNameAlreadyExists,
                ShinkaiMessageDBError::ProfileNameAlreadyExists,
            ) => true,
            (
                ShinkaiMessageDBError::ProfileNameNonExistent,
                ShinkaiMessageDBError::ProfileNameNonExistent,
            ) => true,
            (
                ShinkaiMessageDBError::EncryptionKeyNonExistent,
                ShinkaiMessageDBError::EncryptionKeyNonExistent,
            ) => true,
            (ShinkaiMessageDBError::PublicKeyParseError, ShinkaiMessageDBError::PublicKeyParseError) => true,
            (ShinkaiMessageDBError::IdentityNotFound, ShinkaiMessageDBError::IdentityNotFound) => true,
            (
                ShinkaiMessageDBError::MissingExternalMetadata,
                ShinkaiMessageDBError::MissingExternalMetadata,
            ) => true,
            (ShinkaiMessageDBError::MissingBody, ShinkaiMessageDBError::MissingBody) => true,
            (
                ShinkaiMessageDBError::MissingInternalMetadata,
                ShinkaiMessageDBError::MissingInternalMetadata,
            ) => true,
            (ShinkaiMessageDBError::IOError(_), ShinkaiMessageDBError::IOError(_)) => true,
            (ShinkaiMessageDBError::DecodeError(_), ShinkaiMessageDBError::DecodeError(_)) => true,
            (ShinkaiMessageDBError::RocksDBError(_), ShinkaiMessageDBError::RocksDBError(_)) => true,
            (ShinkaiMessageDBError::SomeError, ShinkaiMessageDBError::SomeError) => true,
            (ShinkaiMessageDBError::InvalidData, ShinkaiMessageDBError::InvalidData) => true,
            (ShinkaiMessageDBError::PermissionDenied(_), ShinkaiMessageDBError::PermissionDenied(_)) => true,
            (ShinkaiMessageDBError::PermissionNotFound, ShinkaiMessageDBError::PermissionNotFound) => true,
            (ShinkaiMessageDBError::InvalidIdentityType, ShinkaiMessageDBError::InvalidIdentityType) => true,
            (ShinkaiMessageDBError::InvalidInboxName, ShinkaiMessageDBError::InvalidInboxName) => true,
            (ShinkaiMessageDBError::Utf8ConversionError, ShinkaiMessageDBError::Utf8ConversionError) => true,
            _ => false,
        }
    }
}
