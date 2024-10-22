use crate::embedding_generator::EmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::file_parser::file_parser::ShinkaiFileParser;
use crate::model_type::EmbeddingModelType;
use crate::resource_errors::VRError;
use crate::shinkai_time::ShinkaiTime;
use crate::source::DistributionInfo;
pub use crate::source::{
    DocumentFileType, ImageFileType, SourceFileReference, SourceFileType, SourceReference, VRSourceReference,
};
use crate::utils::count_tokens_from_message_llama3;
use crate::vector_resource::base_vector_resources::{BaseVectorResource, VRBaseType};
use blake3::hash;
use chrono::{DateTime, Utc};
use ordered_float::NotNan;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Deserializer};
use serde::{Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use utoipa::ToSchema;

/// A node that was retrieved from inside of a Vector Resource. Includes extra data like the retrieval path
/// and the similarity score from the vector search. The resource_header is the VRHeader from the root
/// Vector Resource the RetrievedNode is from.
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
                let _hash = node.node.get_merkle_hash().unwrap_or_default();
                let id_ref_key = node.generate_globally_unique_node_id();
                nodes.insert(id_ref_key.clone(), node.clone());
                (NotNan::new(nodes[&id_ref_key].score).unwrap(), id_ref_key)
            })
            .collect();

        // Use the bin_heap_order_scores function to sort the scores
        let sorted_scores = Embedding::bin_heap_order_scores(scores, num_results as usize);

        // Map the sorted_scores back to a vector of RetrievedNode
        let sorted_data: Vec<RetrievedNode> = sorted_scores
            .into_iter()
            .map(|(_, id_ref_key)| nodes[&id_ref_key].clone())
            .collect();

        sorted_data
    }

    /// Sorts groups of RetrievedNodes based on the highest score within each group.
    /// Returns the groups sorted by the highest score in each group.
    pub fn sort_by_score_groups(
        retrieved_node_groups: &Vec<Vec<RetrievedNode>>,
        num_results: u64,
    ) -> Vec<Vec<RetrievedNode>> {
        let mut highest_score_nodes: Vec<RetrievedNode> = Vec::new();
        let mut group_map: HashMap<String, Vec<RetrievedNode>> = HashMap::new();

        // Iterate over each group, find the node with the highest score, and store it along with the group
        for group in retrieved_node_groups {
            if let Some(highest_node) = group
                .iter()
                .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))
            {
                let highest_node_clone = highest_node.clone();
                let id_ref_key = highest_node.generate_globally_unique_node_id();
                highest_score_nodes.push(highest_node_clone);
                group_map.insert(id_ref_key, group.clone());
            }
        }

        // Sort the highest scoring nodes from each group
        let sorted_highest_nodes = Self::sort_by_score(&highest_score_nodes, num_results);

        // Fetch each group in the order determined by the sorted highest scoring nodes
        let sorted_groups: Vec<Vec<RetrievedNode>> = sorted_highest_nodes
            .into_iter()
            .filter_map(|node| {
                let id_ref_key = node.generate_globally_unique_node_id();
                group_map.remove(&id_ref_key)
            })
            .collect();

        sorted_groups
    }

    /// Generates a unique identifier (across VRs) for the RetrievedNode based on its content and metadata.
    /// This id includes merkle hash, retrieval path, last written datetime, score, and resource id.
    pub fn generate_globally_unique_node_id(&self) -> String {
        let hash = self.node.get_merkle_hash().unwrap_or_default();
        format!(
            "{}-{}-{}-{}-{}",
            hash,
            self.retrieval_path,
            self.node.last_written_datetime.to_rfc3339(),
            self.score,
            self.resource_header.resource_id,
        )
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

    /// Fetches the node's datetime by first checking node metadata, then if none available, returns None.
    pub fn get_datetime_default(&self) -> Option<DateTime<Utc>> {
        self.node.get_metadata_datetime()
    }

    /// Fetches the node's datetime by first checking node metadata, then if none available, returns None.
    /// Returns a string in RFC3339 format without the fractional seconds if datetime is available.
    pub fn get_datetime_default_string(&self) -> Option<String> {
        self.get_datetime_default()
            .map(|dt| dt.to_rfc3339().split('.').next().unwrap_or("").to_string())
    }

    /// Formats the data, source, and metadata together into a single string that is ready
    /// to be included as part of a prompt to an LLM.
    /// Includes `max_characters` to allow specifying a hard-cap maximum that will be respected.
    pub fn format_for_prompt(&self, max_characters: usize) -> Option<String> {
        let source_string = self.resource_header.resource_source.format_source_string();
        let position_string = self.format_position_string();
        let datetime_string = self.get_datetime_default_string();

        // If the text is too long, cut it
        let mut data_string = self.node.get_text_content().ok()?.to_string();
        if data_string.len() > max_characters {
            let amount_over = data_string.len() - max_characters;
            let amount_to_add = source_string.len()
                + position_string.len()
                + datetime_string.as_ref().map_or(0, |s| s.len())
                + amount_over;
            let amount_to_cut = amount_over + amount_to_add + 25;
            data_string = data_string.chars().take(amount_to_cut).collect::<String>();
        }

        let formatted_string = if position_string.len() > 0 {
            if let Some(datetime_string) = datetime_string {
                format!(
                    "- {} (Source: {}, {}) {}",
                    data_string, source_string, position_string, datetime_string
                )
            } else {
                format!("- {} (Source: {}, {})", data_string, source_string, position_string)
            }
        } else {
            if let Some(datetime_string) = datetime_string {
                format!("- {} (Source: {}) {}", data_string, source_string, datetime_string)
            } else {
                format!("- {} (Source: {})", data_string, source_string)
            }
        };

        Some(formatted_string)
    }

    /// Parses node position in the content using metadata/retrieved node data.
    pub fn format_position_string(&self) -> String {
        if let Some(metadata) = &self.node.metadata {
            if let Some(page_numbers) = metadata.get(&ShinkaiFileParser::page_numbers_metadata_key()) {
                if !page_numbers.is_empty() {
                    let page_label = if page_numbers.contains(',') { "Pages" } else { "Page" };
                    return format!("{}: {}", page_label, page_numbers);
                }
            }
        }

        // Cut up the retrieval path into shortened strings
        let section_strings = self
            .retrieval_path
            .path_ids
            .iter()
            .map(|id| {
                let mut shortened_id = id.to_string();
                if shortened_id.len() > 20 {
                    let mut char_iter = shortened_id.chars();
                    shortened_id = char_iter.by_ref().take(20).collect::<String>();
                    // shortened_id.push_str("...");
                }
                shortened_id
            })
            .collect::<Vec<String>>();

        // Create a relative position based on parents node ids
        let final_section_string = if section_strings.len() > 3 {
            format!(
                ".../{}",
                section_strings
                    .iter()
                    .skip(section_strings.len() - 3)
                    .map(|s| s.as_str())
                    .collect::<Vec<&str>>()
                    .join("/")
            )
        } else {
            section_strings.join("/")
        };

        format!("Section: {}", final_section_string)
    }

    /// Sets the proximity_group_id in the node's metadata.
    pub fn set_proximity_group_id(&mut self, proximity_group_id: String) {
        let metadata = self.node.metadata.get_or_insert_with(HashMap::new);
        metadata.insert("proximity_group_id".to_string(), proximity_group_id);
    }

    /// Gets the proximity_group_id from the node's metadata if it exists.
    pub fn get_proximity_group_id(&self) -> Option<&String> {
        self.node.metadata.as_ref()?.get("proximity_group_id")
    }

    /// Removes the proximity_group_id from the node's metadata if it exists.
    pub fn remove_proximity_group_id(&mut self) {
        if let Some(metadata) = &mut self.node.metadata {
            metadata.remove("proximity_group_id");
            if metadata.is_empty() {
                self.node.metadata = None;
            }
        }
    }

    /// Groups the given RetrievedNodes by their proximity_group_id.
    /// Can only be used with nodes returned using `ResultsMode::ProximitySearch`, else errors.
    pub fn group_proximity_results(nodes: &Vec<RetrievedNode>) -> Result<Vec<Vec<RetrievedNode>>, VRError> {
        let mut grouped_results: Vec<Vec<RetrievedNode>> = Vec::new();
        let mut current_group: Vec<RetrievedNode> = Vec::new();
        let mut current_group_id: Option<String> = None;

        for node in nodes {
            match node.get_proximity_group_id() {
                Some(group_id) => {
                    if current_group_id.as_ref() == Some(group_id) {
                        // Current node belongs to the current group
                        current_group.push(node.clone());
                    } else {
                        // Current node starts a new group
                        if !current_group.is_empty() {
                            grouped_results.push(current_group);
                            current_group = Vec::new();
                        }
                        current_group.push(node.clone());
                        current_group_id = Some(group_id.clone());
                    }
                }
                None => {
                    // If the node does not have a proximity_group_id, return an error
                    return Err(VRError::ResourceDoesNotSupportOrderedOperations(
                        node.resource_header.reference_string(),
                    ));
                }
            }
        }

        // Add the last group if it's not empty
        if !current_group.is_empty() {
            grouped_results.push(current_group);
        }

        Ok(grouped_results)
    }

    // Normalizes the scores of the retrieved nodes based on the embedding normalization factor.
    // Used during vector search to support different embedding models.
    pub fn normalize_scores(nodes: &mut Vec<RetrievedNode>) {
        // Skip normalization if every model is the same
        let first_node = nodes.first();
        if let Some(first_node) = first_node {
            let model = &first_node.resource_header.resource_embedding_model_used;

            if nodes
                .iter()
                .all(|node| node.resource_header.resource_embedding_model_used == *model)
            {
                return;
            }
        }

        for node in nodes {
            let factor = node
                .resource_header
                .resource_embedding_model_used
                .embedding_normalization_factor();

            node.score = node.score * factor;
        }
    }
}

