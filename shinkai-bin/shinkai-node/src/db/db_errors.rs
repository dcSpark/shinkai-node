use crate::{tools::error::ToolError, vector_fs::vector_fs_error::VectorFSError};
use core::fmt;
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxNameError, shinkai_name::ShinkaiNameError},
    shinkai_message::shinkai_message_error::ShinkaiMessageError,
};
use shinkai_vector_resources::resource_errors::VRError;
use std::{io, str::{ParseBoolError, Utf8Error}};

#[derive(Debug)]
pub enum ShinkaiDBError {
    ShinkaiNameError(ShinkaiNameError),
    RocksDBError(rocksdb::Error),
    IOError(io::Error),
    InvalidIdentityType(String),
    Utf8ConversionError,
    PermissionNotFound(String),
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
    JsonSerializationError(serde_json::Error),
    DataConversionError(String),
    DataNotFound,
    VRError(VRError),
    FailedFetchingCF,
    FailedFetchingValue,
    BincodeError(bincode::Error),
    InboxNameError(InboxNameError),
    ProfileNotFound(String),
    DeviceIdentityAlreadyExists(String),
    InvalidPermissionsType,
    MissingValue(String),
    ColumnFamilyNotFound(String),
    InvalidInboxPermission(String),
    InvalidPermissionType(String),
    InvalidProfileName(String),
    InvalidIdentityName(String),
    DeviceNameNonExistent(String),
    ShinkaiNameLacksProfile,
    ToolError(ToolError),
    MessageEncodingError(String),
    ShinkaiMessageError(String),
    JobAlreadyExists(String),
    CronTaskNotFound(String),
    VectorFSError(String),
    InvalidAttributeName(String),
    BoolParseError(String),
    ToolNotFound(String),
    DeserializationFailed(String),
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
            ShinkaiDBError::MessageNotFound => write!(f, "Message not found"),
            ShinkaiDBError::SomeError(e) => write!(f, "Some error: {}", e),
            ShinkaiDBError::ShinkaiNameLacksProfile => write!(
                f,
                "Provided ShinkaiName does not specify a profile which is required for DB action.",
            ),

