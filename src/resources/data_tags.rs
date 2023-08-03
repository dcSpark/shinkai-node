use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

use super::resource::DataChunk;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataTag {
    name: String,
    description: String,
    regex_str: String,
}

impl DataTag {
    /// Validates the provided regex string and creates a new DataTag
    pub fn new(name: &str, description: &str, regex_str: &str) -> Result<Self, Box<dyn Error>> {
        if regex_str.len() > 0 {
            Regex::new(&regex_str)?; // Attempt to compile regex, will error if invalid
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
    pub fn validate_tag_list(input_string: &str, tag_list: &Vec<DataTag>) -> Vec<DataTag> {
        tag_list
            .iter()
            .filter(|tag| tag.validate(input_string))
            .cloned() // Clone each DataTag that passes validation
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataTagIndex {
    index: HashMap<String, Vec<String>>,
}

impl DataTagIndex {
    /// Initialzies an empty DataTagIndex
    pub fn new() -> Self {
        Self { index: HashMap::new() }
    }

    /// Returns list of names of all data tags part of the index
    pub fn data_tag_names(&self) -> Vec<String> {
        self.index.keys().cloned().collect()
    }

    /// Adds reference to the data chunk (id) to all tags in the index that are
    /// annotated on the chunk in chunk.data_tags
    pub fn add_chunk(&mut self, chunk: &DataChunk) {
        self.add_chunk_id_multi_tags(&chunk.id, &chunk.data_tags);
    }

    /// Removes all references to the data chunk in the index
    pub fn remove_chunk(&mut self, chunk: &DataChunk) {
        self.remove_chunk_id_multi_tags(&chunk.id, &chunk.data_tags);
    }

    /// Deletes all references in the index associated with old_chunk,
    /// replacing them with the new_chunk
    pub fn replace_chunk(&mut self, old_chunk: &DataChunk, new_chunk: &DataChunk) {
        self.remove_chunk(&old_chunk);
        self.add_chunk(&new_chunk);
    }

    /// Add a chunk id under several DataTags in the index
    fn add_chunk_id_multi_tags(&mut self, id: &str, tags: &Vec<DataTag>) {
        for tag in tags {
            self.add_chunk_id(id, tag)
        }
    }

    /// Removes a chunk id under several DataTags in the index
    fn remove_chunk_id_multi_tags(&mut self, id: &str, tags: &Vec<DataTag>) {
        for tag in tags {
            self.remove_chunk_id(id, tag)
        }
    }

    /// Add a chunk id under a DataTag in the index
    fn add_chunk_id(&mut self, id: &str, tag: &DataTag) {
        let entry = self.index.entry(tag.name.clone()).or_insert_with(Vec::new);
        if !entry.contains(&id.to_string()) {
            entry.push(id.to_string());
        }
    }

    /// Remove a chunk id associated with a DataTag in the index
    fn remove_chunk_id(&mut self, id: &str, tag: &DataTag) {
        if let Some(ids) = self.index.get_mut(&tag.name) {
            ids.retain(|x| x != id);
        }
    }

    /// Get chunk ids associated with a data tag name
    pub fn get_chunk_ids(&self, data_tag_name: &str) -> Option<&Vec<String>> {
        self.index.get(data_tag_name)
    }
}
