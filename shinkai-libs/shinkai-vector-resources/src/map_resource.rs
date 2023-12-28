use crate::base_vector_resources::{BaseVectorResource, VRBaseType};
use crate::data_tags::{DataTag, DataTagIndex};
use crate::embeddings::Embedding;
use crate::metadata_index::MetadataIndex;
use crate::model_type::{EmbeddingModelType, TextEmbeddingsInference};
use crate::resource_errors::VRError;
use crate::shinkai_time::ShinkaiTime;
use crate::source::{SourceReference, VRSource};
use crate::vector_resource::{Node, NodeContent, VRPath, VectorResource};
use crate::vector_search_traversal::VRHeader;
use serde_json;
use std::collections::HashMap;

/// A VectorResource which uses a HashMap data model, thus providing a
/// native key-value interface. Ideal for use cases such as field-based data sources, classical DBs,
/// constantly-updating data streams, or any unordered/mutating source data.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapVectorResource {
    name: String,
    description: Option<String>,
    source: VRSource,
    resource_id: String,
    resource_embedding: Embedding,
    resource_base_type: VRBaseType,
    embedding_model_used: EmbeddingModelType,
    embeddings: HashMap<String, Embedding>,
    node_count: u64,
    nodes: HashMap<String, Node>,
    data_tag_index: DataTagIndex,
    created_datetime: String,
    last_modified_datetime: String,
    metadata_index: MetadataIndex,
}

impl VectorResource for MapVectorResource {
    /// RFC3339 Datetime when then Vector Resource was created
    fn created_datetime(&self) -> String {
        self.created_datetime.clone()
    }
    /// RFC3339 Datetime when then Vector Resource was last modified
    fn last_modified_datetime(&self) -> String {
        self.last_modified_datetime.clone()
    }
    /// Set a RFC Datetime of when then Vector Resource was last modified
    fn set_last_modified_datetime(&mut self, datetime: String) -> Result<(), VRError> {
        if ShinkaiTime::validate_datetime_string(&datetime) {
            self.last_modified_datetime = datetime;
            return Ok(());
        }
        return Err(VRError::InvalidDateTimeString(datetime));
    }

    fn data_tag_index(&self) -> &DataTagIndex {
        &self.data_tag_index
    }

    fn metadata_index(&self) -> &MetadataIndex {
        &self.metadata_index
    }

    fn embedding_model_used(&self) -> EmbeddingModelType {
        self.embedding_model_used.clone()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn source(&self) -> VRSource {
        self.source.clone()
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

    fn get_embeddings(&self) -> Vec<Embedding> {
        self.embeddings.values().cloned().collect()
    }

    fn to_json(&self) -> Result<String, VRError> {
        serde_json::to_string(self).map_err(|_| VRError::FailedJSONParsing)
    }

    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType) {
        self.update_last_modified_to_now();
        self.embedding_model_used = model_type;
    }

    fn set_resource_embedding(&mut self, embedding: Embedding) {
        self.update_last_modified_to_now();
        self.resource_embedding = embedding;
    }

    fn set_resource_id(&mut self, id: String) {
        self.update_last_modified_to_now();
        self.resource_id = id;
    }

    /// Retrieves a node's embedding given its key (id)
    fn get_embedding(&self, key: String) -> Result<Embedding, VRError> {
        Ok(self.embeddings.get(&key).ok_or(VRError::InvalidNodeId)?.clone())
    }

    /// Retrieves a node given its key (id)
    fn get_node(&self, key: String) -> Result<Node, VRError> {
        self.nodes.get(&key).cloned().ok_or(VRError::InvalidNodeId)
    }

    /// Returns all nodes in the MapVectorResource
    fn get_nodes(&self) -> Vec<Node> {
        self.nodes.values().cloned().collect()
    }

    /// Insert a Node/Embedding into the VR using the provided id (root level depth). Overwrites existing data.
    fn insert_node(&mut self, id: String, node: Node, embedding: Embedding) -> Result<(), VRError> {
        // Update ids to match supplied id
        let mut updated_node = node;
        updated_node.id = id.to_string();
        let mut embedding = embedding.clone();
        embedding.set_id(id.to_string());

        // Insert node/embeddings
        self._insert_node(updated_node.clone());
        self.embeddings.insert(updated_node.id.clone(), embedding);

        // Update indices
        self.data_tag_index.add_node(&updated_node);
        self.metadata_index.add_node(&updated_node);

        self.update_last_modified_to_now();
        Ok(())
    }

    /// Replace a Node/Embedding in the VR using the provided id (root level depth)
    fn replace_node(&mut self, id: String, node: Node, embedding: Embedding) -> Result<(Node, Embedding), VRError> {
        // Replace old node, and get old embedding
        let new_node = node;
        let old_node = self
            .nodes
            .insert(id.to_string(), new_node.clone())
            .ok_or(VRError::InvalidNodeId)?;
        let old_embedding = self.get_embedding(id.clone())?;

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
        self.update_last_modified_to_now();

        Ok((old_node, old_embedding))
    }

    /// Remove a Node/Embedding in the VR using the provided id (root level depth)
    fn remove_node(&mut self, id: String) -> Result<(Node, Embedding), VRError> {
        let path = VRPath::from_string(&("/".to_owned() + &id));
        self.remove_node_at_path(path)
    }
}

impl MapVectorResource {
    /// Create a new MapVectorResource
    pub fn new(
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        resource_embedding: Embedding,
        embeddings: HashMap<String, Embedding>,
        nodes: HashMap<String, Node>,
        embedding_model_used: EmbeddingModelType,
    ) -> Self {
        let current_time = ShinkaiTime::generate_time_now();
        let mut resource = MapVectorResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source,
            resource_id: String::from("default"),
            resource_embedding,
            embeddings,
            node_count: nodes.len() as u64,
            resource_base_type: VRBaseType::Map,
            nodes,
            embedding_model_used,
            data_tag_index: DataTagIndex::new(),
            created_datetime: current_time.clone(),
            last_modified_datetime: current_time,
            metadata_index: MetadataIndex::new(),
        };
        // Generate a unique resource_id:
        resource.generate_and_update_resource_id();
        resource
    }

