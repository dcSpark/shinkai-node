use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{search_mode::VectorSearchMode, shinkai_path::ShinkaiPath};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct MinimalJobScope {
    // pub local_vrkai: Vec<String>,
    // pub local_vrpack: Vec<String>,
    pub vector_fs_items: Vec<ShinkaiPath>, // TODO: rename this to non-vector-fs-items
    pub vector_fs_folders: Vec<ShinkaiPath>,
    #[serde(default)]
    pub vector_search_mode: Vec<VectorSearchMode>,
}

impl MinimalJobScope {
    /// Converts the MinimalJobScope to a JSON value.
    pub fn to_json_value(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }

    /// Converts the MinimalJobScope to a byte vector.
    pub fn from_bytes(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }

    /// Checks if both vector_fs_items and vector_fs_folders are empty.
    pub fn is_empty(&self) -> bool {
        self.vector_fs_items.is_empty() && self.vector_fs_folders.is_empty()
    }
}

impl Default for MinimalJobScope {
    fn default() -> Self {
        Self {
            // // local_vrkai: Vec::new(),
            // local_vrpack: Vec::new(),
            vector_fs_items: Vec::new(),
            vector_fs_folders: Vec::new(),
            vector_search_mode: Vec::new(),
        }
    }
}

// impl From<&JobScope> for MinimalJobScope {
//     fn from(job_scope: &JobScope) -> Self {
//         let local_vrkai_ids: Vec<String> = job_scope
//             .local_vrkai
//             .iter()
//             .map(|entry| match &entry.vrkai.resource {
//                 BaseVectorResource::Document(doc) => doc.reference_string(),
//                 BaseVectorResource::Map(map) => map.reference_string(),
//             })
//             .collect();

//         let local_vrpack_ids: Vec<String> = job_scope.local_vrpack.iter().map(|entry| entry.vrpack.id()).collect();

//         let vector_fs_item_paths: Vec<String> = job_scope
//             .vector_fs_items
//             .iter()
//             .map(|entry| entry.path.to_string())
//             .collect();

//         let vector_fs_folder_paths: Vec<String> = job_scope
//             .vector_fs_folders
//             .iter()
//             .map(|entry| entry.path.to_string())
//             .collect();

//         MinimalJobScope {
//             local_vrkai: local_vrkai_ids,
//             local_vrpack: local_vrpack_ids,
//             vector_fs_items: vector_fs_item_paths,
//             vector_fs_folders: vector_fs_folder_paths,
//         }
//     }
// }

// #[derive(Serialize, Deserialize, Clone, PartialEq, ToSchema)]
// /// Job's scope which includes both Local entries (vrkai stored locally only in job)
// /// and VecFS entries (source/vector resource stored in the FS, accessible to all jobs)
// pub struct JobScope {
//     pub local_vrkai: Vec<LocalScopeVRKaiEntry>,
//     pub local_vrpack: Vec<LocalScopeVRPackEntry>,
//     pub vector_fs_items: Vec<VectorFSItemScopeEntry>,
//     pub vector_fs_folders: Vec<VectorFSFolderScopeEntry>,
//     #[serde(default, deserialize_with = "deserialize_vec")]
//     pub vector_search_mode: Vec<VectorSearchMode>,
// }

// impl JobScope {}
// impl JobScope {
//     /// Create a new JobScope
//     pub fn new(
//         local_vrkai: Vec<LocalScopeVRKaiEntry>,
//         local_vrpack: Vec<LocalScopeVRPackEntry>,
//         vector_fs_items: Vec<VectorFSItemScopeEntry>,
//         vector_fs_folders: Vec<VectorFSFolderScopeEntry>,
//         vector_search_mode: Vec<VectorSearchMode>,
//     ) -> Self {
//         Self {
//             local_vrkai,
//             local_vrpack,
//             vector_fs_items,
//             vector_fs_folders,
//             vector_search_mode,
//         }
//     }

//     /// Create a new JobScope with empty defaults
//     pub fn new_default() -> Self {
//         Self {
//             local_vrkai: Vec::new(),
//             local_vrpack: Vec::new(),
//             vector_fs_items: Vec::new(),
//             vector_fs_folders: Vec::new(),
//             vector_search_mode: Vec::new(),
//         }
//     }

//     /// Checks if the Job Scope is empty (has no entries)
//     pub fn is_empty(&self) -> bool {
//         self.local_vrkai.is_empty()
//             && self.local_vrpack.is_empty()
//             && self.vector_fs_items.is_empty()
//             && self.vector_fs_folders.is_empty()
//     }

//     /// Determines if the JobScope contains significant amount of content to justify
//     /// more advanced vector searching/more iterations in inference chains.
//     pub fn contains_significant_content(&self) -> bool {
//         let mut count = 0;

//         // Each VRKai and VectorFSItem counts as 1
//         count += self.local_vrkai.len() + self.vector_fs_items.len();

//         // Each VRPack and folder (both VectorFS and Network) counts as a multiple.
//         count += (self.local_vrpack.len() + self.vector_fs_folders.len()) * 3;
//         count >= 4
//     }

//     pub fn to_bytes(&self) -> serde_json::Result<Vec<u8>> {
//         let j = serde_json::to_string(self)?;
//         Ok(j.into_bytes())
//     }

//     pub fn from_bytes(bytes: &[u8]) -> serde_json::Result<Self> {
//         serde_json::from_slice(bytes)
//     }

