use crate::base_vector_resources::{BaseVectorResource, VectorResourceBaseType};
use crate::data_tags::{DataTag, DataTagIndex};
use crate::embeddings::Embedding;
use crate::model_type::{EmbeddingModelType, TextEmbeddingsInference};
use crate::resource_errors::VectorResourceError;
use crate::source::VRSource;
use crate::vector_resource::{Node, NodeContent, RetrievedNode, TraversalMethod, VRPath, VectorResource};
use serde_json;
use std::collections::HashMap;

/// A VectorResource which uses an internal numbered/ordered list data model,  
/// thus providing an ideal interface for document-like content such as PDFs,
/// epubs, web content, written works, and more.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentVectorResource {
    name: String,
    description: Option<String>,
    source: VRSource,
    resource_id: String,
    resource_embedding: Embedding,
    embedding_model_used: EmbeddingModelType,
    resource_base_type: VectorResourceBaseType,
    node_embeddings: Vec<Embedding>,
    node_count: u64,
    nodes: Vec<Node>,
    data_tag_index: DataTagIndex,
}

impl VectorResource for DocumentVectorResource {
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
        self.node_embeddings.clone()
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

    /// Efficiently retrieves a Node's matching embedding given its id by fetching it via index.
    fn get_node_embedding(&self, id: String) -> Result<Embedding, VectorResourceError> {
        let id = id.parse::<u64>().map_err(|_| VectorResourceError::InvalidNodeId)?;
        if id == 0 || id > self.node_count {
            return Err(VectorResourceError::InvalidNodeId);
        }
        let index = id.checked_sub(1).ok_or(VectorResourceError::InvalidNodeId)? as usize;
        Ok(self.node_embeddings[index].clone())
    }

    /// Efficiently retrieves a node given its id by fetching it via index.
    fn get_node(&self, id: String) -> Result<Node, VectorResourceError> {
        let id = id.parse::<u64>().map_err(|_| VectorResourceError::InvalidNodeId)?;
        if id == 0 || id > self.node_count {
            return Err(VectorResourceError::InvalidNodeId);
        }
        let index = id.checked_sub(1).ok_or(VectorResourceError::InvalidNodeId)? as usize;
        Ok(self.nodes[index].clone())
    }

    /// Returns all nodes in the MapVectorResource
    fn get_nodes(&self) -> Vec<Node> {
        self.nodes.clone()
    }
}

