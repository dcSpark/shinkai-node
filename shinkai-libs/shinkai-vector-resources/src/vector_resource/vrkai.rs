use super::{BaseVectorResource, RetrievedNode, TraversalMethod, TraversalOption, VRPath, VectorSearchMode};
use crate::{embeddings::Embedding, resource_errors::VRError, source::SourceFileMap};
use base64::{decode, encode};
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use std::collections::HashMap;

// Versions of VRKai that are supported
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, ToSchema)]
pub enum VRKaiVersion {
    #[serde(rename = "V1")]
    V1,
}

impl VRKaiVersion {
    pub fn to_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum RAGStrategy {
    Basic,
}

impl Default for RAGStrategy {
    fn default() -> Self {
        RAGStrategy::Basic
    }
}

/// Represents a parsed VRKai file with a BaseVectorResource, and optional SourceFileMap.
/// To save as a file or transfer the VRKai, call one of the `prepare_as_` methods. To parse from a file/transfer, use the `from_` methods.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct VRKai {
    pub resource: BaseVectorResource,
    pub sfm: Option<SourceFileMap>,
    pub version: VRKaiVersion,
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub rag_strategy: RAGStrategy,
    #[serde(default)]
    pub total_token_count: u64,
}

impl VRKai {
    /// The default VRKai version which is used when creating new VRKais
    pub fn default_vrkai_version() -> VRKaiVersion {
        VRKaiVersion::V1
    }

    /// Creates a new VRKai instance from a BaseVectorResource, with optional SourceFileMap.
    pub fn new(resource: BaseVectorResource, sfm: Option<SourceFileMap>) -> Self {
        let total_token_count = resource.as_trait_object().count_total_tokens();
        VRKai {
            resource,
            sfm,
            version: Self::default_vrkai_version(),
            metadata: HashMap::new(),
            rag_strategy: RAGStrategy::Basic,
            total_token_count,
        }
    }

    /// Returns the name of the Vector Resource stored in the VRKai
    pub fn name(&self) -> String {
        self.resource.as_trait_object().name().to_string()
    }

    /// Prepares the VRKai to be saved or transferred as compressed bytes.
    /// Of note, this is the bytes of the UTF-8 base64 string. This allows for easy compatibility between the two.
    pub fn encode_as_bytes(&self) -> Result<Vec<u8>, VRError> {
        if let VRKaiVersion::V1 = self.version {
            let base64_encoded = self.encode_as_base64()?;
            return Ok(base64_encoded.into_bytes());
        }
        Err(VRError::UnsupportedVRKaiVersion(self.version.to_string()))
    }

    /// Prepares the VRKai to be saved or transferred across the network as a compressed base64 encoded string.
    pub fn encode_as_base64(&self) -> Result<String, VRError> {
        if let VRKaiVersion::V1 = self.version {
            let json_str = serde_json::to_string(self)?;
            let compressed_bytes = compress_prepend_size(json_str.as_bytes());
            let base64_encoded = encode(compressed_bytes);
            return Ok(base64_encoded);
        }
        Err(VRError::UnsupportedVRKaiVersion(self.version.to_string()))
    }

    /// Parses a VRKai from an array of bytes, assuming the bytes are a Base64 encoded string.
    pub fn from_bytes(base64_bytes: &[u8]) -> Result<Self, VRError> {
        // If it is Version V1
        if let Ok(base64_str) = String::from_utf8(base64_bytes.to_vec())
            .map_err(|e| VRError::VRKaiParsingError(format!("UTF-8 conversion error: {}", e)))
        {
            return Self::from_base64(&base64_str);
        }

        Err(VRError::UnsupportedVRKaiVersion("".to_string()))
    }

    /// Parses a VRKai from a Base64 encoded string.
    pub fn from_base64(base64_encoded: &str) -> Result<Self, VRError> {
        // If it is Version V1
        if let Ok(vrkai) = Self::from_base64_v1(base64_encoded) {
            return Ok(vrkai);
        }

        Err(VRError::UnsupportedVRKaiVersion("".to_string()))
    }

    /// Parses a VRKai from a Base64 encoded string using V1 logic.
    fn from_base64_v1(base64_encoded: &str) -> Result<Self, VRError> {
        let bytes =
            decode(base64_encoded).map_err(|e| VRError::VRKaiParsingError(format!("Base64 decoding error: {}", e)))?;
        let decompressed_bytes = decompress_size_prepended(&bytes)
            .map_err(|e| VRError::VRKaiParsingError(format!("Decompression error: {}", e)))?;
        let json_str = String::from_utf8(decompressed_bytes)
            .map_err(|e| VRError::VRKaiParsingError(format!("UTF-8 conversion error: {}", e)))?;
        let vrkai = serde_json::from_str(&json_str)
            .map_err(|e| VRError::VRKaiParsingError(format!("JSON parsing error: {}", e)))?;
        Ok(vrkai)
    }

    /// Parses the VRKai into human-readable JSON (intended for readability in non-production use cases)
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parses the VRKai into human-readable JSON Value (intended for readability in non-production use cases)
    pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }

    /// Parses into a VRKai from human-readable JSON (intended for readability in non-production use cases)
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
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

    /// Performs a vector search that returns the most similar nodes based on the query with
    /// default traversal method/options.
    pub fn vector_search(&self, query: Embedding, num_of_results: u64) -> Vec<RetrievedNode> {
        self.resource.as_trait_object().vector_search(query, num_of_results)
    }

    /// Performs a vector search that returns the most similar nodes based on the query.
    /// The input traversal_method/options allows the developer to choose how the search moves through the levels.
    /// The optional starting_path allows the developer to choose to start searching from a Vector Resource
    /// held internally at a specific path.
    pub fn vector_search_customized(
        &self,
        query: Embedding,
        num_of_results: u64,
        traversal_method: TraversalMethod,
        traversal_options: &Vec<TraversalOption>,
        starting_path: Option<VRPath>,
        vector_search_mode: VectorSearchMode,
    ) -> Vec<RetrievedNode> {
        self.resource.as_trait_object().vector_search_customized(
            query,
            num_of_results,
            traversal_method,
            traversal_options,
            starting_path,
            vector_search_mode,
        )
    }

    pub fn count_total_tokens(&self) -> u64 {
        self.resource.as_trait_object().count_total_tokens()
    }
}
