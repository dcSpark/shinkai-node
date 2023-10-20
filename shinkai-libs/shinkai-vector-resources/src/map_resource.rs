use crate::base_vector_resources::{BaseVectorResource, VectorResourceBaseType};
use crate::data_tags::{DataTag, DataTagIndex};
use crate::embeddings::Embedding;
use crate::model_type::{EmbeddingModelType, TextEmbeddingsInference};
use crate::resource_errors::VectorResourceError;
use crate::source::VRSource;
use crate::vector_resource::{Node, NodeContent, RetrievedNode, VRPath, VectorResource};
use serde_json;
use std::collections::HashMap;

/// A VectorResource which uses an internal HashMap data model, thus providing a
/// native key-value interface.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapVectorResource {
    name: String,
    description: Option<String>,
    source: VRSource,
    resource_id: String,
    resource_embedding: Embedding,
    resource_base_type: VectorResourceBaseType,
    embedding_model_used: EmbeddingModelType,
    node_embeddings: HashMap<String, Embedding>,
    node_count: u64,
    nodes: HashMap<String, Node>,
    data_tag_index: DataTagIndex,
}

impl VectorResource for MapVectorResource {
    fn data_tag_index(&self) -> &DataTagIndex {
        &self.data_tag_index
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

    fn resource_base_type(&self) -> VectorResourceBaseType {
        self.resource_base_type.clone()
    }

    fn node_embeddings(&self) -> Vec<Embedding> {
        self.node_embeddings.values().cloned().collect()
    }

    fn to_json(&self) -> Result<String, VectorResourceError> {
        serde_json::to_string(self).map_err(|_| VectorResourceError::FailedJSONParsing)
    }

    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType) {
        self.embedding_model_used = model_type;
    }

    fn set_resource_embedding(&mut self, embedding: Embedding) {
        self.resource_embedding = embedding;
    }

    /// Retrieves a node's embedding given its key (id)
    fn get_node_embedding(&self, key: String) -> Result<Embedding, VectorResourceError> {
        Ok(self
            .node_embeddings
            .get(&key)
            .ok_or(VectorResourceError::InvalidNodeId)?
            .clone())
    }

    /// Retrieves a node given its key (id)
    fn get_node(&self, key: String) -> Result<Node, VectorResourceError> {
        Ok(self.nodes.get(&key).ok_or(VectorResourceError::InvalidNodeId)?.clone())
    }

    /// Returns all nodes in the MapVectorResource
    fn get_nodes(&self) -> Vec<Node> {
        self.nodes.values().cloned().collect()
    }
}

