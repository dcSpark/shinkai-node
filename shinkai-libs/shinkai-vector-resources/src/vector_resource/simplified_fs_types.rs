use super::MapVectorResource;
use crate::resource_errors::VRError;
use crate::source::DistributionOrigin;
pub use crate::source::{DistributionInfo, VRSource};
pub use crate::vector_resource::vector_resource_types::*;
pub use crate::vector_resource::vector_search_traversal::*;
use chrono::{DateTime, Utc};

/// Enum that holds the simplified representation of file system entries used in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SimplifiedFSEntry {
    Folder(SimplifiedFSFolder),
    Item(SimplifiedFSItem),
    Root(SimplifiedFSRoot),
}

impl SimplifiedFSEntry {
    // Attempts to parse the SimplifiedFSEntry into an SimplifiedFSFolder
    pub fn as_folder(self) -> Result<SimplifiedFSFolder, VRError> {
        match self {
            SimplifiedFSEntry::Folder(folder) => Ok(folder),
            SimplifiedFSEntry::Item(i) => Err(VRError::InvalidSimplifiedFSEntryType(i.path.to_string())),
            SimplifiedFSEntry::Root(root) => Err(VRError::InvalidSimplifiedFSEntryType(root.path.to_string())),
        }
    }

    // Attempts to parse the SimplifiedFSEntry into an SimplifiedFSItem
    pub fn as_item(self) -> Result<SimplifiedFSItem, VRError> {
        match self {
            SimplifiedFSEntry::Item(item) => Ok(item),
            SimplifiedFSEntry::Folder(f) => Err(VRError::InvalidSimplifiedFSEntryType(f.path.to_string())),
            SimplifiedFSEntry::Root(root) => Err(VRError::InvalidSimplifiedFSEntryType(root.path.to_string())),
        }
    }

    // Attempts to parse the SimplifiedFSEntry into an SimplifiedFSItem
    pub fn as_root(self) -> Result<SimplifiedFSRoot, VRError> {
        match self {
            SimplifiedFSEntry::Root(root) => Ok(root),
            SimplifiedFSEntry::Item(item) => Err(VRError::InvalidSimplifiedFSEntryType(item.path.to_string())),
            SimplifiedFSEntry::Folder(f) => Err(VRError::InvalidSimplifiedFSEntryType(f.path.to_string())),
        }
    }

    /// Converts the SimplifiedFSEntry to a JSON string
    pub fn to_json(&self) -> Result<String, VRError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Creates a SimplifiedFSEntry from a JSON string
    pub fn from_json(s: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(s)?)
    }
}

/// Struct that holds the simplified representation of the file system root in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimplifiedFSRoot {
    pub path: VRPath,
    pub child_folders: Vec<SimplifiedFSFolder>,
    // Datetime when the profile's VectorSimplifiedFS was created
    pub created_datetime: DateTime<Utc>,
    /// Datetime which is updated whenever any writes take place. In other words, when
    /// a SimplifiedFSItem or SimplifiedFSFolder is updated/moved/renamed/deleted/etc., last written timestamp is updated.
    pub last_written_datetime: DateTime<Utc>,
    /// Merkle root of the profile's SimplifiedFS
    pub merkle_root: String,
}

impl SimplifiedFSRoot {
    /// Converts to a JSON string
    pub fn to_json(&self) -> Result<String, VRError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes from a JSON string
    pub fn from_json(s: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(s)?)
    }
}

/// Struct that holds the simplified representation of file system folders in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimplifiedFSFolder {
    /// Name of the SimplifiedFSFolder
    pub name: String,
    /// Path where the SimplifiedFSItem is held in the VectorSimplifiedFS
    pub path: VRPath,
    /// SimplifiedFSFolders which are held within this SimplifiedFSFolder
    pub child_folders: Vec<SimplifiedFSFolder>,
    /// SimplifiedFSItems which are held within this SimplifiedFSFolder
    pub child_items: Vec<SimplifiedFSItem>,
    /// Datetime the SimplifiedFSFolder was first created
    pub created_datetime: DateTime<Utc>,
    /// Datetime the SimplifiedFSFolder was last read by any ShinkaiName
    pub last_read_datetime: DateTime<Utc>,
    /// Datetime the SimplifiedFSFolder was last modified, meaning contents of the directory were changed.
    /// Ie. An SimplifiedFSEntry is moved/renamed/deleted/new one added.
    pub last_modified_datetime: DateTime<Utc>,
    /// Datetime the SimplifiedFSFolder was last written to, meaning any write took place under the folder. In other words, even when
    /// a VR is updated or moved/renamed, then last written is always updated.
    pub last_written_datetime: DateTime<Utc>,
    /// Merkle hash comprised of all of the SimplifiedFSEntries within this folder
    pub merkle_hash: String,
}

impl SimplifiedFSFolder {
    /// Converts to a JSON string
    pub fn to_json(&self) -> Result<String, VRError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes from a JSON string
    pub fn from_json(s: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(s)?)
    }
}

/// Struct that holds the simplified representation of file system items in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SimplifiedFSItem {
    /// Name of the SimplifiedFSItem (based on Vector Resource name)
    pub name: String,
    /// Path where the SimplifiedFSItem is held in the VectorSimplifiedFS
    pub path: VRPath,
    /// The VRHeader matching the Vector Resource stored at this SimplifiedFSItem's path
    pub vr_header: VRHeader,
    /// Datetime the Vector Resource in the SimplifiedFSItem was first created
    pub created_datetime: DateTime<Utc>,
    /// Datetime the Vector Resource in the SimplifiedFSItem was last written to, meaning any updates to its contents.
    pub last_written_datetime: DateTime<Utc>,
    /// Datetime the SimplifiedFSItem was last read by any ShinkaiName
    pub last_read_datetime: DateTime<Utc>,
    /// Datetime the Vector Resource in the SimplifiedFSItem was last saved/updated.
    /// For example when saving a VR into the SimplifiedFS that someone else generated on their node, last_written and last_saved will be different.
    pub vr_last_saved_datetime: DateTime<Utc>,
    /// Datetime the SourceFileMap in the SimplifiedFSItem was last saved/updated. None if no SourceFileMap was ever saved.
    pub source_file_map_last_saved_datetime: Option<DateTime<Utc>>,
    /// The original release location/date time where the VectorResource/SourceFileMap in this FSItem were made available from.
    pub distribution_info: DistributionInfo,
    /// The size of the Vector Resource in this SimplifiedFSItem
    pub vr_size: usize,
    /// The size of the SourceFileMap in this SimplifiedFSItem. Will be 0 if no SourceFiles are saved.
    pub source_file_map_size: usize,
    /// Merkle hash, which is in fact the merkle root of the Vector Resource stored in the SimplifiedFSItem
    pub merkle_hash: String,
}

impl SimplifiedFSItem {
    /// Returns the name of the SimplifiedFSItem (based on the name in VRHeader)
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Checks the last saved datetime to determine if it was ever saved into the SimplifiedFSDB
    pub fn is_source_file_map_saved(&self) -> bool {
        self.source_file_map_last_saved_datetime.is_some()
    }

    /// Converts to a JSON string
    pub fn to_json(&self) -> Result<String, VRError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes from a JSON string
    pub fn from_json(s: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(s)?)
    }
}
