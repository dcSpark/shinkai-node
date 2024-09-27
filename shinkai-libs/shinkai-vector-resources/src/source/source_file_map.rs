use super::SourceFile;
use crate::{resource_errors::VRError, vector_resource::VRPath};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

/// A map which stores SourceFiles based on VRPaths within a VectorResource.
/// A SourceFile at root represents the single source file for the whole VR.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct SourceFileMap {
    pub map: HashMap<VRPath, SourceFile>,
    pub source_files_count: u64,
}

impl SourceFileMap {
    /// Creates a new SourceFileMap using the given HashMap and automatically counts the number of entries.
    pub fn new(map: HashMap<VRPath, SourceFile>) -> Self {
        let source_files_count = map.len() as u64;
        SourceFileMap {
            map,
            source_files_count,
        }
    }

    /// Returns the size of the whole SourceFileMap after being encoded as JSON.
    pub fn encoded_size(&self) -> Result<usize, VRError> {
        let json = self.to_json()?;
        Ok(json.as_bytes().len())
    }

    /// Checks if the map contains only a single root SourceFile.
    pub fn contains_only_single_root_sourcefile(&self) -> bool {
        self.source_files_count == 1 && self.map.contains_key(&VRPath::root())
    }

    /// Returns the source file at the given VRPath if it exists.
    pub fn get_source_file(&self, vr_path: VRPath) -> Option<&SourceFile> {
        self.map.get(&vr_path)
    }

    /// Adds a source file to the map and increases the count.
    /// Overwrites any existing SourceFile which already is stored at the same VRPath.
    pub fn add_source_file(&mut self, path: VRPath, source_file: SourceFile) {
        self.map.insert(path, source_file);
        self.source_files_count += 1;
    }

    /// Removes a source file from the map and decreases the count.
    pub fn remove_source_file(&mut self, path: VRPath) -> Option<SourceFile> {
        let res = self.map.remove(&path);
        self.source_files_count -= 1;
        res
    }

    /// Converts the SourceFileMap into a JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }

    /// Creates a SourceFileMap from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
