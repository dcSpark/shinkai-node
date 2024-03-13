use super::{BaseVectorResource, MapVectorResource, VRSource};
use crate::{
    resource_errors::VRError,
    source::{DistributionOrigin, SourceFileMap},
    vector_resource::vrkai,
};
use base64::decode;
use base64::encode;
use lz4_flex::{compress_prepend_size, decompress_size_prepended};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

// Versions of VRPack that are supported
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
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
/// To save as a file or transfer the VRPack, call one of the `encode_as_` methods. To parse from a file/transfer, use the `from_` methods.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct VRPack {
    pub resource: BaseVectorResource,
    pub version: VRPackVersion,
}

impl VRPack {
    /// The default VRPack version which is used when creating new VRPacks
    pub fn default_vrpack_version() -> VRPackVersion {
        VRPackVersion::V1
    }

    /// Creates a new VRPack with the provided BaseVectorResource and the default version.
    pub fn new(resource: BaseVectorResource) -> Self {
        VRPack {
            resource,
            version: Self::default_vrpack_version(),
        }
    }

    /// Creates a new empty VRPack with an empty BaseVectorResource and the default version.
    pub fn new_empty() -> Self {
        VRPack {
            resource: BaseVectorResource::Map(MapVectorResource::new_empty("vrpack", None, VRSource::None, true)),
            version: Self::default_vrpack_version(),
        }
    }

    /// Prepares the VRPack to be saved or transferred as compressed bytes.
    /// Of note, this is the bytes of the UTF-8 base64 string. This allows for easy compatibility between the two.
    pub fn encode_as_bytes(&self) -> Result<Vec<u8>, VRError> {
        if let VRPackVersion::V1 = self.version {
            let base64_encoded = self.encode_as_base64()?;
            return Ok(base64_encoded.into_bytes());
        }
        return Err(VRError::UnsupportedVRPackVersion(self.version.to_string()));
    }

    /// Prepares the VRPack to be saved or transferred across the network as a compressed base64 encoded string.
    pub fn encode_as_base64(&self) -> Result<String, VRError> {
        if let VRPackVersion::V1 = self.version {
            let json_str = serde_json::to_string(self)?;
            let compressed_bytes = compress_prepend_size(json_str.as_bytes());
            let base64_encoded = encode(compressed_bytes);
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

    /// Parses a VRPack from a Base64 encoded string.
    pub fn from_base64(base64_encoded: &str) -> Result<Self, VRError> {
        // If it is Version V1
        if let Ok(vrkai) = Self::from_base64_v1(base64_encoded) {
            return Ok(vrkai);
        }

        return Err(VRError::UnsupportedVRPackVersion("".to_string()));
    }

    /// Parses a VRPack from a Base64 encoded string using V1 logic.
    fn from_base64_v1(base64_encoded: &str) -> Result<Self, VRError> {
        let bytes =
            decode(base64_encoded).map_err(|e| VRError::VRPackParsingError(format!("Base64 decoding error: {}", e)))?;
        let decompressed_bytes = decompress_size_prepended(&bytes)
            .map_err(|e| VRError::VRPackParsingError(format!("Decompression error: {}", e)))?;
        let json_str = String::from_utf8(decompressed_bytes)
            .map_err(|e| VRError::VRPackParsingError(format!("UTF-8 conversion error: {}", e)))?;
        let vrkai = serde_json::from_str(&json_str)
            .map_err(|e| VRError::VRPackParsingError(format!("JSON parsing error: {}", e)))?;
        Ok(vrkai)
    }

    /// Parses the VRPack into human-readable JSON (intended for readability in non-production use cases)
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parses into a VRPack from human-readable JSON (intended for readability in non-production use cases)
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}
