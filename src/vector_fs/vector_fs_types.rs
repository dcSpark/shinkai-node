use super::vector_fs_error::VectorFSError;
use chrono::{DateTime, Utc};
use shinkai_vector_resources::{
    resource_errors::VRError,
    shinkai_time::ShinkaiTime,
    vector_resource::{BaseVectorResource, Node, NodeContent, VRHeader, VRPath},
};

/// Enum that holds the types of external-facing entries used in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FSEntry {
    Folder(FSFolder),
    Item(FSItem),
    Root(FSRoot),
}

impl FSEntry {
    // Attempts to parse the FSEntry into an FSFolder
    pub fn as_folder(self) -> Result<FSFolder, VectorFSError> {
        match self {
            FSEntry::Folder(folder) => Ok(folder),
            FSEntry::Item(i) => Err(VectorFSError::InvalidFSEntryType(i.path.to_string())),
            FSEntry::Root(root) => Err(VectorFSError::InvalidFSEntryType(root.path.to_string())),
        }
    }

    // Attempts to parse the FSEntry into an FSItem
    pub fn as_item(self) -> Result<FSItem, VectorFSError> {
        match self {
            FSEntry::Item(item) => Ok(item),
            FSEntry::Folder(f) => Err(VectorFSError::InvalidFSEntryType(f.path.to_string())),
            FSEntry::Root(root) => Err(VectorFSError::InvalidFSEntryType(root.path.to_string())),
        }
    }

    // Attempts to parse the FSEntry into an FSItem
    pub fn as_fs_root(self) -> Result<FSRoot, VectorFSError> {
        match self {
            FSEntry::Root(root) => Ok(root),
            FSEntry::Item(item) => Err(VectorFSError::InvalidFSEntryType(item.path.to_string())),
            FSEntry::Folder(f) => Err(VectorFSError::InvalidFSEntryType(f.path.to_string())),
        }
    }
}

/// An external facing abstraction representing the VecFS root for a given profile.
/// Actual data represented by a FSRoot is the profile's Core MapVectorResource.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSRoot {
    pub path: VRPath,
    pub child_folders: Vec<FSFolder>,
    pub child_items: Vec<FSItem>,
    pub created_datetime: DateTime<Utc>,
    /// Last written is updated any time any writes take place under the folder. In other words, even when
    /// a VR is updated but not moved/renamed, last written timestamp is updated.
    pub last_written_datetime: DateTime<Utc>,
}

impl From<FSFolder> for FSRoot {
    fn from(folder: FSFolder) -> Self {
        Self {
            path: folder.path,
            child_folders: folder.child_folders,
            child_items: folder.child_items,
            created_datetime: folder.created_datetime,
            last_written_datetime: folder.last_written_datetime,
        }
    }
}

/// An external facing folder abstraction used to make interacting with the VectorFS easier.
/// Actual data represented by a FSFolder is a VectorResource-holding Node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSFolder {
    pub path: VRPath,
    pub child_folders: Vec<FSFolder>,
    pub child_items: Vec<FSItem>,
    pub created_datetime: DateTime<Utc>,
    pub last_read_datetime: DateTime<Utc>,
    /// Last modified only keeps track of when the contents of the directory change.
    /// Ie. An FSEntry is moved/deleted/new one added
    pub last_modified_datetime: DateTime<Utc>,
    /// Last written is updated any time any writes take place under the folder. In other words, even when
    /// a VR is updated but not moved/renamed, last written timestamp is updated.
    pub last_written_datetime: DateTime<Utc>,
}

impl FSFolder {
    /// Initializes a new FSFolder struct
    pub fn new(
        path: VRPath,
        child_folders: Vec<FSFolder>,
        child_items: Vec<FSItem>,
        created_datetime: DateTime<Utc>,
        last_written_datetime: DateTime<Utc>,
        last_read_datetime: DateTime<Utc>,
        last_modified_datetime: DateTime<Utc>,
    ) -> Self {
        Self {
            path,
            child_folders,
            child_items,
            created_datetime,
            last_read_datetime,
            last_modified_datetime,
            last_written_datetime,
        }
    }

    /// Initializes a new FSFolder struct with all datetimes set to the current moment.
    pub fn new_current_time(path: VRPath, child_folders: Vec<FSFolder>, child_items: Vec<FSItem>) -> Self {
        let now = ShinkaiTime::generate_time_now();
        Self::new(
            path,
            child_folders,
            child_items,
            now.clone(),
            now.clone(),
            now.clone(),
            now.clone(),
        )
    }

    /// Generates a new FSFolder using a BaseVectorResource holding Node + the path where the node was retrieved
    /// from in the VecFS internals. Use VRPath::new() if the path is root.
    pub fn from_vector_resource_node(node: Node, node_fs_path: VRPath) -> Result<Self, VectorFSError> {
        match node.content {
            NodeContent::Resource(base_vector_resource) => {
                // Process datetimes from node
                let (last_read_datetime, last_modified_datetime) = Self::process_datetimes_from_node(&node)?;

                // Call from_vector_resource with the parsed datetimes
                Self::from_vector_resource(
                    base_vector_resource,
                    node_fs_path,
                    last_read_datetime,
                    last_modified_datetime,
                )
            }
            _ => Err(VRError::InvalidNodeType(node.id))?,
        }
    }

