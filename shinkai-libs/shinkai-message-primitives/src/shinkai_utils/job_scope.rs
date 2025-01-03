use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{search_mode::VectorSearchMode, shinkai_path::ShinkaiPath};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct MinimalJobScope {
    pub vector_fs_items: Vec<ShinkaiPath>, // TODO: rename this to non-vector-fs-items
    pub vector_fs_folders: Vec<ShinkaiPath>,
    #[serde(default = "default_vector_search_mode")]
    pub vector_search_mode: VectorSearchMode,
}

// Function to provide the default value for vector_search_mode
fn default_vector_search_mode() -> VectorSearchMode {
    VectorSearchMode::FillUpTo25k
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
            vector_fs_items: Vec::new(),
            vector_fs_folders: Vec::new(),
            vector_search_mode: VectorSearchMode::FillUpTo25k,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_minimal_job_scope() {
        let json_data = json!({
            "vector_fs_items": [],
            "vector_fs_folders": ["/My Files (Private)"],
            "vector_search_mode": "FillUpTo25k"
        });

        let deserialized: MinimalJobScope = serde_json::from_value(json_data).expect("Failed to deserialize");

        assert!(deserialized.vector_fs_items.is_empty());
        assert_eq!(deserialized.vector_fs_folders.len(), 1);
        assert_eq!(deserialized.vector_fs_folders[0].relative_path(), "My Files (Private)");
        assert_eq!(deserialized.vector_search_mode, VectorSearchMode::FillUpTo25k);
    }

    #[test]
    fn test_deserialize_minimal_job_scope_with_string_items() {
        let json_data = json!({
            "vector_fs_items": ["/path/to/file1", "/path/to/file2"],
            "vector_fs_folders": [{"path": "/My Files (Private)"}],
            "vector_search_mode": "FillUpTo25k"
        });

        let deserialized: MinimalJobScope = serde_json::from_value(json_data).expect("Failed to deserialize");

        assert_eq!(deserialized.vector_fs_items.len(), 2);
        assert_eq!(deserialized.vector_fs_items[0].relative_path(), "path/to/file1");
        assert_eq!(deserialized.vector_fs_items[1].relative_path(), "path/to/file2");
        assert_eq!(deserialized.vector_fs_folders.len(), 1);
        assert_eq!(deserialized.vector_fs_folders[0].relative_path(), "My Files (Private)");
        assert_eq!(deserialized.vector_search_mode, VectorSearchMode::FillUpTo25k);
    }
}
