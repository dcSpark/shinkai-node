use crate::embeddings::Embedding;
use crate::model_type::EmbeddingModelType;
use crate::resource_errors::VRError;
use crate::shinkai_time::{ShinkaiStringTime, ShinkaiTime};
pub use crate::source::{
    DocumentFileType, ImageFileType, SourceFileReference, SourceFileType, SourceReference, VRSource,
};
use crate::vector_resource::base_vector_resources::{BaseVectorResource, VRBaseType};
use chrono::{DateTime, Utc};
use ordered_float::NotNan;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A node that was retrieved from a search.
/// Includes extra data like the resource_header of the resource it was from
/// and the similarity score from the vector search. Resource header is especially
/// helpful when you have multiple layers of VectorResources inside of each other.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetrievedNode {
    pub node: Node,
    pub score: f32,
    pub resource_header: VRHeader,
    pub retrieval_path: VRPath,
}

impl RetrievedNode {
    /// Create a new RetrievedNode
    pub fn new(node: Node, score: f32, resource_header: VRHeader, retrieval_path: VRPath) -> Self {
        Self {
            node,
            score,
            resource_header,
            retrieval_path,
        }
    }

    /// Sorts the list of RetrievedNodes based on their scores.
    /// Uses a binary heap for efficiency, returns num_results of highest scored.
    pub fn sort_by_score(retrieved_data: &Vec<RetrievedNode>, num_results: u64) -> Vec<RetrievedNode> {
        // Create a HashMap to store the RetrievedNode instances for post-scoring retrieval
        let mut nodes: HashMap<String, RetrievedNode> = HashMap::new();

        // Map the retrieved_data to a vector of tuples (NotNan<f32>, id_ref_key)
        // We create id_ref_key to support sorting RetrievedNodes from
        // different Resources together and avoid node id collision problems.
        let scores: Vec<(NotNan<f32>, String)> = retrieved_data
            .into_iter()
            .map(|node| {
                let ref_key = node.resource_header.reference_string().clone();
                let id_ref_key = format!("{}-{}", node.node.id.clone(), ref_key);
                nodes.insert(id_ref_key.clone(), node.clone());
                (NotNan::new(nodes[&id_ref_key].score).unwrap(), id_ref_key)
            })
            .collect();

        // Use the bin_heap_order_scores function to sort the scores
        let sorted_scores = Embedding::bin_heap_order_scores(scores, num_results as usize);

        // Map the sorted_scores back to a vector of RetrievedNode
        let sorted_data: Vec<RetrievedNode> = sorted_scores
            .into_iter()
            .map(|(_, id_db_key)| nodes[&id_db_key].clone())
            .collect();

        sorted_data
    }

    /// Formats the retrieval path to a string, adding a trailing `/`
    /// if the node at the path is a Vector Resource
    pub fn format_path_to_string(&self) -> String {
        let mut path_string = self.retrieval_path.format_to_string();
        if let NodeContent::Resource(_) = self.node.content {
            path_string.push('/');
        }
        path_string
    }

    /// Formats the data, source, and metadata of all provided `RetrievedNode`s into a bullet-point
    /// list as a single string. This is to be included inside of a prompt to an LLM.
    /// Includes `max_characters` to allow specifying a hard-cap maximum that will be respected.
    pub fn format_ret_nodes_for_prompt_single_string(ret_nodes: Vec<RetrievedNode>, max_characters: usize) -> String {
        if ret_nodes.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        let mut remaining_chars = max_characters;

        for ret_node in ret_nodes {
            if let Some(formatted_node) = ret_node.format_for_prompt(remaining_chars) {
                if formatted_node.len() > remaining_chars {
                    break;
                }
                result.push_str(&formatted_node);
                result.push_str("\n\n ");
                remaining_chars -= formatted_node.len();
            }
        }

        result
    }

    /// Formats the data, source, and metadata together into a single string that is ready
    /// to be included as part of a prompt to an LLM.
    /// Includes `max_characters` to allow specifying a hard-cap maximum that will be respected.
    pub fn format_for_prompt(&self, max_characters: usize) -> Option<String> {
        let source_string = self.resource_header.resource_source.format_source_string();
        let metadata_string = self.format_metadata_string();

        let base_length = source_string.len() + metadata_string.len() + 20; // 20 chars of actual content as a minimum amount to bother including

        if base_length > max_characters {
            return None;
        }

        let data_string = self.node.get_text_content().ok()?;
        let data_length = max_characters - base_length;

        let data_string = if data_string.len() > data_length {
            data_string[..data_length].to_string()
        } else {
            data_string
        };

        let formatted_string = if metadata_string.len() > 0 {
            format!("- {} (Source: {}, {})", data_string, source_string, metadata_string)
        } else {
            format!("- {} (Source: {})", data_string, source_string)
        };

        Some(formatted_string)
    }

