use super::vector_resource::Node;
use regex::Regex;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use std::collections::HashMap;
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataTag {
    pub name: String,
    pub description: String,
    pub regex_str: String,
}

impl DataTag {
    /// Validates the provided regex string and creates a new DataTag
    pub fn new(name: &str, description: &str, regex_str: &str) -> Result<Self, Box<dyn Error>> {
        if !regex_str.is_empty() {
            Regex::new(regex_str)?; // Attempt to compile regex, will error if invalid
        }
        Ok(Self {
            name: name.to_string(),
            description: description.to_string(),
            regex_str: regex_str.to_string(),
        })
    }

    /// Checks if the provided input string matches the regex of the DataTag
    pub fn validate(&self, input_string: &str) -> bool {
        // This should never fail in practice because in new we already checked
        match Regex::new(&self.regex_str) {
            Ok(regex) => regex.is_match(input_string),
            Err(_) => false,
        }
    }

    /// Validates a list of tags and returns those that pass validation
    pub fn validate_tag_list(input_string: &str, tag_list: &[DataTag]) -> Vec<DataTag> {
        tag_list
            .iter()
            .filter(|tag| tag.validate(input_string))
            .cloned() // Clone each DataTag that passes validation
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct DataTagIndex {
    index: HashMap<String, Vec<String>>,
}

// Add Default implementation for DataTagIndex
impl Default for DataTagIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl DataTagIndex {
    /// Initializes an empty DataTagIndex
    pub fn new() -> Self {
        Self { index: HashMap::new() }
    }

    /// Returns list of names of all data tags part of the index
    pub fn data_tag_names(&self) -> Vec<String> {
        self.index.keys().cloned().collect()
    }

    /// Adds reference to the node (id) to all tags in the index that are
    /// annotated on the node in node.content_tags
    pub fn add_node(&mut self, node: &Node) {
        self.add_node_id_multi_tags(&node.id, &node.data_tag_names);
    }

    /// Removes all references to the node in the index
    pub fn remove_node(&mut self, node: &Node) {
        self.remove_node_id_multi_tags(&node.id, &node.data_tag_names);
    }

    /// Deletes all references in the index associated with old_node,
    /// replacing them with the new_node
    pub fn replace_node(&mut self, old_node: &Node, new_node: &Node) {
        self.remove_node(old_node);
        self.add_node(new_node);
    }

    /// Add a node id under several DataTags in the index
    fn add_node_id_multi_tags(&mut self, id: &str, tag_names: &Vec<String>) {
        for name in tag_names {
            self.add_node_id(id, name)
        }
    }

    /// Removes a node id under several DataTags in the index
    fn remove_node_id_multi_tags(&mut self, id: &str, tag_names: &Vec<String>) {
        for name in tag_names {
            self.remove_node_id(id, name)
        }
    }

    /// Add a node id under a DataTag in the index
    fn add_node_id(&mut self, id: &str, tag_name: &str) {
        let entry = self.index.entry(tag_name.to_string()).or_default();
        if !entry.contains(&id.to_string()) {
            entry.push(id.to_string());
        }
    }

    /// Remove a node id associated with a DataTag in the index
    fn remove_node_id(&mut self, id: &str, tag_name: &str) {
        if let Some(ids) = self.index.get_mut(tag_name) {
            ids.retain(|x| x != id);
        }
    }

    /// Get node ids associated with a data tag name
    pub fn get_node_ids(&self, data_tag_name: &str) -> Option<&Vec<String>> {
        self.index.get(data_tag_name)
    }
}
