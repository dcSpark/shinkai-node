use super::{BaseVectorResource, VRBaseType, VRHeader, VRKeywords, VectorResourceSearch};
use crate::data_tags::{DataTag, DataTagIndex};
use crate::embeddings::Embedding;
use crate::metadata_index::MetadataIndex;
use crate::model_type::{EmbeddingModelType, EmbeddingModelTypeString, OllamaTextEmbeddingsInference};
use crate::resource_errors::VRError;
use crate::shinkai_time::ShinkaiTime;
use crate::source::{DistributionInfo, SourceReference, VRSourceReference};
use crate::vector_resource::{Node, NodeContent, OrderedVectorResource, VRPath, VectorResource, VectorResourceCore};

use chrono::{DateTime, Utc};
use serde_json;
use std::any::Any;
use std::collections::HashMap;
use utoipa::ToSchema;

/// A VectorResource which uses an internal numbered/ordered list data model,  
/// thus providing an ideal interface for document-like content such as PDFs,
/// epubs, web content, written works, and more.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ToSchema)]
pub struct DocumentVectorResource {
    pub name: String,
    pub description: Option<String>,
    pub source: VRSourceReference,
    pub resource_id: String,
    pub resource_embedding: Embedding,
    #[schema(value_type = String)]
    pub embedding_model_used_string: EmbeddingModelTypeString,
    pub resource_base_type: VRBaseType,
    pub embeddings: Vec<Embedding>,
    pub node_count: u64,
    pub nodes: Vec<Node>,
    pub data_tag_index: DataTagIndex,
    #[schema(value_type = String, format = Date)]
    pub created_datetime: DateTime<Utc>,
    #[schema(value_type = String, format = Date)]
    pub last_written_datetime: DateTime<Utc>,
    pub metadata_index: MetadataIndex,
    pub merkle_root: Option<String>,
    pub keywords: VRKeywords,
    pub distribution_info: DistributionInfo,
}
impl VectorResource for DocumentVectorResource {}
impl VectorResourceSearch for DocumentVectorResource {}

impl OrderedVectorResource for DocumentVectorResource {
    /// Id of the first node held internally
    fn first_node_id(&self) -> Option<String> {
        if self.node_count > 0 {
            Some("1".to_string())
        } else {
            None
        }
    }

    /// Id of the last node held internally
    fn last_node_id(&self) -> Option<String> {
        if self.node_count > 0 {
            Some(self.node_count.to_string())
        } else {
            None
        }
    }

    /// Retrieve the first node held internally
    fn get_first_node(&self) -> Option<Node> {
        self.nodes.first().cloned()
    }

    /// Retrieve the second node held internally
    fn get_second_node(&self) -> Option<Node> {
        if self.nodes.len() >= 2 {
            Some(self.nodes[1].clone())
        } else {
            None
        }
    }

    /// Retrieve the third node held internally
    fn get_third_node(&self) -> Option<Node> {
        if self.nodes.len() >= 3 {
            Some(self.nodes[2].clone())
        } else {
            None
        }
    }

    /// Retrieve the last node held internally
    fn get_last_node(&self) -> Option<Node> {
        self.nodes.last().cloned()
    }

    /// Id to be used when pushing a new node
    fn new_push_node_id(&self) -> String {
        (self.node_count + 1).to_string()
    }

    /// Takes the first N nodes held internally and returns them as references
    fn take(&self, n: usize) -> Vec<&Node> {
        self.nodes.iter().take(n).collect()
    }

    /// Takes the first N nodes held internally and returns cloned copies of them
    fn take_cloned(&self, n: usize) -> Vec<Node> {
        self.nodes.iter().take(n).cloned().collect()
    }