    /// Generates a new FSFolder from a BaseVectorResource + the path where it was retrieved
    /// from inside of the VectorFS. Use VRPath::new() if the path is root.
    pub fn from_vector_resource(
        resource: BaseVectorResource,
        resource_fs_path: VRPath,
        last_read_datetime: DateTime<Utc>,
        last_modified_datetime: DateTime<Utc>,
    ) -> Result<Self, VectorFSError> {
        let mut child_folders = Vec::new();
        let mut child_items = Vec::new();

        for node in resource.as_trait_object().get_nodes() {
            match node.content {
                // If it's a Resource, then create a FSFolder by recursing, and push it to child_folders
                NodeContent::Resource(inner_resource) => {
                    // Process datetimes from node
                    let (lr_datetime, lm_datetime) = Self::process_datetimes_from_node(&node)?;
                    let new_path = resource_fs_path.push_cloned(inner_resource.as_trait_object().name().to_string());
                    child_folders.push(Self::from_vector_resource(
                        inner_resource,
                        new_path,
                        lr_datetime,
                        lm_datetime,
                    )?);
                }
                // If it's a VRHeader, then create a FSEntry and push it to child_items
                NodeContent::VRHeader(_) => {
                    let new_path = resource_fs_path.push_cloned(node.id.clone());
                    let fs_item = FSItem::from_vr_header_node(node, new_path)?;
                    child_items.push(fs_item);
                }
                _ => {}
            }
        }

        let created_datetime = resource.as_trait_object().created_datetime();
        let last_written_datetime = resource.as_trait_object().last_written_datetime();
        Ok(Self::new(
            VRPath::new(),
            child_folders,
            child_items,
            created_datetime,
            last_written_datetime,
            last_read_datetime,
            last_modified_datetime,
        ))
    }

    /// Process last_read/last_modified datetimes in a Node from the VectorFS core resource.
    /// The node must be an FSFolder for this to succeed.
    pub fn process_datetimes_from_node(node: &Node) -> Result<(DateTime<Utc>, DateTime<Utc>), VectorFSError> {
        // Read last_read_datetime and last_modified_datetime from metadata
        let last_read_str = node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get(&Self::last_read_key()))
            .ok_or(VectorFSError::InvalidMetadata(Self::last_read_key()))?;

        let last_modified_str = node
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.get(&Self::last_modified_key()))
            .ok_or(VectorFSError::InvalidMetadata(Self::last_modified_key()))?;

        // Parse the datetime strings
        let last_read_datetime = ShinkaiTime::from_rfc3339_string(last_read_str)
            .map_err(|_| VectorFSError::InvalidMetadata(Self::last_read_key()))?;

        let last_modified_datetime = ShinkaiTime::from_rfc3339_string(last_modified_str)
            .map_err(|_| VectorFSError::InvalidMetadata(Self::last_modified_key()))?;

        Ok((last_read_datetime, last_modified_datetime))
    }

    /// Returns the metadata key for the last read datetime.
    pub fn last_read_key() -> String {
        String::from("last_read")
    }

    /// Returns the metadata key for the last modified datetime.
    pub fn last_modified_key() -> String {
        String::from("last_modified")
    }
}

/// An external facing "file" abstraction used to make interacting with the VectorFS easier.
/// Each FSItem always represents a single stored VectorResource, which sometimes also has an optional SourceFile.
/// Actual data represented by a FSItem is a VRHeader-holding Node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSItem {
    pub path: VRPath,
    pub vr_header: VRHeader,
    pub source_file_saved: bool,
    /// From header
    pub created_datetime: DateTime<Utc>,
    /// From header, last time the VR contents were written to
    pub last_written_datetime: DateTime<Utc>,
    /// Last time the FSItem was read
    pub last_read_datetime: DateTime<Utc>,
    /// Last saved is the time when the VR was last saved in the VectorFS
    pub last_saved_datetime: DateTime<Utc>,
}

impl FSItem {
    /// Initialize a new FSItem struct
    pub fn new(
        path: VRPath,
        vr_header: VRHeader,
        source_file_saved: bool,
        created_datetime: DateTime<Utc>,
        last_written_datetime: DateTime<Utc>,
        last_read_datetime: DateTime<Utc>,
        last_saved_datetime: DateTime<Utc>,
    ) -> Self {
        Self {
            path,
            vr_header,
            source_file_saved,
            created_datetime,
            last_written_datetime,
            last_read_datetime,
            last_saved_datetime,
        }
    }

    /// Metadata key where source_file_saved will be found in a Node.
    pub fn source_file_saved_metadata_key() -> String {
        String::from("sf_saved")
    }

    /// DB key where the Vector Resource matching this FSEntry is held.
    /// Uses the VRHeader reference string.
    pub fn resource_db_key(&self) -> String {
        self.vr_header.reference_string()
    }

    /// Returns the DB key where the SourceFile matching this FSEntry is held.
    /// If the FSEntry is marked as having no source file saved, then returns an VectorFSError.
    pub fn source_file_db_key(&self) -> Result<String, VectorFSError> {
        if self.source_file_saved {
            Ok(self.resource_db_key())
        } else {
            Err(VectorFSError::NoSourceFileAvailable(self.vr_header.reference_string()))
        }
    }

    /// Generates a new FSItem using a VRHeader holding Node + the path where the node was retrieved
    /// from in the VecFS internals. Use VRPath::new() if the path is root.
    pub fn from_vr_header_node(node: Node, node_fs_path: VRPath) -> Result<Self, VectorFSError> {
        match node.content {
            NodeContent::VRHeader(header) => {
                // Read source_file_saved from metadata
                let source_file_saved = node
                    .metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get(&FSItem::source_file_saved_metadata_key()))
                    .map_or(false, |value| value == "true");
                Ok(FSItem::new(node_fs_path, header, source_file_saved))
            }

            _ => Err(VRError::InvalidNodeType(node.id))?,
        }
    }
}
