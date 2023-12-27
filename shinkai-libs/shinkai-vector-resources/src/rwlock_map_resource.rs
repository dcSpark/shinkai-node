use crate::base_vector_resources::{BaseVectorResource, VRBaseType};
use crate::data_tags::{DataTag, DataTagIndex};
use crate::embeddings::Embedding;
use crate::metadata_index::MetadataIndex;
use crate::model_type::{EmbeddingModelType, TextEmbeddingsInference};
use crate::resource_errors::VRError;
use crate::shinkai_time::ShinkaiTime;
use crate::source::{SourceReference, VRSource};
use crate::vector_resource::{Node, NodeContent, RetrievedNode, VRPath, VectorResource};
use crate::vector_search_traversal::VRHeader;
use serde::{Deserialize, Serialize};
use serde_json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::Metadata;
use std::sync::{RwLock, RwLockWriteGuard};

/// A VectorResource which uses a HashMap data model backed by a RwLock,
/// thereby allowing for seamless mutability of any internally held node, no matter the depth.
/// Intended to be used for low level mutation-heavy use cases that need Vector Search support,
/// not for normal documents/data due to complexity of the interface.
#[derive(Debug)]
pub struct RwLockMapVectorResource {
    name: String,
    description: Option<String>,
    source: VRSource,
    resource_id: String,
    resource_embedding: Embedding,
    resource_base_type: VRBaseType,
    embedding_model_used: EmbeddingModelType,
    embeddings: HashMap<String, Embedding>,
    node_count: u64,
    nodes: RwLock<HashMap<String, Node>>,
    data_tag_index: DataTagIndex,
    created_datetime: String,
    last_modified_datetime: String,
    metadata_index: MetadataIndex,
}

impl VectorResource for RwLockMapVectorResource {
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

    /// Retrieves a copy of a Node given its key (id)
    fn get_node(&self, key: String) -> Result<Node, VRError> {
        match self.nodes.read() {
            Ok(nodes) => nodes.get(&key).cloned().ok_or(VRError::InvalidNodeId),
            Err(_) => Err(VRError::LockAcquisitionFailed("get_node".to_string())),
        }
    }

    /// Returns copies of all nodes in the RwLockMapVectorResource
    fn get_nodes(&self) -> Vec<Node> {
        match self.nodes.read() {
            Ok(nodes) => nodes.values().cloned().collect(),
            Err(_) => vec![],
        }
    }
}