    /// Attempts to fetch a node (using the provided id) and proximity_window before/after, at root depth.
    /// Returns the nodes in its default ordering as determined by the internal VR struct.
    fn get_node_and_embedding_proximity(
        &self,
        id: String,
        proximity_window: u64,
    ) -> Result<Vec<(Node, Embedding)>, VRError> {
        let id = id.parse::<u64>().map_err(|_| VRError::InvalidNodeId(id.to_string()))?;

        // Check if id is within valid range
        if id == 0 || id > self.node_count {
            return Err(VRError::InvalidNodeId(id.to_string()));
        }

        // Calculate Start/End ids
        let start_id = if id > proximity_window {
            id - proximity_window
        } else {
            1
        };
        let end_id = if let Some(potential_end_id) = id.checked_add(proximity_window) {
            potential_end_id.min(self.node_count)
        } else {
            self.node_count
        };

        // Acquire all nodes
        let mut nodes_and_embeddings = Vec::new();
        for id in start_id..=end_id {
            if let Ok(node) = self.get_root_node(id.to_string()) {
                if let Ok(embedding) = self.get_root_embedding(id.to_string()) {
                    nodes_and_embeddings.push((node, embedding));
                }
            }
        }

        Ok(nodes_and_embeddings)
    }
}

impl VectorResourceCore for DocumentVectorResource {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    /// Attempts to cast the VectorResource into an OrderedVectorResource. Fails if
    /// the struct does not support the OrderedVectorResource trait.
    fn as_ordered_vector_resource(&self) -> Result<&dyn OrderedVectorResource, VRError> {
        Ok(self as &dyn OrderedVectorResource)
    }

    /// Attempts to cast the VectorResource into an mut OrderedVectorResource. Fails if
    /// the struct does not support the OrderedVectorResource trait.
    fn as_ordered_vector_resource_mut(&mut self) -> Result<&mut dyn OrderedVectorResource, VRError> {
        Ok(self as &mut dyn OrderedVectorResource)
    }

    /// Returns the merkle root of the Vector Resource (if it is not None).
    fn get_merkle_root(&self) -> Result<String, VRError> {
        self.merkle_root
            .clone()
            .ok_or(VRError::MerkleRootNotFound(self.reference_string()))
    }

    /// Sets the merkle root of the Vector Resource, errors if provided hash is not a valid Blake3 hash.
    fn set_merkle_root(&mut self, merkle_hash: String) -> Result<(), VRError> {
        // Validate the hash format
        if blake3::Hash::from_hex(&merkle_hash).is_ok() {
            self.merkle_root = Some(merkle_hash);
            Ok(())
        } else {
            Err(VRError::InvalidMerkleHashString(merkle_hash))
        }
    }

    /// RFC3339 Datetime when then Vector Resource was created
    fn created_datetime(&self) -> DateTime<Utc> {
        self.created_datetime.clone()
    }
    /// RFC3339 Datetime when then Vector Resource was last written
    fn last_written_datetime(&self) -> DateTime<Utc> {
        self.last_written_datetime.clone()
    }
    /// Set a RFC Datetime of when then Vector Resource was last written
    fn set_last_written_datetime(&mut self, datetime: DateTime<Utc>) {
        self.last_written_datetime = datetime;
    }

    fn data_tag_index(&self) -> &DataTagIndex {
        &self.data_tag_index
    }

    fn metadata_index(&self) -> &MetadataIndex {
        &self.metadata_index
    }

    fn distribution_info(&self) -> &DistributionInfo {
        &self.distribution_info
    }

    fn set_distribution_info(&mut self, dist_info: DistributionInfo) {
        self.distribution_info = dist_info;
    }

    fn embedding_model_used_string(&self) -> EmbeddingModelTypeString {
        self.embedding_model_used_string.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn source(&self) -> VRSourceReference {
        self.source.clone()
    }

    fn keywords(&self) -> &VRKeywords {
        &self.keywords
    }

    fn keywords_mut(&mut self) -> &mut VRKeywords {
        &mut self.keywords
    }

    fn set_name(&mut self, new_name: String) {
        self.name = new_name;
    }

    fn set_description(&mut self, new_description: Option<String>) {
        self.description = new_description;
    }

    fn set_source(&mut self, new_source: VRSourceReference) {
        self.source = new_source;
    }

    fn resource_id(&self) -> &str {
        &self.resource_id
    }

    fn resource_embedding(&self) -> &Embedding {
        &self.resource_embedding
    }

    fn resource_base_type(&self) -> VRBaseType {
        self.resource_base_type.clone()
    }

    fn get_root_embeddings(&self) -> Vec<Embedding> {
        self.embeddings.clone()
    }

    fn to_json(&self) -> Result<String, VRError> {
        Ok(serde_json::to_string(self)?)
    }

    fn to_json_value(&self) -> Result<serde_json::Value, VRError> {
        Ok(serde_json::to_value(self)?)
    }

    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType) {
        self.update_last_written_to_now();
        self.embedding_model_used_string = model_type.to_string();
    }