/// Represents a Vector Resource Node which holds a unique id, one of the types of NodeContent,
/// metadata, and other internal relevant data.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ToSchema)]
pub struct Node {
    pub id: String,
    pub content: NodeContent,
    pub metadata: Option<HashMap<String, String>>,
    pub data_tag_names: Vec<String>,
    #[schema(value_type = String, format = Date)]
    pub last_written_datetime: DateTime<Utc>,
    pub merkle_hash: Option<String>,
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

        let node = Self {
            id,
            content: NodeContent::Text(text.to_string()),
            metadata,
            data_tag_names: data_tag_names.clone(),
            last_written_datetime: current_time,
            merkle_hash: None,
        };
        let _ = node._generate_merkle_hash();
        node
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
        let node = Node {
            id: id,
            content: NodeContent::Resource(vector_resource.clone()),
            metadata: metadata,
            data_tag_names: vector_resource.as_trait_object().data_tag_index().data_tag_names(),
            last_written_datetime: current_time,
            merkle_hash: None,
        };

        let _ = node._generate_merkle_hash();
        node
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
        let node = Node {
            id,
            content: NodeContent::ExternalContent(external_content.clone()),
            metadata,
            data_tag_names: vec![],
            last_written_datetime: current_time,
            merkle_hash: None,
        };

