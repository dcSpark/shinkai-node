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
    InvalidIdentityType(String),
    Utf8ConversionError,
    PermissionNotFound(String),
    MissingExternalMetadata,
    MissingBody,
    MissingInternalMetadata,
    MessageNotFound,
    CodeAlreadyUsed,
    CodeNonExistent,
    ProfileNameAlreadyExists,
    SomeError(String),
    ProfileNameNonExistent(String),
    EncryptionKeyNonExistent,
    PublicKeyParseError,
    InboxNotFound(String),
    IdentityNotFound(String),
    InvalidData,
    InvalidInboxName,
    JsonSerializationError(serde_json::Error),
    DataConversionError(String),
    DataNotFound,
    ResourceError(ResourceError),
    FailedFetchingCF,
    FailedFetchingValue,
    BincodeError(bincode::Error),
    InboxNameError(InboxNameError),
    ProfileNotFound(String),
    DeviceIdentityAlreadyExists,
    ProfileNameNotProvided,
    InvalidPermissionsType,
    MissingValue(String),
    ColumnFamilyNotFound(String),
    InvalidInboxPermission(String),
    InvalidPermissionType(String),
    InvalidProfileName(String),
    InvalidIdentityName(String),
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
            ShinkaiDBError::SomeError(e) => write!(f, "Some error: {}", e),
            ShinkaiDBError::ProfileNameNonExistent(e) => {
                write!(f, "Profile name does not exist: {}", e)
            }
            ShinkaiDBError::EncryptionKeyNonExistent => {
                write!(f, "Encryption key does not exist")
            }
            ShinkaiDBError::PublicKeyParseError => write!(f, "Error parsing public key"),
            ShinkaiDBError::InboxNotFound(e) => write!(f, "Inbox not found: {}", e),
            ShinkaiDBError::IOError(e) => write!(f, "IO Error: {}", e),
            ShinkaiDBError::MissingExternalMetadata => {
                write!(f, "Missing external metadata")
            }
            ShinkaiDBError::MissingBody => write!(f, "Missing body"),
            ShinkaiDBError::MissingInternalMetadata => {
                write!(f, "Missing internal metadata")
            }
            ShinkaiDBError::IdentityNotFound(e) => write!(f, "Identity not found: {}", e),
            ShinkaiDBError::InvalidData => write!(f, "Invalid data"),
            ShinkaiDBError::PermissionDenied(e) => write!(f, "Permission denied: {}", e),
            ShinkaiDBError::PermissionNotFound(e) => write!(f, "Permission not found: {}", e),
            ShinkaiDBError::InvalidIdentityType(e) => write!(f, "Invalid identity type: {}", e),
            ShinkaiDBError::InvalidInboxName => write!(f, "Invalid inbox name"),
            ShinkaiDBError::Utf8ConversionError => write!(f, "UTF8 conversion error"),
            ShinkaiDBError::JsonSerializationError(e) => write!(f, "Json Serialization Error: {}", e),
            ShinkaiDBError::DataNotFound => write!(f, "Data not found"),
            ShinkaiDBError::FailedFetchingCF => write!(f, "Failed fetching Column Family"),
            ShinkaiDBError::FailedFetchingValue => write!(f, "Failed fetching value. Likely invalid CF or key."),
            ShinkaiDBError::ResourceError(e) => write!(f, "{}", e),
            ShinkaiDBError::BincodeError(e) => write!(f, "Bincode error: {}", e),
            ShinkaiDBError::InboxNameError(e) => write!(f, "Inbox name error: {}", e),
            ShinkaiDBError::ProfileNotFound(e) => write!(f, "Profile not found: {}", e),
            ShinkaiDBError::DeviceIdentityAlreadyExists => write!(f, "Device identity already exists"),
            ShinkaiDBError::ProfileNameNotProvided => write!(f, "Profile name not provided"),
            ShinkaiDBError::InvalidPermissionsType => write!(f, "Invalid permissions type"),
            ShinkaiDBError::MissingValue(e) => write!(f, "Missing value: {}", e),
            ShinkaiDBError::ColumnFamilyNotFound(e) => write!(f, "Column family not found: {}", e),
            ShinkaiDBError::DataConversionError(e) => write!(f, "Data conversion error: {}", e),
            ShinkaiDBError::InvalidInboxPermission(e) => write!(f, "Invalid inbox permission: {}", e),
            ShinkaiDBError::InvalidPermissionType(e) => write!(f, "Invalid permission type: {}", e),
            ShinkaiDBError::InvalidProfileName(e) => write!(f, "Invalid profile name: {}", e),
            ShinkaiDBError::InvalidIdentityName(e) => write!(f, "Invalid identity name: {}", e),
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
            (ShinkaiDBError::MessageNotFound, ShinkaiDBError::MessageNotFound) => true,
            (ShinkaiDBError::CodeAlreadyUsed, ShinkaiDBError::CodeAlreadyUsed) => true,
            (ShinkaiDBError::CodeNonExistent, ShinkaiDBError::CodeNonExistent) => true,
            (ShinkaiDBError::ProfileNameAlreadyExists, ShinkaiDBError::ProfileNameAlreadyExists) => true,
            (ShinkaiDBError::EncryptionKeyNonExistent, ShinkaiDBError::EncryptionKeyNonExistent) => true,
            (ShinkaiDBError::PublicKeyParseError, ShinkaiDBError::PublicKeyParseError) => true,
            (ShinkaiDBError::MissingExternalMetadata, ShinkaiDBError::MissingExternalMetadata) => true,
            (ShinkaiDBError::MissingBody, ShinkaiDBError::MissingBody) => true,
            (ShinkaiDBError::MissingInternalMetadata, ShinkaiDBError::MissingInternalMetadata) => true,
            (ShinkaiDBError::PermissionDenied(msg1), ShinkaiDBError::PermissionDenied(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InvalidIdentityType(msg1), ShinkaiDBError::InvalidIdentityType(msg2)) => msg1 == msg2,
            (ShinkaiDBError::PermissionNotFound(msg1), ShinkaiDBError::PermissionNotFound(msg2)) => msg1 == msg2,
            (ShinkaiDBError::SomeError(msg1), ShinkaiDBError::SomeError(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InboxNotFound(msg1), ShinkaiDBError::InboxNotFound(msg2)) => msg1 == msg2,
            (ShinkaiDBError::IdentityNotFound(msg1), ShinkaiDBError::IdentityNotFound(msg2)) => msg1 == msg2,
            (ShinkaiDBError::ProfileNameNonExistent(msg1), ShinkaiDBError::ProfileNameNonExistent(msg2)) => msg1 == msg2,
            (ShinkaiDBError::MissingValue(msg1), ShinkaiDBError::MissingValue(msg2)) => msg1 == msg2,
            (ShinkaiDBError::ColumnFamilyNotFound(msg1), ShinkaiDBError::ColumnFamilyNotFound(msg2)) => msg1 == msg2,
            (ShinkaiDBError::DataConversionError(msg1), ShinkaiDBError::DataConversionError(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InvalidInboxPermission(msg1), ShinkaiDBError::InvalidInboxPermission(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InvalidPermissionType(msg1), ShinkaiDBError::InvalidPermissionType(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InvalidProfileName(msg1), ShinkaiDBError::InvalidProfileName(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InvalidIdentityName(msg1), ShinkaiDBError::InvalidIdentityName(msg2)) => msg1 == msg2,
            //
            (ShinkaiDBError::IOError(_), ShinkaiDBError::IOError(_)) => true,
            (ShinkaiDBError::DecodeError(_), ShinkaiDBError::DecodeError(_)) => true,
            (ShinkaiDBError::InvalidInboxName, ShinkaiDBError::InvalidInboxName) => true,
            (ShinkaiDBError::Utf8ConversionError, ShinkaiDBError::Utf8ConversionError) => true,
            (ShinkaiDBError::JsonSerializationError(_), ShinkaiDBError::JsonSerializationError(_)) => true,
            (ShinkaiDBError::DataNotFound, ShinkaiDBError::DataNotFound) => true,
            (ShinkaiDBError::FailedFetchingCF, ShinkaiDBError::FailedFetchingCF) => true,
            (ShinkaiDBError::FailedFetchingValue, ShinkaiDBError::FailedFetchingValue) => true,
            (ShinkaiDBError::ResourceError(_), ShinkaiDBError::ResourceError(_)) => true,
            (ShinkaiDBError::BincodeError(_), ShinkaiDBError::BincodeError(_)) => true,
            (ShinkaiDBError::InboxNameError(_), ShinkaiDBError::InboxNameError(_)) => true,
            (ShinkaiDBError::ProfileNotFound(_), ShinkaiDBError::ProfileNotFound(_)) => true,
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