    fn set_resource_embedding(&mut self, embedding: Embedding) {
        self.update_last_written_to_now();
        self.resource_embedding = embedding;
    }

    fn node_count(&self) -> u64 {
        self.node_count
    }

    fn set_resource_id(&mut self, id: String) {
        self.update_last_written_to_now();
        self.resource_id = id;
    }

    fn get_data_tag_index(&self) -> &DataTagIndex {
        &self.data_tag_index
    }

    fn set_data_tag_index(&mut self, data_tag_index: DataTagIndex) {
        self.data_tag_index = data_tag_index;
    }

    fn get_metadata_index(&self) -> &MetadataIndex {
        &self.metadata_index
    }

    fn set_metadata_index(&mut self, metadata_index: MetadataIndex) {
        self.metadata_index = metadata_index;
    }

    /// Efficiently retrieves a Node's matching embedding given its id by fetching it via index.
    fn get_root_embedding(&self, id: String) -> Result<Embedding, VRError> {
        let id = id.parse::<u64>().map_err(|_| VRError::InvalidNodeId(id.to_string()))?;
        if id == 0 || id > self.node_count {
            return Err(VRError::InvalidNodeId(id.to_string()));
        }
        let index = id.checked_sub(1).ok_or(VRError::InvalidNodeId(id.to_string()))? as usize;
        Ok(self.embeddings[index].clone())
    }

    /// Efficiently retrieves a node given its id by fetching it via index.
    fn get_root_node(&self, id: String) -> Result<Node, VRError> {
        let id = id.parse::<u64>().map_err(|_| VRError::InvalidNodeId(id.to_string()))?;
        if id == 0 || id > self.node_count {
            return Err(VRError::InvalidNodeId(id.to_string()));
        }
        let index = id.checked_sub(1).ok_or(VRError::InvalidNodeId(id.to_string()))? as usize;
        self.nodes
            .get(index)
            .cloned()
            .ok_or(VRError::InvalidNodeId(id.to_string()))
    }

    /// Returns all nodes in the DocumentVectorResource
    fn get_root_nodes(&self) -> Vec<Node> {
        self.nodes.iter().cloned().collect()
    }

    /// Returns all nodes in the DocumentVectorResource as references to nodes
    fn get_root_nodes_ref(&self) -> Vec<&Node> {
        self.nodes.iter().collect()
    }

    /// Returns all embeddings in the DocumentVectorResource as references to embeddings
    fn get_root_embeddings_ref(&self) -> Vec<&Embedding> {
        self.embeddings.iter().collect()
    }

