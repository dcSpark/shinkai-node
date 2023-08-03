use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataTag {
    name: String,
    description: String,
    regex_str: String,
}

impl DataTag {
    // Validates the provided regex string and creates a new DataTag
    pub fn new(name: String, description: String, regex_str: String) -> Result<Self, Box<dyn Error>> {
        Regex::new(&regex_str)?; // Attempt to compile regex, will error if invalid
        Ok(Self {
            name,
            description,
            regex_str,
        })
    }

    // Checks if the provided input string matches the regex of the DataTag
    pub fn validate(&self, input_string: &str) -> bool {
        // This should never fail in practice because in new we already checked
        match Regex::new(&self.regex_str) {
            Ok(regex) => regex.is_match(input_string),
            Err(_) => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataTagIndex {
    index: HashMap<String, Vec<String>>,
}

impl DataTagIndex {
    // Initialzies an empty DataTagIndex
    pub fn new() -> Self {
        Self { index: HashMap::new() }
    }

    // Add a chunk id under a DataTag in the index
    pub fn add_chunk_id(&mut self, id: &str, tag: &DataTag) {
        let entry = self.index.entry(tag.name.clone()).or_insert_with(Vec::new);
        if !entry.contains(&id.to_string()) {
            entry.push(id.to_string());
        }
    }

    // Remove a chunk id associated with a DataTag in the index
    pub fn remove_chunk_id(&mut self, id: &str, tag: &DataTag) {
        if let Some(ids) = self.index.get_mut(&tag.name) {
            ids.retain(|x| x != id);
        }
    }

    // Replace a chunk id associated with a DataTag's in the index
    pub fn replace_chunk_id(&mut self, original_id: &str, new_id: &str, tag: &DataTag) {
        if let Some(ids) = self.index.get_mut(&tag.name) {
            if let Some(position) = ids.iter().position(|x| x == original_id) {
                ids[position] = new_id.to_string();
            }
        }
    }

    // Get chunk ids associated with a DataTag
    pub fn get_chunk_ids(&self, tag: &DataTag) -> Option<&Vec<String>> {
        self.index.get(&tag.name)
    }
}
