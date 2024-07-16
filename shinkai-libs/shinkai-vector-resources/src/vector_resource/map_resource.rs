use super::{VRKeywords, VectorResourceSearch};
use crate::data_tags::{DataTag, DataTagIndex};
use crate::embeddings::Embedding;
use crate::metadata_index::MetadataIndex;
use crate::model_type::{EmbeddingModelType, EmbeddingModelTypeString, OllamaTextEmbeddingsInference};
use crate::resource_errors::VRError;
use crate::shinkai_time::ShinkaiTime;
use crate::source::{DistributionInfo, SourceReference, VRSourceReference};
use crate::vector_resource::base_vector_resources::{BaseVectorResource, VRBaseType};
use crate::vector_resource::vector_search_traversal::VRHeader;
use crate::vector_resource::{Node, NodeContent, OrderedVectorResource, VRPath, VectorResource, VectorResourceCore};
use chrono::{DateTime, Utc};
use serde_json;
use std::any::Any;
use std::collections::HashMap;

/// A VectorResource which uses a HashMap data model, thus providing a
/// native key-value interface. Ideal for use cases such as field-based data sources, classical DBs,
/// constantly-updating data streams, or any unordered/mutating source data.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapVectorResource {
    name: String,
    description: Option<String>,
    source: VRSourceReference,
    resource_id: String,
    resource_embedding: Embedding,
    resource_base_type: VRBaseType,
    embedding_model_used_string: EmbeddingModelTypeString,
    embeddings: HashMap<String, Embedding>,
    node_count: u64,
    nodes: HashMap<String, Node>,
    data_tag_index: DataTagIndex,
    created_datetime: DateTime<Utc>,
    last_written_datetime: DateTime<Utc>,
    metadata_index: MetadataIndex,
    merkle_root: Option<String>,
    keywords: VRKeywords,
    distribution_info: DistributionInfo,
}
impl VectorResource for MapVectorResource {}
impl VectorResourceSearch for MapVectorResource {}

impl VectorResourceCore for MapVectorResource {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    /// OrderedVectorResource trait not supported. Simply returns error .
    fn as_ordered_vector_resource(&self) -> Result<&dyn OrderedVectorResource, VRError> {
        Err(VRError::ResourceDoesNotSupportOrderedOperations(
            self.resource_base_type().to_str().to_string(),
        ))
    }

    /// OrderedVectorResource trait not supported. Simply returns error .
    fn as_ordered_vector_resource_mut(&mut self) -> Result<&mut dyn OrderedVectorResource, VRError> {
        Err(VRError::ResourceDoesNotSupportOrderedOperations(
            self.resource_base_type().to_str().to_string(),
        ))
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
        self.created_datetime
    }
    /// RFC3339 Datetime when then Vector Resource was last written
    fn last_written_datetime(&self) -> DateTime<Utc> {
        self.last_written_datetime
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
        self.embedding_model_used_string.to_string()
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
        self.embeddings.values().cloned().collect()
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

    /// Retrieves a node's embedding given its key (id)
    fn get_root_embedding(&self, key: String) -> Result<Embedding, VRError> {
        let key = VRPath::clean_string(&key);
        Ok(self
            .embeddings
            .get(&key)
            .ok_or(VRError::InvalidNodeId(key.to_string()))?
            .clone())
    }

    /// Retrieves a node given its key (id)
    fn get_root_node(&self, key: String) -> Result<Node, VRError> {
        let key = VRPath::clean_string(&key);
        self.nodes
            .get(&key)
            .cloned()
            .ok_or(VRError::InvalidNodeId(key.to_string()))
    }

    /// Returns all nodes in the MapVectorResource
    fn get_root_nodes(&self) -> Vec<Node> {
        self.nodes.values().cloned().collect()
    }

    /// Returns all nodes in the DocumentVectorResource as references to nodes
    fn get_root_nodes_ref(&self) -> Vec<&Node> {
        self.nodes.iter().map(|(_, node)| node).collect()
    }

    /// Returns all embeddings in the DocumentVectorResource as references to embeddings
    fn get_root_embeddings_ref(&self) -> Vec<&Embedding> {
        self.embeddings.iter().map(|(_, embedding)| embedding).collect()
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

        let id = VRPath::clean_string(&id);
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

        // Insert node/embeddings
        self._insert_root_node(updated_node.clone());
        self.embeddings.insert(updated_node.id.clone(), embedding);

        // Update indices
        self.data_tag_index.add_node(&updated_node);
        self.metadata_index.add_node(&updated_node);

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
        let id = VRPath::clean_string(&id);
        let current_datetime = if let Some(dt) = new_written_datetime {
            dt
        } else {
            ShinkaiTime::generate_time_now()
        };

        // Update new_node id/last written
        let mut new_node = node;
        new_node.id = id.clone();
        new_node.set_last_written(current_datetime);
        // Update the node merkle hash if the VR is merkelized. This guarantees merkle hash is always up to date.
        if self.is_merkelized() && update_merkle_hashes {
            new_node.update_merkle_hash()?;
        }

        // Replace old node, and get old embedding
        let old_node = self
            .nodes
            .insert(id.to_string(), new_node.clone())
            .ok_or(VRError::InvalidNodeId(id.to_string()))?;
        let old_embedding = self.get_root_embedding(id.clone())?;

        // Then deletion of old node from indexes and addition of new node
        if old_node.data_tag_names != new_node.data_tag_names {
            self.data_tag_index.remove_node(&old_node);
            self.data_tag_index.add_node(&new_node);
        }
        if old_node.metadata_keys() != new_node.metadata_keys() {
            self.metadata_index.remove_node(&old_node);
            self.metadata_index.add_node(&new_node);
        }

        // Finally replacing the embedding
        let mut embedding = embedding.clone();
        embedding.set_id(id.to_string());
        self.embeddings.insert(id.to_string(), embedding);
        self.set_last_written_datetime(current_datetime);

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

        let id = VRPath::clean_string(&id);
        let results = self.remove_root_node(&id);
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
        let ids: Vec<String> = self.nodes.keys().cloned().collect();
        let mut results = vec![];

        for id in ids {
            let result = self.remove_node_dt_specified(id.to_string(), new_written_datetime, update_merkle_hashes)?;
            results.push(result);
        }

        Ok(results)
    }
}

impl MapVectorResource {
    /// Create a new MapVectorResource.
    /// If is_merkelized == true, then this VR will automatically generate a merkle root
    /// & merkle hashes for all nodes.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &str,
        desc: Option<&str>,
        source: VRSourceReference,
        resource_embedding: Embedding,
        embeddings: HashMap<String, Embedding>,
        nodes: HashMap<String, Node>,
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

