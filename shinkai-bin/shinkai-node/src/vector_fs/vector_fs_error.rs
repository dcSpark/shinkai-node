use crate::{tools::error::ToolError};
use core::fmt;
use shinkai_message_primitives::{
    schemas::{
        shinkai_name::{ShinkaiName, ShinkaiNameError},
    },
    shinkai_message::shinkai_message_error::ShinkaiMessageError,
};
use shinkai_vector_resources::{model_type::EmbeddingModelType, resource_errors::VRError, vector_resource::VRPath};
use std::{io, str::Utf8Error};

#[derive(Debug)]
pub enum VectorFSError {
    ShinkaiNameError(ShinkaiNameError),
    RocksDBError(rocksdb::Error),
    IOError(io::Error),
    InvalidIdentityType(String),
    Utf8ConversionError,
    SomeError(String),
    ProfileNameNonExistent(String),
    InvalidData,
    JsonSerializationError(serde_json::Error),
    DataConversionError(String),
    DataNotFound,
    VRError(VRError),
    FailedFetchingCF,
    FailedFetchingValue,
    ShinkaiMessageError(String),
    BincodeError(bincode::Error),
    MissingValue(String),
    ColumnFamilyNotFound(String),
    ShinkaiNameLacksProfile,
    ToolError(ToolError),
    InvalidNodeActionPermission(ShinkaiName, String),
    InvalidProfileActionPermission(ShinkaiName, String),
    InvalidReaderPermission(ShinkaiName, ShinkaiName, VRPath),
    InvalidWriterPermission(ShinkaiName, ShinkaiName, VRPath),
    InvalidReadPermission(ShinkaiName, VRPath),
    InvalidWritePermission(ShinkaiName, VRPath),
    NoSourceFileAvailable(String),
    InvalidFSEntryType(String),
    EmbeddingModelTypeMismatch(EmbeddingModelType, EmbeddingModelType),
    EmbeddingMissingInResource(String),
    InvalidMetadata(String),
    FailedCreatingProfileBoundWriteBatch(String),
    CannotOverwriteFolder(VRPath),
    CannotOverwriteFSEntry(VRPath),
    PathDoesNotPointAtItem(VRPath),
    PathDoesNotPointAtFolder(VRPath),
    NoEntryAtPath(VRPath),
    NoPermissionEntryAtPath(VRPath),
    EntryAlreadyExistsAtPath(VRPath),
    DateTimeParseError(String),
    FailedGettingFSPathOfRetrievedNode(String),
    CannotMoveFolderIntoItself(VRPath),
    LockAcquisitionFailed
}

