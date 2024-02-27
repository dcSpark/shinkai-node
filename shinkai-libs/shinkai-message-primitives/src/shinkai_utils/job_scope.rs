use serde::{Deserialize, Serialize};
use shinkai_vector_resources::vector_resource::{VRKai, VRPath};
use shinkai_vector_resources::vector_resource::{VectorResource, VectorResourceCore};
use shinkai_vector_resources::{
    source::{SourceFile, VRSource},
    vector_resource::BaseVectorResource,
    vector_resource::VRHeader,
};
use std::fmt;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
/// Job's scope which includes both Local entries (source/vector resource stored locally only in job)
/// and VecFS entries (source/vector resource stored in the DB, accessible to all jobs)
pub struct JobScope {
    pub local: Vec<LocalScopeEntry>,
    pub vector_fs: Vec<VectorFSScopeEntry>,
}

impl JobScope {}
impl JobScope {
    pub fn new(local: Vec<LocalScopeEntry>, vector_fs: Vec<VectorFSScopeEntry>) -> Self {
        Self { local, vector_fs }
    }

    pub fn new_default() -> Self {
        Self {
            local: Vec::new(),
            vector_fs: Vec::new(),
        }
    }

    /// Checks if the Job Scope is empty (has no entries pointing to VRs)
    pub fn is_empty(&self) -> bool {
        self.local.is_empty() && self.vector_fs.is_empty()
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

        let vector_fs_ids: Vec<String> = self
            .vector_fs
            .iter()
            .map(|entry| entry.resource_header.reference_string())
            .collect();

        f.debug_struct("JobScope")
            .field("local", &format_args!("{:?}", local_ids))
            .field("vector_fs", &format_args!("{:?}", vector_fs_ids))
            .finish()
    }
}

/// Enum holding both Local and VectorFS scope entries
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ScopeEntry {
    Local(LocalScopeEntry),
    VectorFS(VectorFSScopeEntry),
}

/// A Scope Entry for a local file/vector resource that only lives in the
/// Job's scope (not in the DB proper/not available to other jobs)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LocalScopeEntry {
    pub vrkai: VRKai,
}

/// A Scope Entry for a file/vector resource that is saved in the VectorFS
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VectorFSScopeEntry {
    pub resource_header: VRHeader,
    pub vector_fs_path: VRPath,
}

// /// A Scope Entry for a file/vector resource that is saved in the VectorFS
// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
// pub struct VectorFSItemEntry {
//     pub item: FSItem
// }

// /// A Scope Entry for a file/vector resource that is saved in the VectorFS
// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
// pub struct VectorFSFolderEntry {
//     pub folder: FSFolder
//     pub item: FSItem
// }