    /// Insert a Node/Embedding into the VR using the provided id (root level depth). Overwrites existing data.
    fn insert_node_dt_specified(
        &mut self,
        id: String,
        node: Node,
        embedding: Embedding,
        new_written_datetime: Option<DateTime<Utc>>,
        update_merkle_hashes: bool,
    ) -> Result<(), VRError> {
        let current_datetime = if let Some(dt) = new_written_datetime {
            dt
        } else {
            ShinkaiTime::generate_time_now()
        };

        // Id + index logic
        let mut integer_id = id.parse::<u64>().map_err(|_| VRError::InvalidNodeId(id.to_string()))?;
        integer_id = if integer_id == 0 { 1 } else { integer_id };
        if integer_id > self.node_count + 1 {
            // We do +1 since we resize the vectors explicitly in this method
            return Err(VRError::InvalidNodeId(id.to_string()));
        }
        let index = if integer_id == 0 { 0 } else { (integer_id - 1) as usize };

        // Resize the vectors to accommodate the new node and embedding
        let node_default = self
            .nodes
            .last()
            .cloned()
            .unwrap_or_else(|| Node::new_text("".to_string(), "".to_string(), None, &vec![]));
        let embedding_default = self.embeddings.last().cloned().unwrap_or_else(Embedding::new_empty);
        self.nodes
            .resize_with((self.node_count + 1) as usize, || node_default.clone());
        self.embeddings
            .resize_with((self.node_count + 1) as usize, || embedding_default.clone());

        // Shift all nodes and embeddings one index up
        for i in (index..self.node_count as usize).rev() {
            self.nodes[i + 1] = self.nodes[i].clone();
            self.embeddings[i + 1] = self.embeddings[i].clone();
            self.nodes[i + 1].id = format!("{}", i + 2);
            self.embeddings[i + 1].set_id_with_integer((i + 2) as u64);
        }

        // Update ids to match supplied id
        let mut updated_node = node;
        updated_node.id = id.to_string();
        updated_node.set_last_written(current_datetime);
        let mut embedding = embedding.clone();
        embedding.set_id(id.to_string());
        // Update the node merkle hash if the VR is merkelized. This guarantees merkle hash is always up to date.
        if self.is_merkelized() && update_merkle_hashes {
            updated_node.update_merkle_hash()?;
        }

        // Insert the new node and embedding
        self.nodes[index] = updated_node.clone();
        self.embeddings[index] = embedding;

        self.data_tag_index.add_node(&updated_node);
        self.metadata_index.add_node(&updated_node);

        self.node_count += 1;
        self.set_last_written_datetime(current_datetime);

        // Regenerate the Vector Resource's merkle root after updating its contents
        if self.is_merkelized() && update_merkle_hashes {
            self.update_merkle_root()?;
        }

        Ok(())
    }

    /// Replace a Node/Embedding in the VR using the provided id (root level depth)
    fn replace_node_dt_specified(
        &mut self,
        id: String,
        node: Node,
        embedding: Embedding,
        new_written_datetime: Option<DateTime<Utc>>,
        update_merkle_hashes: bool,
    ) -> Result<(Node, Embedding), VRError> {
        let current_datetime = if let Some(dt) = new_written_datetime {
            dt
        } else {
            ShinkaiTime::generate_time_now()
        };

        // Id + index logic
        let mut integer_id = id.parse::<u64>().map_err(|_| VRError::InvalidNodeId(id.to_string()))?;
        integer_id = if integer_id == 0 { 1 } else { integer_id };
        if integer_id > self.node_count {
            return Err(VRError::InvalidNodeId(id.to_string()));
        }
        let index = if integer_id == 0 { 0 } else { (integer_id - 1) as usize };

        // Update ids to match supplied id
        let mut new_node = node;
        new_node.id = id.to_string();
        let mut embedding = embedding.clone();
        embedding.set_id(id.to_string());
        new_node.set_last_written(current_datetime);
        // Update the node merkle hash if the VR is merkelized. This guarantees merkle hash is always up to date.
        if self.is_merkelized() && update_merkle_hashes {
            new_node.update_merkle_hash()?;
        }

        // Replace the old node and fetch old embedding
        let old_node = std::mem::replace(&mut self.nodes[index], new_node.clone());
        let old_embedding = self.get_root_embedding(id.clone())?;

        // Replacing the embedding
        self.embeddings[index] = embedding;
        self.set_last_written_datetime(current_datetime);

        // Then deletion of old node from indexes and addition of new node
        if old_node.data_tag_names != new_node.data_tag_names {
            self.data_tag_index.remove_node(&old_node);
            self.data_tag_index.add_node(&new_node);
        }
        if old_node.metadata_keys() != new_node.metadata_keys() {
            self.metadata_index.remove_node(&old_node);
            self.metadata_index.add_node(&new_node);
        }

        // Regenerate the Vector Resource's merkle root after updating its contents
        if self.is_merkelized() && update_merkle_hashes {
            self.update_merkle_root()?;
        }

        Ok((old_node, old_embedding))
    }