impl fmt::Display for VectorFSError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VectorFSError::RocksDBError(e) => write!(f, "RocksDB error: {}", e),
            VectorFSError::SomeError(e) => write!(f, "Some error: {}", e),
            VectorFSError::ShinkaiNameLacksProfile => write!(
                f,
                "Provided ShinkaiName does not specify a profile which is required for DB action.",
            ),

            VectorFSError::ProfileNameNonExistent(e) => {
                write!(f, "Profile name does not exist: {}", e)
            }
            VectorFSError::IOError(e) => write!(f, "IO Error: {}", e),
            VectorFSError::InvalidData => write!(f, "Invalid data"),
            VectorFSError::ShinkaiMessageError(e) => write!(f, "ShinkaiMessage error: {}", e),
            VectorFSError::InvalidIdentityType(e) => write!(f, "Invalid identity type: {}", e),
            VectorFSError::ShinkaiNameError(e) => write!(f, "Shinkai name error: {}", e),
            VectorFSError::MissingValue(e) => write!(f, "Missing value: {}", e),
            VectorFSError::ColumnFamilyNotFound(e) => write!(f, "Column family not found: {}", e),
            VectorFSError::DataConversionError(e) => write!(f, "Data conversion error: {}", e),
            VectorFSError::Utf8ConversionError => write!(f, "UTF8 conversion error"),
            VectorFSError::JsonSerializationError(e) => write!(f, "Json Serialization Error: {}", e),
            VectorFSError::DataNotFound => write!(f, "Data not found"),
            VectorFSError::FailedFetchingCF => write!(f, "Failed fetching Column Family"),
            VectorFSError::FailedFetchingValue => write!(f, "Failed fetching value. Likely invalid CF or key."),
            VectorFSError::VRError(e) => write!(f, "{}", e),
            VectorFSError::BincodeError(e) => write!(f, "Bincode error: {}", e),
            VectorFSError::ToolError(e) => write!(f, "Tool error: {}", e),
            VectorFSError::InvalidNodeActionPermission(name, error_message) => write!(
                f,
                "{} has no permission to perform a VectorFS Node action: {}",
                name, error_message
            ),
            VectorFSError::InvalidProfileActionPermission(name, error_message) => write!(
                f,
                "{} has no permission to perform a VectorFS Profile action: {}",
                name, error_message
            ),
            VectorFSError::InvalidReaderPermission(name, profile, path) => write!(
                f,
                "{} has no permission to read {}'s VectorFS at path: {}",
                name,
                profile,
                path.format_to_string()
            ),
            VectorFSError::InvalidWriterPermission(name, profile, path) => write!(
                f,
                "{} has no permission to write in {}'s VectorFS at path: {}",
                name,
                profile,
                path.format_to_string()
            ),
            VectorFSError::NoSourceFileAvailable(s) => write!(f, "No SourceFile available for: {}", s),
            VectorFSError::InvalidFSEntryType(s) => {
                write!(f, "Parsing FSEntry into specific type failed at path: {}", s)
            }
            VectorFSError::EmbeddingModelTypeMismatch(a, b) => {
                write!(f, "Embedding model mismatch: {} vs. {}", a, b)
            }
            VectorFSError::EmbeddingMissingInResource(s) => {
                write!(f, "Embedding is not defined in resource: {} ", s)
            }
            VectorFSError::InvalidMetadata(e) => write!(f, "Invalid metadata at key: {}", e),
            VectorFSError::FailedCreatingProfileBoundWriteBatch(e) => {
                write!(f, "Failed parsing profile and creating a write batch for: {}", e)
            }
            VectorFSError::CannotOverwriteFolder(e) => write!(f, "Cannot write over existing folder at: {}", e),
            VectorFSError::CannotOverwriteFSEntry(e) => write!(f, "Cannot write over existing filesystem entry at: {}", e),
            VectorFSError::PathDoesNotPointAtFolder(e) => {
                write!(f, "Entry at supplied path does not hold a Filesystem Folder: {}", e)
            }
            VectorFSError::PathDoesNotPointAtItem(e) => {
                write!(f, "Entry at supplied path does not hold a Filesystem Item: {}", e)
            }
            VectorFSError::NoEntryAtPath(e) => {
                write!(
                    f,
                    "Supplied path does not exist in the VectorFS: {}",
                    e
                )
            }
            VectorFSError::NoPermissionEntryAtPath(e) => {
                write!(
                    f,
                    "Path does not have a path permission specified in the VectorFS: {}",
                    e
                )
            }
            VectorFSError::EntryAlreadyExistsAtPath(p) => {
                write!(f, "FSEntry already exists at path, and cannot overwrite: {}", p)
            }

            VectorFSError::DateTimeParseError(e) => write!(f, "Datetime Parse Error: {}", e),
            VectorFSError::InvalidReadPermission(n, p) => {
                write!(f, "{} does not have read permissions for path: {}", n, p)
            }
            VectorFSError::InvalidWritePermission(n, p) => {
                write!(f, "{} does not have write permissions for path: {}", n, p)
            }
            VectorFSError::FailedGettingFSPathOfRetrievedNode(s) => write!(f, "While performing 2-tier 'deep' vector search, unable to get VectorFS path of the VR the retrieved node was from: {}", s),
            VectorFSError::CannotMoveFolderIntoItself(e) => write!(f, "Cannot move folder into itself at a deeper level: {}", e),
            VectorFSError::LockAcquisitionFailed => write!(f, "Failed to acquire lock"),
        }
    }
}