    /// Parses the metdata of the node, and outputs a readable string which includes
    /// any metadata relevant to provide to an LLM as context about the retrieved node.
    pub fn format_metadata_string(&self) -> String {
        match &self.node.metadata {
            Some(metadata) => {
                if let Some(page_numbers) = metadata.get("page_numbers") {
                    format!("Pgs: {}", page_numbers)
                } else {
                    String::new()
                }
            }
            None => String::new(),
        }
    }
}

/// Represents a Vector Resource Node which holds a unique id, one of the types of NodeContent,
/// metadata, and other internal relevant data.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub id: String,
    pub content: NodeContent,
    pub metadata: Option<HashMap<String, String>>,
    pub data_tag_names: Vec<String>,
    pub last_modified_datetime: DateTime<Utc>,
}

impl Node {
    /// Create a new text-holding Node with a provided String id
    pub fn new_text(
        id: String,
        text: String,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: &Vec<String>,
    ) -> Self {
        let current_time = ShinkaiTime::generate_time_now();

        Self {
            id,
            content: NodeContent::Text(text.to_string()),
            metadata,
            data_tag_names: data_tag_names.clone(),
            last_modified_datetime: current_time,
        }
    }

    /// Create a new text-holding Node with a provided u64 id, which gets converted to string internally
    pub fn new_text_with_integer_id(
        id: u64,
        text: String,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: &Vec<String>,
    ) -> Self {
        Self::new_text(id.to_string(), text, metadata, data_tag_names)
    }

    /// Create a new BaseVectorResource-holding Node with a provided String id
    pub fn new_vector_resource(
        id: String,
        vector_resource: &BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
    ) -> Self {
        let current_time = ShinkaiTime::generate_time_now();
        Node {
            id: id,
            content: NodeContent::Resource(vector_resource.clone()),
            metadata: metadata,
            data_tag_names: vector_resource.as_trait_object().data_tag_index().data_tag_names(),
            last_modified_datetime: current_time,
        }
    }

    /// Create a new BaseVectorResource-holding Node with a provided u64 id, which gets converted to string internally
    pub fn new_vector_resource_with_integer_id(
        id: u64,
        vector_resource: &BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
    ) -> Self {
        Self::new_vector_resource(id.to_string(), vector_resource, metadata)
    }

    /// Create a new ExternalContent-holding Node with a provided String id
    pub fn new_external_content(
        id: String,
        external_content: &SourceReference,
        metadata: Option<HashMap<String, String>>,
    ) -> Self {
        let current_time = ShinkaiTime::generate_time_now();
        Node {
            id,
            content: NodeContent::ExternalContent(external_content.clone()),
            metadata,
            data_tag_names: vec![],
            last_modified_datetime: current_time,
        }
    }

    /// Create a new ExternalContent-holding Node with a provided u64 id, which gets converted to string internally
    pub fn new_external_content_with_integer_id(
        id: u64,
        external_content: &SourceReference,
        metadata: Option<HashMap<String, String>>,
    ) -> Self {
        Self::new_external_content(id.to_string(), external_content, metadata)
    }

    /// Create a new VRHeader-holding Node with a provided String id
    pub fn new_vr_header(
        id: String,
        vr_header: &VRHeader,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: &Vec<String>,
    ) -> Self {
        let current_time = ShinkaiTime::generate_time_now();

        Self {
            id,
            content: NodeContent::VRHeader(vr_header.clone()),
            metadata,
            data_tag_names: data_tag_names.clone(),
            last_modified_datetime: current_time,
        }
    }

    /// Create a new VRHeader-holding Node with a provided u64 id, which gets converted to string internally
    pub fn new_vr_header_with_integer_id(
        id: u64,
        vr_header: &VRHeader,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: &Vec<String>,
    ) -> Self {
        Self::new_vr_header(id.to_string(), vr_header, metadata, data_tag_names)
    }

    /// Creates a new Node using provided content with a String id.
    pub fn from_node_content(
        id: String,
        content: NodeContent,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: Vec<String>,
    ) -> Self {
        let current_time = ShinkaiTime::generate_time_now();
        Self {
            id,
            content,
            metadata,
            data_tag_names,
            last_modified_datetime: current_time,
        }
    }

