use crate::resources::resource_errors::ResourceError;
use core::fmt;
use std::{io, str::Utf8Error};
use shinkai_message_wasm::schemas::inbox_name::InboxNameError;

#[derive(Debug)]
pub enum ShinkaiDBError {
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
    JsonSerializationError(serde_json::Error),
    DataConversionError,
    DataNotFound,
    ResourceError(ResourceError),
    FailedFetchingCF,
    FailedFetchingValue,
    BincodeError(bincode::Error),
    InboxNameError(InboxNameError),
    ProfileNotFound,
    DeviceIdentityAlreadyExists,
    ProfileNameNotProvided,
    InvalidPermissionsType
}

impl fmt::Display for ShinkaiDBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShinkaiDBError::RocksDBError(e) => write!(f, "RocksDB error: {}", e),
            ShinkaiDBError::CodeAlreadyUsed => {
                write!(f, "Registration code has already been used")
            }
            ShinkaiDBError::CodeNonExistent => write!(f, "Registration code does not exist"),
            ShinkaiDBError::ProfileNameAlreadyExists => {
                write!(f, "Profile name already exists")
            }
            ShinkaiDBError::DecodeError(e) => write!(f, "Decoding Error: {}", e),
            ShinkaiDBError::MessageNotFound => write!(f, "Message not found"),
            ShinkaiDBError::SomeError => write!(f, "Some mysterious error..."),
            ShinkaiDBError::ProfileNameNonExistent => {
                write!(f, "Profile name does not exist")
            }
            ShinkaiDBError::EncryptionKeyNonExistent => {
                write!(f, "Encryption key does not exist")
            }
            ShinkaiDBError::PublicKeyParseError => write!(f, "Error parsing public key"),
            ShinkaiDBError::InboxNotFound => write!(f, "Inbox not found"),
            ShinkaiDBError::IOError(e) => write!(f, "IO Error: {}", e),
            ShinkaiDBError::MissingExternalMetadata => {
                write!(f, "Missing external metadata")
            }
            ShinkaiDBError::MissingBody => write!(f, "Missing body"),
            ShinkaiDBError::MissingInternalMetadata => {
                write!(f, "Missing internal metadata")
            }
            ShinkaiDBError::IdentityNotFound => write!(f, "Identity not found"),
            ShinkaiDBError::InvalidData => write!(f, "Invalid data"),
            ShinkaiDBError::PermissionDenied(e) => write!(f, "Permission denied: {}", e),
            ShinkaiDBError::PermissionNotFound => write!(f, "Permission not found"),
            ShinkaiDBError::InvalidIdentityType => write!(f, "Invalid permission type"),
            ShinkaiDBError::InvalidInboxName => write!(f, "Invalid inbox name"),
            ShinkaiDBError::Utf8ConversionError => write!(f, "UTF8 conversion error"),
            ShinkaiDBError::JsonSerializationError(e) => write!(f, "Json Serialization Error: {}", e),
            ShinkaiDBError::DataConversionError => write!(f, "Data conversion error"),
            ShinkaiDBError::DataNotFound => write!(f, "Data not found"),
            ShinkaiDBError::FailedFetchingCF => write!(f, "Failed fetching Column Family"),
            ShinkaiDBError::FailedFetchingValue => write!(f, "Failed fetching value. Likely invalid CF or key."),
            ShinkaiDBError::ResourceError(e) => write!(f, "{}", e),
            ShinkaiDBError::BincodeError(e) => write!(f, "Bincode error: {}", e),
            ShinkaiDBError::InboxNameError(e) => write!(f, "Inbox name error: {}", e),
            ShinkaiDBError::ProfileNotFound => write!(f, "Profile not found"),
            ShinkaiDBError::DeviceIdentityAlreadyExists => write!(f, "Device identity already exists"),
            ShinkaiDBError::ProfileNameNotProvided => write!(f, "Profile name not provided"),
            ShinkaiDBError::InvalidPermissionsType => write!(f, "Invalid permissions type"),
        }
    }
}

impl std::error::Error for ShinkaiDBError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShinkaiDBError::RocksDBError(e) => Some(e),
            ShinkaiDBError::DecodeError(e) => Some(e),
            ShinkaiDBError::JsonSerializationError(e) => Some(e),
            _ => None,
        }
    }
}