impl DocumentVectorResource {
    /// * `resource_id` - For DocumentVectorResources this should be a Sha256 hash as a String
    ///  from the bytes of the original data.
    pub fn new(
        name: &str,
        desc: Option<&str>,
        source: VRSource,
        resource_id: &str,
        resource_embedding: Embedding,
        node_embeddings: Vec<Embedding>,
        nodes: Vec<Node>,
        embedding_model_used: EmbeddingModelType,
    ) -> Self {
        DocumentVectorResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source,
            resource_id: String::from(resource_id),
            resource_embedding,
            node_embeddings,
            node_count: nodes.len() as u64,
            nodes: nodes,
            embedding_model_used,
            resource_base_type: VectorResourceBaseType::Document,
            data_tag_index: DataTagIndex::new(),
        }
    }

    /// Initializes an empty `DocumentVectorResource` with empty defaults.
    pub fn new_empty(name: &str, desc: Option<&str>, source: VRSource, resource_id: &str) -> Self {
        DocumentVectorResource::new(
            name,
            desc,
            source,
            resource_id,
            Embedding::new(&String::new(), vec![]),
            Vec::new(),
            Vec::new(),
            EmbeddingModelType::TextEmbeddingsInference(TextEmbeddingsInference::AllMiniLML6v2),
        )
    }

    /// Performs a vector search using a query embedding, and then
    /// fetches a specific number of Nodes below and above the most
    /// similar Node.
    ///
    /// Does not traverse past the top level.
    pub fn vector_search_proximity(
        &self,
        query: Embedding,
        proximity_window: u64,
    ) -> Result<Vec<RetrievedNode>, VectorResourceError> {
        let search_results = self.vector_search_with_options(query, 1, &TraversalMethod::UntilDepth(0), None);
        let most_similar_node = search_results.first().ok_or(VectorResourceError::VectorResourceEmpty)?;
        let most_similar_id = most_similar_node
            .node
            .id
            .parse::<u64>()
            .map_err(|_| VectorResourceError::InvalidNodeId)?;

        // Get Start/End ids
        let start_id = if most_similar_id >= proximity_window {
            most_similar_id - proximity_window
        } else {
            1
        };
        let end_id = if let Some(end_boundary) = self.node_count.checked_sub(1) {
            if let Some(potential_end_id) = most_similar_id.checked_add(proximity_window) {
                potential_end_id.min(end_boundary)
            } else {
                end_boundary // Or any appropriate default
            }
        } else {
            1
        };

        // Acquire surrounding nodes
        let mut nodes = Vec::new();
        for id in start_id..=(end_id + 1) {
            if let Ok(node) = self.get_node(id.to_string()) {
                nodes.push(RetrievedNode {
                    node: node.clone(),
                    score: 0.00,
                    resource_pointer: self.get_resource_pointer(),
                    retrieval_path: VRPath::new(),
                });
            }
        }

        Ok(nodes)
    }

    /// Returns all Nodes with a matching key/value pair in the metadata hashmap.
    /// Does not perform any traversal.
    pub fn metadata_search(
        &self,
        metadata_key: &str,
        metadata_value: &str,
    ) -> Result<Vec<RetrievedNode>, VectorResourceError> {
        let mut matching_nodes = Vec::new();

        for node in &self.nodes {
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

    /// Appends a new node (with a BaseVectorResource) to the document
    /// and updates the data tags index. Of note, we use the resource's data tags
    /// and resource embedding.
    pub fn append_vector_resource(&mut self, resource: BaseVectorResource, metadata: Option<HashMap<String, String>>) {
        let embedding = resource.as_trait_object().resource_embedding().clone();
        let tag_names = resource.as_trait_object().data_tag_index().data_tag_names();
        self._append_data_without_tag_validation(NodeContent::Resource(resource), metadata, &embedding, &tag_names)
    }

    /// Appends a new node (with a data_string) and an associated embedding to the document
    /// and updates the data tags index.
    pub fn append_data(
        &mut self,
        data_string: &str,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the data with
    ) {
        let validated_data_tags = DataTag::validate_tag_list(data_string, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._append_data_without_tag_validation(
            NodeContent::Text(data_string.to_string()),
            metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Appends a new node and associated embedding to the document
    /// without checking if tags are valid. Used for internal purposes/the routing resource.
    pub fn _append_data_without_tag_validation(
        &mut self,
        data: NodeContent,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        tag_names: &Vec<String>,
    ) {
        let id = self.node_count + 1;
        let node = match data {
            NodeContent::Text(data_string) => Node::new_with_integer_id(id, &data_string, metadata.clone(), tag_names),
            NodeContent::Resource(resource) => {
                Node::new_vector_resource_with_integer_id(id, &resource, metadata.clone())
            }
        };
        self.data_tag_index.add_node(&node);

        // Embedding details
        let mut embedding = embedding.clone();
        embedding.set_id_with_integer(id);
        self.append_node(node);
        self.node_embeddings.push(embedding);
    }

    /// Replaces an existing node and associated embedding in the Document resource
    /// with a BaseVectorResource in the new Node, and updates the data tags index.
    pub fn replace_vector_resource(
        &mut self,
        id: u64,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
    ) -> Result<Node, VectorResourceError> {
        let embedding = new_resource.as_trait_object().resource_embedding().clone();
        let tag_names = new_resource.as_trait_object().data_tag_index().data_tag_names();
        self._replace_data_without_tag_validation(
            id,
            NodeContent::Resource(new_resource),
            new_metadata,
            &embedding,
            &tag_names,
        )
    }

    /// Replaces an existing node & associated embedding and updates the data tags index.
    /// * `id` - The id of the node to be replaced.
    pub fn replace_data(
        &mut self,
        id: u64,
        new_data: &str,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the new data with
    ) -> Result<Node, VectorResourceError> {
        // Validate which tags will be saved with the new data
        let validated_data_tags = DataTag::validate_tag_list(new_data, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._replace_data_without_tag_validation(
            id,
            NodeContent::Text(new_data.to_string()),
            new_metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Pops and returns the last node and associated embedding
    /// and updates the data tags index.
    pub fn pop_data(&mut self) -> Result<(Node, Embedding), VectorResourceError> {
        let popped_node = self.nodes.pop();
        let popped_embedding = self.node_embeddings.pop();

        match (popped_node, popped_embedding) {
            (Some(node), Some(embedding)) => {
                // Remove node from data tag index
                self.data_tag_index.remove_node(&node);
                self.node_count -= 1;
                Ok((node, embedding))
            }
            _ => Err(VectorResourceError::VectorResourceEmpty),
        }
    }

    /// Replaces an existing node & associated embedding in the Document resource
    /// without checking if tags are valid. Used for resource router.
    pub fn _replace_data_without_tag_validation(
        &mut self,
        id: u64,
        new_data: NodeContent,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        new_tag_names: &Vec<String>,
    ) -> Result<Node, VectorResourceError> {
        // Id + index
        if id > self.node_count {
            return Err(VectorResourceError::InvalidNodeId);
        }
        let index = (id - 1) as usize;

        // Next create the new node, and replace the old node in the nodes list
        let new_node = match new_data {
            NodeContent::Text(data_string) => {
                Node::new_with_integer_id(id, &data_string, new_metadata.clone(), new_tag_names)
            }
            NodeContent::Resource(resource) => {
                Node::new_vector_resource_with_integer_id(id, &resource, new_metadata.clone())
            }
        };
        let old_node = std::mem::replace(&mut self.nodes[index], new_node.clone());

        // Then deletion of old node from index and addition of new node
        self.data_tag_index.remove_node(&old_node);
        self.data_tag_index.add_node(&new_node);

        // Finally replacing the embedding
        let mut embedding = embedding.clone();
        embedding.set_id_with_integer(id);
        self.node_embeddings[index] = embedding;

        Ok(old_node)
    }

    /// Deletes a node and associated embedding from the resource
    /// and updates the data tags index.
    pub fn delete_data(&mut self, id: u64) -> Result<(Node, Embedding), VectorResourceError> {
        let deleted_node = self.delete_node(id)?;
        self.data_tag_index.remove_node(&deleted_node);

        let index = (id - 1) as usize;
        let deleted_embedding = self.node_embeddings.remove(index);

        // Adjust the ids of the remaining embeddings
        for i in index..self.node_embeddings.len() {
            self.node_embeddings[i].set_id_with_integer((i + 1) as u64);
        }

        Ok((deleted_node, deleted_embedding))
    }

    /// Internal node deletion
    fn delete_node(&mut self, id: u64) -> Result<Node, VectorResourceError> {
        if id > self.node_count {
            return Err(VectorResourceError::InvalidNodeId);
        }
        let index = (id - 1) as usize;
        let removed_node = self.nodes.remove(index);
        self.node_count -= 1;
        for node in self.nodes.iter_mut().skip(index) {
            let node_id: u64 = node.id.parse().unwrap();
            node.id = format!("{}", node_id - 1);
        }
        Ok(removed_node)
    }

    fn append_node(&mut self, mut node: Node) {
        self.node_count += 1;
        node.id = self.node_count.to_string();
        self.nodes.push(node);
    }

    pub fn from_json(json: &str) -> Result<Self, VectorResourceError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn set_resource_id(&mut self, resource_id: String) {
        self.resource_id = resource_id;
    }
}