        let mut resource = MapVectorResource {
            name: String::from(name),
            description: desc.map(String::from),
            source,
            resource_id: String::from("default"),
            resource_embedding,
            embeddings,
            node_count: nodes.len() as u64,
            resource_base_type: VRBaseType::Map,
            nodes,
            embedding_model_used_string: embedding_model_used.to_string(),
            data_tag_index: DataTagIndex::new(),
            created_datetime: current_time,
            last_written_datetime: current_time,
            metadata_index: MetadataIndex::new(),
            merkle_root,
            keywords: VRKeywords::new(),
            distribution_info,
        };
        // Generate a unique resource_id:
        resource.generate_and_update_resource_id();
        resource
    }

    /// Initializes an empty `MapVectorResource` with empty defaults. Of note, make sure EmbeddingModelType
    /// is correct before adding any nodes into the VR.
    pub fn new_empty(name: &str, desc: Option<&str>, source: VRSourceReference, is_merkelized: bool) -> Self {
        MapVectorResource::new(
            name,
            desc,
            source,
            Embedding::new(&String::new(), vec![]),
            HashMap::new(),
            HashMap::new(),
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M),
            is_merkelized,
            DistributionInfo::new_empty(),
        )
    }

    /// Inserts a new node (with a BaseVectorResource) with the provided embedding
    /// at the specified key in the Map resource root.
    pub fn insert_vector_resource_node(
        &mut self,
        key: &str,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        self.insert_vector_resource_node_at_path(VRPath::new(), key, resource, metadata, embedding)
    }

    /// Inserts a new node (with a BaseVectorResource) into the specified parent_path using the provided key.
    pub fn insert_vector_resource_node_at_path(
        &mut self,
        parent_path: VRPath,
        key: &str,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        let tag_names = resource.as_trait_object().data_tag_index().data_tag_names();
        let node_content = NodeContent::Resource(resource.clone());
        let new_internal_node = Node::from_node_content(key.to_string(), node_content, metadata.clone(), tag_names);

        self.insert_node_at_path(parent_path, key.to_string(), new_internal_node, embedding, true)
    }

    /// Inserts a new node (with a BaseVectorResource) using the resource's included embedding
    /// at the specified key in the Map resource root.
    pub fn insert_vector_resource_node_auto(
        &mut self,
        key: &str,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<(), VRError> {
        let embedding = resource.as_trait_object().resource_embedding().clone();
        self.insert_vector_resource_node(key, resource, metadata, embedding)
    }

    /// Inserts a new text node and associated embedding at the specified key in the Map resource root.
    pub fn insert_text_node(
        &mut self,
        key: String,
        text_value: String,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the data with
    ) -> Result<(), VRError> {
        self.insert_text_node_at_path(VRPath::new(), key, text_value, metadata, embedding, parsing_tags)
    }

    /// Inserts a new text node and associated embedding into the specified parent_path using the provided key.
    pub fn insert_text_node_at_path(
        &mut self,
        parent_path: VRPath,
        key: String,
        text_value: String,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the data with
    ) -> Result<(), VRError> {
        let validated_data_tags = DataTag::validate_tag_list(&text_value, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        let node_content = NodeContent::Text(text_value);
        let new_node = Node::from_node_content(key.clone(), node_content, metadata.clone(), data_tag_names);

        self.insert_node_at_path(parent_path, key, new_node, embedding, true)
    }

    /// Inserts a new node (with ExternalContent) at the specified key in the Map resource root.
    /// Uses the supplied Embedding.
    pub fn insert_external_content_node(
        &mut self,
        key: String,
        external_content: SourceReference,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        self.insert_external_content_node_at_path(VRPath::new(), key, external_content, metadata, embedding)
    }

    /// Inserts a new node (with ExternalContent) into the specified parent_path using the provided key.
    /// Uses the supplied Embedding.
    pub fn insert_external_content_node_at_path(
        &mut self,
        parent_path: VRPath,
        key: String,
        external_content: SourceReference,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        // As ExternalContent doesn't have data tags, we pass an empty vector
        let node_content = NodeContent::ExternalContent(external_content);
        let new_node = Node::from_node_content(key.clone(), node_content, metadata.clone(), Vec::new());

        self.insert_node_at_path(parent_path, key, new_node, embedding, true)
    }

    /// Inserts a new node (with VRHeader) at the specified key in the Map resource root.
    /// Uses the supplied Embedding.
    pub fn insert_vr_header_node(
        &mut self,
        key: String,
        vr_header: VRHeader,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        self.insert_vr_header_node_at_path(VRPath::new(), key, vr_header, metadata, embedding)
    }

    /// Inserts a new node (with VRHeader) into the specified parent_path using the provided key.
    /// Uses the supplied Embedding.
    pub fn insert_vr_header_node_at_path(
        &mut self,
        parent_path: VRPath,
        key: String,
        vr_header: VRHeader,
        metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
    ) -> Result<(), VRError> {
        let data_tag_names = vr_header.data_tag_names.clone();
        let node_content = NodeContent::VRHeader(vr_header);
        let new_node = Node::from_node_content(key.clone(), node_content, metadata.clone(), data_tag_names);

        self.insert_node_at_path(parent_path, key, new_node, embedding, true)
    }

    /// Insert a new node and associated embeddings to the Map resource
    /// without checking if tags are valid.
    pub fn _insert_kv_without_tag_validation(
        &mut self,
        key: &str,
        data: NodeContent,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        tag_names: &Vec<String>,
    ) {
        let node = Node::from_node_content(key.to_string(), data.clone(), metadata.clone(), tag_names.clone());
        let _ = self.insert_root_node(key.to_string(), node, embedding.clone());
    }

    /// Replaces an existing node & associated embedding with a new BaseVectorResource at the specified key at root depth.
    pub fn replace_with_vector_resource_node(
        &mut self,
        key: String,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let path = VRPath::from_string(&("/".to_owned() + &key))?;
        self.replace_with_vector_resource_node_at_path(path, new_resource, new_metadata, embedding)
    }

    /// Replaces an existing node & associated embedding with a new BaseVectorResource at the specified path.
    /// Of note, path must include the node's id as the final part of the path.
    pub fn replace_with_vector_resource_node_at_path(
        &mut self,
        path: VRPath,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let tag_names = new_resource.as_trait_object().data_tag_index().data_tag_names();
        let node_content = NodeContent::Resource(new_resource);

        if let Some(key) = path.path_ids.last() {
            let new_node = Node::from_node_content(key.clone(), node_content, new_metadata.clone(), tag_names);
            self.replace_node_at_path(path, new_node, embedding.clone(), true)
        } else {
            Err(VRError::InvalidVRPath(path.clone()))
        }
    }

    /// Replaces an existing node & associated embedding with a new text node at the specified key at root depth.
    pub fn replace_with_text_node(
        &mut self,
        key: String,
        new_text: String,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: Vec<DataTag>, // List of datatags you want to parse the new data with.
    ) -> Result<(Node, Embedding), VRError> {
        let path = VRPath::from_string(&("/".to_owned() + &key))?;
        self.replace_with_text_node_at_path(path, new_text, new_metadata, embedding, parsing_tags)
    }

    /// Replaces an existing node & associated embedding with a new text node at the specified path.
    /// Of note, path must include the node's id as the final part of the path.
    pub fn replace_with_text_node_at_path(
        &mut self,
        path: VRPath,
        new_text: String,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: Vec<DataTag>, // List of datatags you want to parse the new data with.
    ) -> Result<(Node, Embedding), VRError> {
        // Validate which tags will be saved with the new data
        let validated_data_tags = DataTag::validate_tag_list(&new_text, &parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();

        if let Some(key) = path.path_ids.last() {
            let node_content = NodeContent::Text(new_text);
            let new_node = Node::from_node_content(key.clone(), node_content, new_metadata.clone(), data_tag_names);
            self.replace_node_at_path(path, new_node, embedding.clone(), true)
        } else {
            Err(VRError::InvalidVRPath(path.clone()))
        }
    }

    /// Replaces an existing node & associated embedding with a new ExternalContent node at the specified key at root depth.
    pub fn replace_with_external_content_node(
        &mut self,
        key: String,
        new_external_content: SourceReference,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let path = VRPath::from_string(&("/".to_owned() + &key))?;
        self.replace_with_external_content_node_at_path(path, new_external_content, new_metadata, embedding)
    }

    /// Replaces an existing node & associated embedding with a new ExternalContent node at the specified path.
    /// Of note, path must include the node's id as the final part of the path.
    pub fn replace_with_external_content_node_at_path(
        &mut self,
        path: VRPath,
        new_external_content: SourceReference,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let node_content = NodeContent::ExternalContent(new_external_content);

        if let Some(key) = path.path_ids.last() {
            let new_node = Node::from_node_content(key.clone(), node_content, new_metadata.clone(), Vec::new());
            self.replace_node_at_path(path, new_node, embedding.clone(), true)
        } else {
            Err(VRError::InvalidVRPath(path.clone()))
        }
    }

    /// Replaces an existing node & associated embedding with a new VRHeader node at the specified key at root depth.
    pub fn replace_with_vr_header_node(
        &mut self,
        key: String,
        new_vr_header: VRHeader,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let path = VRPath::from_string(&("/".to_owned() + &key))?;
        self.replace_with_vr_header_node_at_path(path, new_vr_header, new_metadata, embedding)
    }

    /// Replaces an existing node & associated embedding with a new VRHeader node at the specified path.
    /// Of note, path must include the node's id as the final part of the path.
    pub fn replace_with_vr_header_node_at_path(
        &mut self,
        path: VRPath,
        new_vr_header: VRHeader,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let data_tag_names = new_vr_header.data_tag_names.clone();
        let node_content = NodeContent::VRHeader(new_vr_header);

        if let Some(key) = path.path_ids.last() {
            let new_node = Node::from_node_content(key.clone(), node_content, new_metadata.clone(), data_tag_names);
            self.replace_node_at_path(path, new_node, embedding.clone(), true)
        } else {
            Err(VRError::InvalidVRPath(path.clone())) // Replace with your actual error
        }
    }

    /// Replaces an existing node & associated embeddings in the Map resource
    /// without checking if tags are valid.
    pub fn _replace_kv_without_tag_validation(
        &mut self,
        key: &str,
        new_data: NodeContent,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        new_tag_names: &Vec<String>,
    ) -> Result<(Node, Embedding), VRError> {
        let new_node = Node::from_node_content(
            key.to_string(),
            new_data.clone(),
            new_metadata.clone(),
            new_tag_names.clone(),
        );
        self.replace_root_node(key.to_string(), new_node, embedding.clone())
    }

    /// Internal method for removing root node/embedding, and updating indexes.
    fn remove_root_node(&mut self, key: &str) -> Result<(Node, Embedding), VRError> {
        let deleted_node = self._remove_root_node(key)?;
        let deleted_embedding = self
            .embeddings
            .remove(key)
            .ok_or(VRError::InvalidNodeId(key.to_string()))?;

        self.data_tag_index.remove_node(&deleted_node);
        self.metadata_index.remove_node(&deleted_node);

        self.update_last_written_to_now();
        Ok((deleted_node, deleted_embedding))
    }

    /// Internal method. Node deletion from the hashmap
    fn _remove_root_node(&mut self, key: &str) -> Result<Node, VRError> {
        self.node_count -= 1;
        let removed_node = self.nodes.remove(key).ok_or(VRError::InvalidNodeId(key.to_string()))?;
        self.update_last_written_to_now();
        Ok(removed_node)
    }

    // Internal method. Inserts a node into the nodes hashmap
    fn _insert_root_node(&mut self, node: Node) {
        self.node_count += 1;
        self.nodes.insert(node.id.clone(), node);
        self.update_last_written_to_now();
    }

    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn set_resource_id(&mut self, resource_id: String) {
        self.resource_id = resource_id;
        self.update_last_written_to_now();
    }
}