            ShinkaiDBError::ProfileNameNonExistent(e) => {
                write!(f, "Profile name does not exist: {}", e)
            }
            ShinkaiDBError::EncryptionKeyNonExistent => {
                write!(f, "Encryption key does not exist")
            }
            ShinkaiDBError::PublicKeyParseError => write!(f, "Error parsing public key"),
            ShinkaiDBError::InboxNotFound(e) => write!(f, "Inbox not found: {}", e),
            ShinkaiDBError::IOError(e) => write!(f, "IO Error: {}", e),
            ShinkaiDBError::IdentityNotFound(e) => write!(f, "Identity not found: {}", e),
            ShinkaiDBError::InvalidData => write!(f, "Invalid data"),
            ShinkaiDBError::PermissionNotFound(e) => write!(f, "Permission not found: {}", e),
            ShinkaiDBError::InvalidIdentityType(e) => write!(f, "Invalid identity type: {}", e),
            ShinkaiDBError::ShinkaiNameError(e) => write!(f, "Shinkai name error: {}", e),
            ShinkaiDBError::InvalidInboxPermission(e) => write!(f, "Invalid inbox permission: {}", e),
            ShinkaiDBError::InvalidPermissionType(e) => write!(f, "Invalid permission type: {}", e),
            ShinkaiDBError::InvalidProfileName(e) => write!(f, "Invalid profile name: {}", e),
            ShinkaiDBError::InvalidIdentityName(e) => write!(f, "Invalid identity name: {}", e),
            ShinkaiDBError::InvalidPermissionsType => write!(f, "Invalid permissions type"),
            ShinkaiDBError::MissingValue(e) => write!(f, "Missing value: {}", e),
            ShinkaiDBError::ColumnFamilyNotFound(e) => write!(f, "Column family not found: {}", e),
            ShinkaiDBError::DataConversionError(e) => write!(f, "Data conversion error: {}", e),
            ShinkaiDBError::Utf8ConversionError => write!(f, "UTF8 conversion error"),
            ShinkaiDBError::JsonSerializationError(e) => write!(f, "Json Serialization Error: {}", e),
            ShinkaiDBError::DataNotFound => write!(f, "Data not found"),
            ShinkaiDBError::FailedFetchingCF => write!(f, "Failed fetching Column Family"),
            ShinkaiDBError::FailedFetchingValue => write!(f, "Failed fetching value. Likely invalid CF or key."),
            ShinkaiDBError::VRError(e) => write!(f, "{}", e),
            ShinkaiDBError::BincodeError(e) => write!(f, "Bincode error: {}", e),
            ShinkaiDBError::ToolError(e) => write!(f, "Tool error: {}", e),
            ShinkaiDBError::InboxNameError(e) => write!(f, "Inbox name error: {}", e),
            ShinkaiDBError::ProfileNotFound(e) => write!(f, "Profile not found: {}", e),
            ShinkaiDBError::DeviceIdentityAlreadyExists(e) => write!(f, "Device identity already exists: {}", e),
            ShinkaiDBError::DeviceNameNonExistent(e) => write!(f, "Device name does not exist: {}", e),
            ShinkaiDBError::MessageEncodingError(e) => write!(f, "Message encoding error: {}", e),
            ShinkaiDBError::ShinkaiMessageError(e) => write!(f, "ShinkaiMessage error: {}", e),
            ShinkaiDBError::JobAlreadyExists(e) => write!(f, "Job attempted to be created, but already exists: {}", e),
            ShinkaiDBError::CronTaskNotFound(e) => write!(f, "Cron task not found: {}", e),
            ShinkaiDBError::VectorFSError(e) => write!(f, "VectorFS error: {}", e),
            ShinkaiDBError::InvalidAttributeName(e) => write!(f, "Invalid attribute name: {}", e),
            ShinkaiDBError::BoolParseError(e) => write!(f, "Bool parse error: {}", e),
            ShinkaiDBError::ToolNotFound(e) => write!(f, "Tool not found: {}", e),
            ShinkaiDBError::DeserializationFailed(e) => write!(f, "Deserialization failed: {}", e),
        }
    }
}

