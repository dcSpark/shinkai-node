use serde::{Deserialize, Serialize};
use shinkai_vector_resources::vector_resource::{VRKai, VRPack, VRPath};
use shinkai_vector_resources::vector_resource::{VectorResourceCore, VectorSearchMode};
use shinkai_vector_resources::{source::VRSourceReference, vector_resource::BaseVectorResource};
use std::fmt;
use utoipa::ToSchema;

use crate::schemas::shinkai_name::ShinkaiName;

#[derive(Serialize, Deserialize, Clone, PartialEq, ToSchema)]
/// Job's scope which includes both Local entries (vrkai stored locally only in job)
/// and VecFS entries (source/vector resource stored in the FS, accessible to all jobs)
pub struct JobScope {
    pub local_vrkai: Vec<LocalScopeVRKaiEntry>,
    pub local_vrpack: Vec<LocalScopeVRPackEntry>,
    pub vector_fs_items: Vec<VectorFSItemScopeEntry>,
    pub vector_fs_folders: Vec<VectorFSFolderScopeEntry>,
    pub network_folders: Vec<NetworkFolderScopeEntry>,
    pub vector_search_mode: Option<VectorSearchMode>,
}

impl JobScope {}
impl JobScope {
    /// Create a new JobScope
    pub fn new(
        local_vrkai: Vec<LocalScopeVRKaiEntry>,
        local_vrpack: Vec<LocalScopeVRPackEntry>,
        vector_fs_items: Vec<VectorFSItemScopeEntry>,
        vector_fs_folders: Vec<VectorFSFolderScopeEntry>,
        network_folders: Vec<NetworkFolderScopeEntry>,
        vector_search_mode: Option<VectorSearchMode>,
    ) -> Self {
        Self {
            local_vrkai,
            local_vrpack,
            vector_fs_items,
            vector_fs_folders,
            network_folders,
            vector_search_mode,
        }
    }

    /// Create a new JobScope with empty defaults
    pub fn new_default() -> Self {
        Self {
            local_vrkai: Vec::new(),
            local_vrpack: Vec::new(),
            vector_fs_items: Vec::new(),
            vector_fs_folders: Vec::new(),
            network_folders: Vec::new(),
            vector_search_mode: None,
        }
    }

    /// Checks if the Job Scope is empty (has no entries)
    pub fn is_empty(&self) -> bool {
        self.local_vrkai.is_empty()
            && self.local_vrpack.is_empty()
            && self.vector_fs_items.is_empty()
            && self.vector_fs_folders.is_empty()
            && self.network_folders.is_empty()
    }

    /// Determines if the JobScope contains significant amount of content to justify
    /// more advanced vector searching/more iterations in inference chains.
    pub fn contains_significant_content(&self) -> bool {
        let mut count = 0;

        // Each VRKai and VectorFSItem counts as 1
        count += self.local_vrkai.len() + self.vector_fs_items.len();

        // Each VRPack and folder (both VectorFS and Network) counts as a multiple.
        count += (self.local_vrpack.len() + self.vector_fs_folders.len() + self.network_folders.len()) * 3;
        count >= 4
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

    /// Serializes the JobScope to a JSON value.
    pub fn to_json_value(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }

    /// Serializes the JobScope to a minimal JSON value similar to the Debug output.
    pub fn to_json_value_minimal(&self) -> serde_json::Result<serde_json::Value> {
        let local_vrkai_ids: Vec<String> = self
            .local_vrkai
            .iter()
            .map(|entry| match &entry.vrkai.resource {
                BaseVectorResource::Document(doc) => doc.reference_string(),
                BaseVectorResource::Map(map) => map.reference_string(),
            })
            .collect();

        let local_vrpack_ids: Vec<String> = self.local_vrpack.iter().map(|entry| entry.vrpack.id()).collect();

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

        let minimal_json = serde_json::json!({
            "local_vrkai": local_vrkai_ids,
            "local_vrpack": local_vrpack_ids,
            "vector_fs_items": vector_fs_item_paths,
            "vector_fs_folders": vector_fs_folder_paths,
        });

        Ok(minimal_json)
    }
}

impl fmt::Debug for JobScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let local_vrkai_ids: Vec<String> = self
            .local_vrkai
            .iter()
            .map(|entry| match &entry.vrkai.resource {
                BaseVectorResource::Document(doc) => doc.reference_string(),
                BaseVectorResource::Map(map) => map.reference_string(),
            })
            .collect();

        let local_vrpack_ids: Vec<String> = self.local_vrpack.iter().map(|entry| entry.vrpack.id()).collect();

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
            .field("local_vrkai", &format_args!("{:?}", local_vrkai_ids))
            .field("local_vrpack", &format_args!("{:?}", local_vrpack_ids))
            .field("vector_fs_items", &format_args!("{:?}", vector_fs_item_paths))
            .field("vector_fs_folders", &format_args!("{:?}", vector_fs_folder_paths))
            .finish()
    }
}

/// Enum holding both Local and VectorFS scope entries
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum ScopeEntry {
    LocalScopeVRKai(LocalScopeVRKaiEntry),
    LocalScopeVRPack(LocalScopeVRPackEntry),
    VectorFSItem(VectorFSItemScopeEntry),
    VectorFSFolder(VectorFSFolderScopeEntry),
    NetworkFolder(NetworkFolderScopeEntry),
}

/// A Scope Entry for a local VRKai that only lives in the
/// Job's scope (not in the VectorFS & thus not available to other jobs)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct LocalScopeVRKaiEntry {
    pub vrkai: VRKai,
}

/// A Scope Entry for a local VRPack that only lives in the
/// Job's scope (not in the VectorFS & thus not available to other jobs)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct LocalScopeVRPackEntry {
    pub vrpack: VRPack,
}

/// A Scope Entry for a FSItem saved in the VectorFS.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct VectorFSItemScopeEntry {
    pub name: String,
    pub path: VRPath,
    pub source: VRSourceReference,
}

/// A Scope Entry for a FSFolder saved in the VectorFS.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct VectorFSFolderScopeEntry {
    pub name: String,
    pub path: VRPath,
}

/// A Scope Entry for a FSFolder that (potentially) exists on another node's VectorFS (if your node has perms).
/// Unsupported currently, struct added for future compatibility.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct NetworkFolderScopeEntry {
    pub name: String,
    /// This should be the profile on the external node where the folder is stored
    pub external_identity: ShinkaiName,
    pub path: VRPath,
}