impl PartialEq for ShinkaiDBError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ShinkaiDBError::InboxNotFound, ShinkaiDBError::InboxNotFound) => true,
            (ShinkaiDBError::MessageNotFound, ShinkaiDBError::MessageNotFound) => true,
            (ShinkaiDBError::CodeAlreadyUsed, ShinkaiDBError::CodeAlreadyUsed) => true,
            (ShinkaiDBError::CodeNonExistent, ShinkaiDBError::CodeNonExistent) => true,
            (ShinkaiDBError::ProfileNameAlreadyExists, ShinkaiDBError::ProfileNameAlreadyExists) => true,
            (ShinkaiDBError::ProfileNameNonExistent, ShinkaiDBError::ProfileNameNonExistent) => true,
            (ShinkaiDBError::EncryptionKeyNonExistent, ShinkaiDBError::EncryptionKeyNonExistent) => true,
            (ShinkaiDBError::PublicKeyParseError, ShinkaiDBError::PublicKeyParseError) => true,
            (ShinkaiDBError::IdentityNotFound, ShinkaiDBError::IdentityNotFound) => true,
            (ShinkaiDBError::MissingExternalMetadata, ShinkaiDBError::MissingExternalMetadata) => true,
            (ShinkaiDBError::MissingBody, ShinkaiDBError::MissingBody) => true,
            (ShinkaiDBError::MissingInternalMetadata, ShinkaiDBError::MissingInternalMetadata) => true,
            (ShinkaiDBError::IOError(_), ShinkaiDBError::IOError(_)) => true,
            (ShinkaiDBError::DecodeError(_), ShinkaiDBError::DecodeError(_)) => true,
            (ShinkaiDBError::RocksDBError(_), ShinkaiDBError::RocksDBError(_)) => true,
            (ShinkaiDBError::SomeError, ShinkaiDBError::SomeError) => true,
            (ShinkaiDBError::InvalidData, ShinkaiDBError::InvalidData) => true,
            (ShinkaiDBError::PermissionDenied(_), ShinkaiDBError::PermissionDenied(_)) => true,
            (ShinkaiDBError::PermissionNotFound, ShinkaiDBError::PermissionNotFound) => true,
            (ShinkaiDBError::InvalidIdentityType, ShinkaiDBError::InvalidIdentityType) => true,
            (ShinkaiDBError::InvalidInboxName, ShinkaiDBError::InvalidInboxName) => true,
            (ShinkaiDBError::Utf8ConversionError, ShinkaiDBError::Utf8ConversionError) => true,
            (ShinkaiDBError::JsonSerializationError(_), ShinkaiDBError::JsonSerializationError(_)) => true,
            (ShinkaiDBError::DataConversionError, ShinkaiDBError::DataConversionError) => true,
            (ShinkaiDBError::DataNotFound, ShinkaiDBError::DataNotFound) => true,
            (ShinkaiDBError::FailedFetchingCF, ShinkaiDBError::FailedFetchingCF) => true,
            (ShinkaiDBError::FailedFetchingValue, ShinkaiDBError::FailedFetchingValue) => true,
            (ShinkaiDBError::ResourceError(_), ShinkaiDBError::ResourceError(_)) => true,
            (ShinkaiDBError::BincodeError(_), ShinkaiDBError::BincodeError(_)) => true,
            (ShinkaiDBError::InboxNameError(_), ShinkaiDBError::InboxNameError(_)) => true,
            (ShinkaiDBError::ProfileNotFound, ShinkaiDBError::ProfileNotFound) => true,
            (ShinkaiDBError::DeviceIdentityAlreadyExists, ShinkaiDBError::DeviceIdentityAlreadyExists) => true,
            _ => false,
        }
    }
}

impl From<ResourceError> for ShinkaiDBError {
    fn from(err: ResourceError) -> ShinkaiDBError {
        ShinkaiDBError::ResourceError(err)
    }
}

impl From<rocksdb::Error> for ShinkaiDBError {
    fn from(error: rocksdb::Error) -> Self {
        ShinkaiDBError::RocksDBError(error)
    }
}

impl From<prost::DecodeError> for ShinkaiDBError {
    fn from(error: prost::DecodeError) -> Self {
        ShinkaiDBError::DecodeError(error)
    }
}

impl From<io::Error> for ShinkaiDBError {
    fn from(error: io::Error) -> Self {
        ShinkaiDBError::IOError(error)
    }
}

impl From<&str> for ShinkaiDBError {
    fn from(_: &str) -> Self {
        ShinkaiDBError::PublicKeyParseError
    }
}

impl From<serde_json::Error> for ShinkaiDBError {
    fn from(error: serde_json::Error) -> Self {
        ShinkaiDBError::JsonSerializationError(error)
    }
}

impl From<Utf8Error> for ShinkaiDBError {
    fn from(_: Utf8Error) -> Self {
        ShinkaiDBError::Utf8ConversionError
    }
}

impl From<bincode::Error> for ShinkaiDBError {
    fn from(error: bincode::Error) -> Self {
        ShinkaiDBError::BincodeError(error)
    }
}

impl From<InboxNameError> for ShinkaiDBError {
    fn from(error: InboxNameError) -> Self {
        ShinkaiDBError::InboxNameError(error)
    }
}