impl MapVectorResource {
    /// * `resource_id` - This can be the Sha256 hash as a String from the bytes of the original data
    /// or anything that is deterministic to ensure duplicates are not possible.
    pub fn new(
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        resource_id: &str,
        resource_embedding: Embedding,
        node_embeddings: HashMap<String, Embedding>,
        nodes: HashMap<String, Node>,
        embedding_model_used: EmbeddingModelType,
    ) -> Self {
        MapVectorResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source,
            resource_id: String::from(resource_id),
            resource_embedding,
            node_embeddings,
            node_count: nodes.len() as u64,
            resource_base_type: VectorResourceBaseType::Map,
            nodes,
            embedding_model_used,
            data_tag_index: DataTagIndex::new(),
        }
    }

    /// Initializes an empty `MapVectorResource` with empty defaults.
    pub fn new_empty(name: &str, desc: Option<&str>, source: VRSource, resource_id: &str) -> Self {
        MapVectorResource::new(
            name,
            desc,
            source,
            resource_id,
            Embedding::new(&String::new(), vec![]),
            HashMap::new(),
            HashMap::new(),
            EmbeddingModelType::TextEmbeddingsInference(TextEmbeddingsInference::AllMiniLML6v2),
        )
    }

    /// Returns all Nodes with a matching key/value pair in the metadata hashmap
    /// Does not perform any traversal.
    pub fn metadata_search(
        &self,
        metadata_key: &str,
        metadata_value: &str,
    ) -> Result<Vec<RetrievedNode>, VectorResourceError> {
        let mut matching_nodes = Vec::new();

        for node in self.nodes.values() {
            match &node.metadata {
                Some(metadata) if metadata.get(metadata_key) == Some(&metadata_value.to_string()) => matching_nodes
                    .push(RetrievedNode {
                        node: node.clone(),
                        score: 0.00,
                        resource_pointer: self.get_resource_pointer(),
                        retrieval_path: VRPath::new(),
                    }),
                _ => (),
            }
        }

        if matching_nodes.is_empty() {
            return Err(VectorResourceError::NoNodeFound);
        }

        Ok(matching_nodes)
    }

    /// Inserts a new node (with a BaseVectorResource) and associated embeddings to the Map resource
    /// and updates the data tags index.
    pub fn insert_vector_resource(
        &mut self,
        key: &str,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
    ) {
        let embedding = resource.as_trait_object().resource_embedding().clone();
        let tag_names = resource.as_trait_object().data_tag_index().data_tag_names();
        self._insert_kv_without_tag_validation(key, NodeContent::Resource(resource), metadata, &embedding, &tag_names)
    }

    /// Inserts a new node (with a String value) and associated embeddings to the Map resource
    /// and updates the data tags index.
    pub fn insert_kv(
        &mut self,
        key: &str,
        value: &str,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the data with
    ) {
        let validated_data_tags = DataTag::validate_tag_list(value, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._insert_kv_without_tag_validation(
            key,
            NodeContent::Text(value.to_string()),
            metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Insert a new node and associated embeddings to the Map resource
    /// without checking if tags are valid. Also used by resource router.
    pub fn _insert_kv_without_tag_validation(
        &mut self,
        key: &str,
        data: NodeContent,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        tag_names: &Vec<String>,
    ) {
        let node = match data {
            NodeContent::Text(data_string) => Node::new(key.to_string(), &data_string, metadata.clone(), tag_names),
            NodeContent::Resource(resource) => Node::new_vector_resource(key.to_string(), &resource, metadata.clone()),
        };
        self.data_tag_index.add_node(&node);

        // Embedding details
        let mut embedding = embedding.clone();
        embedding.set_id(key.to_string());
        self.insert_node(node.clone());
        self.node_embeddings.insert(node.id.clone(), embedding);
    }

    /// Replaces an existing node (based on key) and associated embeddings in the Map resource
    /// with a BaseVectorResource in the new Node, and updates the data tags index.
    pub fn replace_vector_resource(
        &mut self,
        key: &str,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
    ) -> Result<Node, VectorResourceError> {
        let embedding = new_resource.as_trait_object().resource_embedding().clone();
        let tag_names = new_resource.as_trait_object().data_tag_index().data_tag_names();
        self._replace_kv_without_tag_validation(
            key,
            NodeContent::Resource(new_resource),
            new_metadata,
            &embedding,
            &tag_names,
        )
    }

    /// Replaces an existing node & associated embedding and updates the data tags index.
    /// * `id` - The id of the node to be replaced.
    pub fn replace_kv(
        &mut self,
        key: &str,
        new_value: &str,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the new data with
    ) -> Result<Node, VectorResourceError> {
        // Validate which tags will be saved with the new data
        let validated_data_tags = DataTag::validate_tag_list(new_value, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._replace_kv_without_tag_validation(
            key,
            NodeContent::Text(new_value.to_string()),
            new_metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Replaces an existing node & associated embeddings in the Map resource
    /// without checking if tags are valid. Used for resource router.
    pub fn _replace_kv_without_tag_validation(
        &mut self,
        key: &str,
        new_data: NodeContent,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        new_tag_names: &Vec<String>,
    ) -> Result<Node, VectorResourceError> {
        // Next create the new node, and replace the old node in the nodes list
        let new_node = match new_data {
            NodeContent::Text(data_string) => {
                Node::new(key.to_string(), &data_string, new_metadata.clone(), new_tag_names)
            }
            NodeContent::Resource(resource) => {
                Node::new_vector_resource(key.to_string(), &resource, new_metadata.clone())
            }
        };
        let old_node = self
            .nodes
            .insert(key.to_string(), new_node.clone())
            .ok_or(VectorResourceError::InvalidNodeId)?;

        // Then deletion of old node from index and addition of new node
        self.data_tag_index.remove_node(&old_node);
        self.data_tag_index.add_node(&new_node);

        // Finally replacing the embedding
        let mut embedding = embedding.clone();
        embedding.set_id(key.to_string());
        self.node_embeddings.insert(key.to_string(), embedding);

        Ok(old_node)
    }

    /// Deletes a node and associated embedding from the resource
    /// and updates the data tags index.
    pub fn delete_kv(&mut self, key: &str) -> Result<(Node, Embedding), VectorResourceError> {
        let deleted_node = self.delete_node(key)?;
        self.data_tag_index.remove_node(&deleted_node);
        let deleted_embedding = self
            .node_embeddings
            .remove(key)
            .ok_or(VectorResourceError::InvalidNodeId)?;

        Ok((deleted_node, deleted_embedding))
    }

    /// Internal node deletion from the hashmap
    fn delete_node(&mut self, key: &str) -> Result<Node, VectorResourceError> {
        self.node_count -= 1;
        let removed_node = self.nodes.remove(key).ok_or(VectorResourceError::InvalidNodeId)?;
        Ok(removed_node)
    }

    // Inserts a node into the nodes hashmap
    fn insert_node(&mut self, node: Node) {
        self.node_count += 1;
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn from_json(json: &str) -> Result<Self, VectorResourceError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn set_resource_id(&mut self, resource_id: String) {
        self.resource_id = resource_id;
    }
}