//     pub fn from_json_str(s: &str) -> serde_json::Result<Self> {
//         let deserialized: Self = serde_json::from_str(s)?;
//         Ok(deserialized)
//     }

//     pub fn to_json_str(&self) -> serde_json::Result<String> {
//         let json_str = serde_json::to_string(self)?;
//         Ok(json_str)
//     }

//     /// Serializes the JobScope to a JSON value.
//     pub fn to_json_value(&self) -> serde_json::Result<serde_json::Value> {
//         serde_json::to_value(self)
//     }

//     /// Serializes the JobScope to a minimal JSON value similar to the Debug output.
//     pub fn to_json_value_minimal(&self) -> serde_json::Result<serde_json::Value> {
//         let local_vrkai_ids: Vec<String> = self
//             .local_vrkai
//             .iter()
//             .map(|entry| match &entry.vrkai.resource {
//                 BaseVectorResource::Document(doc) => doc.reference_string(),
//                 BaseVectorResource::Map(map) => map.reference_string(),
//             })
//             .collect();

//         let local_vrpack_ids: Vec<String> = self.local_vrpack.iter().map(|entry| entry.vrpack.id()).collect();

//         let vector_fs_item_paths: Vec<String> = self
//             .vector_fs_items
//             .iter()
//             .map(|entry| entry.path.to_string())
//             .collect();

//         let vector_fs_folder_paths: Vec<String> = self
//             .vector_fs_folders
//             .iter()
//             .map(|entry| entry.path.to_string())
//             .collect();

//         let minimal_json = serde_json::json!({
//             "local_vrkai": local_vrkai_ids,
//             "local_vrpack": local_vrpack_ids,
//             "vector_fs_items": vector_fs_item_paths,
//             "vector_fs_folders": vector_fs_folder_paths
//         });

//         Ok(minimal_json)
//     }
// }

// impl fmt::Debug for JobScope {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         let local_vrkai_ids: Vec<String> = self
//             .local_vrkai
//             .iter()
//             .map(|entry| match &entry.vrkai.resource {
//                 BaseVectorResource::Document(doc) => doc.reference_string(),
//                 BaseVectorResource::Map(map) => map.reference_string(),
//             })
//             .collect();

//         let local_vrpack_ids: Vec<String> = self.local_vrpack.iter().map(|entry| entry.vrpack.id()).collect();

//         let vector_fs_item_paths: Vec<String> = self
//             .vector_fs_items
//             .iter()
//             .map(|entry| entry.path.to_string())
//             .collect();

//         let vector_fs_folder_paths: Vec<String> = self
//             .vector_fs_folders
//             .iter()
//             .map(|entry| entry.path.to_string())
//             .collect();

//         f.debug_struct("JobScope")
//             .field("local_vrkai", &format_args!("{:?}", local_vrkai_ids))
//             .field("local_vrpack", &format_args!("{:?}", local_vrpack_ids))
//             .field("vector_fs_items", &format_args!("{:?}", vector_fs_item_paths))
//             .field("vector_fs_folders", &format_args!("{:?}", vector_fs_folder_paths))
//             .finish()
//     }
// }

// // Convert null values to empty vectors
// fn deserialize_vec<'de, D>(deserializer: D) -> Result<Vec<VectorSearchMode>, D::Error>
// where
//     D: serde::Deserializer<'de>,
// {
//     Option::deserialize(deserializer).map(|opt| opt.unwrap_or_else(Vec::new))
// }

// /// Enum holding both Local and VectorFS scope entries
// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
// pub enum ScopeEntry {
//     LocalScopeVRKai(LocalScopeVRKaiEntry),
//     LocalScopeVRPack(LocalScopeVRPackEntry),
//     VectorFSItem(VectorFSItemScopeEntry),
//     VectorFSFolder(VectorFSFolderScopeEntry),
// }

// /// A Scope Entry for a local VRKai that only lives in the
// /// Job's scope (not in the VectorFS & thus not available to other jobs)
// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
// pub struct LocalScopeVRKaiEntry {
//     pub vrkai: VRKai,
// }

// /// A Scope Entry for a local VRPack that only lives in the
// /// Job's scope (not in the VectorFS & thus not available to other jobs)
// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
// pub struct LocalScopeVRPackEntry {
//     pub vrpack: VRPack,
// }

// /// A Scope Entry for a FSItem saved in the VectorFS.
// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
// pub struct VectorFSItemScopeEntry {
//     pub name: String,
//     pub path: VRPath,
//     pub source: VRSourceReference,
// }

// /// A Scope Entry for a FSFolder saved in the VectorFS.
// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
// pub struct VectorFSFolderScopeEntry {
//     pub name: String,
//     pub path: VRPath,
// }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_minimal_job_scope() {
        let json_data = json!({
            "vector_fs_items": [],
            "vector_fs_folders": [{"path": "/My Files (Private)"}],
            "vector_search_mode": []
        });

        let deserialized: MinimalJobScope = serde_json::from_value(json_data).expect("Failed to deserialize");

        assert!(deserialized.vector_fs_items.is_empty());
        assert_eq!(deserialized.vector_fs_folders.len(), 1);
        assert_eq!(deserialized.vector_fs_folders[0].as_str(), "/My Files (Private)");
        assert!(deserialized.vector_search_mode.is_empty());
    }
}
