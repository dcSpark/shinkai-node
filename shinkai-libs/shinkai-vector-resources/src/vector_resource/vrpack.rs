use std::collections::HashMap;

use super::{
    deep_search_scores_average_out, BaseVectorResource, MapVectorResource, Node, NodeContent, RetrievedNode,
    ScoringMode, TraversalMethod, TraversalOption, VRKai, VRPath, VRSourceReference,
};
#[cfg(feature = "desktop-only")]
use crate::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use crate::model_type::EmbeddingModelTypeString;
use crate::{embeddings::Embedding, resource_errors::VRError};
use base64::{decode, encode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use utoipa::ToSchema;

// Versions of VRPack that are supported
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, ToSchema)]
pub enum VRPackVersion {
    #[serde(rename = "V1")]
    V1,
}

impl VRPackVersion {
    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

/// Represents a parsed VRPack file, which contains a Map Vector Resource that holds a tree structure of folders & encoded VRKai nodes.
/// In other words, a `.vrpack` file is akin to a "compressed archive" of internally held VRKais with folder structure preserved.
/// Of note, VRPacks are not compressed at the top level because the VRKais held inside already are. This improves performance for large VRPacks.
/// To save as a file or transfer the VRPack, call one of the `encode_as_` methods. To parse from a file/transfer, use the `from_` methods.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct VRPack {
    pub name: String,
    pub resource: BaseVectorResource,
    pub version: VRPackVersion,
    pub vrkai_count: u64,
    pub folder_count: u64,
    pub embedding_models_used: HashMap<EmbeddingModelTypeString, u64>,
    /// VRPack metadata enables users to add extra info that may be needed for unique use cases
    pub metadata: HashMap<String, String>,
}

impl VRPack {
    /// The default VRPack version which is used when creating new VRPacks
    pub fn default_vrpack_version() -> VRPackVersion {
        VRPackVersion::V1
    }

    /// Creates a new VRPack with the provided BaseVectorResource and the default version.
    pub fn new(
        name: &str,
        resource: BaseVectorResource,
        embedding_models_used: HashMap<EmbeddingModelTypeString, u64>,
        metadata: Option<HashMap<String, String>>,
    ) -> Self {
        let (vrkai_count, folder_count) = Self::num_of_vrkais_and_folders(&resource);

        VRPack {
            name: name.to_string(),
            resource,
            version: Self::default_vrpack_version(),
            vrkai_count,
            folder_count,
            embedding_models_used,
            metadata: metadata.unwrap_or_default(),
        }
    }