        let _ = node._generate_merkle_hash();
        node
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

        let node = Self {
            id,
            content: NodeContent::VRHeader(vr_header.clone()),
            metadata,
            data_tag_names: data_tag_names.clone(),
            last_written_datetime: current_time,
            merkle_hash: None,
        };

        let _ = node._generate_merkle_hash();
        node
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
        let node = Self {
            id,
            content,
            metadata,
            data_tag_names,
            last_written_datetime: current_time,
            merkle_hash: None,
        };

        let _ = node._generate_merkle_hash();
        node
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

    /// Updates the last_written_datetime to the provided datetime
    pub fn set_last_written(&mut self, datetime: DateTime<Utc>) {
        self.last_written_datetime = datetime;
    }

    /// Updates the last_written_datetime to the current time
    pub fn update_last_written_to_now(&mut self) {
        let current_time = ShinkaiTime::generate_time_now();
        self.set_last_written(current_time);
    }

    /// Attempts to return a reference to the text content from the Node. Errors if is different type
    pub fn get_text_content(&self) -> Result<&str, VRError> {
        match &self.content {
            NodeContent::Text(s) => Ok(s),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Attempts to return a reference to the BaseVectorResource from the Node. Errors if is different type
    pub fn get_vector_resource_content(&self) -> Result<&BaseVectorResource, VRError> {
        match &self.content {
            NodeContent::Resource(resource) => Ok(resource),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Attempts to return a reference to the ExternalContent from the Node. Errors if content is not ExternalContent
    pub fn get_external_content(&self) -> Result<&SourceReference, VRError> {
        match &self.content {
            NodeContent::ExternalContent(external_content) => Ok(external_content),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Attempts to return a reference to the VRHeader from the Node. Errors if content is not VRHeader
    pub fn get_vr_header_content(&self) -> Result<&VRHeader, VRError> {
        match &self.content {
            NodeContent::VRHeader(vr_header) => Ok(vr_header),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Attempts to return a mutable reference to the text content from the Node. Errors if is different type
    pub fn get_text_content_mut(&mut self) -> Result<&mut String, VRError> {
        match &mut self.content {
            NodeContent::Text(s) => Ok(s),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Attempts to return a mutable reference to the BaseVectorResource from the Node. Errors if is different type
    pub fn get_vector_resource_content_mut(&mut self) -> Result<&mut BaseVectorResource, VRError> {
        match &mut self.content {
            NodeContent::Resource(resource) => Ok(resource),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Attempts to return a mutable reference to the ExternalContent from the Node. Errors if content is not ExternalContent
    pub fn get_external_content_mut(&mut self) -> Result<&mut SourceReference, VRError> {
        match &mut self.content {
            NodeContent::ExternalContent(external_content) => Ok(external_content),
            _ => Err(VRError::ContentIsNonMatchingType),
        }
    }

    /// Attempts to return a mutable reference to the VRHeader from the Node. Errors if content is not VRHeader
    pub fn get_vr_header_content_mut(&mut self) -> Result<&mut VRHeader, VRError> {
        match &mut self.content {
            NodeContent::VRHeader(vr_header) => Ok(vr_header),
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

    /// Gets the Merkle hash of the Node.
    /// For VRHeader/Vector Resource nodes, uses the resource merkle_root.
    pub fn get_merkle_hash(&self) -> Result<String, VRError> {
        match &self.content {
            NodeContent::VRHeader(header) => header
                .resource_merkle_root
                .clone()
                .ok_or(VRError::MerkleRootNotFound(header.reference_string())),
            NodeContent::Resource(resource) => resource.as_trait_object().get_merkle_root(),
            _ => self
                .merkle_hash
                .clone()
                .ok_or(VRError::MerkleHashNotFoundInNode(self.id.clone())),
        }
    }

    /// Updates the Merkle hash of the Node using its current content.
    /// This should be called whenever content in the Node is updated internally.
    pub fn update_merkle_hash(&mut self) -> Result<(), VRError> {
        match &mut self.content {
            NodeContent::Resource(resource) => resource.as_trait_object_mut().update_merkle_root(),
            _ => {
                let new_hash = self._generate_merkle_hash()?;
                self.set_merkle_hash(new_hash)
            }
        }
    }

    /// Generates a Merkle hash based on the node content.
    /// For VRHeader and BaseVectorResource nodes, returns the resource merkle_root if it is available,
    /// however if root == None, then generates a new hash from the content.
    pub fn _generate_merkle_hash(&self) -> Result<String, VRError> {
        match &self.content {
            NodeContent::VRHeader(header) => match header.resource_merkle_root.clone() {
                Some(hash) => Ok(hash),
                None => Self::hash_node_content(&self.content),
            },
            NodeContent::Resource(resource) => match resource.as_trait_object().get_merkle_root() {
                Ok(hash) => Ok(hash),
                Err(_) => Self::hash_node_content(&self.content),
            },
            _ => Self::hash_node_content(&self.content),
        }
    }

    /// Creates a Blake3 hash of the NodeContent.
    fn hash_node_content(content: &NodeContent) -> Result<String, VRError> {
        let json = content.to_json()?;
        let content = json.as_bytes();
        let hash = hash(content);
        Ok(hash.to_hex().to_string())
    }

    /// Sets the Merkle hash of the Node.
    /// For Vector Resource & VRHeader nodes, sets the resource merkle_root.
    fn set_merkle_hash(&mut self, merkle_hash: String) -> Result<(), VRError> {
        match &mut self.content {
            NodeContent::VRHeader(header) => {
                header.resource_merkle_root = Some(merkle_hash);
                Ok(())
            }
            NodeContent::Resource(resource) => {
                resource.as_trait_object_mut().set_merkle_root(merkle_hash);
                Ok(())
            }
            _ => {
                self.merkle_hash = Some(merkle_hash);
                Ok(())
            }
        }
    }

    /// Returns the key used for storing the Merkle hash in the metadata.
    fn merkle_hash_metadata_key() -> &'static str {
        "merkle_hash"
    }

    /// Tries to fetch the node's datetime by reading it from the default datetime metadata key
    pub fn get_metadata_datetime(&self) -> Option<DateTime<Utc>> {
        if let Some(metadata) = &self.metadata {
            if let Some(datetime) = metadata.get(&ShinkaiFileParser::datetime_metadata_key()) {
                let datetime_option = DateTime::parse_from_rfc3339(datetime).ok();
                if let Some(dt) = datetime_option {
                    return Some(dt.into());
                }
            }
        }
        None
    }

    pub fn count_total_tokens(&self) -> u64 {
        match &self.content {
            NodeContent::Text(text) => count_tokens_from_message_llama3(text),
            NodeContent::Resource(resource) => resource.as_trait_object().count_total_tokens(),
            _ => 0,
        }
    }
}

/// Contents of a Node
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ToSchema)]
pub enum NodeContent {
    Text(String),
    Resource(BaseVectorResource),
    ExternalContent(SourceReference),
    VRHeader(VRHeader),
}

impl NodeContent {
    /// Converts the NodeContent to a JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Creates a NodeContent from a JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Struct which holds descriptive information about a given Vector Resource.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ToSchema)]
pub struct VRHeader {
    pub resource_name: String,
    pub resource_id: String,
    pub resource_base_type: VRBaseType,
    pub resource_source: VRSourceReference,
    pub resource_embedding: Option<Embedding>,
    /// ISO RFC3339 when then Vector Resource was created
    #[schema(value_type = String, format = Date)]
    pub resource_created_datetime: DateTime<Utc>,
    /// ISO RFC3339 when then Vector Resource was last written into (a node was modified)
    #[schema(value_type = String, format = Date)]
    pub resource_last_written_datetime: DateTime<Utc>,
    #[schema(value_type = String)]
    pub resource_embedding_model_used: EmbeddingModelType,
    pub resource_merkle_root: Option<String>,
    pub resource_keywords: VRKeywords,
    pub resource_distribution_info: DistributionInfo,
    /// List of data tag names matching in internal nodes
    pub data_tag_names: Vec<String>,
    /// List of metadata keys held in internal nodes
    pub metadata_index_keys: Vec<String>,
}

impl VRHeader {
    /// Create a new VRHeader
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        resource_name: &str,
        resource_id: &str,
        resource_base_type: VRBaseType,
        resource_embedding: Option<Embedding>,
        data_tag_names: Vec<String>,
        resource_source: VRSourceReference,
        resource_created_datetime: DateTime<Utc>,
        resource_last_written_datetime: DateTime<Utc>,
        metadata_index_keys: Vec<String>,
        resource_embedding_model_used: EmbeddingModelType,
        resource_merkle_root: Option<String>,
        resource_keywords: VRKeywords,
        resource_distribution_info: DistributionInfo,
    ) -> Self {
        Self {
            resource_name: resource_name.to_string(),
            resource_id: resource_id.to_string(),
            resource_base_type,
            resource_embedding: resource_embedding.clone(),
            data_tag_names: data_tag_names,
            resource_source,
            resource_created_datetime,
            resource_last_written_datetime,
            metadata_index_keys,
            resource_embedding_model_used,
            resource_merkle_root,
            resource_keywords,
            resource_distribution_info,
        }
    }

    /// Create a new VRHeader using a reference_string instead of the name/id directly
    pub fn new_with_reference_string(
        reference_string: String,
        resource_base_type: VRBaseType,
        resource_embedding: Option<Embedding>,
        data_tag_names: Vec<String>,
        resource_source: VRSourceReference,
        resource_created_datetime: DateTime<Utc>,
        resource_last_written_datetime: DateTime<Utc>,
        metadata_index_keys: Vec<String>,
        resource_embedding_model_used: EmbeddingModelType,
        resource_merkle_root: Option<String>,
        resource_keywords: VRKeywords,
        resource_distribution_info: DistributionInfo,
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
            resource_last_written_datetime,
            metadata_index_keys,
            resource_embedding_model_used,
            resource_merkle_root,
            resource_keywords,
            resource_distribution_info,
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
        let name = VRPath::clean_string(&name);
        let resource_id = VRPath::clean_string(&resource_id);
        format!("{}:::{}", name, resource_id)
    }

    /// Attempts to return the DistributionInfo datetime, if not available, returns
    /// the resource_last_written_datetime.
    pub fn get_resource_datetime_default(&self) -> DateTime<Utc> {
        if let Some(datetime) = &self.resource_distribution_info.datetime {
            datetime.clone()
        } else {
            self.resource_last_written_datetime
        }
    }

    /// Attempts to return the DistributionInfo datetime, if not available, returns
    /// the resource_created_datetime.
    pub fn get_resource_datetime_default_created(&self) -> DateTime<Utc> {
        if let Some(datetime) = &self.resource_distribution_info.datetime {
            datetime.clone()
        } else {
            self.resource_created_datetime
        }
    }
}

/// A struct which holds a Vector Resource's keywords/optional
/// keywords embedding
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct VRKeywords {
    pub keyword_list: Vec<String>,
    pub keywords_embedding: Option<KeywordEmbedding>,
}

impl VRKeywords {
    /// Creates a new instance of VRKeywords.
    pub fn new() -> Self {
        VRKeywords {
            keyword_list: Vec::new(),
            keywords_embedding: None,
        }
    }

    /// Adds a keyword to the list.
    pub fn add_keyword(&mut self, keyword: String) {
        self.keyword_list.push(keyword);
    }

    /// Removes the last keyword from the list and returns it.
    pub fn pop_keyword(&mut self) -> Option<String> {
        self.keyword_list.pop()
    }

    /// Sets the entire list of keywords.
    pub fn set_keywords(&mut self, keywords: Vec<String>) {
        self.keyword_list = keywords;
    }

    /// Sets the keyword embedding, overwriting the previous value.
    pub fn set_embedding(&mut self, embedding: Embedding, model_type: EmbeddingModelType) {
        let keyword_embedding = KeywordEmbedding::new(embedding, model_type);
        self.keywords_embedding = Some(keyword_embedding);
    }

    /// Removes the keyword embedding and returns it.
    pub fn remove_embedding(&mut self) -> Option<KeywordEmbedding> {
        self.keywords_embedding.take()
    }

    #[cfg(feature = "desktop-only")]
    /// Asynchronously regenerates and updates the keywords' embedding using the provided keywords.
    pub async fn update_keywords_embedding(&mut self, generator: &dyn EmbeddingGenerator) -> Result<(), VRError> {
        let formatted_keywords = format!("Keywords: [{}]", self.keyword_list.join(","));
        let new_embedding = generator.generate_embedding(&formatted_keywords, "KE").await?;
        self.set_embedding(new_embedding, generator.model_type());
        Ok(())
    }

    #[cfg(feature = "desktop-only")]
    /// Synchronously regenerates and updates the keywords' embedding using the provided keywords.
    pub fn update_keywords_embedding_blocking(&mut self, generator: &dyn EmbeddingGenerator) -> Result<(), VRError> {
        let formatted_keywords = format!("Keywords: [{}]", self.keyword_list.join(","));
        let new_embedding = generator.generate_embedding_blocking(&formatted_keywords, "KE")?;
        self.set_embedding(new_embedding, generator.model_type());
        Ok(())
    }
    /// Randomly replaces a specified number of keywords in `keyword_list` with the first `actual_num_to_replace` keywords from the provided list.
    pub fn random_replace_keywords(&mut self, num_to_replace: usize, replacement_keywords: Vec<String>) {
        // Calculate the actual number of keywords to replace
        let actual_num_to_replace = std::cmp::min(
            num_to_replace,
            std::cmp::min(self.keyword_list.len(), replacement_keywords.len()),
        );

        // Take the first `actual_num_to_replace` keywords from the input list
        let replacement_keywords = &replacement_keywords[..actual_num_to_replace];

        // Randomly select indices in the current keyword list to replace
        let mut rng = StdRng::from_entropy();
        let mut indices_to_replace: Vec<usize> = (0..self.keyword_list.len()).collect();
        indices_to_replace.shuffle(&mut rng);
        let indices_to_replace = &indices_to_replace[..actual_num_to_replace];

        // Perform the replacement
        for (&index, replacement_keyword) in indices_to_replace.iter().zip(replacement_keywords.iter()) {
            self.keyword_list[index] = replacement_keyword.clone();
        }
    }
}

/// Struct which holds the embedding for a Vector Resource's keywords
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct KeywordEmbedding {
    pub embedding: Embedding,
    #[schema(value_type = String)]
    pub model_used: EmbeddingModelType,
}

impl KeywordEmbedding {
    /// Creates a new instance of KeywordEmbedding.
    pub fn new(embedding: Embedding, model_used: EmbeddingModelType) -> Self {
        KeywordEmbedding { embedding, model_used }
    }

    /// Sets the embedding and model type.
    pub fn set_embedding(&mut self, embedding: Embedding, model_type: EmbeddingModelType) {
        self.embedding = embedding;
        self.model_used = model_type;
    }
}

/// A path inside of a Vector Resource to a Node which exists somewhere in the hierarchy.
/// Internally the path is made up of an ordered list of Node ids (Int-holding strings for Docs, any string for Maps).
#[derive(Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct VRPath {
    pub path_ids: Vec<String>,
}

impl VRPath {
    /// Create a new VRPath, defaulting to root `/`.
    /// Equivalent to VRPath::root().
    pub fn new() -> Self {
        Self { path_ids: vec![] }
    }

    /// Create a new VRPath at root `/`.
    /// Equivalent to VRPath::new().
    pub fn root() -> Self {
        Self::new()
    }

    /// Returns if the path is empty (aka pointing at root, `/`). Equivalent to `.is_root()`
    pub fn is_empty(&self) -> bool {
        self.path_ids.len() == 0
    }

    /// Returns if the path is  pointing at root, `/`. Equivalent to `.is_empty()`
    pub fn is_root(&self) -> bool {
        self.is_empty()
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
        self.path_ids.push(VRPath::clean_string(&id));
    }

    /// Removes an element from the end of the path_ids
    pub fn pop(&mut self) -> Option<String> {
        self.path_ids.pop()
    }

    /// Removes the first element from the path_ids and returns it as an Option.
    pub fn front_pop(&mut self) -> Option<String> {
        if self.path_ids.is_empty() {
            None
        } else {
            Some(self.path_ids.remove(0))
        }
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

    /// Returns a cloned VRPath with the last id removed from the end
    pub fn pop_cloned(&self) -> Self {
        let mut new_path = self.clone();
        new_path.pop();
        new_path
    }

    /// Returns a cloned VRPath with the first id removed from the start.
    pub fn front_pop_cloned(&self) -> Self {
        let mut new_path = self.clone();
        new_path.front_pop();
        new_path
    }

    /// Appends the path ids from `input_path` to the end of this VRPath.
    pub fn append_path(&mut self, input_path: &VRPath) {
        for path_id in &input_path.path_ids {
            self.push(path_id.clone());
        }
    }

    /// Returns a new VRPath which is a clone of self with the path ids from `input_path` appended to the end.
    pub fn append_path_cloned(&self, input_path: &VRPath) -> Self {
        let mut new_path = self.clone();
        new_path.append_path(input_path);
        new_path
    }

    /// Returns a VRPath which is the path prior to self (the "parent path").
    /// Ie. For path "/a/b/c", this will return "/a/b".
    pub fn parent_path(&self) -> Self {
        self.pop_cloned()
    }

    /// Checks if the given path is the immediate parent of self.
    pub fn is_parent_path(&self, path: &VRPath) -> bool {
        self.parent_path() == *path
    }

    /// Checks if the input path is a descendant of self.
    /// A descendant path is one that starts with the same ids as self but is longer.
    pub fn is_descendant_path(&self, path: &VRPath) -> bool {
        if path.path_ids.len() <= self.path_ids.len() {
            return false;
        }

        self.path_ids
            .iter()
            .zip(&path.path_ids)
            .all(|(self_id, path_id)| self_id == path_id)
    }

    /// Checks if self is an ancestor of the input path.
    /// An ancestor path is one that is a prefix of self but is shorter.
    pub fn is_ancestor_path(&self, path: &VRPath) -> bool {
        if self.path_ids.len() == 0 && path.path_ids.len() != 0 {
            return true;
        }

        if path.path_ids.len() >= self.path_ids.len() {
            return false;
        }

        path.path_ids
            .iter()
            .zip(&self.path_ids)
            .all(|(path_id, self_id)| path_id == self_id)
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
    /// characters which would break a VRPath, or cause issues generally for the VectorFS.
    pub fn clean_string(s: &str) -> String {
        s.replace("/", "-").replace(":", "_")
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

impl Serialize for VRPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert the VRPath into a string here
        let s = self.format_to_string();
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for VRPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize the VRPath from a string
        let s = String::deserialize(deserializer)?;
        VRPath::from_string(&s).map_err(serde::de::Error::custom)
    }
}

/// Alters default vector search behavior that modifies the result context. Each mode can be enabled separately or together.
/// Default: fill context window up to maximum tokens.
/// FillUpTo25k: fill context window up to 25k tokens.
/// MergeSiblings: add previous 3 and next 3 nodes to each found node and merge them together.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum VectorSearchMode {
    FillUpTo25k,
    MergeSiblings,
}