impl std::error::Error for VectorFSError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VectorFSError::RocksDBError(e) => Some(e),
            VectorFSError::JsonSerializationError(e) => Some(e),
            VectorFSError::IOError(e) => Some(e),
            VectorFSError::VRError(e) => Some(e),
            VectorFSError::BincodeError(e) => Some(e),
            _ => None,
        }
    }
}

impl PartialEq for VectorFSError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (VectorFSError::InvalidIdentityType(msg1), VectorFSError::InvalidIdentityType(msg2)) => msg1 == msg2,
            (VectorFSError::SomeError(msg1), VectorFSError::SomeError(msg2)) => msg1 == msg2,
            (VectorFSError::ProfileNameNonExistent(msg1), VectorFSError::ProfileNameNonExistent(msg2)) => msg1 == msg2,
            (VectorFSError::MissingValue(msg1), VectorFSError::MissingValue(msg2)) => msg1 == msg2,
            (VectorFSError::ColumnFamilyNotFound(msg1), VectorFSError::ColumnFamilyNotFound(msg2)) => msg1 == msg2,
            (VectorFSError::DataConversionError(msg1), VectorFSError::DataConversionError(msg2)) => msg1 == msg2,
            (VectorFSError::IOError(e1), VectorFSError::IOError(e2)) => e1.to_string() == e2.to_string(),
            (VectorFSError::RocksDBError(e1), VectorFSError::RocksDBError(e2)) => e1.to_string() == e2.to_string(),
            (VectorFSError::Utf8ConversionError, VectorFSError::Utf8ConversionError) => true,
            (VectorFSError::JsonSerializationError(e1), VectorFSError::JsonSerializationError(e2)) => {
                e1.to_string() == e2.to_string()
            }
            (VectorFSError::DataNotFound, VectorFSError::DataNotFound) => true,
            (VectorFSError::FailedFetchingCF, VectorFSError::FailedFetchingCF) => true,
            (VectorFSError::FailedFetchingValue, VectorFSError::FailedFetchingValue) => true,
            (VectorFSError::VRError(e1), VectorFSError::VRError(e2)) => e1 == e2, // assuming VRError implements PartialEq
            (VectorFSError::BincodeError(e1), VectorFSError::BincodeError(e2)) => e1.to_string() == e2.to_string(),
            _ => false,
        }
    }
}

impl From<ToolError> for VectorFSError {
    fn from(err: ToolError) -> VectorFSError {
        VectorFSError::ToolError(err)
    }
}

impl From<VRError> for VectorFSError {
    fn from(err: VRError) -> VectorFSError {
        VectorFSError::VRError(err)
    }
}

impl From<rocksdb::Error> for VectorFSError {
    fn from(error: rocksdb::Error) -> Self {
        VectorFSError::RocksDBError(error)
    }
}

impl From<io::Error> for VectorFSError {
    fn from(error: io::Error) -> Self {
        VectorFSError::IOError(error)
    }
}

impl From<serde_json::Error> for VectorFSError {
    fn from(error: serde_json::Error) -> Self {
        VectorFSError::JsonSerializationError(error)
    }
}

impl From<Utf8Error> for VectorFSError {
    fn from(_: Utf8Error) -> Self {
        VectorFSError::Utf8ConversionError
    }
}

impl From<bincode::Error> for VectorFSError {
    fn from(error: bincode::Error) -> Self {
        VectorFSError::BincodeError(error)
    }
}

impl From<ShinkaiNameError> for VectorFSError {
    fn from(error: ShinkaiNameError) -> Self {
        VectorFSError::ShinkaiNameError(error)
    }
}

impl From<ShinkaiMessageError> for VectorFSError {
    fn from(err: ShinkaiMessageError) -> VectorFSError {
        // Convert the ShinkaiMessageError into a VectorFSError
        // You might want to add a new variant to VectorFSError for this
        VectorFSError::ShinkaiMessageError(err.to_string())
    }
}

impl From<chrono::ParseError> for VectorFSError {
    fn from(err: chrono::ParseError) -> VectorFSError {
        VectorFSError::DateTimeParseError(err.to_string())
    }
}
