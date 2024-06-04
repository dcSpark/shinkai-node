pub use super::vector_resource_search::VectorResourceSearch;
use super::OrderedVectorResource;
use crate::data_tags::DataTagIndex;
#[cfg(feature = "desktop-only")]
use crate::embedding_generator::EmbeddingGenerator;
#[cfg(feature = "desktop-only")]
use crate::embedding_generator::RemoteEmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::metadata_index::MetadataIndex;
use crate::model_type::EmbeddingModelType;
use crate::model_type::EmbeddingModelTypeString;
use crate::model_type::OllamaTextEmbeddingsInference;
use crate::model_type::TextEmbeddingsInference;
use crate::resource_errors::VRError;
use crate::shinkai_time::ShinkaiTime;
use crate::source::DistributionInfo;
pub use crate::source::VRSourceReference;
use crate::utils::{hash_string, random_string};
use crate::vector_resource::base_vector_resources::VRBaseType;
pub use crate::vector_resource::vector_resource_types::*;
pub use crate::vector_resource::vector_search_traversal::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::any::Any;

#[async_trait]
pub trait VectorResource: Send + Sync + VectorResourceCore + VectorResourceSearch {}

/// Represents a VectorResource as an abstract trait where new variants can be implemented as structs.
/// `resource_id` is expected to always be unique between different Resources.
#[async_trait]
pub trait VectorResourceCore: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn source(&self) -> VRSourceReference;
    fn keywords(&self) -> &VRKeywords;
    fn keywords_mut(&mut self) -> &mut VRKeywords;
    fn set_name(&mut self, new_name: String);
    fn set_description(&mut self, new_description: Option<String>);
    fn set_source(&mut self, new_source: VRSourceReference);
    fn resource_id(&self) -> &str;
    fn set_resource_id(&mut self, id: String);
    fn resource_embedding(&self) -> &Embedding;
    fn set_resource_embedding(&mut self, embedding: Embedding);
    fn resource_base_type(&self) -> VRBaseType;
    fn embedding_model_used_string(&self) -> EmbeddingModelTypeString;
    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType);
    fn distribution_info(&self) -> &DistributionInfo;
    fn set_distribution_info(&mut self, dist_info: DistributionInfo);
    fn data_tag_index(&self) -> &DataTagIndex;
    fn metadata_index(&self) -> &MetadataIndex;
    /// Retrieves an Embedding given its id, at the root level depth.
    fn get_root_embedding(&self, id: String) -> Result<Embedding, VRError>;
    /// Retrieves all Embeddings at the root level depth of the Vector Resource.
    fn get_root_embeddings(&self) -> Vec<Embedding>;
    /// Retrieves references to all Embeddings at the root level of the Vector Resource
    fn get_root_embeddings_ref(&self) -> Vec<&Embedding>;
    /// Retrieves a copy of a Node given its id, at the root level depth.
    fn get_root_node(&self, id: String) -> Result<Node, VRError>;
    /// Retrieves copies of all Nodes at the root level of the Vector Resource
    fn get_root_nodes(&self) -> Vec<Node>;
    /// Retrieves references to all Nodes at the root level of the Vector Resource
    fn get_root_nodes_ref(&self) -> Vec<&Node>;
    /// Returns the merkle root of the Vector Resource (if it is not None).
    fn get_merkle_root(&self) -> Result<String, VRError>;
    /// Sets the merkle root of the Vector Resource, errors if provided hash is not a Blake3 hash.
    fn set_merkle_root(&mut self, merkle_hash: String) -> Result<(), VRError>;
    /// Insert a Node/Embedding into the VR using the provided id (root level depth). Overwrites existing data.
    ///  If no new written datetime is provided, generates now.
    fn insert_node_dt_specified(
        &mut self,
        id: String,
        node: Node,
        embedding: Embedding,
        new_written_datetime: Option<DateTime<Utc>>,
        update_merkle_hashes: bool,
    ) -> Result<(), VRError>;
    /// Replace a Node/Embedding in the VR using the provided id (root level depth). If no new written datetime is provided, generates now.
    fn replace_node_dt_specified(
        &mut self,
        id: String,
        node: Node,
        embedding: Embedding,
        new_written_datetime: Option<DateTime<Utc>>,
        update_merkle_hashes: bool,
    ) -> Result<(Node, Embedding), VRError>;
    /// Remove a Node/Embedding in the VR using the provided id (root level depth). If no new written datetime is provided, generates now.
    fn remove_node_dt_specified(
        &mut self,
        id: String,
        new_written_datetime: Option<DateTime<Utc>>,
        update_merkle_hashes: bool,
    ) -> Result<(Node, Embedding), VRError>;
    /// Removes all Nodes/Embeddings at the root level depth. If no new written datetime is provided, generates now.
    fn remove_root_nodes_dt_specified(
        &mut self,
        new_written_datetime: Option<DateTime<Utc>>,
        update_merkle_hashes: bool,
    ) -> Result<Vec<(Node, Embedding)>, VRError>;
    /// ISO RFC3339 when then Vector Resource was created
    fn created_datetime(&self) -> DateTime<Utc>;
    /// ISO RFC3339 when then Vector Resource was last written into (a node was modified)
    fn last_written_datetime(&self) -> DateTime<Utc>;
    /// Set a RFC3339 Datetime of when then Vector Resource was last written
    fn set_last_written_datetime(&mut self, datetime: DateTime<Utc>);
    // Returns the Vector Resource's DataTagIndex
    fn get_data_tag_index(&self) -> &DataTagIndex;
    // Sets the Vector Resource's DataTagIndex
    fn set_data_tag_index(&mut self, data_tag_index: DataTagIndex);
    // Returns the Vector Resource's MetadataIndex
    fn get_metadata_index(&self) -> &MetadataIndex;
    // Sets the Vector Resource's MetadataIndex
    fn set_metadata_index(&mut self, metadata_index: MetadataIndex);
    // Note we cannot add from_json in the trait due to trait object limitations
    fn to_json(&self) -> Result<String, VRError>;
    // Note we cannot add from_json in the trait due to trait object limitations
    fn to_json_value(&self) -> Result<serde_json::Value, VRError>;
    // Convert the VectorResource into a &dyn Any
    fn as_any(&self) -> &dyn Any;
    // Convert the VectorResource into a &dyn Any
    fn as_any_mut(&mut self) -> &mut dyn Any;
    //// Attempts to cast the VectorResource into an OrderedVectorResource. Fails if
    /// the struct does not support the OrderedVectorResource trait.
    fn as_ordered_vector_resource(&self) -> Result<&dyn OrderedVectorResource, VRError>;
    /// Attempts to cast the VectorResource into an mut OrderedVectorResource. Fails if
    /// the struct does not support the OrderedVectorResource trait.
    fn as_ordered_vector_resource_mut(&mut self) -> Result<&mut dyn OrderedVectorResource, VRError>;

    /// Insert a Node/Embedding into the VR using the provided id (root level depth). Overwrites existing data.
    fn insert_root_node(&mut self, id: String, node: Node, embedding: Embedding) -> Result<(), VRError> {
        self.insert_node_dt_specified(id, node, embedding, None, true)
    }

    /// Replace a Node/Embedding in the VR using the provided id (root level depth).
    fn replace_root_node(
        &mut self,
        id: String,
        node: Node,
        embedding: Embedding,
    ) -> Result<(Node, Embedding), VRError> {
        self.replace_node_dt_specified(id, node, embedding, None, true)
    }

    /// Remove a Node/Embedding in the VR using the provided id (root level depth).
    fn remove_root_node(&mut self, id: String) -> Result<(Node, Embedding), VRError> {
        self.remove_node_dt_specified(id, None, true)
    }

    /// Removes all Nodes/Embeddings at the root level depth.
    fn remove_root_nodes(&mut self) -> Result<Vec<(Node, Embedding)>, VRError> {
        self.remove_root_nodes_dt_specified(None, true)
    }

    /// Retrieves all Nodes and their corresponding Embeddings at the root level depth of the Vector Resource.
    fn get_root_nodes_and_embeddings(&self) -> Vec<(Node, Embedding)> {
        let nodes = self.get_root_nodes();
        let embeddings = self.get_root_embeddings();
        nodes.into_iter().zip(embeddings.into_iter()).collect()
    }

    /// Returns the size of the whole Vector Resource after being encoded as JSON.
    /// Of note, encoding as JSON ensures we get accurate numbers when the user transfers/saves the VR to file.
    fn encoded_size(&self) -> Result<usize, VRError> {
        let json = self.to_json()?;
        Ok(json.as_bytes().len())
    }

    /// Checks if the Vector Resource is merkelized. Some VRs may opt to not be merkelized
    /// for specific use cases or for extremely large datasets where maximum performance is required.
    fn is_merkelized(&self) -> bool {
        self.get_merkle_root().is_ok()
    }

    /// Returns the embedding model type used by the Vector Resource.
    fn embedding_model_used(&self) -> EmbeddingModelType {
        EmbeddingModelType::from_string(&self.embedding_model_used_string()).unwrap_or(
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::Other(
                self.embedding_model_used_string().to_string(),
            )),
        )
    }

    /// Updates the merkle root of the Vector Resource by hashing the merkle hashes of all root nodes.
    /// Errors if the Vector Resource is not merkelized.
    fn update_merkle_root(&mut self) -> Result<(), VRError> {
        if !self.is_merkelized() {
            return Err(VRError::VectorResourceIsNotMerkelized(self.reference_string()));
        }

        let nodes = self.get_root_nodes();
        let mut hashes = Vec::new();

        // Collect the merkle hash of each node
        for node in nodes {
            let node_hash = node.get_merkle_hash()?;
            hashes.push(node_hash);
        }

        // Combine the hashes to create a root hash
        let combined_hashes = hashes.join("");
        let root_hash = blake3::hash(combined_hashes.as_bytes());

        // Set the new merkle root hash
        self.set_merkle_root(root_hash.to_hex().to_string())
    }

    #[cfg(feature = "desktop-only")]
    /// Regenerates and updates the resource's embedding using the name/description/source and the provided keywords.
    /// If keyword_list is None, will use the resource's set keywords (enables flexibility of which keywords get added to which embedding)
    async fn update_resource_embedding(
        &mut self,
        generator: &dyn EmbeddingGenerator,
        keyword_list: Option<Vec<String>>,
    ) -> Result<(), VRError> {
        let keywords = keyword_list.unwrap_or(self.keywords().keyword_list.clone());
        let formatted = self.format_embedding_string(keywords, generator.model_type());
        let new_embedding = generator.generate_embedding(&formatted, "RE").await?;
        self.set_resource_embedding(new_embedding);
        Ok(())
    }

    #[cfg(feature = "desktop-only")]
    /// Regenerates and updates the resource's embedding using the name/description/source and the provided keywords.
    /// If keyword_list is None, will use the resource's set keywords (enables flexibility of which keywords get added to which embedding)
    fn update_resource_embedding_blocking(
        &mut self,
        generator: &dyn EmbeddingGenerator,
        keyword_list: Option<Vec<String>>,
    ) -> Result<(), VRError> {
        let keywords = keyword_list.unwrap_or(self.keywords().keyword_list.clone());
        let formatted = self.format_embedding_string(keywords, generator.model_type());
        let new_embedding = generator.generate_embedding_blocking(&formatted, "RE")?;
        self.set_resource_embedding(new_embedding);
        Ok(())
    }

    /// Updates the last_written_datetime to the current time
    fn update_last_written_to_now(&mut self) {
        let current_time = ShinkaiTime::generate_time_now();
        self.set_last_written_datetime(current_time);
    }

    /// Generates a random new id string and sets it as the resource_id.
    /// Used in the VectorFS to guarantee each VR stored has a unique id.
    fn generate_and_update_resource_id(&mut self) {
        let mut data_string = ShinkaiTime::generate_time_now().to_rfc3339();
        data_string = data_string + self.resource_id() + self.name() + &random_string();
        let hashed_string = hash_string(&data_string);
        self.set_resource_id(hashed_string)
    }

    #[cfg(feature = "desktop-only")]
    /// Initializes a `RemoteEmbeddingGenerator` that is compatible with this VectorResource
    /// (targets the same model and interface for embedding generation). Of note, you need
    /// to make sure the api_url/api_key match for the model used.
    fn initialize_compatible_embeddings_generator(
        &self,
        api_url: &str,
        api_key: Option<String>,
    ) -> RemoteEmbeddingGenerator {
        RemoteEmbeddingGenerator::new(self.embedding_model_used(), api_url, api_key)
    }

    /// Generates a formatted string that represents the text to be used for
    /// generating the resource embedding.
    fn format_embedding_string(&self, keywords: Vec<String>, model: EmbeddingModelType) -> String {
        let name = format!("Name: {}", self.name());
        let desc = self
            .description()
            .map(|description| format!(", Description: {}", description))
            .unwrap_or_default();
        let source_string = format!("Source: {}", self.source().format_source_string());

        // Take keywords until we hit an upper token cap to ensure
        // we do not go past the embedding LLM window.
        let pre_keyword_length = name.len() + desc.len() + source_string.len();
        let mut keyword_string = String::new();
        for phrase in keywords {
            if pre_keyword_length + keyword_string.len() + phrase.len() <= model.max_input_token_count() {
                keyword_string = format!("{}, {}", keyword_string, phrase);
            }
        }

        let mut result = format!("{}{}{}, Keywords: [{}]", name, source_string, desc, keyword_string);
        if result.len() > model.max_input_token_count() {
            result = result.chars().take(model.max_input_token_count()).collect();
        }
        result
    }

    /// Returns a "reference string" that uniquely identifies the VectorResource (formatted as: `{name}:::{resource_id}`).
    /// This is also used as the unique identifier of the Vector Resource in the VectorFS.
    fn reference_string(&self) -> String {
        VRHeader::generate_resource_reference_string(self.name().to_string(), self.resource_id().to_string())
    }

    /// Generates a VRHeader out of the VectorResource.
    /// Allows specifying a resource_location to identify where the VectorResource is
    /// being stored.
    fn generate_resource_header(&self) -> VRHeader {
        // Fetch list of data tag names from the index
        let tag_names = self.data_tag_index().data_tag_names();
        let embedding = self.resource_embedding().clone();
        let metadata_index_keys = self.metadata_index().get_all_metadata_keys();
        let merkle_root = self.get_merkle_root().ok();
        let keywords = self.keywords().clone();

        VRHeader::new(
            self.name(),
            self.resource_id(),
            self.resource_base_type(),
            Some(embedding),
            tag_names,
            self.source(),
            self.created_datetime(),
            self.last_written_datetime(),
            metadata_index_keys,
            self.embedding_model_used(),
            merkle_root,
            keywords,
            self.distribution_info().clone(),
        )
    }

    /// Validates whether the VectorResource has a valid BaseVectorResourceType by checking its .resource_base_type()
    fn is_base_vector_resource(&self) -> Result<(), VRError> {
        VRBaseType::is_base_vector_resource(self.resource_base_type())
    }

    /// Retrieves a node and its embedding at any depth, given its path.
    /// If the path is invalid at any part, or empty, then method will error.
    fn retrieve_node_and_embedding_at_path(
        &self,
        path: VRPath,
        query_embedding: Option<Embedding>,
    ) -> Result<(RetrievedNode, Embedding), VRError> {
        let results = self._internal_retrieve_node_at_path(path.clone(), None)?;
        if results.is_empty() {
            return Err(VRError::InvalidVRPath(path));
        }

        // Score the retrieved node if a query embedding is provided
        let (mut ret_node, embedding) = results[0].clone();
        if let Some(query) = query_embedding {
            let score = query.score_similarity(&embedding);
            ret_node.score = score;
        }

        Ok((results[0].0.clone(), results[0].1.clone()))
    }

    /// Retrieves a node at any depth, given its path.
    /// If the path is invalid at any part, or empty, then method will error.
    fn retrieve_node_at_path(
        &self,
        path: VRPath,
        query_embedding: Option<Embedding>,
    ) -> Result<RetrievedNode, VRError> {
        let (node, _) = self.retrieve_node_and_embedding_at_path(path, query_embedding)?;
        Ok(node)
    }

    /// Retrieves the embedding of a node at any depth, given its path.
    /// If the path is invalid at any part, or empty, then method will error.
    fn retrieve_embedding_at_path(&self, path: VRPath) -> Result<Embedding, VRError> {
        let (_, embedding) = self.retrieve_node_and_embedding_at_path(path, None)?;
        Ok(embedding)
    }

    /// Retrieves a node and `proximity_window` number of nodes before/after it, given a path.
    /// If query_embedding is Some, also scores the retrieved nodes by using it (otherwise their scores default to 0.0);
    /// If the path is invalid at any part, or empty, then method will error.
    fn proximity_retrieve_nodes_at_path(
        &self,
        path: VRPath,
        proximity_window: u64,
        query_embedding: Option<Embedding>,
    ) -> Result<Vec<RetrievedNode>, VRError> {
        self.proximity_retrieve_nodes_and_embeddings_at_path(path, proximity_window, query_embedding)
            .map(|nodes| nodes.into_iter().map(|(node, _)| node).collect())
    }

    /// Retrieves a node and `proximity_window` number of nodes before/after it (including their embeddings), given a path.
    /// If query_embedding is Some, also scores the retrieved nodes by using it (otherwise their scores default to 0.0);
    /// If the path is invalid at any part, or empty, then method will error.
    fn proximity_retrieve_nodes_and_embeddings_at_path(
        &self,
        path: VRPath,
        proximity_window: u64,
        query_embedding: Option<Embedding>,
    ) -> Result<Vec<(RetrievedNode, Embedding)>, VRError> {
        let mut ret_nodes_embeddings = self._internal_retrieve_node_at_path(path.clone(), Some(proximity_window))?;

        if let Some(query) = query_embedding {
            for (ret_node, embedding) in ret_nodes_embeddings.iter_mut() {
                let score = query.score_similarity(embedding);
                ret_node.score = score;
            }
        }

        Ok(ret_nodes_embeddings)
    }

    /// Internal method shared by retrieved node at path methods
    fn _internal_retrieve_node_at_path(
        &self,
        path: VRPath,
        proximity_window: Option<u64>,
    ) -> Result<Vec<(RetrievedNode, Embedding)>, VRError> {
        if path.path_ids.is_empty() {
            return Err(VRError::InvalidVRPath(path.clone()));
        }
        // Fetch the node at root depth directly, then iterate through the rest
        let self_header = self.generate_resource_header();
        let mut node = self.get_root_node(path.path_ids[0].clone())?;
        let mut embedding = self.get_root_embedding(path.path_ids[0].clone())?;
        let mut last_resource_header = self.generate_resource_header();
        let mut retrieved_nodes = Vec::new();

        // Iterate through the path, going into each Vector Resource until end of path
        let mut traversed_path = VRPath::from_string(&(String::from("/") + &path.path_ids[0]))?;
        for id in path.path_ids.iter().skip(1) {
            traversed_path.push(id.to_string());
            match &node.content {
                NodeContent::Resource(resource) => {
                    let resource_obj = resource.as_trait_object();
                    last_resource_header = resource_obj.generate_resource_header();

                    // If we have arrived at the final node, then perform node fetching/adding to results
                    if traversed_path == path {
                        // If returning proximity, then try to coerce into an OrderedVectorResource and perform proximity get
                        if let Some(prox_window) = proximity_window {
                            if let Ok(ord_res) = resource.as_ordered_vector_resource() {
                                retrieved_nodes = ord_res.get_node_and_embedding_proximity(id.clone(), prox_window)?;
                            } else {
                                return Err(VRError::ResourceDoesNotSupportOrderedOperations(
                                    resource.as_trait_object().reference_string(),
                                ));
                            }
                        }
                    }
                    embedding = resource_obj.get_root_embedding(id.clone())?;
                    node = resource_obj.get_root_node(id.clone())?;
                }
                // If we hit a non VR-holding node before the end of the path, then the path is invalid
                _ => {
                    return Err(VRError::InvalidVRPath(path.clone()));
                }
            }
        }

        // If there are no retrieved nodes, then access via root
        if retrieved_nodes.is_empty() {
            // If returning proximity, then try to coerce into an OrderedVectorResource and perform proximity get
            if let Some(prox_window) = proximity_window {
                if let Ok(ord_res) = self.as_ordered_vector_resource() {
                    retrieved_nodes = ord_res.get_node_and_embedding_proximity(node.id.clone(), prox_window)?;
                } else {
                    return Err(VRError::ResourceDoesNotSupportOrderedOperations(
                        self.reference_string(),
                    ));
                }
            } else {
                retrieved_nodes.push((node, embedding));
            }
        }

        // Convert the results into retrieved nodes
        let mut final_nodes = vec![];
        for n in retrieved_nodes {
            let mut node_path = path.pop_cloned();
            node_path.push(n.0.id.clone());
            final_nodes.push((RetrievedNode::new(n.0, 0.0, self_header.clone(), node_path), n.1));
        }
        Ok(final_nodes)
    }

    /// Boolean check to see if a node exists at a given path
    fn check_node_exists_at_path(&self, path: VRPath) -> bool {
        self.retrieve_node_at_path(path, None).is_ok()
    }

    /// Applies a mutator function on a node and its embedding at a given path, thereby enabling updating data within a specific node.
    /// If the path is invalid at any part, or is 0 length, then method will error, and no changes will be applied to the VR.
    fn mutate_node_at_path(
        &mut self,
        path: VRPath,
        mutator: &mut dyn Fn(&mut Node, &mut Embedding) -> Result<(), VRError>,
        update_merkle_hashes: bool,
    ) -> Result<(), VRError> {
        let current_time = ShinkaiTime::generate_time_now();
        let mut deconstructed_nodes = self._deconstruct_nodes_along_path(path.clone(), update_merkle_hashes)?;

        // Update last written time for all nodes
        for node in deconstructed_nodes.iter_mut() {
            let (_, node, _) = node;
            node.set_last_written(current_time);
        }

        // Apply mutator to the last node
        if let Some(last_node) = deconstructed_nodes.last_mut() {
            let (_, node, embedding) = last_node;
            mutator(node, embedding)?;
        }

        let (node_key, node, embedding) =
            self._rebuild_deconstructed_nodes(deconstructed_nodes, update_merkle_hashes)?;
        self.replace_node_dt_specified(node_key, node, embedding, Some(current_time), update_merkle_hashes)?;
        Ok(())
    }

    /// Removes a specific node from the Vector Resource, based on the provided path. Returns removed Node/Embedding.
    /// If the path is invalid at any part, or is 0 length, then method will error, and no changes will be applied to the VR.
    fn remove_node_at_path(&mut self, path: VRPath, update_merkle_hashes: bool) -> Result<(Node, Embedding), VRError> {
        let current_time = ShinkaiTime::generate_time_now();
        let mut deconstructed_nodes = self._deconstruct_nodes_along_path(path.clone(), update_merkle_hashes)?;
        let removed_node = deconstructed_nodes.pop().ok_or(VRError::InvalidVRPath(path))?;

        // Update last written time for all nodes
        for node in deconstructed_nodes.iter_mut() {
            let (_, node, _) = node;
            node.set_last_written(current_time);
        }

        // Rebuild the nodes after removing the target node
        if !deconstructed_nodes.is_empty() {
            let (node_key, node, embedding) =
                self._rebuild_deconstructed_nodes(deconstructed_nodes, update_merkle_hashes)?;
            self.replace_node_dt_specified(node_key, node, embedding, Some(current_time), update_merkle_hashes)?;
        } else {
            // Else remove the node directly if deleting at the root level
            self.remove_node_dt_specified(removed_node.0, Some(current_time), update_merkle_hashes)?;
        }

        Ok((removed_node.1, removed_node.2))
    }

    /// Replaces a specific node from the Vector Resource, based on the provided path. Returns removed Node/Embedding.
    /// If the path is invalid at any part, or is 0 length, then method will error, and no changes will be applied to the VR.
    fn replace_node_at_path(
        &mut self,
        path: VRPath,
        new_node: Node,
        new_embedding: Embedding,
        update_merkle_hashes: bool,
    ) -> Result<(Node, Embedding), VRError> {
        let current_time = ShinkaiTime::generate_time_now();
        // Remove the node at the end of the deconstructed nodes
        let mut deconstructed_nodes = self._deconstruct_nodes_along_path(path.clone(), update_merkle_hashes)?;
        deconstructed_nodes.pop().ok_or(VRError::InvalidVRPath(path.clone()))?;

        // Insert the new node at the end of the deconstructed nodes
        if let Some(key) = path.path_ids.last() {
            deconstructed_nodes.push((key.clone(), new_node, new_embedding));

            // Update last written time for all nodes
            for node in deconstructed_nodes.iter_mut() {
                let (_, node, _) = node;
                node.set_last_written(current_time);
            }

            // Rebuild the nodes after replacing the node
            let (node_key, node, embedding) =
                self._rebuild_deconstructed_nodes(deconstructed_nodes, update_merkle_hashes)?;
            let result =
                self.replace_node_dt_specified(node_key, node, embedding, Some(current_time), update_merkle_hashes)?;

            Ok(result)
        } else {
            Err(VRError::InvalidVRPath(path.clone()))
        }
    }

    /// Inserts a node underneath the provided parent_path, using the supplied id. Supports inserting at root level `/`.
    /// If the parent_path is invalid at any part, then method will error, and no changes will be applied to the VR.
    fn insert_node_at_path(
        &mut self,
        parent_path: VRPath,
        node_to_insert_id: String,
        node_to_insert: Node,
        node_to_insert_embedding: Embedding,
        update_merkle_hashes: bool,
    ) -> Result<(), VRError> {
        let current_time = ShinkaiTime::generate_time_now();
        // If inserting at root, just do it directly
        if parent_path.path_ids.is_empty() {
            self.insert_node_dt_specified(
                node_to_insert_id,
                node_to_insert,
                node_to_insert_embedding,
                Some(current_time),
                update_merkle_hashes,
            )?;
            return Ok(());
        }
        // Insert the new node at the end of the deconstructed nodes
        let mut deconstructed_nodes = self._deconstruct_nodes_along_path(parent_path.clone(), update_merkle_hashes)?;
        deconstructed_nodes.push((node_to_insert_id, node_to_insert, node_to_insert_embedding));

        // Update last written time for all nodes
        for node in deconstructed_nodes.iter_mut() {
            let (_, node, _) = node;
            node.set_last_written(current_time);
        }

        // Rebuild the nodes after inserting the new node
        let (node_key, node, embedding) =
            self._rebuild_deconstructed_nodes(deconstructed_nodes, update_merkle_hashes)?;
        self.replace_node_dt_specified(node_key, node, embedding, Some(current_time), update_merkle_hashes)?;
        Ok(())
    }

    /// Appends a node underneath the provided parent_path if the resource held there implements OrderedVectorResource trait.
    /// If the parent_path is invalid at any part then method will error, and no changes will be applied to the VR.
    fn append_node_at_path(
        &mut self,
        parent_path: VRPath,
        new_node: Node,
        new_embedding: Embedding,
        update_merkle_hashes: bool,
    ) -> Result<(), VRError> {
        // If the path is root, then immediately insert into self at root path.
        // This is required since retrieve_node_at_path() cannot retrieved self as a node and will error.
        if parent_path.path_ids.len() == 0 {
            let ord_resource = self.as_ordered_vector_resource()?;
            return self.insert_node_at_path(
                parent_path.clone(),
                ord_resource.new_push_node_id(),
                new_node,
                new_embedding,
                update_merkle_hashes,
            );
        } else {
            // Get the resource node at parent_path
            let mut retrieved_node = self.retrieve_node_at_path(parent_path.clone(), None)?;
            if let NodeContent::Resource(resource) = &mut retrieved_node.node.content {
                let ord_resource = resource.as_ordered_vector_resource()?;
                return self.insert_node_at_path(
                    parent_path.clone(),
                    ord_resource.new_push_node_id(),
                    new_node,
                    new_embedding,
                    update_merkle_hashes,
                );
            }
            Err(VRError::InvalidVRPath(parent_path.clone()))
        }
    }

    /// Pops a node underneath the provided parent_path if the resource held there implements OrderedVectorResource trait.
    /// If the parent_path is invalid at any part, or is 0 length, then method will error, and no changes will be applied to the VR.
    fn pop_node_at_path(
        &mut self,
        parent_path: VRPath,
        update_merkle_hashes: bool,
    ) -> Result<(Node, Embedding), VRError> {
        // If the path is root, then immediately pop from self at root path to avoid error.
        if parent_path.is_empty() {
            let ord_resource = self.as_ordered_vector_resource_mut()?;
            if let Some(last_node_id) = ord_resource.last_node_id() {
                let node_path = parent_path.push_cloned(last_node_id);
                self.remove_node_at_path(node_path, update_merkle_hashes)
            } else {
                Err(VRError::InvalidNodeId("Last node id not found".to_string()))
            }
        } else {
            // Get the resource node at parent_path
            let mut retrieved_node = self.retrieve_node_at_path(parent_path.clone(), None)?;
            // Check if its a DocumentVectorResource
            if let NodeContent::Resource(resource) = &mut retrieved_node.node.content {
                let ord_resource = resource.as_ordered_vector_resource_mut()?;

                if let Some(last_node_id) = ord_resource.last_node_id() {
                    let node_path = parent_path.push_cloned(last_node_id);
                    self.remove_node_at_path(node_path.clone(), update_merkle_hashes)
                } else {
                    Err(VRError::InvalidNodeId("Last node id not found".to_string()))
                }
            } else {
                Err(VRError::InvalidVRPath(parent_path.clone()))
            }
        }
    }

    /// Internal method. Given a path, pops out each node along the path in order
    /// and returned as a list, including its original key and embedding.
    fn _deconstruct_nodes_along_path(
        &mut self,
        path: VRPath,
        update_merkle_hashes: bool,
    ) -> Result<Vec<(String, Node, Embedding)>, VRError> {
        if path.path_ids.is_empty() {
            return Err(VRError::InvalidVRPath(path.clone()));
        }

        let first_node = self.get_root_node(path.path_ids[0].clone())?;
        let first_embedding = self.get_root_embedding(path.path_ids[0].clone())?;
        let mut deconstructed_nodes = vec![(path.path_ids[0].clone(), first_node, first_embedding)];

        for id in path.path_ids.iter().skip(1) {
            let last_mut = deconstructed_nodes
                .last_mut()
                .ok_or(VRError::InvalidVRPath(path.clone()))?;
            let (_node_key, ref mut node, ref mut _embedding) = last_mut;
            match &mut node.content {
                NodeContent::Resource(resource) => {
                    let (removed_node, removed_embedding) = resource.as_trait_object_mut().remove_node_dt_specified(
                        id.to_string(),
                        None,
                        update_merkle_hashes,
                    )?;
                    deconstructed_nodes.push((id.clone(), removed_node, removed_embedding));
                }
                _ => {
                    if id != path.path_ids.last().ok_or(VRError::InvalidVRPath(path.clone()))? {
                        return Err(VRError::InvalidVRPath(path.clone()));
                    }
                }
            }
        }

        Ok(deconstructed_nodes)
    }

    /// Internal method. Given a list of deconstructed_nodes, iterate backwards and rebuild them
    /// into a single top-level node.
    fn _rebuild_deconstructed_nodes(
        &mut self,
        mut deconstructed_nodes: Vec<(String, Node, Embedding)>,
        update_merkle_hashes: bool,
    ) -> Result<(String, Node, Embedding), VRError> {
        let mut current_node = deconstructed_nodes.pop().ok_or(VRError::InvalidVRPath(VRPath::new()))?;
        for (id, mut node, embedding) in deconstructed_nodes.into_iter().rev() {
            if let NodeContent::Resource(resource) = &mut node.content {
                // Preserve the last written datetime on the node assigned by prior functions
                let current_node_last_written = current_node.1.last_written_datetime;
                resource.as_trait_object_mut().insert_node_dt_specified(
                    current_node.0,
                    current_node.1,
                    current_node.2,
                    Some(current_node_last_written),
                    update_merkle_hashes,
                )?;
                current_node = (id, node, embedding);
            }
        }
        Ok(current_node)
    }

    /// Note: Intended for internal use only (used by VectorFS).
    /// Sets the Merkle hash of a Resource node at the specified path.
    /// Does not update any other merkle hashes, thus for internal use.
    fn _set_resource_merkle_hash_at_path(&mut self, path: VRPath, merkle_hash: String) -> Result<(), VRError> {
        self.mutate_node_at_path(
            path,
            &mut |node: &mut Node, _embedding: &mut Embedding| {
                if let NodeContent::Resource(resource) = &mut node.content {
                    resource.as_trait_object_mut().set_merkle_root(merkle_hash.clone())?;
                    Ok(())
                } else {
                    Err(VRError::InvalidNodeType("Expected a folder node".to_string()))
                }
            },
            false,
        )
    }

    /// Note: Intended for internal use only (used by VectorFS).
    /// Updates the Merkle root of a Resource node at the specified path.
    fn _update_resource_merkle_hash_at_path(
        &mut self,
        path: VRPath,
        update_ancestor_merkle_hashes: bool,
    ) -> Result<(), VRError> {
        self.mutate_node_at_path(
            path,
            &mut |node: &mut Node, _embedding: &mut Embedding| {
                if let NodeContent::Resource(resource) = &mut node.content {
                    resource.as_trait_object_mut().update_merkle_root()?;
                    Ok(())
                } else {
                    Err(VRError::InvalidNodeType(
                        "Cannot update merkle root of a non-Resource node".to_string(),
                    ))
                }
            },
            update_ancestor_merkle_hashes,
        )
    }

    /// Attempts to return the DistributionInfo datetime, if not available, returns
    /// the resource_last_written_datetime.
    fn get_resource_datetime_default(&self) -> DateTime<Utc> {
        if let Some(datetime) = &self.distribution_info().datetime {
            datetime.clone()
        } else {
            self.last_written_datetime()
        }
    }

    /// Attempts to return the DistributionInfo datetime, if not available, returns
    /// the resource_created_datetime.
    fn get_resource_datetime_default_created(&self) -> DateTime<Utc> {
        if let Some(datetime) = &self.distribution_info().datetime {
            datetime.clone()
        } else {
            self.created_datetime()
        }
    }

    /// Retrieve all nodes in the Vector Resource with all hierarchy flattened.
    /// If the resource is an OrderedVectorResource, the ordering is preserved.
    fn get_all_nodes_flattened(&self) -> Vec<Node> {
        self.get_all_nodes_embeddings_flattened()
            .iter()
            .map(|(node, _)| node.clone())
            .collect()
    }

    /// Retrieve all embeddings in the Vector Resource with all hierarchy flattened.
    /// If the resource is an OrderedVectorResource, the ordering is preserved.
    fn get_all_embeddings_flattened(&self) -> Vec<Embedding> {
        self.get_all_nodes_embeddings_flattened()
            .iter()
            .map(|(_, embedding)| embedding.clone())
            .collect()
    }

    /// Retrieve all nodes and embeddings in the Vector Resource with all hierarchy flattened.
    /// If the resource is an OrderedVectorResource, the ordering is preserved.
    fn get_all_nodes_embeddings_flattened(&self) -> Vec<(Node, Embedding)> {
        let mut result_nodes = vec![];

        let mut root_nodes = self.get_root_nodes();
        for node in &mut root_nodes {
            if let Ok(resource) = node.get_vector_resource_content_mut() {
                // First remove the children nodes, and push the emptied resource node to results w/its embedding
                let child_nodes_res = resource.as_trait_object_mut().remove_root_nodes();
                let embedding_res = self.get_root_embedding(node.id.clone());
                if let Ok(embedding) = embedding_res {
                    result_nodes.push((node.clone(), embedding));
                }

                // Then we iterate through the children and retrieve their nodes/embeddings
                if let Ok(child_nodes) = child_nodes_res {
                    for (child_node, child_embedding) in child_nodes {
                        if let Ok(resource) = child_node.get_vector_resource_content() {
                            let child_ret_nodes = resource.as_trait_object().get_all_nodes_embeddings_flattened();
                            result_nodes.extend(child_ret_nodes);
                        } else {
                            result_nodes.push((child_node.clone(), child_embedding.clone()));
                        }
                    }
                }
            }
            // If its not a VR node
            else {
                let embedding_res = self.get_root_embedding(node.id.clone());
                if let Ok(embedding) = embedding_res {
                    result_nodes.push((node.clone(), embedding));
                }
            }
        }

        result_nodes
    }

    /// Generates 2 RetrievedNodes which contain either the description + 2nd node, or the first two nodes if no description is available.
    ///  Sets their score to `1.0` with empty retrieval path & id. This is intended for job vector searches to prepend the intro text about relevant VRs.
    /// Only works on OrderedVectorResources, errors otherwise.
    fn generate_intro_ret_nodes(&self) -> Result<Vec<RetrievedNode>, VRError> {
        let self_header = self.generate_resource_header();
        let mut description_node = None;

        // Create the description RetrievedNode if description exists
        if let Some(desc) = self.description() {
            let node = Node::new_text("".to_string(), desc.to_string(), None, &vec![]);
            description_node = Some(flatten_and_convert_into_intro_retrieved_node(node, &self_header));
        }

        if let Ok(ord_resource) = self.as_ordered_vector_resource() {
            if let Some(second_node) = ord_resource.get_second_node() {
                let second_node_ret = flatten_and_convert_into_intro_retrieved_node(second_node, &self_header);
                if let Some(desc) = description_node.clone() {
                    return Ok(vec![desc, second_node_ret]);
                } else {
                    if let Some(first_node) = ord_resource.get_first_node() {
                        let first_node_ret = flatten_and_convert_into_intro_retrieved_node(first_node, &self_header);
                        return Ok(vec![first_node_ret, second_node_ret]);
                    }
                }
            }
        } else if let Some(node) = description_node {
            return Ok(vec![node]);
        }
        return Err(VRError::InvalidNodeType(
            "Expected an OrderedVectorResource or a description".to_string(),
        ));
    }
}

/// Takes an intro node, flattens it if it holds a VR , and converts it into a RetrievedNode.
fn flatten_and_convert_into_intro_retrieved_node(intro_node: Node, header: &VRHeader) -> RetrievedNode {
    let node_id = intro_node.id.clone();

    if let NodeContent::Resource(resource) = &intro_node.content {
        let mut flattened_node_text = resource.as_trait_object().name().to_string();
        for node in resource.as_trait_object().get_root_nodes() {
            if flattened_node_text.len() > 500 {
                break;
            }
            if let Ok(text) = node.get_text_content() {
                flattened_node_text += " ";
                flattened_node_text += text;
            }
            if let Ok(_) = node.get_vector_resource_content() {
                flatten_and_convert_into_intro_retrieved_node(node, header);
            }
        }

        // Now create a new node with the flattened text
        let mut new_node = intro_node.clone();
        new_node.content = NodeContent::Text(flattened_node_text);

        RetrievedNode::new(new_node, 1.0, header.clone(), VRPath::new().push_cloned(node_id))
    } else {
        RetrievedNode::new(intro_node, 1.0, header.clone(), VRPath::new().push_cloned(node_id))
    }
}
