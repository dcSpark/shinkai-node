use super::vector_fs_error::VectorFSError;
use shinkai_vector_resources::{
    resource_errors::VRError,
    vector_resource::{BaseVectorResource, Node, NodeContent, VRHeader, VRPath},
};

/// An external facing folder abstraction used to make interacting with the VectorFS easier.
/// Actual data represented by a FSFolder is a VectorResource-holding Node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSFolder {
    pub path: VRPath,
    pub child_folders: Vec<FSFolder>,
    pub child_entries: Vec<FSEntry>,
}

impl FSFolder {
    pub fn new(path: VRPath, child_folders: Vec<FSFolder>, child_entries: Vec<FSEntry>) -> Self {
        Self {
            path,
            child_folders,
            child_entries,
        }
    }

    /// Generates a new FSFolder using a BaseVectorResource holding Node + the path where the node was retrieved
    /// from in the VecFS internals. Use VRPath::new() if the path is root.
    pub fn from_vector_resource_node(node: Node, node_fs_path: VRPath) -> Result<Self, VectorFSError> {
        match node.content {
            NodeContent::Resource(base_vector_resource) => {
                Self::from_vector_resource(base_vector_resource, node_fs_path)
            }
            _ => Err(VRError::InvalidVRBaseType)?,
        }
    }

    /// Generates a new FSFolder from a BaseVectorResource + the path where it was retrieved
    /// from inside of the VectorFS. Use VRPath::new() if the path is root.
    pub fn from_vector_resource(resource: BaseVectorResource, resource_fs_path: VRPath) -> Result<Self, VectorFSError> {
        let mut child_folders = Vec::new();
        let mut child_entries = Vec::new();

        for node in resource.as_trait_object().get_nodes() {
            match node.content {
                // If it's a Resource, then create a FSFolder by recursing, and push it to child_folders
                NodeContent::Resource(inner_resource) => {
                    let new_path = resource_fs_path.push_cloned(inner_resource.as_trait_object().name().to_string());
                    child_folders.push(Self::from_vector_resource(inner_resource, new_path)?);
                }
                // If it's a VRHeader, then create a FSEntry and push it to child_entries
                NodeContent::VRHeader(vr_header) => {
                    // Read source_file_saved from metadata
                    let source_file_saved = node
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.get(&FSEntry::source_file_saved_metadata_key()))
                        .map_or(false, |value| value == "true");
                    let new_path = resource_fs_path.push_cloned(node.id);
                    let fs_entry = FSEntry::new(new_path, vr_header, source_file_saved);
                    child_entries.push(fs_entry);
                }
                _ => {}
            }
        }

        Ok(Self::new(VRPath::new(), child_folders, child_entries))
    }
}

/// An external facing file entry abstraction used to make interacting with the VectorFS easier.
/// Actual data represented by a FSEntry is a VRHeader-holding Node.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FSEntry {
    pub path: VRPath,
    pub vr_header: VRHeader,
    pub source_file_saved: bool,
}

impl FSEntry {
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
}