    /// Creates a new Node using provided content with a u64 id
    pub fn from_node_content_with_integer_id(
        id: u64,
        content: NodeContent,
        metadata: Option<HashMap<String, String>>,
        data_tag_names: Vec<String>,
    ) -> Self {
        Self::from_node_content(id.to_string(), content, metadata, data_tag_names)
    }

    /// Updates the last_modified_datetime to the current time
    pub fn update_last_modified_to_now(&mut self) {
        let current_time = ShinkaiTime::generate_time_now();
        self.last_modified_datetime = current_time;
    }

    /// Attempts to return the text content from the Node. Errors if is different type
    pub fn get_text_content(&self) -> Result<String, VRError> {
        match &self.content {
            NodeContent::Text(s) => Ok(s.clone()),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Attempts to return the BaseVectorResource from the Node. Errors if is different type
    pub fn get_vector_resource_content(&self) -> Result<BaseVectorResource, VRError> {
        match &self.content {
            NodeContent::Resource(resource) => Ok(resource.clone()),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Attempts to return the ExternalContent from the Node. Errors if content is not ExternalContent
    pub fn get_external_content(&self) -> Result<SourceReference, VRError> {
        match &self.content {
            NodeContent::ExternalContent(external_content) => Ok(external_content.clone()),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Returns the keys of all kv pairs in the Node's metadata field,
    /// and all metadata keys of internal nodes for Vector Resources and VRHeaders.
    /// None if no keys exist.
    pub fn metadata_keys(&self) -> Option<Vec<String>> {
        let mut keys = self
            .metadata
            .as_ref()
            .map(|metadata| metadata.keys().cloned().collect::<Vec<String>>())
            .unwrap_or_else(Vec::new);

        if let NodeContent::Resource(resource) = &self.content {
            let internal_keys = resource.as_trait_object().metadata_index().get_all_metadata_keys();
            keys.extend(internal_keys);
        } else if let NodeContent::VRHeader(header) = &self.content {
            keys.extend(header.metadata_index_keys.clone());
        }

        if keys.is_empty() {
            None
        } else {
            Some(keys)
        }
    }
}

/// Contents of a Node
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NodeContent {
    Text(String),
    Resource(BaseVectorResource),
    ExternalContent(SourceReference),
    VRHeader(VRHeader),
}

/// Struct which holds descriptive information about a given Vector Resource.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VRHeader {
    pub resource_name: String,
    pub resource_id: String,
    pub resource_base_type: VRBaseType,
    pub resource_source: VRSource,
    pub resource_embedding: Option<Embedding>,
    pub resource_created_datetime: DateTime<Utc>,
    pub resource_last_modified_datetime: DateTime<Utc>,
    pub resource_embedding_model_used: EmbeddingModelType,
    /// List of data tag names matching in internal nodes
    pub data_tag_names: Vec<String>,
    /// List of metadata keys held in internal nodes
    pub metadata_index_keys: Vec<String>,
}

impl VRHeader {
    /// Create a new VRHeader
    pub fn new(
        resource_name: &str,
        resource_id: &str,
        resource_base_type: VRBaseType,
        resource_embedding: Option<Embedding>,
        data_tag_names: Vec<String>,
        resource_source: VRSource,
        resource_created_datetime: DateTime<Utc>,
        resource_last_modified_datetime: DateTime<Utc>,
        metadata_index_keys: Vec<String>,
        resource_embedding_model_used: EmbeddingModelType,
    ) -> Self {
        Self {
            resource_name: resource_name.to_string(),
            resource_id: resource_id.to_string(),
            resource_base_type,
            resource_embedding: resource_embedding.clone(),
            data_tag_names: data_tag_names,
            resource_source,
            resource_created_datetime,
            resource_last_modified_datetime,
            metadata_index_keys,
            resource_embedding_model_used,
        }
    }

    /// Create a new VRHeader using a reference_string instead of the name/id directly
    pub fn new_with_reference_string(
        reference_string: String,
        resource_base_type: VRBaseType,
        resource_embedding: Option<Embedding>,
        data_tag_names: Vec<String>,
        resource_source: VRSource,
        resource_created_datetime: DateTime<Utc>,
        resource_last_modified_datetime: DateTime<Utc>,
        metadata_index_keys: Vec<String>,
        resource_embedding_model_used: EmbeddingModelType,
    ) -> Result<Self, VRError> {
        let parts: Vec<&str> = reference_string.split(":::").collect();
        if parts.len() != 2 {
            return Err(VRError::InvalidReferenceString(reference_string.clone()));
        }
        let resource_name = parts[0].to_string();
        let resource_id = parts[1].to_string();

        Ok(Self {
            resource_name,
            resource_id,
            resource_base_type,
            resource_embedding: resource_embedding.clone(),
            data_tag_names: data_tag_names,
            resource_source,
            resource_created_datetime,
            resource_last_modified_datetime,
            metadata_index_keys,
            resource_embedding_model_used,
        })
    }

    /// Returns a "reference string" that uniquely identifies the VectorResource (formatted as: `{name}:::{resource_id}`).
    /// This is also used in the Shinkai Node as the key where the VectorResource is stored in the DB.
    pub fn reference_string(&self) -> String {
        Self::generate_resource_reference_string(self.resource_name.clone(), self.resource_id.clone())
    }

    /// Returns a "reference string" that uniquely identifies the VectorResource (formatted as: `{name}:::{resource_id}`).
    /// This is also used in the Shinkai Node as the key where the VectorResource is stored in the DB.
    pub fn generate_resource_reference_string(name: String, resource_id: String) -> String {
        let name = name.replace(" ", "_").replace(":", "_");
        let resource_id = resource_id.replace(" ", "_").replace(":", "_");
        format!("{}:::{}", name, resource_id)
    }
}

/// A path inside of a Vector Resource to a Node which exists somewhere in the hierarchy.
/// Internally the path is made up of an ordered list of Node ids (Int-holding strings for Docs, any string for Maps).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VRPath {
    pub path_ids: Vec<String>,
}

impl VRPath {
    /// Create a new VRPath
    pub fn new() -> Self {
        Self { path_ids: vec![] }
    }

    /// Returns if the path is empty (aka pointing at root, `/`)
    pub fn is_empty(&self) -> bool {
        self.path_ids.len() == 0
    }

    /// Get the depth of the VRPath. Of note, this will return 0 in both cases if
    /// the path is empty, or if it is in the root path (because depth starts at 0
    /// for Vector Resources). This matches the TraversalMethod::UntilDepth interface.
    pub fn depth(&self) -> u64 {
        if self.path_ids.is_empty() {
            0
        } else {
            (self.path_ids.len() - 1) as u64
        }
    }

    /// Get the inclusive depth of the VRPath, meaning we include all parts of the path, including
    /// the final id. (In practice, generally +1 compared to .depth())
    pub fn depth_inclusive(&self) -> u64 {
        self.path_ids.len() as u64
    }

    /// Adds an id to the end of the VRPath's path_ids. Automatically cleans the id String
    /// to remove unsupported characters that would break the path.
    pub fn push(&mut self, id: String) {
        self.path_ids.push(id);
    }

    /// Removes an element from the end of the path_ids
    pub fn pop(&mut self) -> Option<String> {
        self.path_ids.pop()
    }

    /// Returns a copy of the final id in the path, if it exists.
    /// This is the id of the actual node that the path points to.
    pub fn last_path_id(&self) -> Result<String, VRError> {
        self.path_ids
            .last()
            .cloned()
            .ok_or(VRError::InvalidVRPath(self.clone()))
    }

    /// Creates a cloned VRPath and adds an id to the end of the VRPath's path_ids.
    /// Automatically cleans the id String to remove unsupported characters that would break the path.
    pub fn push_cloned(&self, id: String) -> Self {
        let mut new_path = self.clone();
        new_path.push(id);
        new_path
    }

    /// Creates a cloned VRPath and removes an element from the end
    pub fn pop_cloned(&self) -> Self {
        let mut new_path = self.clone();
        new_path.pop();
        new_path
    }

    /// Create a VRPath from a path string
    pub fn from_string(path_string: &str) -> Result<Self, VRError> {
        if !path_string.starts_with('/') {
            return Err(VRError::InvalidPathString(path_string.to_string()));
        }

        let mut path = Self::new();
        if path_string != "/" {
            let path_ids_string = path_string.trim_start_matches('/').trim_end_matches('/');
            let elements: Vec<&str> = path_ids_string.split('/').collect();
            for element in elements {
                path.push(element.to_string());
            }
        }
        Ok(path)
    }

    /// Formats the VRPath to a string
    pub fn format_to_string(&self) -> String {
        format!("/{}", self.path_ids.join("/"))
    }

    /// Cleans an input string to ensure that it does not have any
    /// characters which would break a VRPath.
    pub fn clean_string(s: &str) -> String {
        s.replace(" ", "_").replace("/", "-")
    }
}

impl Hash for VRPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.format_to_string().hash(state);
    }
}

impl fmt::Display for VRPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.format_to_string())
    }
}
