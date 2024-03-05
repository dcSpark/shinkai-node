use serde::{Deserialize, Serialize};
use shinkai_vector_resources::vector_resource::{VRKai, VRPath};
use shinkai_vector_resources::vector_resource::{VectorResource, VectorResourceCore};
use shinkai_vector_resources::{
    source::{SourceFile, VRSource},
    vector_resource::BaseVectorResource,
    vector_resource::VRHeader,
};
use std::fmt;

use crate::schemas::shinkai_name::ShinkaiName;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
/// Job's scope which includes both Local entries (vrkai stored locally only in job)
/// and VecFS entries (source/vector resource stored in the FS, accessible to all jobs)
pub struct JobScope {
    pub local: Vec<LocalScopeEntry>,
    pub vector_fs_items: Vec<VectorFSItemScopeEntry>,
    pub vector_fs_folders: Vec<VectorFSFolderScopeEntry>,
    pub network_folders: Vec<NetworkFolderScopeEntry>,
}

impl JobScope {}
impl JobScope {
    /// Create a new JobScope
    pub fn new(
        local: Vec<LocalScopeEntry>,
        vector_fs_items: Vec<VectorFSItemScopeEntry>,
        vector_fs_folders: Vec<VectorFSFolderScopeEntry>,
        network_folders: Vec<NetworkFolderScopeEntry>,
    ) -> Self {
        Self {
            local,
            vector_fs_items,
            vector_fs_folders,
            network_folders,
        }
    }

    /// Create a new JobScope with empty defaults
    pub fn new_default() -> Self {
        Self {
            local: Vec::new(),
            vector_fs_items: Vec::new(),
            vector_fs_folders: Vec::new(),
            network_folders: Vec::new(),
        }
    }

    /// Checks if the Job Scope is empty (has no entries)
    pub fn is_empty(&self) -> bool {
        self.local.is_empty()
            && self.vector_fs_items.is_empty()
            && self.vector_fs_folders.is_empty()
            && self.network_folders.is_empty()
    }

    pub fn to_bytes(&self) -> serde_json::Result<Vec<u8>> {
        let j = serde_json::to_string(self)?;
        Ok(j.into_bytes())
    }

    pub fn from_bytes(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }

    pub fn from_json_str(s: &str) -> serde_json::Result<Self> {
        let deserialized: Self = serde_json::from_str(s)?;
        Ok(deserialized)
    }

    pub fn to_json_str(&self) -> serde_json::Result<String> {
        let json_str = serde_json::to_string(self)?;
        Ok(json_str)
    }
}

impl fmt::Debug for JobScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let local_ids: Vec<String> = self
            .local
            .iter()
            .map(|entry| match &entry.vrkai.resource {
                BaseVectorResource::Document(doc) => doc.resource_id().to_string(),
                BaseVectorResource::Map(map) => map.resource_id().to_string(),
            })
            .collect();

        let vector_fs_item_paths: Vec<String> = self
            .vector_fs_items
            .iter()
            .map(|entry| entry.path.to_string())
            .collect();

        let vector_fs_folder_paths: Vec<String> = self
            .vector_fs_folders
            .iter()
            .map(|entry| entry.path.to_string())
            .collect();

        f.debug_struct("JobScope")
            .field("local", &format_args!("{:?}", local_ids))
            .field("vector_fs_items", &format_args!("{:?}", vector_fs_item_paths))
            .field("vector_fs_folders", &format_args!("{:?}", vector_fs_folder_paths))
            .finish()
    }
}

/// Enum holding both Local and VectorFS scope entries
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ScopeEntry {
    Local(LocalScopeEntry),
    VectorFSItem(VectorFSItemScopeEntry),
    VectorFSFolder(VectorFSFolderScopeEntry),
    NetworkFolder(NetworkFolderScopeEntry),
}

/// A Scope Entry for a local VRKai that only lives in the
/// Job's scope (not in the VectorFS & thus not available to other jobs)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LocalScopeEntry {
    pub vrkai: VRKai,
}

/// A Scope Entry for a FSItem saved in the VectorFS.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VectorFSItemScopeEntry {
    pub name: String,
    pub path: VRPath,
    pub source: VRSource,
}

/// A Scope Entry for a FSFolder saved in the VectorFS.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VectorFSFolderScopeEntry {
    pub name: String,
    pub path: VRPath,
}

/// A Scope Entry for a FSFolder that (potentially) exists on another node's VectorFS (if your node has perms).
/// Unsupported currently, struct added for future compatibility.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct NetworkFolderScopeEntry {
    pub name: String,
    /// This should be the profile on the external node where the folder is stored
    pub external_identity: ShinkaiName,
    pub path: VRPath,
}
