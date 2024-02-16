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
/// VRKai files are saved directly as JSON
#[derive(Debug, Serialize, Deserialize)]
struct VRKai {
    pub resource: BaseVectorResource,
    pub sfm: Option<SourceFileMap>,
    pub distribution_origin: Option<DistributionOrigin>,
}

impl VRKai {
    /// Prepares the VRKai to be saved or transferred across the network as a vector of bytes.
    pub fn prepare_for_saving_as_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let json_str = serde_json::to_string(self)?;
        let compressed_bytes = compress_prepend_size(json_str.as_bytes());
        Ok(compressed_bytes)
    }

    /// Prepares the VRKai to be saved or transferred across the network as a base64 encoded string.
    pub fn prepare_for_saving_as_base64(&self) -> Result<String, Box<dyn Error>> {
        let compressed_bytes = self.prepare_for_saving_as_bytes()?;
        let base64_encoded = encode(compressed_bytes);
        Ok(base64_encoded)
    }

    /// Parses a VRKai from an array of bytes.
    pub fn from_bytes(compressed_bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        let decompressed_bytes = decompress_size_prepended(compressed_bytes)?;
        let json_str = String::from_utf8(decompressed_bytes)?;
        let vrkai = serde_json::from_str(&json_str)?;
        Ok(vrkai)
    }

    /// Parses a VRKai from a Base64 encoded string.
    pub fn from_base64(base64_encoded: &str) -> Result<Self, Box<dyn Error>> {
        let bytes = decode(base64_encoded)?;
        Self::from_bytes(&bytes)
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
