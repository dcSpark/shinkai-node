use super::vector_fs_error::VectorFSError;
use chrono::{DateTime, Utc};
use shinkai_vector_resources::{
    resource_errors::VRError,
    vector_resource::{BaseVectorResource, Node, NodeContent, VRHeader, VRPath},
};

/// Enum that holds the types of external-facing entries used in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FSEntry {
    Folder(FSFolder),
    Item(FSItem),
}

impl FSEntry {
    // Attempts to parse the FSEntry into an FSFolder
    pub fn as_folder(self) -> Result<FSFolder, VectorFSError> {
        match self {
            FSEntry::Folder(folder) => Ok(folder),
            FSEntry::Item(i) => Err(VectorFSError::InvalidFSEntryType(i.path.to_string())),
        }
    }

    // Attempts to parse the FSEntry into an FSItem
    pub fn as_item(self) -> Result<FSItem, VectorFSError> {
        match self {
            FSEntry::Item(item) => Ok(item),
            FSEntry::Folder(f) => Err(VectorFSError::InvalidFSEntryType(f.path.to_string())),
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
    // pub created_datetime: String
    // pub last_read_datetime: String

    // Last modified only keeps track of when the contents of the directory change.
    // Ie. An FSEntry is moved/deleted/new one added
    // pub last_modified_datetime: String

    // Last saved is updated any time any writes take place under the folder. In other words, even when
    // a VR is updated, last saved timestamp is updated.
    // pub last_saved: String
}

impl FSFolder {
    pub fn new(path: VRPath, child_folders: Vec<FSFolder>, child_items: Vec<FSItem>) -> Self {
        Self {
            path,
            child_folders,
            child_items,
        }
    }

    /// Generates a new FSFolder using a BaseVectorResource holding Node + the path where the node was retrieved
    /// from in the VecFS internals. Use VRPath::new() if the path is root.
    pub fn from_vector_resource_node(node: Node, node_fs_path: VRPath) -> Result<Self, VectorFSError> {
        match node.content {
            NodeContent::Resource(base_vector_resource) => {
                Self::from_vector_resource(base_vector_resource, node_fs_path)
            }
            _ => Err(VRError::InvalidNodeType(node.id))?,
        }
    }

    /// Generates a new FSFolder from a BaseVectorResource + the path where it was retrieved
    /// from inside of the VectorFS. Use VRPath::new() if the path is root.
    pub fn from_vector_resource(resource: BaseVectorResource, resource_fs_path: VRPath) -> Result<Self, VectorFSError> {
        let mut child_folders = Vec::new();
        let mut child_items = Vec::new();

        for node in resource.as_trait_object().get_nodes() {
            match node.content {
                // If it's a Resource, then create a FSFolder by recursing, and push it to child_folders
                NodeContent::Resource(inner_resource) => {
                    let new_path = resource_fs_path.push_cloned(inner_resource.as_trait_object().name().to_string());
                    child_folders.push(Self::from_vector_resource(inner_resource, new_path)?);
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

        Ok(Self::new(VRPath::new(), child_folders, child_items))
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
    // From header
    // pub created_datetime: DateTime<Utc>
    // From header, last time the VR contents were modified
    // pub last_modified_datetime: DateTime<Utc>
    // pub last_read_datetime: DateTime<Utc>
    // Last saved is the time when the VR was last mutated/saved in the VectorFS
    // pub last_saved: DateTime<Utc>
}

impl FSItem {
    pub fn new(path: VRPath, vr_header: VRHeader, source_file_saved: bool) -> Self {
        Self {
            path,
            vr_header,
            source_file_saved,
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