impl RwLockMapVectorResource {
    /// Create a new RwLockMapVectorResource
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
        let mut resource = RwLockMapVectorResource {
            name: String::from(name),
            description: desc.map(String::from),
            source: source,
            resource_id: String::from("default"),
            resource_embedding,
            embeddings,
            node_count: nodes.len() as u64,
            resource_base_type: VRBaseType::Map,
            nodes: RwLock::new(nodes),
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

    /// Initializes an empty `RwLockMapVectorResource` with empty defaults.
    pub fn new_empty(name: &str, desc: Option<&str>, source: VRSource) -> Self {
        RwLockMapVectorResource::new(
            name,
            desc,
            source,
            Embedding::new(&String::new(), vec![]),
            HashMap::new(),
            HashMap::new(),
            EmbeddingModelType::TextEmbeddingsInference(TextEmbeddingsInference::AllMiniLML6v2),
        )
    }

    // fn mutate_node_with_path(&self, path: VRPath, new_content: NodeContent) -> Result<(), VRError> {
    //     if path.path_ids.is_empty() {
    //         return Err(VRError::InvalidVRPath(path.clone()));
    //     }

    //     // Fetch the first node directly, then iterate through the rest
    //     let mut node_guard = self.get_node_mut(path.path_ids[0].clone())?;
    //     for id in path.path_ids.iter().skip(1) {
    //         match &mut *node_guard.content {
    //             NodeContent::Resource(resource) => {
    //                 node_guard = resource.as_trait_object().get_node_mut(id.clone())?;
    //             }
    //             _ => {
    //                 if let Some(last) = path.path_ids.last() {
    //                     if id != last {
    //                         return Err(VRError::InvalidVRPath(path.clone()));
    //                     }
    //                 }
    //             }
    //         }
    //     }

    //     // Mutate the node
    //     node_guard.content = new_content;
    //     Ok(())
    // }

    /// Inserts a new node (with a BaseVectorResource) with the provided embedding
    /// at the specified key in the Map resource, and updates the indexes.
    pub fn insert_vector_resource_node(
        &mut self,
        key: &str,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) {
        let tag_names = resource.as_trait_object().data_tag_index().data_tag_names();
        self._insert_kv_without_tag_validation(key, NodeContent::Resource(resource), metadata, &embedding, &tag_names)
    }

    /// Inserts a new node (with a BaseVectorResource) with the resource's included embedding
    /// at the specified key in the Map resource, and updates the indexes.
    pub fn insert_vector_resource_node_auto(
        &mut self,
        key: &str,
        resource: BaseVectorResource,
        metadata: Option<HashMap<String, String>>,
    ) {
        let embedding = resource.as_trait_object().resource_embedding().clone();
        self.insert_vector_resource_node(key, resource, metadata, &embedding)
    }

    /// Inserts a new text node and associated embedding
    /// at the specified key in the Map resource.
    pub fn insert_text_node(
        &mut self,
        key: &str,
        text_value: &str,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the data with
    ) {
        let validated_data_tags = DataTag::validate_tag_list(text_value, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._insert_kv_without_tag_validation(
            key,
            NodeContent::Text(text_value.to_string()),
            metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Inserts a new node (with ExternalContent) at the specified key in the Map resource.
    /// Uses the supplied Embedding.
    pub fn insert_external_content_node(
        &mut self,
        key: &str,
        external_content: SourceReference,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) {
        // As ExternalContent doesn't have data tags, we pass an empty vector
        self._insert_kv_without_tag_validation(
            key,
            NodeContent::ExternalContent(external_content),
            metadata,
            embedding,
            &Vec::new(),
        )
    }

    /// Inserts a new node (with VRHeader) at the specified key in the Map resource.
    /// Uses the supplied Embedding.
    pub fn insert_vr_header_node(
        &mut self,
        key: &str,
        vr_header: VRHeader,
        metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) {
        let data_tag_names = vr_header.data_tag_names.clone();
        self._insert_kv_without_tag_validation(
            key,
            NodeContent::VRHeader(vr_header),
            metadata,
            embedding,
            &data_tag_names,
        )
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
        self.data_tag_index.add_node(&node);
        self.metadata_index.add_node(&node);

        // Embedding details
        let mut embedding = embedding.clone();
        embedding.set_id(key.to_string());
        self._insert_node(node.clone());
        self.embeddings.insert(node.id.clone(), embedding);
        self.update_last_modified_to_now();
    }

    /// Replaces an existing node & associated embedding with a new
    /// BaseVectorResource.
    pub fn replace_with_vector_resource_node(
        &mut self,
        key: &str,
        new_resource: BaseVectorResource,
        new_metadata: Option<HashMap<String, String>>,
    ) -> Result<Node, VRError> {
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

    /// Replaces an existing node & associated embedding with a new text node.
    pub fn replace_with_text_node(
        &mut self,
        key: &str,
        new_text_value: &str,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
        parsing_tags: &Vec<DataTag>, // list of datatags you want to parse the new data with
    ) -> Result<Node, VRError> {
        // Validate which tags will be saved with the new data
        let validated_data_tags = DataTag::validate_tag_list(new_text_value, parsing_tags);
        let data_tag_names = validated_data_tags.iter().map(|tag| tag.name.clone()).collect();
        self._replace_kv_without_tag_validation(
            key,
            NodeContent::Text(new_text_value.to_string()),
            new_metadata,
            embedding,
            &data_tag_names,
        )
    }

    /// Replaces an existing node & associated embedding with a new ExternalContent node.
    pub fn replace_with_external_content_node(
        &mut self,
        key: &str,
        new_external_content: SourceReference,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) -> Result<Node, VRError> {
        // As ExternalContent doesn't have data tags, we pass an empty vector
        self._replace_kv_without_tag_validation(
            key,
            NodeContent::ExternalContent(new_external_content),
            new_metadata,
            embedding,
            &Vec::new(),
        )
    }

    /// Replaces an existing node & associated embedding with a new VRHeader node.
    pub fn replace_with_vr_header_node(
        &mut self,
        key: &str,
        new_vr_header: VRHeader,
        new_metadata: Option<HashMap<String, String>>,
        embedding: &Embedding,
    ) -> Result<Node, VRError> {
        let data_tag_names = new_vr_header.data_tag_names.clone();
        self._replace_kv_without_tag_validation(
            key,
            NodeContent::VRHeader(new_vr_header),
            new_metadata,
            embedding,
            &data_tag_names,
        )
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
    ) -> Result<Node, VRError> {
        // Next create the new node, and replace the old node in the nodes by inserting (updating)
        let new_node = Node::from_node_content(
            key.to_string(),
            new_data.clone(),
            new_metadata.clone(),
            new_tag_names.clone(),
        );
        let old_node = {
            let mut nodes = match self.nodes.write() {
                Ok(nodes) => nodes,
                Err(_) => {
                    return Err(VRError::LockAcquisitionFailed(
                        "_replace_kv_without_tag_validation".to_string(),
                    ))
                }
            };
            nodes
                .insert(key.to_string(), new_node.clone())
                .ok_or(VRError::InvalidNodeId)?
        };

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
        embedding.set_id(key.to_string());
        self.embeddings.insert(key.to_string(), embedding);
        self.update_last_modified_to_now();

        Ok(old_node)
    }

    /// Deletes a node and associated embedding from the resource.
    pub fn remove_node(&mut self, key: &str) -> Result<(Node, Embedding), VRError> {
        let deleted_node = self._remove_node(key)?;
        self.data_tag_index.remove_node(&deleted_node);
        self.metadata_index.remove_node(&deleted_node);
        let deleted_embedding = self.embeddings.remove(key).ok_or(VRError::InvalidNodeId)?;

        Ok((deleted_node, deleted_embedding))
    }

    /// Internal node deletion from the hashmap
    fn _remove_node(&mut self, key: &str) -> Result<Node, VRError> {
        let removed_node = {
            let mut nodes = match self.nodes.write() {
                Ok(nodes) => nodes,
                Err(_) => return Err(VRError::LockAcquisitionFailed("_remove_node".to_string())),
            };
            nodes.remove(key).ok_or(VRError::InvalidNodeId)?
        };
        self.node_count -= 1;
        self.update_last_modified_to_now();
        Ok(removed_node)
    }

    // Inserts a node into the nodes hashmap
    fn _insert_node(&mut self, node: Node) -> Result<(), VRError> {
        {
            let mut nodes = match self.nodes.write() {
                Ok(nodes) => nodes,
                Err(_) => return Err(VRError::LockAcquisitionFailed("_insert_node".to_string())),
            };
            nodes.insert(node.id.clone(), node);
        }
        self.node_count += 1;
        self.update_last_modified_to_now();
        Ok(())
    }

    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn set_resource_id(&mut self, resource_id: String) {
        self.resource_id = resource_id;
        self.update_last_modified_to_now();
    }
}

/// Temporary struct used for serializing/deserializing a RwLockMapVectorResource
#[derive(Serialize, Deserialize)]
struct RwLockMapVectorResourceTemp {
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

impl Serialize for RwLockMapVectorResource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let nodes = self.nodes.read().unwrap();
        let value = RwLockMapVectorResourceTemp {
            name: self.name.clone(),
            description: self.description.clone(),
            source: self.source.clone(),
            resource_id: self.resource_id.clone(),
            resource_embedding: self.resource_embedding.clone(),
            resource_base_type: self.resource_base_type.clone(),
            embedding_model_used: self.embedding_model_used.clone(),
            embeddings: self.embeddings.clone(),
            node_count: self.node_count,
            nodes: nodes.clone(),
            data_tag_index: self.data_tag_index.clone(),
            created_datetime: self.created_datetime.clone(),
            last_modified_datetime: self.last_modified_datetime.clone(),
            metadata_index: self.metadata_index.clone(),
        };
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RwLockMapVectorResource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let temp = RwLockMapVectorResourceTemp::deserialize(deserializer)?;
        Ok(RwLockMapVectorResource {
            name: temp.name,
            description: temp.description,
            source: temp.source,
            resource_id: temp.resource_id,
            resource_embedding: temp.resource_embedding,
            resource_base_type: temp.resource_base_type,
            embedding_model_used: temp.embedding_model_used,
            embeddings: temp.embeddings,
            node_count: temp.node_count,
            nodes: RwLock::new(temp.nodes),
            data_tag_index: temp.data_tag_index,
            created_datetime: temp.created_datetime,
            last_modified_datetime: temp.last_modified_datetime,
            metadata_index: temp.metadata_index,
        })
    }
}

impl Clone for RwLockMapVectorResource {
    fn clone(&self) -> Self {
        let nodes = self.nodes.read().unwrap().clone();
        Self {
            name: self.name.clone(),
            description: self.description.clone(),
            source: self.source.clone(),
            resource_id: self.resource_id.clone(),
            resource_embedding: self.resource_embedding.clone(),
            resource_base_type: self.resource_base_type.clone(),
            embedding_model_used: self.embedding_model_used.clone(),
            embeddings: self.embeddings.clone(),
            node_count: self.node_count,
            nodes: RwLock::new(nodes),
            data_tag_index: self.data_tag_index.clone(),
            created_datetime: self.created_datetime.clone(),
            last_modified_datetime: self.last_modified_datetime.clone(),
            metadata_index: self.metadata_index.clone(),
        }
    }
}

impl PartialEq for RwLockMapVectorResource {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.description == other.description
            && self.source == other.source
            && self.resource_id == other.resource_id
            && self.resource_embedding == other.resource_embedding
            && self.resource_base_type == other.resource_base_type
            && self.embedding_model_used == other.embedding_model_used
            && self.embeddings == other.embeddings
            && self.node_count == other.node_count
            && *self.nodes.read().unwrap() == *other.nodes.read().unwrap()
            && self.data_tag_index == other.data_tag_index
            && self.created_datetime == other.created_datetime
            && self.last_modified_datetime == other.last_modified_datetime
            && self.metadata_index == other.metadata_index
    }
}
