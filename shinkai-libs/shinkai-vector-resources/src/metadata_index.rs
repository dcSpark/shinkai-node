use super::vector_resource::Node;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct MetadataIndex {
    index: HashMap<String, Vec<String>>,
}

impl MetadataIndex {
    /// Initializes an empty MetadataIndex
    pub fn new() -> Self {
        Self { index: HashMap::new() }
    }

    /// Adds id of node to the index using all of the node's
    /// metadata keys
    pub fn add_node(&mut self, node: &Node) {
        if let Some(keys) = node.metadata_keys() {
            self.add_node_id_multi_keys(&node.id, &keys);
        }
    }

    /// Removes the node from the index
    pub fn remove_node(&mut self, node: &Node) {
        if let Some(keys) = node.metadata_keys() {
            self.remove_node_id_multi_keys(&node.id, &keys);
        }
    }

    /// Deletes the old_node from the index replacing it with the new_node
    pub fn replace_node(&mut self, old_node: &Node, new_node: &Node) {
        self.remove_node(&old_node);
        self.add_node(&new_node);
    }

    /// Add a node id under several Metadata keys in the index
    fn add_node_id_multi_keys(&mut self, id: &str, metadata_keys: &Vec<String>) {
        for key in metadata_keys {
            self.add_node_id(id, key)
        }
    }

    /// Removes a node id under several Metadata keys in the index
    fn remove_node_id_multi_keys(&mut self, id: &str, metadata_keys: &Vec<String>) {
        for key in metadata_keys {
            self.remove_node_id(id, key)
        }
    }

    /// Add a node id under a Metadata key in the index
    fn add_node_id(&mut self, id: &str, metadata_key: &str) {
        let entry = self.index.entry(metadata_key.to_string()).or_insert_with(Vec::new);
        if !entry.contains(&id.to_string()) {
            entry.push(id.to_string());
        }
    }

    /// Remove a node id associated with a Metadata in the index
    fn remove_node_id(&mut self, id: &str, metadata_key: &str) {
        if let Some(ids) = self.index.get_mut(metadata_key) {
            ids.retain(|x| x != id);
        }
    }

    /// Get node ids associated with a specific metadata key
    pub fn get_node_ids(&self, metadata_key: &str) -> Option<&Vec<String>> {
        self.index.get(metadata_key)
    }

    /// Returns list of all metadata keys part of the index
    pub fn get_all_metadata_keys(&self) -> Vec<String> {
        self.index.keys().cloned().collect()
    }

    /// Returns a reference to the internal index Hashmap for reading
    pub fn get_metdata_index_hashmap(&self) -> &HashMap<String, Vec<String>> {
        &self.index
    }
}