    /// Remove a Node/Embedding in the VR using the provided id (root level depth)
    fn remove_node_dt_specified(
        &mut self,
        id: String,
        new_written_datetime: Option<DateTime<Utc>>,
        update_merkle_hashes: bool,
    ) -> Result<(Node, Embedding), VRError> {
        let current_datetime = if let Some(dt) = new_written_datetime {
            dt
        } else {
            ShinkaiTime::generate_time_now()
        };
        // Id + index logic
        let mut integer_id = id.parse::<u64>().map_err(|_| VRError::InvalidNodeId(id.to_string()))?;
        integer_id = if integer_id == 0 { 1 } else { integer_id };
        if integer_id > self.node_count {
            return Err(VRError::InvalidNodeId(id.to_string()));
        }
        let results = self.remove_node_with_integer(integer_id);
        self.set_last_written_datetime(current_datetime);

        // Regenerate the Vector Resource's merkle root after updating its contents
        if self.is_merkelized() && update_merkle_hashes {
            self.update_merkle_root()?;
        }

        results
    }

    /// Removes all Nodes/Embeddings at the root level depth.
    fn remove_root_nodes_dt_specified(
        &mut self,
        new_written_datetime: Option<DateTime<Utc>>,
        update_merkle_hashes: bool,
    ) -> Result<Vec<(Node, Embedding)>, VRError> {
        let mut ids: Vec<u64> = (0..self.node_count()).collect();
        ids.reverse();
        let mut results = vec![];

        for id in ids {
            let result =
                self.remove_node_dt_specified(id.to_string(), new_written_datetime.clone(), update_merkle_hashes)?;
            results.push(result);
        }

        Ok(results)
    }

    fn count_total_tokens(&self) -> u64 {
        self.nodes.iter().map(|node| node.count_total_tokens()).sum()
    }
}

impl DocumentVectorResource {
    /// Create a new MapVectorResource
    /// If is_merkelized == true, then this VR will automatically generate a merkle root
    /// & merkle hashes for all nodes.
    pub fn new(
        name: &str,
        desc: Option<&str>,
        source: VRSourceReference,
        resource_embedding: Embedding,
        embeddings: Vec<Embedding>,
        nodes: Vec<Node>,
        embedding_model_used: EmbeddingModelType,
        is_merkelized: bool,
        distribution_info: DistributionInfo,
    ) -> Self {
        let current_time = ShinkaiTime::generate_time_now();
        let merkle_root = if is_merkelized {
            let empty_hash = blake3::Hasher::new().finalize();
            Some(empty_hash.to_hex().to_string())
        } else {
            None
        };

        let mut resource = DocumentVectorResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source,
            resource_id: String::from("default"),
            resource_embedding,
            embeddings,
            node_count: nodes.len() as u64,
            nodes: nodes,
            embedding_model_used_string: embedding_model_used.to_string(),
            resource_base_type: VRBaseType::Document,
            data_tag_index: DataTagIndex::new(),
            created_datetime: current_time.clone(),
            last_written_datetime: current_time,
            metadata_index: MetadataIndex::new(),
            merkle_root,
            keywords: VRKeywords::new(),
            distribution_info,
        };