    /// Creates a new empty VRPack with an empty BaseVectorResource and the default version.
    pub fn new_empty(name: &str) -> Self {
        VRPack {
            name: name.to_string(),
            resource: BaseVectorResource::Map(MapVectorResource::new_empty(
                "vrpack",
                None,
                VRSourceReference::None,
                true,
            )),
            version: Self::default_vrpack_version(),
            vrkai_count: 0,
            folder_count: 0,
            embedding_models_used: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Prepares the VRPack to be saved or transferred as bytes.
    /// Of note, this is the bytes of the UTF-8 base64 string. This allows for easy compatibility between the two.
    pub fn encode_as_bytes(&self) -> Result<Vec<u8>, VRError> {
        if let VRPackVersion::V1 = self.version {
            let base64_encoded = self.encode_as_base64()?;
            return Ok(base64_encoded.into_bytes());
        }
        return Err(VRError::UnsupportedVRPackVersion(self.version.to_string()));
    }

    /// Prepares the VRPack to be saved or transferred across the network as a base64 encoded string.
    pub fn encode_as_base64(&self) -> Result<String, VRError> {
        if let VRPackVersion::V1 = self.version {
            let json_str = serde_json::to_string(self)?;
            let base64_encoded = encode(json_str.as_bytes());
            return Ok(base64_encoded);
        }
        return Err(VRError::UnsupportedVRPackVersion(self.version.to_string()));
    }

    /// Parses a VRPack from an array of bytes, assuming the bytes are a Base64 encoded string.
    pub fn from_bytes(base64_bytes: &[u8]) -> Result<Self, VRError> {
        // If it is Version V1
        if let Ok(base64_str) = String::from_utf8(base64_bytes.to_vec())
            .map_err(|e| VRError::VRPackParsingError(format!("UTF-8 conversion error: {}", e)))
        {
            return Self::from_base64(&base64_str);
        }

        return Err(VRError::UnsupportedVRPackVersion("".to_string()));
    }

    /// Parses a VRPack from a Base64 encoded string without compression.
    pub fn from_base64(base64_encoded: &str) -> Result<Self, VRError> {
        // If it is Version V1
        let v1 = Self::from_base64_v1(base64_encoded);
        if let Ok(vrkai) = v1 {
            return Ok(vrkai);
        } else if let Err(e) = v1 {
            eprintln!("VRPack Error: {}", e);
        }

        return Err(VRError::UnsupportedVRPackVersion("".to_string()));
    }

    /// Parses a VRPack from a Base64 encoded string using V1 logic without compression.
    fn from_base64_v1(base64_encoded: &str) -> Result<Self, VRError> {
        let bytes =
            decode(base64_encoded).map_err(|e| VRError::VRPackParsingError(format!("Base64 decoding error: {}", e)))?;
        let json_str = String::from_utf8(bytes)
            .map_err(|e| VRError::VRPackParsingError(format!("UTF-8 conversion error: {}", e)))?;
        let vrkai = serde_json::from_str(&json_str)
            .map_err(|e| VRError::VRPackParsingError(format!("JSON parsing error: {}", e)))?;
        Ok(vrkai)
    }

    /// Parses the VRPack into human-readable JSON (intended for readability in non-production use cases)
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parses the VRPack into human-readable JSON Value (intended for readability in non-production use cases)
    pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    /// Parses into a VRPack from human-readable JSON (intended for readability in non-production use cases)
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Sets the name of the VRPack.
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Sets the resource of the VRPack.
    pub fn set_resource(
        &mut self,
        resource: BaseVectorResource,
        embedding_models_used: HashMap<EmbeddingModelTypeString, u64>,
    ) {
        let (vrkai_count, folder_count) = Self::num_of_vrkais_and_folders(&resource);
        self.resource = resource;
        self.vrkai_count = vrkai_count;
        self.folder_count = folder_count;
        self.embedding_models_used = embedding_models_used;
    }

    /// Returns the ID of the VRPack.
    pub fn id(&self) -> String {
        self.resource.as_trait_object().resource_id().to_string()
    }

    /// Returns the Merkle root of the VRPack.
    pub fn merkle_root(&self) -> Result<String, VRError> {
        self.resource.as_trait_object().get_merkle_root()
    }

    /// Adds a VRKai into the VRPack inside of the specified parent path (folder or root).
    pub fn insert_vrkai(
        &mut self,
        vrkai: &VRKai,
        parent_path: VRPath,
        update_merkle_hashes: bool,
    ) -> Result<(), VRError> {
        let resource_name = vrkai.resource.as_trait_object().name().to_string();
        let embedding = vrkai.resource.as_trait_object().resource_embedding().clone();
        let metadata = None;
        let enc_vrkai = vrkai.encode_as_base64()?;
        let mut node = Node::new_text(resource_name.clone(), enc_vrkai, metadata, &vec![]);
        // We always take the merkle root of the resource, no matter what
        node.merkle_hash = Some(vrkai.resource.as_trait_object().get_merkle_root()?);

        self.resource.as_trait_object_mut().insert_node_at_path(
            parent_path,
            resource_name,
            node,
            embedding,
            update_merkle_hashes,
        )?;

        // Add the embedding model used to the hashmap
        let model = vrkai.resource.as_trait_object().embedding_model_used();
        if !self.embedding_models_used.contains_key(&model.to_string()) {
            self.embedding_models_used
                .entry(model.to_string())
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
        self.vrkai_count += 1;

        Ok(())
    }

    /// Creates a folder inside the VRPack at the specified parent path.
    pub fn create_folder(&mut self, folder_name: &str, parent_path: VRPath) -> Result<(), VRError> {
        let resource = BaseVectorResource::Map(MapVectorResource::new_empty(
            folder_name,
            None,
            VRSourceReference::None,
            true,
        ));
        let node = Node::new_vector_resource(folder_name.to_string(), &resource, None);
        let embedding = Embedding::new_empty();

        self.resource.as_trait_object_mut().insert_node_at_path(
            parent_path,
            folder_name.to_string(),
            node,
            embedding,
            true,
        )?;

        self.folder_count += 1;

        Ok(())
    }

    /// Parses a node into a VRKai.
    fn parse_node_to_vrkai(node: &Node) -> Result<VRKai, VRError> {
        match &node.content {
            NodeContent::Text(content) => {
                return VRKai::from_base64(content);
            }
            _ => Err(VRError::VRKaiParsingError("Invalid node content type".to_string())),
        }
    }

    /// Fetches the VRKai node at the specified path and parses it into a VRKai.
    pub fn get_vrkai(&self, path: VRPath) -> Result<VRKai, VRError> {
        let node = self
            .resource
            .as_trait_object()
            .retrieve_node_at_path(path.clone(), None)?;
        Self::parse_node_to_vrkai(&node.node)
    }

    /// Fetches the merkle hash of the folder at the specified path.
    pub fn get_folder_merkle_hash(&self, path: VRPath) -> Result<String, VRError> {
        let node = self
            .resource
            .as_trait_object()
            .retrieve_node_at_path(path.clone(), None)?;
        match node.node.content {
            NodeContent::Resource(resource) => Ok(resource.as_trait_object().get_merkle_root()?),
            _ => Err(VRError::InvalidNodeType(format!(
                "Node is not a folder: {} ",
                path.format_to_string()
            ))),
        }
    }

    /// Removes a node (VRKai or folder) from the VRPack at the specified path.
    pub fn remove_at_path(&mut self, path: VRPath) -> Result<(), VRError> {
        let removed_node = self.resource.as_trait_object_mut().remove_node_at_path(path, true)?;
        match removed_node.0.content {
            NodeContent::Text(vrkai_base64) => {
                // Decrease the embedding model count in the hashmap
                let vrkai = VRKai::from_base64(&vrkai_base64)?;
                let model = vrkai.resource.as_trait_object().embedding_model_used();
                if let Some(count) = self.embedding_models_used.get_mut(&model.to_string()) {
                    if *count > 1 {
                        *count -= 1;
                    } else {
                        self.embedding_models_used.remove(&model.to_string());
                    }
                }
                // Decrease vrkai count
                self.vrkai_count -= 1;
            }
            NodeContent::Resource(_) => self.folder_count += 1,
            _ => (),
        }
        Ok(())
    }

    /// Unpacks all VRKais in the VRPack, each as a tuple containing a VRKai and its corresponding VRPath where it was held at.
    pub fn unpack_all_vrkais(&self) -> Result<Vec<(VRKai, VRPath)>, VRError> {
        let nodes = self
            .resource
            .as_trait_object()
            .retrieve_nodes_exhaustive_unordered(None);

        let mut vrkais_with_paths = Vec::new();
        for retrieved_node in nodes {
            match retrieved_node.node.content {
                NodeContent::Text(_) => match Self::parse_node_to_vrkai(&retrieved_node.node) {
                    Ok(vrkai) => vrkais_with_paths.push((vrkai, retrieved_node.retrieval_path.clone())),
                    Err(e) => return Err(e),
                },
                _ => continue,
            }
        }

        Ok(vrkais_with_paths)
    }

    /// Prints the internal structure of the VRPack, starting from a given path.
    pub fn print_internal_structure(&self, starting_path: Option<VRPath>) {
        println!("{} VRPack Internal Structure:", self.name);
        println!("------------------------------------------------------------");
        let nodes = self
            .resource
            .as_trait_object()
            .retrieve_nodes_exhaustive_unordered(starting_path);
        for node in nodes {
            let ret_path = node.retrieval_path;
            let _path = ret_path.format_to_string();
            let path_depth = ret_path.path_ids.len();
            let data = match &node.node.content {
                NodeContent::Text(s) => {
                    let _text_content = if s.chars().count() > 25 {
                        s.chars().take(25).collect::<String>() + "..."
                    } else {
                        s.to_string()
                    };
                    format!("VRKai: {}", node.node.id)
                }
                NodeContent::Resource(resource) => {
                    if path_depth == 1 {
                        println!(" ");
                    }
                    format!(
                        "{} <Folder> - {} Nodes Held Inside",
                        resource.as_trait_object().name(),
                        resource.as_trait_object().get_root_embeddings().len()
                    )
                }
                _ => continue, // Skip ExternalContent and VRHeader
            };
            // Adding merkle hash if it exists to output string
            let mut merkle_hash = String::new();
            if let Ok(hash) = node.node.get_merkle_hash() {
                if hash.chars().count() > 15 {
                    merkle_hash = hash.chars().take(15).collect::<String>() + "..."
                } else {
                    merkle_hash = hash.to_string()
                }
            }

            // Create indent string and do the final print
            let indent_string = " ".repeat(path_depth * 2) + &">".repeat(path_depth);
            if merkle_hash.is_empty() {
                println!("{}{}", indent_string, data);
            } else {
                println!("{}{} | Merkle Hash: {}", indent_string, data, merkle_hash);
            }
        }
    }

    /// Performs a standard vector search within the VRPack and returns the most similar VRKais based on the input query String.
    /// Requires that there is only 1 single Embedding Model Used within the VRPack or errors.
    pub async fn vector_search_vrkai(&self, query: Embedding, num_of_results: u64) -> Result<Vec<VRKai>, VRError> {
        self.vector_search_vrkai_customized(query, num_of_results, TraversalMethod::Exhaustive, &vec![], None)
            .await
    }

    /// Performs a standard vector search within the VRPack and returns the most similar VRKais based on the input query String.
    /// Supports customizing the search starting path/traversal method/traversal options.
    /// Requires that there is only 1 single Embedding Model Used within the VRPack or errors.
    pub async fn vector_search_vrkai_customized(
        &self,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        starting_path: Option<VRPath>,
    ) -> Result<Vec<VRKai>, VRError> {
        let results = self
            .vector_search_vrkai_with_score_customized(
                query,
                num_of_results,
                traversal_method,
                traversal_options,
                starting_path,
            )
            .await?;
        let vrkais: Vec<VRKai> = results.into_iter().map(|(vrkai, _)| vrkai).collect();
        Ok(vrkais)
    }

    /// Performs a standard vector search within the VRPack and returns the most similar (VRKais, score) based on the input query String.
    /// Supports customizing the search starting path/traversal method/traversal options.
    /// Requires that there is only 1 single Embedding Model Used within the VRPack or errors.
    pub async fn vector_search_vrkai_with_score_customized(
        &self,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        starting_path: Option<VRPath>,
    ) -> Result<Vec<(VRKai, f32)>, VRError> {
        if self.embedding_models_used.keys().len() != 1 {
            return Err(VRError::VRPackEmbeddingModelError(
                "Multiple embedding models used within the VRPack, meaning standard vector searching is not supported."
                    .to_string(),
            ));
        }

        let retrieved_nodes = self.resource.as_trait_object().vector_search_customized(
            query,
            num_of_results,
            traversal_method,
            traversal_options,
            starting_path,
        );

        // Process the vrkais and the score
        let vrkais_with_score: Vec<(VRKai, f32)> = retrieved_nodes
            .into_iter()
            .filter_map(|node| {
                let vrkai = Self::parse_node_to_vrkai(&node.node);
                if let Ok(vrkai) = vrkai {
                    Some((vrkai, node.score))
                } else {
                    None
                }
            })
            .collect();

        Ok(vrkais_with_score)
    }

    /// Performs a standard deep vector search within the VRPack, returning the highest scored `RetrievedNode`s across the VRKais stored in the VRPack.
    /// Requires that there is only 1 single Embedding Model Used within the VRPack or errors.
    pub async fn deep_vector_search(
        &self,
        query: Embedding,
        num_of_vrkais_to_search_into: u64,
        num_of_results: u64,
    ) -> Result<Vec<RetrievedNode>, VRError> {
        self.deep_vector_search_customized(
            query,
            num_of_vrkais_to_search_into,
            TraversalMethod::Exhaustive,
            &vec![],
            None,
            num_of_results,
            TraversalMethod::Exhaustive,
            &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
            true,
        )
        .await
    }

    /// Performs a standard deep vector search within the VRPack, returning the highest scored `RetrievedNode`s across
    /// the VRKais stored in the VRPack. Requires that there is only 1 single Embedding Model Used within the VRPack or errors.
    /// Customized allows specifying options for the first top-level search for VRKais, and then "deep" options/method for the vector searches into the VRKais to acquire the `RetrievedNode`s.
    /// average_out_deep_search_scores: If true, averages out the VRKai top level search score, with the scores found in the nodes inside the VRKai.
    pub async fn deep_vector_search_customized(
        &self,
        query: Embedding,
        num_of_vrkais_to_search_into: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        vr_pack_starting_path: Option<VRPath>,
        num_of_results: u64,
        deep_traversal_method: TraversalMethod,
        deep_traversal_options: &Vec<TraversalOption>,
        average_out_deep_search_scores: bool,
    ) -> Result<Vec<RetrievedNode>, VRError> {
        if self.embedding_models_used.keys().len() != 1 {
            return Err(VRError::VRPackEmbeddingModelError(
                "Multiple embedding models used within the VRPack, meaning standard vector searching is not supported."
                    .to_string(),
            ));
        }

        let vrkai_results = self
            .vector_search_vrkai_with_score_customized(
                query.clone(),
                num_of_vrkais_to_search_into,
                traversal_method,
                traversal_options,
                vr_pack_starting_path,
            )
            .await?;

        // Perform vector search on all VRKai resources
        let mut retrieved_nodes = Vec::new();
        for (vrkai, score) in vrkai_results {
            let mut results = vrkai.resource.as_trait_object().vector_search_customized(
                query.clone(),
                num_of_results,
                deep_traversal_method.clone(),
                deep_traversal_options,
                None,
            );

            // If the average out deep search scores flag is set, we average the scores of the retrieved nodes
            if average_out_deep_search_scores {
                for ret_node in &mut results {
                    ret_node.score = deep_search_scores_average_out(
                        None,
                        score,
                        vrkai
                            .resource
                            .as_trait_object()
                            .description()
                            .unwrap_or_else(|| "")
                            .to_string(),
                        ret_node.score,
                        ret_node.node.get_text_content().unwrap_or_else(|_| "").to_string(),
                    );
                }
            }
            retrieved_nodes.extend(results);
        }

        // Sort the retrieved nodes by score before returning
        let sorted_retrieved_nodes = RetrievedNode::sort_by_score(&retrieved_nodes, num_of_results);

        Ok(sorted_retrieved_nodes)
    }

    #[cfg(feature = "desktop-only")]
    /// Performs a dynamic vector search within the VRPack and returns the most similar VRKais based on the input query String.
    /// This allows for multiple embedding models to be used within the VRPack, as it automatically generates the input query embedding.
    pub async fn dynamic_vector_search_vrkai(
        &self,
        input_query: String,
        num_of_results: u64,
        embedding_generator: RemoteEmbeddingGenerator,
    ) -> Result<Vec<VRKai>, VRError> {
        self.dynamic_vector_search_vrkai_customized(input_query, num_of_results, &vec![], None, embedding_generator)
            .await
    }

    #[cfg(feature = "desktop-only")]
    /// Performs a dynamic vector search within the VRPack and returns the most similar VRKais based on the input query String.
    /// Supports customizing the search starting path/traversal options.
    /// This allows for multiple embedding models to be used within the VRPack, as it automatically generates the input query embedding.
    pub async fn dynamic_vector_search_vrkai_customized(
        &self,
        input_query: String,
        num_of_results: u64,
        traversal_options: &Vec<TraversalOption>,
        starting_path: Option<VRPath>,
        embedding_generator: RemoteEmbeddingGenerator,
    ) -> Result<Vec<VRKai>, VRError> {
        let results = self
            .dynamic_vector_search_vrkai_with_score_and_path_customized(
                input_query,
                num_of_results,
                traversal_options,
                starting_path,
                embedding_generator,
            )
            .await?;
        let vrkais: Vec<VRKai> = results.into_iter().map(|(vrkai, _, _)| vrkai).collect();
        Ok(vrkais)
    }

    #[cfg(feature = "desktop-only")]
    /// Performs a dynamic vector search within the VRPack and returns the most similar (VRKai, score) based on the input query String.
    /// Supports customizing the search starting path/traversal options.
    /// This allows for multiple embedding models to be used within the VRPack, as it automatically generates the input query embedding.
    pub async fn dynamic_vector_search_vrkai_with_score_and_path_customized(
        &self,
        input_query: String,
        num_of_results: u64,
        traversal_options: &Vec<TraversalOption>,
        starting_path: Option<VRPath>,
        embedding_generator: RemoteEmbeddingGenerator,
    ) -> Result<Vec<(VRKai, f32, VRPath)>, VRError> {
        let retrieved_nodes = self
            .resource
            .as_trait_object()
            .dynamic_vector_search_customized(
                input_query,
                num_of_results,
                traversal_options,
                starting_path,
                embedding_generator,
            )
            .await?;

        // Process the vrkais and the score
        let vrkais_with_score: Vec<(VRKai, f32, VRPath)> = retrieved_nodes
            .into_iter()
            .filter_map(|node| {
                let vrkai = Self::parse_node_to_vrkai(&node.node);
                if let Ok(vrkai) = vrkai {
                    Some((vrkai, node.score, node.retrieval_path))
                } else {
                    None
                }
            })
            .collect();

        Ok(vrkais_with_score)
    }

    #[cfg(feature = "desktop-only")]
    /// Performs a dynamic deep vector search within the VRPack, returning the highest scored `RetrievedNode`s across
    /// the VRKais stored in the VRPack.
    /// This allows for multiple embedding models to be used within the VRPack, as it automatically generates the input query embedding.
    pub async fn dynamic_deep_vector_search(
        &self,
        input_query: String,
        num_of_vrkais_to_search_into: u64,
        num_of_results: u64,
        embedding_generator: RemoteEmbeddingGenerator,
    ) -> Result<Vec<RetrievedNode>, VRError> {
        self.dynamic_deep_vector_search_customized(
            input_query,
            num_of_vrkais_to_search_into,
            &vec![],
            None,
            num_of_results,
            TraversalMethod::Exhaustive,
            &vec![TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring)],
            embedding_generator,
            true,
        )
        .await
    }

    #[cfg(feature = "desktop-only")]
    /// Performs a dynamic deep vector search within the VRPack, returning the highest scored `RetrievedNode`s across
    /// the VRKais stored in the VRPack. This allows for multiple embedding models to be used within the VRPack, as it automatically generates the input query embedding.
    /// Customized allows specifying options for the first top-level search for VRKais, and then "deep" options/method for the vector searches into the VRKais to acquire the `RetrievedNode`s.
    /// average_out_deep_search_scores: If true, averages out the VRKai top level search score, with the scores found in the nodes inside the VRKai.
    pub async fn dynamic_deep_vector_search_customized(
        &self,
        input_query: String,
        num_of_vrkais_to_search_into: u64,
        traversal_options: &Vec<TraversalOption>,
        vr_pack_starting_path: Option<VRPath>,
        num_of_results: u64,
        deep_traversal_method: TraversalMethod,
        deep_traversal_options: &Vec<TraversalOption>,
        embedding_generator: RemoteEmbeddingGenerator,
        average_out_deep_search_scores: bool,
    ) -> Result<Vec<RetrievedNode>, VRError> {
        self.dynamic_deep_vector_search_with_vrkai_path_customized(
            input_query,
            num_of_vrkais_to_search_into,
            traversal_options,
            vr_pack_starting_path,
            num_of_results,
            deep_traversal_method,
            deep_traversal_options,
            embedding_generator,
            average_out_deep_search_scores,
        )
        .await
        .map(|retrieved_nodes| retrieved_nodes.into_iter().map(|(ret_node, _)| ret_node).collect())
    }

    #[cfg(feature = "desktop-only")]
    /// Performs a dynamic deep vector search within the VRPack, returning the highest scored `RetrievedNode`s across
    /// the VRKais stored in the VRPack (with the relative VRPath of the VRKai in the VRPack). This allows for multiple embedding models to be used within the VRPack, as it automatically generates the input query embedding.
    /// Customized allows specifying options for the first top-level search for VRKais, and then "deep" options/method for the vector searches into the VRKais to acquire the `RetrievedNode`s.
    /// average_out_deep_search_scores: If true, averages out the VRKai top level search score, with the scores found in the nodes inside the VRKai.
    pub async fn dynamic_deep_vector_search_with_vrkai_path_customized(
        &self,
        input_query: String,
        num_of_vrkais_to_search_into: u64,
        traversal_options: &Vec<TraversalOption>,
        vr_pack_starting_path: Option<VRPath>,
        num_of_results: u64,
        deep_traversal_method: TraversalMethod,
        deep_traversal_options: &Vec<TraversalOption>,
        embedding_generator: RemoteEmbeddingGenerator,
        average_out_deep_search_scores: bool,
    ) -> Result<Vec<(RetrievedNode, VRPath)>, VRError> {
        let mut path_hashmap: HashMap<String, VRPath> = HashMap::new();

        let vrkai_results = self
            .dynamic_vector_search_vrkai_with_score_and_path_customized(
                input_query.clone(),
                num_of_vrkais_to_search_into,
                traversal_options,
                vr_pack_starting_path.clone(),
                embedding_generator.clone(),
            )
            .await?;

        let mut retrieved_nodes = Vec::new();
        // Perform vector search on all VRKai resources
        for (vrkai, score, path) in vrkai_results {
            let query_embedding = embedding_generator.generate_embedding_default(&input_query).await?;
            let mut results = vrkai.resource.as_trait_object().vector_search_customized(
                query_embedding,
                num_of_results,
                deep_traversal_method.clone(),
                &deep_traversal_options,
                None,
            );

            // Populate the path hashmap with the VRKai header string as the key and the VRPath as the value
            let vrkai_header = vrkai.resource.as_trait_object().generate_resource_header();
            path_hashmap.entry(vrkai_header.reference_string()).or_insert(path);

            // If the average out deep search scores flag is set, we average the scores of the retrieved nodes
            if average_out_deep_search_scores {
                for ret_node in &mut results {
                    ret_node.score = deep_search_scores_average_out(
                        Some(input_query.clone()),
                        score,
                        vrkai
                            .resource
                            .as_trait_object()
                            .description()
                            .unwrap_or_else(|| "")
                            .to_string(),
                        ret_node.score,
                        ret_node.node.get_text_content().unwrap_or_else(|_| "").to_string(),
                    );
                }
            }

            retrieved_nodes.extend(results);
        }

        // Sort the retrieved nodes by score before returning
        let sorted_retrieved_nodes = RetrievedNode::sort_by_score(&retrieved_nodes, num_of_results);

        // Reattach the VRPath from the path hashmap
        let retrieved_nodes_with_path = sorted_retrieved_nodes
            .into_iter()
            .map(|retrieved_node| {
                let ref_string = retrieved_node.resource_header.reference_string();
                let default_path = VRPath::root();
                let path = path_hashmap.get(&ref_string).unwrap_or_else(|| &default_path).clone();
                (retrieved_node, path)
            })
            .collect();

        Ok(retrieved_nodes_with_path)
    }

    /// Counts the number of VRKais and folders in the BaseVectorResource.
    fn num_of_vrkais_and_folders(resource: &BaseVectorResource) -> (u64, u64) {
        let nodes = resource.as_trait_object().retrieve_nodes_exhaustive_unordered(None);

        let (vrkais_count, folders_count) = nodes.iter().fold((0u64, 0u64), |(vrkais, folders), retrieved_node| {
            match retrieved_node.node.content {
                NodeContent::Text(_) => (vrkais + 1, folders),
                NodeContent::Resource(_) => (vrkais, folders + 1),
                _ => (vrkais, folders),
            }
        });

        (vrkais_count, folders_count)
    }

    /// Generates a simplified JSON representation of the contents of the VRPack.
    pub fn to_json_contents_simplified(&self) -> Result<String, VRError> {
        let nodes = self
            .resource
            .as_trait_object()
            .retrieve_nodes_exhaustive_unordered(None);

        let mut content_vec = Vec::new();

        for retrieved_node in nodes {
            let ret_path = retrieved_node.retrieval_path;
            let path = ret_path.format_to_string();
            let path_depth = ret_path.path_ids.len();

            let json_node = match &retrieved_node.node.content {
                NodeContent::Text(_) => {
                    json!({
                        "name": retrieved_node.node.id,
                        "type": "vrkai",
                        "path": path,
                        "merkle_hash": retrieved_node.node.get_merkle_hash().unwrap_or_default(),
                    })
                }
                NodeContent::Resource(_) => {
                    json!({
                        "name": retrieved_node.node.id,
                        "type": "folder",
                        "path": path,
                        "contents": [],
                    })
                }
                _ => continue,
            };

            if path_depth == 0 {
                content_vec.push(json_node);
            } else {
                let parent_path = ret_path.parent_path().format_to_string();
                Self::insert_node_into_json_vec(&mut content_vec, parent_path, json_node);
            }
        }

        // Convert hashmap into simpler list of embedding model used strings
        let embeddings_models_used_list = self
            .embedding_models_used
            .keys()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        let simplified_json = json!({
            "name": self.name,
            "vrkai_count": self.vrkai_count,
            "folder_count": self.folder_count,
            "version": self.version.to_string(),
            "embedding_models_used": embeddings_models_used_list,
            "metadata": self.metadata,
            "content": content_vec,
        });

        serde_json::to_string(&simplified_json)
            .map_err(|e| VRError::VRPackParsingError(format!("JSON serialization error: {}", e)))
    }

    fn insert_node_into_json_vec(content_vec: &mut Vec<JsonValue>, parent_path: String, json_node: JsonValue) {
        for node in content_vec.iter_mut() {
            if let Some(path) = node["path"].as_str() {
                if path == parent_path {
                    if let Some(contents) = node["contents"].as_array_mut() {
                        contents.push(json_node);
                        return;
                    }
                } else if parent_path.starts_with(path) {
                    if let Some(contents) = node["contents"].as_array_mut() {
                        Self::insert_node_into_json_vec(contents, parent_path, json_node);
                        return;
                    }
                }
            }
        }
        // If the parent node is not found, it means the json_node should be added to the root content_vec
        content_vec.push(json_node);
    }

    /// Inserts a key-value pair into the VRPack's metadata. Replaces existing value if key already exists.
    pub fn metadata_insert(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Retrieves the value associated with a key from the VRPack's metadata.
    pub fn metadata_get(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Removes a key-value pair from the VRPack's metadata given the key.
    pub fn metadata_remove(&mut self, key: &str) -> Option<String> {
        self.metadata.remove(key)
    }

    /// Note: Intended for internal use only (used by VectorFS).
    /// Sets the Merkle hash of a folder node at the specified path.
    pub fn _set_folder_merkle_hash(&mut self, path: VRPath, merkle_hash: String) -> Result<(), VRError> {
        self.resource
            .as_trait_object_mut()
            ._set_resource_merkle_hash_at_path(path, merkle_hash)?;
        Ok(())
    }

    /// Generates 2 RetrievedNodes which contain either the description + 2nd node, or the first two nodes if no description is available.
    ///  Sets their score to `1.0` with empty retrieval path & id. This is intended for job vector searches to prepend the intro text about relevant VRs.
    /// Only works on OrderedVectorResources, errors otherwise.
    pub fn get_vrkai_intro_ret_nodes(&self, path: VRPath) -> Result<Vec<RetrievedNode>, VRError> {
        let vrkai = self.get_vrkai(path)?;
        vrkai.resource.as_trait_object().generate_intro_ret_nodes()
    }
}