impl std::error::Error for ShinkaiDBError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShinkaiDBError::RocksDBError(e) => Some(e),
            ShinkaiDBError::JsonSerializationError(e) => Some(e),
            ShinkaiDBError::IOError(e) => Some(e),
            ShinkaiDBError::VRError(e) => Some(e),
            ShinkaiDBError::BincodeError(e) => Some(e),
            ShinkaiDBError::InboxNameError(e) => Some(e),
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
            (ShinkaiDBError::InvalidIdentityType(msg1), ShinkaiDBError::InvalidIdentityType(msg2)) => msg1 == msg2,
            (ShinkaiDBError::PermissionNotFound(msg1), ShinkaiDBError::PermissionNotFound(msg2)) => msg1 == msg2,
            (ShinkaiDBError::SomeError(msg1), ShinkaiDBError::SomeError(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InboxNotFound(msg1), ShinkaiDBError::InboxNotFound(msg2)) => msg1 == msg2,
            (ShinkaiDBError::IdentityNotFound(msg1), ShinkaiDBError::IdentityNotFound(msg2)) => msg1 == msg2,
            (ShinkaiDBError::ProfileNameNonExistent(msg1), ShinkaiDBError::ProfileNameNonExistent(msg2)) => {
                msg1 == msg2
            }
            (ShinkaiDBError::MissingValue(msg1), ShinkaiDBError::MissingValue(msg2)) => msg1 == msg2,
            (ShinkaiDBError::ColumnFamilyNotFound(msg1), ShinkaiDBError::ColumnFamilyNotFound(msg2)) => msg1 == msg2,
            (ShinkaiDBError::DataConversionError(msg1), ShinkaiDBError::DataConversionError(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InvalidInboxPermission(msg1), ShinkaiDBError::InvalidInboxPermission(msg2)) => {
                msg1 == msg2
            }
            (ShinkaiDBError::InvalidPermissionType(msg1), ShinkaiDBError::InvalidPermissionType(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InvalidProfileName(msg1), ShinkaiDBError::InvalidProfileName(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InvalidIdentityName(msg1), ShinkaiDBError::InvalidIdentityName(msg2)) => msg1 == msg2,
            (ShinkaiDBError::IOError(e1), ShinkaiDBError::IOError(e2)) => e1.to_string() == e2.to_string(),
            (ShinkaiDBError::RocksDBError(e1), ShinkaiDBError::RocksDBError(e2)) => e1.to_string() == e2.to_string(),
            (ShinkaiDBError::Utf8ConversionError, ShinkaiDBError::Utf8ConversionError) => true,
            (ShinkaiDBError::JsonSerializationError(e1), ShinkaiDBError::JsonSerializationError(e2)) => {
                e1.to_string() == e2.to_string()
            }
            (ShinkaiDBError::DataNotFound, ShinkaiDBError::DataNotFound) => true,
            (ShinkaiDBError::FailedFetchingCF, ShinkaiDBError::FailedFetchingCF) => true,
            (ShinkaiDBError::FailedFetchingValue, ShinkaiDBError::FailedFetchingValue) => true,
            (ShinkaiDBError::VRError(e1), ShinkaiDBError::VRError(e2)) => e1 == e2, // assuming VRError implements PartialEq
            (ShinkaiDBError::BincodeError(e1), ShinkaiDBError::BincodeError(e2)) => e1.to_string() == e2.to_string(),
            (ShinkaiDBError::InboxNameError(e1), ShinkaiDBError::InboxNameError(e2)) => e1 == e2, // assuming InboxNameError implements PartialEq
            (ShinkaiDBError::ProfileNotFound(msg1), ShinkaiDBError::ProfileNotFound(msg2)) => msg1 == msg2,
            (ShinkaiDBError::InvalidPermissionsType, ShinkaiDBError::InvalidPermissionsType) => true,
            (ShinkaiDBError::DeviceIdentityAlreadyExists(msg1), ShinkaiDBError::DeviceIdentityAlreadyExists(msg2)) => {
                msg1 == msg2
            }
            (ShinkaiDBError::DeviceNameNonExistent(msg1), ShinkaiDBError::DeviceNameNonExistent(msg2)) => msg1 == msg2,
            _ => false,
        }
    }
}

impl From<ToolError> for ShinkaiDBError {
    fn from(err: ToolError) -> ShinkaiDBError {
        ShinkaiDBError::ToolError(err)
    }
}

impl From<chrono::ParseError> for ShinkaiDBError {
    fn from(error: chrono::ParseError) -> Self {
        ShinkaiDBError::SomeError(error.to_string())
    }
}

impl From<VRError> for ShinkaiDBError {
    fn from(err: VRError) -> ShinkaiDBError {
        ShinkaiDBError::VRError(err)
    }
}

impl From<rocksdb::Error> for ShinkaiDBError {
    fn from(error: rocksdb::Error) -> Self {
        ShinkaiDBError::RocksDBError(error)
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

impl From<ShinkaiNameError> for ShinkaiDBError {
    fn from(error: ShinkaiNameError) -> Self {
        ShinkaiDBError::ShinkaiNameError(error)
    }
}

impl From<ShinkaiMessageError> for ShinkaiDBError {
    fn from(err: ShinkaiMessageError) -> ShinkaiDBError {
        // Convert the ShinkaiMessageError into a ShinkaiDBError
        // You might want to add a new variant to ShinkaiDBError for this
        ShinkaiDBError::ShinkaiMessageError(err.to_string())
    }
}

impl From<VectorFSError> for ShinkaiDBError {
    fn from(err: VectorFSError) -> ShinkaiDBError {
        ShinkaiDBError::VectorFSError(err.to_string())
    }
}

impl From<std::string::FromUtf8Error> for ShinkaiDBError {
    fn from(_err: std::string::FromUtf8Error) -> ShinkaiDBError {
        ShinkaiDBError::Utf8ConversionError
    }
}

impl From<ParseBoolError> for ShinkaiDBError {
    fn from(err: ParseBoolError) -> ShinkaiDBError {
        ShinkaiDBError::BoolParseError(err.to_string())
    }
}