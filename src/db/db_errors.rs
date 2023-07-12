use core::fmt;
use std::io;

#[derive(Debug)]
pub enum ShinkaiMessageDBError {
    RocksDBError(rocksdb::Error),
    DecodeError(prost::DecodeError),
    IOError(io::Error),
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
