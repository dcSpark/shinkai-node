use super::BaseVectorResource;
use crate::{
    resource_errors::VRError,
    source::{DistributionOrigin, SourceFileMap},
};
use base64::decode;
use base64::encode;
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Represents a parsed VRKai file with a BaseVectorResource, and optional SourceFileMap/DistributionOrigin.
/// To save as a file or transfer the VRKai, call one of the `prepare_as_` methods. To parse from a file/transfer, use the `from_` methods.
#[derive(Debug, Serialize, Deserialize)]
pub struct VRKai {
    pub resource: BaseVectorResource,
    pub sfm: Option<SourceFileMap>,
    pub distribution_origin: Option<DistributionOrigin>,
}

impl VRKai {
    /// Creates a new VRKai instance from a BaseVectorResource, with optional SourceFileMap and DistributionOrigin.
    pub fn from_base_vector_resource(
        resource: BaseVectorResource,
        sfm: Option<SourceFileMap>,
        distribution_origin: Option<DistributionOrigin>,
    ) -> Self {
        VRKai {
            resource,
            sfm,
            distribution_origin,
        }
    }

    /// Prepares the VRKai to be saved or transferred as a compressed bytes (from the base64 String).
    pub fn prepare_as_bytes(&self) -> Result<Vec<u8>, VRError> {
        let base64_encoded = self.prepare_as_base64()?;
        Ok(base64_encoded.into_bytes())
    }

    /// Prepares the VRKai to be saved or transferred across the network as a compressed base64 encoded string.
    pub fn prepare_as_base64(&self) -> Result<String, VRError> {
        let json_str = serde_json::to_string(self)?;
        let compressed_bytes = compress_prepend_size(json_str.as_bytes());
        let base64_encoded = encode(compressed_bytes);
        Ok(base64_encoded)
    }

    /// Parses a VRKai from an array of bytes, assuming the bytes are a Base64 encoded string.
    pub fn from_bytes(base64_bytes: &[u8]) -> Result<Self, VRError> {
        let base64_str = String::from_utf8(base64_bytes.to_vec())
            .map_err(|e| VRError::VRKaiParsingError(format!("UTF-8 conversion error: {}", e)))?;
        Self::from_base64(&base64_str)
    }

    /// Parses a VRKai from a Base64 encoded string.
    pub fn from_base64(base64_encoded: &str) -> Result<Self, VRError> {
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

    /// Parses into a VRKai from human-readable JSON (intended for readability in non-production use cases)
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}