        // Generate a unique resource_id
        resource.generate_and_update_resource_id();
        resource
    }

    /// Initializes an empty `DocumentVectorResource` with empty defaults. Of note, make sure EmbeddingModelType
    /// is correct before adding any nodes into the VR.
    pub fn new_empty(name: &str, desc: Option<&str>, source: VRSourceReference, is_merkelized: bool) -> Self {
        DocumentVectorResource::new(
            name,
            desc,
            source,
            Embedding::new(&String::new(), vec![]),
            Vec::new(),
            Vec::new(),
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M),
            is_merkelized,
            DistributionInfo::new_empty(),
        )
    }

    /// Appends a new node (with a BaseVectorResource) to the document at the root depth.
    pub fn append_vector_resource_node(
        &mut self,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        let path = ShinkaiPath::from_string("/")?;
        self.append_vector_resource_node_at_path(path, resource, metadata, embedding)
    }

    /// Appends a new node (with a BaseVectorResource) at a specific path in the document.
    pub fn append_vector_resource_node_at_path(
        &mut self,
        path: VRPath,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        let tag_names = resource.as_trait_object().data_tag_index().data_tag_names();
        let node_content = NodeContent::Resource(resource);
        let new_node = Node::from_node_content("".to_string(), node_content, metadata, tag_names);
        self.append_node_at_path(path, new_node, embedding, true)
    }

    /// Appends a new node (with a BaseVectorResource) to the document at the root depth.
    /// Automatically uses the existing resource embedding.
    pub fn append_vector_resource_node_auto(
        &mut self,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<(), VRError> {
        let embedding = resource.as_trait_object().resource_embedding().clone();
        self.append_vector_resource_node(resource, metadata, embedding.clone())
    }

    /// Appends a new text node to the document at the root depth.
    pub fn append_text_node(
        &mut self,
        text: &str,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the data with
    ) -> Result<(), VRError> {
        let path = ShinkaiPath::from_string("/")?;
        self.append_text_node_at_path(path, text, metadata, embedding, parsing_tags)
    }

    /// Appends a new text node at a specific path in the document.
    pub fn append_text_node_at_path(
        &mut self,
        path: VRPath,
        text: &str,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the data with
    ) -> Result<(), VRError> {
        let validated_data_tags = DataTag::validate_tag_list(text, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        let node_content = NodeContent::Text(text.to_string());
        let new_node = Node::from_node_content("".to_string(), node_content, metadata, data_tag_names);
        self.append_node_at_path(path, new_node, embedding, true)
    }

    /// Appends a new node (with ExternalContent) to the document at root path.
    pub fn append_external_content_node(
        &mut self,
        external_content: SourceReference,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        let path = ShinkaiPath::from_string("/")?;
        self.append_external_content_node_at_path(path, external_content, metadata, embedding)
    }

    /// Appends a new node (with ExternalContent) at a specific path in the document.
    pub fn append_external_content_node_at_path(
        &mut self,
        path: VRPath,
        external_content: SourceReference,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        let node_content = NodeContent::ExternalContent(external_content);
        let new_node = Node::from_node_content("".to_string(), node_content, metadata, vec![]);
        self.append_node_at_path(path, new_node, embedding, true)
    }

    /// Appends a new node (with VRHeader) to the document at root depth.
    pub fn append_vr_header_node(
        &mut self,
        vr_header: VRHeader,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        let path = ShinkaiPath::from_string("/")?;
        self.append_vr_header_node_at_path(path, vr_header, metadata, embedding)
    }

    /// Appends a new node (with VRHeader) at a specific path in the document.
    pub fn append_vr_header_node_at_path(
        &mut self,
        path: VRPath,
        vr_header: VRHeader,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        let data_tag_names = vr_header.data_tag_names.clone();
        let node_content = NodeContent::VRHeader(vr_header);
        let new_node = Node::from_node_content("".to_string(), node_content, metadata, data_tag_names);
        self.append_node_at_path(path, new_node, embedding, true)
    }

    /// Replaces an existing node and associated embedding in the Document resource at root depth,
    /// with a BaseVectorResource in the new Node.
    pub fn replace_with_vector_resource_node(
        &mut self,
        id: u64,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let path = ShinkaiPath::from_string(&("/".to_string() + &id.to_string()))?;
        self.replace_with_vector_resource_node_at_path(path, new_resource, new_metadata, embedding)
    }

    /// Replaces an existing node and associated embedding at a specific path in the Document resource
    /// with a BaseVectorResource in the new Node.
    pub fn replace_with_vector_resource_node_at_path(
        &mut self,
        path: VRPath,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let tag_names = new_resource.as_trait_object().data_tag_index().data_tag_names();
        let node_content = NodeContent::Resource(new_resource);
        let new_node = Node::from_node_content("".to_string(), node_content, new_metadata, tag_names);
        self.replace_node_at_path(path, new_node, embedding, true)
    }

    /// Replaces an existing node & associated embedding at the root depth,
    /// with a text node.
    pub fn replace_with_text_node(
        &mut self,
        id: u64,
        new_text: &str,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: Vec<DataTag>, // List of datatags you want to parse the new data with.
    ) -> Result<(Node, Embedding), VRError> {
        let path = ShinkaiPath::from_string(&("/".to_string() + &id.to_string()))?;
        self.replace_with_text_node_at_path(path, new_text, new_metadata, embedding, parsing_tags)
    }

    /// Replaces an existing node & associated embedding at a specific path in the Document resource
    /// with a text node.
    pub fn replace_with_text_node_at_path(
        &mut self,
        path: VRPath,
        new_text: &str,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: Vec<DataTag>, // List of datatags you want to parse the new data with.
    ) -> Result<(Node, Embedding), VRError> {
        // Validate which tags will be saved with the new data
        let validated_data_tags = DataTag::validate_tag_list(&new_text, &parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        let node_content = NodeContent::Text(new_text.to_string());
        let new_node = Node::from_node_content("".to_string(), node_content, new_metadata, data_tag_names);
        self.replace_node_at_path(path, new_node, embedding, true)
    }

    /// Replaces an existing node & associated embedding with a new ExternalContent node at root depth.
    pub fn replace_with_external_content_node(
        &mut self,
        id: u64,
        new_external_content: SourceReference,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let path = ShinkaiPath::from_string(&("/".to_string() + &id.to_string()))?;
        self.replace_with_external_content_node_at_path(path, new_external_content, new_metadata, embedding)
    }

    /// Replaces an existing node & associated embedding with a new ExternalContent node at a specific path.
    pub fn replace_with_external_content_node_at_path(
        &mut self,
        path: VRPath,
        new_external_content: SourceReference,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let node_content = NodeContent::ExternalContent(new_external_content);
        let new_node = Node::from_node_content("".to_string(), node_content, new_metadata, vec![]);
        self.replace_node_at_path(path, new_node, embedding, true)
    }

    /// Replaces an existing node & associated embedding with a new VRHeader node at root depth.
    pub fn replace_with_vr_header_node(
        &mut self,
        id: u64,
        new_vr_header: VRHeader,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let path = ShinkaiPath::from_string(&("/".to_string() + &id.to_string()))?;
        self.replace_with_vr_header_node_at_path(path, new_vr_header, new_metadata, embedding)
    }

    /// Replaces an existing node & associated embedding with a new VRHeader node at a specific path.
    pub fn replace_with_vr_header_node_at_path(
        &mut self,
        path: VRPath,
        new_vr_header: VRHeader,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let data_tag_names = new_vr_header.data_tag_names.clone();
        let node_content = NodeContent::VRHeader(new_vr_header);
        let new_node = Node::from_node_content("".to_string(), node_content, new_metadata, data_tag_names);
        self.replace_node_at_path(path, new_node, embedding, true)
    }

    /// Pops and returns the last node and associated embedding.
    pub fn pop_node(&mut self) -> Result<(Node, Embedding), VRError> {
        let path = ShinkaiPath::from_string("/")?;
        self.pop_node_at_path(path, true)
    }

    /// Deletes a node and associated embedding from the resource.
    pub fn remove_node_with_integer(&mut self, id: u64) -> Result<(Node, Embedding), VRError> {
        // Remove the node + adjust remaining node ids
        let deleted_node = self._remove_root_node(id)?;
        self.data_tag_index.remove_node(&deleted_node);
        self.metadata_index.remove_node(&deleted_node);

        // Remove the embedding
        let index = if id == 0 { 0 } else { (id - 1) as usize };
        let deleted_embedding = self.embeddings.remove(index);

        // Adjust the ids of the remaining embeddings
        for i in index..self.embeddings.len() {
            self.embeddings[i].set_id_with_integer((i + 1) as u64);
        }

        Ok((deleted_node, deleted_embedding))
    }

    /// Internal node deletion
    fn _remove_root_node(&mut self, id: u64) -> Result<Node, VRError> {
        if id > self.node_count {
            return Err(VRError::InvalidNodeId(id.to_string()));
        }
        let index = if id == 0 { 0 } else { (id - 1) as usize };
        let removed_node = self.nodes.remove(index);
        self.node_count -= 1;
        for node in self.nodes.iter_mut().skip(index) {
            let node_id: u64 = node.id.parse().unwrap();
            node.id = format!("{}", node_id - 1);
        }
        self.update_last_written_to_now();
        Ok(removed_node)
    }

    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn set_resource_id(&mut self, resource_id: String) {
        self.resource_id = resource_id;
        self.update_last_written_to_now();
    }
}