    /// Initializes an empty `MapVectorResource` with empty defaults.
    pub fn new_empty(name: &str, desc: Option<&str>, source: VRSource) -> Self {
        MapVectorResource::new(
            name,
            desc,
            source,
            Embedding::new(&String::new(), vec![]),
            HashMap::new(),
            HashMap::new(),
            EmbeddingModelType::TextEmbeddingsInference(TextEmbeddingsInference::AllMiniLML6v2),
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

        self.insert_node_at_path(parent_path, key.to_string(), new_internal_node, embedding)
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

        self.insert_node_at_path(parent_path, key, new_node, embedding)
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

        self.insert_node_at_path(parent_path, key, new_node, embedding)
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

        self.insert_node_at_path(parent_path, key, new_node, embedding)
    }

    /// Insert a new node and associated embeddings to the Map resource
    /// without checking if tags are valid.
    /// TODO: Deprecate once switch over to VectorFS fully
    pub fn _insert_kv_without_tag_validation(
        &mut self,
        key: &str,
        data: NodeContent,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        tag_names: &Vec<String>,
    ) {
        let node = Node::from_node_content(key.to_string(), data.clone(), metadata.clone(), tag_names.clone());
        self.insert_node(key.to_string(), node, embedding.clone());
    }

    /// Replaces an existing node & associated embedding with a new BaseVectorResource at the specified key at root depth.
    pub fn replace_with_vector_resource_node(
        &mut self,
        key: String,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        let path = VRPath::from_string(&("/".to_owned() + &key));
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
            self.replace_node_at_path(path, new_node, embedding.clone())
        } else {
            Err(VRError::InvalidVRPath(path.clone()))
        }
    }

    /// Replaces an existing node & associated embedding with a new text node at the specified key at root depth.
    pub fn replace_with_text_node(
        &mut self,
        key: String,
        new_text_value: String,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: Option<Vec<DataTag>>, // List of datatags you want to parse the new data with. If None will preserve previous tags.
    ) -> Result<(Node, Embedding), VRError> {
        let path = VRPath::from_string(&("/".to_owned() + &key));
        self.replace_with_text_node_at_path(path, new_text_value, new_metadata, embedding, parsing_tags)
    }

    /// Replaces an existing node & associated embedding with a new text node at the specified path.
    /// Of note, path must include the node's id as the final part of the path.
    pub fn replace_with_text_node_at_path(
        &mut self,
        path: VRPath,
        new_text_value: String,
        new_metadata: Option<HashMap<String, String>>,
        embedding: Embedding,
        parsing_tags: Option<Vec<DataTag>>, // List of datatags you want to parse the new data with. If None will preserve previous tags.
    ) -> Result<(Node, Embedding), VRError> {
        // Validate which tags will be saved with the new data
        let mut data_tag_names = vec![];
        if let Some(tags) = parsing_tags {
            let validated_data_tags = DataTag::validate_tag_list(&new_text_value, &tags);
            data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        } else {
            if let Some(key) = path.path_ids.last() {
                data_tag_names = self.get_node(key.clone())?.data_tag_names.clone();
            } else {
                return Err(VRError::InvalidVRPath(path.clone()));
            }
        }

        if let Some(key) = path.path_ids.last() {
            let node_content = NodeContent::Text(new_text_value);
            let new_node = Node::from_node_content(key.clone(), node_content, new_metadata.clone(), data_tag_names);
            self.replace_node_at_path(path, new_node, embedding)
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
        let path = VRPath::from_string(&("/".to_owned() + &key));
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
            self.replace_node_at_path(path, new_node, embedding.clone())
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
        let path = VRPath::from_string(&("/".to_owned() + &key));
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
            self.replace_node_at_path(path, new_node, embedding.clone())
        } else {
            Err(VRError::InvalidVRPath(path.clone())) // Replace with your actual error
        }
    }

    /// Replaces an existing node & associated embeddings in the Map resource
    /// without checking if tags are valid.
    /// TODO: Deprecate once VectorFS is used strictly.
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
        self.replace_node(key.to_string(), new_node, embedding.clone())
    }

    /// Internal method. Node deletion from the hashmap
    fn _remove_node(&mut self, key: &str) -> Result<Node, VRError> {
        self.node_count -= 1;
        let removed_node = self.nodes.remove(key).ok_or(VRError::InvalidNodeId)?;
        self.update_last_modified_to_now();
        Ok(removed_node)
    }

    // Internal method. Inserts a node into the nodes hashmap
    fn _insert_node(&mut self, node: Node) {
        self.node_count += 1;
        self.nodes.insert(node.id.clone(), node);
        self.update_last_modified_to_now();
    }

    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn set_resource_id(&mut self, resource_id: String) {
        self.resource_id = resource_id;
        self.update_last_modified_to_now();
    }
}
