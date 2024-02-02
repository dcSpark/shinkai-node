pub use super::vector_resource_search::VectorResourceSearch;
use super::OrderedVectorResource;
use crate::data_tags::DataTagIndex;
#[cfg(feature = "native-http")]
use crate::embedding_generator::EmbeddingGenerator;
#[cfg(feature = "native-http")]
use crate::embedding_generator::RemoteEmbeddingGenerator;
use crate::embeddings::Embedding;
use crate::metadata_index::MetadataIndex;
use crate::model_type::EmbeddingModelType;
use crate::resource_errors::VRError;
use crate::shinkai_time::ShinkaiStringTime;
use crate::shinkai_time::ShinkaiTime;
pub use crate::source::VRSource;
use crate::utils::{hash_string, random_string};
use crate::vector_resource::base_vector_resources::VRBaseType;
pub use crate::vector_resource::vector_resource_types::*;
pub use crate::vector_resource::vector_search_traversal::*;
use async_trait::async_trait;
use blake3::Hash;
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
    fn source(&self) -> VRSource;
    fn keywords(&self) -> &VRKeywords;
    fn keywords_mut(&mut self) -> &mut VRKeywords;
    fn set_name(&mut self, new_name: String);
    fn set_description(&mut self, new_description: Option<String>);
    fn set_source(&mut self, new_source: VRSource);
    fn resource_id(&self) -> &str;
    fn set_resource_id(&mut self, id: String);
    fn resource_embedding(&self) -> &Embedding;
    fn set_resource_embedding(&mut self, embedding: Embedding);
    fn resource_base_type(&self) -> VRBaseType;
    fn embedding_model_used(&self) -> EmbeddingModelType;
    fn set_embedding_model_used(&mut self, model_type: EmbeddingModelType);
    fn data_tag_index(&self) -> &DataTagIndex;
    fn metadata_index(&self) -> &MetadataIndex;
    /// Retrieves an Embedding given its id, at the root level depth.
    fn get_embedding(&self, id: String) -> Result<Embedding, VRError>;
    /// Retrieves all Embeddings at the root level depth of the Vector Resource.
    fn get_root_embeddings(&self) -> Vec<Embedding>;
    /// Retrieves a copy of a Node given its id, at the root level depth.
    fn get_node(&self, id: String) -> Result<Node, VRError>;
    /// Retrieves copies of all Nodes at the root level of the Vector Resource
    fn get_root_nodes(&self) -> Vec<Node>;
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
    ) -> Result<(), VRError>;
    /// Replace a Node/Embedding in the VR using the provided id (root level depth). If no new written datetime is provided, generates now.
    fn replace_node_dt_specified(
        &mut self,
        id: String,
        node: Node,
        embedding: Embedding,
        new_written_datetime: Option<DateTime<Utc>>,
    ) -> Result<(Node, Embedding), VRError>;
    /// Remove a Node/Embedding in the VR using the provided id (root level depth). If no new written datetime is provided, generates now.
    fn remove_node_dt_specified(
        &mut self,
        id: String,
        new_written_datetime: Option<DateTime<Utc>>,
    ) -> Result<(Node, Embedding), VRError>;
    /// Removes all Nodes/Embeddings at the root level depth. If no new written datetime is provided, generates now.
    fn remove_root_nodes_dt_specified(
        &mut self,
        new_written_datetime: Option<DateTime<Utc>>,
    ) -> Result<Vec<(Node, Embedding)>, VRError>;
    /// ISO RFC3339 when then Vector Resource was created
    fn created_datetime(&self) -> DateTime<Utc>;
    /// ISO RFC3339 when then Vector Resource was last written
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
    fn insert_node(&mut self, id: String, node: Node, embedding: Embedding) -> Result<(), VRError> {
        self.insert_node_dt_specified(id, node, embedding, None)
    }

    /// Replace a Node/Embedding in the VR using the provided id (root level depth).
    fn replace_node(&mut self, id: String, node: Node, embedding: Embedding) -> Result<(Node, Embedding), VRError> {
        self.replace_node_dt_specified(id, node, embedding, None)
    }

    /// Remove a Node/Embedding in the VR using the provided id (root level depth).
    fn remove_node(&mut self, id: String) -> Result<(Node, Embedding), VRError> {
        self.remove_node_dt_specified(id, None)
    }

    /// Removes all Nodes/Embeddings at the root level depth.
    fn remove_root_nodes(&mut self) -> Result<Vec<(Node, Embedding)>, VRError> {
        self.remove_root_nodes_dt_specified(None)
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

    #[cfg(feature = "native-http")]
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

    #[cfg(feature = "native-http")]
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

    #[cfg(feature = "native-http")]
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
            result.truncate(model.max_input_token_count());
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
        )
    }

    /// Validates whether the VectorResource has a valid BaseVectorResourceType by checking its .resource_base_type()
    fn is_base_vector_resource(&self) -> Result<(), VRError> {
        VRBaseType::is_base_vector_resource(self.resource_base_type())
    }

    /// Retrieves a node and its embedding at any depth, given its path.
    /// If the path is invalid at any part, or empty, then method will error.
    fn retrieve_node_and_embedding_at_path(&self, path: VRPath) -> Result<(RetrievedNode, Embedding), VRError> {
        let results = self._internal_retrieve_node_at_path(path.clone(), None)?;
        if results.is_empty() {
            return Err(VRError::InvalidVRPath(path));
        }
        Ok((results[0].0.clone(), results[0].1.clone()))
    }

    /// Retrieves a node at any depth, given its path.
    /// If the path is invalid at any part, or empty, then method will error.
    fn retrieve_node_at_path(&self, path: VRPath) -> Result<RetrievedNode, VRError> {
        let (node, _) = self.retrieve_node_and_embedding_at_path(path)?;
        Ok(node)
    }

    /// Retrieves the embedding of a node at any depth, given its path.
    /// If the path is invalid at any part, or empty, then method will error.
    fn retrieve_embedding_at_path(&self, path: VRPath) -> Result<Embedding, VRError> {
        let (_, embedding) = self.retrieve_node_and_embedding_at_path(path)?;
        Ok(embedding)
    }

    /// Retrieves a node and `proximity_window` number of nodes before/after it, given a path.
    /// If the path is invalid at any part, or empty, then method will error.
    fn proximity_retrieve_node_at_path(
        &self,
        path: VRPath,
        proximity_window: u64,
    ) -> Result<Vec<RetrievedNode>, VRError> {
        self._internal_retrieve_node_at_path(path.clone(), Some(proximity_window))
            .map(|nodes| nodes.into_iter().map(|(node, _)| node).collect())
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
        let mut node = self.get_node(path.path_ids[0].clone())?;
        let mut embedding = self.get_embedding(path.path_ids[0].clone())?;
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
                                // TODO: Eventually update get_node_and_proximity to also return their embeddings.
                                // Technically not important, because for now we only use the embedding for non-proximity methods.
                                let new_ret_nodes = ord_res.get_node_and_proximity(id.clone(), prox_window)?;
                                retrieved_nodes = new_ret_nodes
                                    .iter()
                                    .map(|ret_node| (ret_node.clone(), embedding.clone()))
                                    .collect();
                            } else {
                                return Err(VRError::InvalidVRBaseType);
                            }
                        }
                    }
                    embedding = resource_obj.get_embedding(id.clone())?;
                    node = resource_obj.get_node(id.clone())?;
                }
                // If we hit a non VR-holding node before the end of the path, then the path is invalid
                _ => {
                    return Err(VRError::InvalidVRPath(path.clone()));
                }
            }
        }

        // If there are no retrieved nodes, then simply add the final node that was at the path
        if retrieved_nodes.is_empty() {
            retrieved_nodes.push((node, embedding));
        }

        // Convert the results into retrieved nodes
        let mut final_nodes = vec![];
        for n in retrieved_nodes {
            let mut node_path = path.pop_cloned();
            node_path.push(n.0.id.clone());
            final_nodes.push((
                RetrievedNode::new(n.0, 0.0, last_resource_header.clone(), node_path),
                n.1,
            ));
        }
        Ok(final_nodes)
    }

    /// Boolean check to see if a node exists at a given path
    fn check_node_exists_at_path(&self, path: VRPath) -> bool {
        self.retrieve_node_at_path(path).is_ok()
    }

    /// Applies a mutator function on a node and its embedding at a given path, thereby enabling updating data within a specific node.
    /// If the path is invalid at any part, or is 0 length, then method will error, and no changes will be applied to the VR.
    fn mutate_node_at_path(
        &mut self,
        path: VRPath,
        mutator: &mut dyn Fn(&mut Node, &mut Embedding) -> Result<(), VRError>,
    ) -> Result<(), VRError> {
        let current_time = ShinkaiTime::generate_time_now();
        let mut deconstructed_nodes = self._deconstruct_nodes_along_path(path.clone())?;

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

        let (node_key, node, embedding) = self._rebuild_deconstructed_nodes(deconstructed_nodes)?;
        self.replace_node_dt_specified(node_key, node, embedding, Some(current_time))?;
        Ok(())
    }

    /// Removes a specific node from the Vector Resource, based on the provided path. Returns removed Node/Embedding.
    /// If the path is invalid at any part, or is 0 length, then method will error, and no changes will be applied to the VR.
    fn remove_node_at_path(&mut self, path: VRPath) -> Result<(Node, Embedding), VRError> {
        let current_time = ShinkaiTime::generate_time_now();
        let mut deconstructed_nodes = self._deconstruct_nodes_along_path(path.clone())?;
        let removed_node = deconstructed_nodes.pop().ok_or(VRError::InvalidVRPath(path))?;

        // Update last written time for all nodes
        for node in deconstructed_nodes.iter_mut() {
            let (_, node, _) = node;
            node.set_last_written(current_time);
        }

        // Rebuild the nodes after removing the target node
        if !deconstructed_nodes.is_empty() {
            let (node_key, node, embedding) = self._rebuild_deconstructed_nodes(deconstructed_nodes)?;
            self.replace_node_dt_specified(node_key, node, embedding, Some(current_time))?;
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
    ) -> Result<(Node, Embedding), VRError> {
        let current_time = ShinkaiTime::generate_time_now();
        // Remove the node at the end of the deconstructed nodes
        let mut deconstructed_nodes = self._deconstruct_nodes_along_path(path.clone())?;
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
            let (node_key, node, embedding) = self._rebuild_deconstructed_nodes(deconstructed_nodes)?;
            let result = self.replace_node_dt_specified(node_key, node, embedding, Some(current_time))?;

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
    ) -> Result<(), VRError> {
        let current_time = ShinkaiTime::generate_time_now();
        // If inserting at root, just do it directly
        if parent_path.path_ids.is_empty() {
            self.insert_node_dt_specified(
                node_to_insert_id,
                node_to_insert,
                node_to_insert_embedding,
                Some(current_time),
            )?;
            return Ok(());
        }
        // Insert the new node at the end of the deconstructed nodes
        let mut deconstructed_nodes = self._deconstruct_nodes_along_path(parent_path.clone())?;
        deconstructed_nodes.push((node_to_insert_id, node_to_insert, node_to_insert_embedding));

        // Update last written time for all nodes
        for node in deconstructed_nodes.iter_mut() {
            let (_, node, _) = node;
            node.set_last_written(current_time);
        }

        // Rebuild the nodes after inserting the new node
        let (node_key, node, embedding) = self._rebuild_deconstructed_nodes(deconstructed_nodes)?;
        self.replace_node_dt_specified(node_key, node, embedding, Some(current_time))?;
        Ok(())
    }

    /// Appends a node underneath the provided parent_path if the resource held there implements OrderedVectorResource trait.
    /// If the parent_path is invalid at any part then method will error, and no changes will be applied to the VR.
    fn append_node_at_path(
        &mut self,
        parent_path: VRPath,
        new_node: Node,
        new_embedding: Embedding,
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
            );
        } else {
            // Get the resource node at parent_path
            let mut retrieved_node = self.retrieve_node_at_path(parent_path.clone())?;
            if let NodeContent::Resource(resource) = &mut retrieved_node.node.content {
                let ord_resource = resource.as_ordered_vector_resource()?;
                return self.insert_node_at_path(
                    parent_path.clone(),
                    ord_resource.new_push_node_id(),
                    new_node,
                    new_embedding,
                );
            }
            return Err(VRError::InvalidVRPath(parent_path.clone()));
        }
    }

    /// Pops a node underneath the provided parent_path if the resource held there implements OrderedVectorResource trait.
    /// If the parent_path is invalid at any part, or is 0 length, then method will error, and no changes will be applied to the VR.
    fn pop_node_at_path(&mut self, parent_path: VRPath) -> Result<(Node, Embedding), VRError> {
        // If the path is root, then immediately pop from self at root path to avoid error.
        if parent_path.is_empty() {
            let ord_resource = self.as_ordered_vector_resource_mut()?;
            let node_path = parent_path.push_cloned(ord_resource.last_node_id());
            return self.remove_node_at_path(node_path);
        } else {
            // Get the resource node at parent_path
            let mut retrieved_node = self.retrieve_node_at_path(parent_path.clone())?;
            // Check if its a DocumentVectorResource
            if let NodeContent::Resource(resource) = &mut retrieved_node.node.content {
                let ord_resource = resource.as_ordered_vector_resource_mut()?;
                let node_path = parent_path.push_cloned(ord_resource.last_node_id());
                self.remove_node_at_path(node_path.clone())
            } else {
                Err(VRError::InvalidVRPath(parent_path.clone()))
            }
        }
    }

    /// Internal method. Given a path, pops out each node along the path in order
    /// and returned as a list, including its original key and embedding.
    fn _deconstruct_nodes_along_path(&mut self, path: VRPath) -> Result<Vec<(String, Node, Embedding)>, VRError> {
        if path.path_ids.is_empty() {
            return Err(VRError::InvalidVRPath(path.clone()));
        }

        let first_node = self.get_node(path.path_ids[0].clone())?;
        let first_embedding = self.get_embedding(path.path_ids[0].clone())?;
        let mut deconstructed_nodes = vec![(path.path_ids[0].clone(), first_node, first_embedding)];

        for id in path.path_ids.iter().skip(1) {
            let last_mut = deconstructed_nodes
                .last_mut()
                .ok_or(VRError::InvalidVRPath(path.clone()))?;
            let (_node_key, ref mut node, ref mut _embedding) = last_mut;
            match &mut node.content {
                NodeContent::Resource(resource) => {
                    let (removed_node, removed_embedding) = resource
                        .as_trait_object_mut()
                        .remove_node_dt_specified(id.to_string(), None)?;
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
                )?;
                current_node = (id, node, embedding);
            }
        }
        Ok(current_node)
    }
}
