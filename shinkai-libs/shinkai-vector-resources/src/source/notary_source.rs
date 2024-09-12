use super::DistributionInfo;

use crate::resource_errors::VRError;
use crate::source::TextChunkingStrategy;
use crate::vector_resource::SourceFileType;

use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// Struct which holds the contents of the TLSNotary proof for the source file
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct TLSNotaryProof {}

impl TLSNotaryProof {
    pub fn new() -> Self {
        TLSNotaryProof {}
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
/// The source file that data was extracted from to create a VectorResource
pub struct TLSNotarizedSourceFile {
    pub file_name: String,
    pub file_type: SourceFileType,
    pub file_content: Vec<u8>,
    pub distribution_info: Option<DistributionInfo>,
    pub proof: TLSNotaryProof,
}

impl TLSNotarizedSourceFile {
    /// Returns the size of the file content in bytes
    pub fn size(&self) -> usize {
        self.file_content.len()
    }

    /// Creates a new instance of SourceFile struct
    pub fn new(
        file_name: String,
        file_type: SourceFileType,
        file_content: Vec<u8>,
        distribution_info: Option<DistributionInfo>,
        proof: TLSNotaryProof,
    ) -> Self {
        Self {
            file_name,
            file_type,
            file_content,
            distribution_info,
            proof,
        }
    }

    pub fn format_source_string(&self) -> String {
        format!("{}.{}", self.file_name, self.file_type)
    }

    /// Serializes the SourceFile to a JSON string
    pub fn to_json(&self) -> Result<String, VRError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserializes a SourceFile from a JSON string
    pub fn from_json(json: &str) -> Result<Self, VRError> {
        Ok(serde_json::from_str(json)?)
    }
}

/// Type that acts as a reference to a notarized source file
/// (meaning one that has some cryptographic proof/signature of origin)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub enum NotarizedSourceReference {
    /// Reference to TLSNotary notarized web content
    TLSNotarized(TLSNotarizedReference),
}

impl NotarizedSourceReference {
    pub fn format_source_string(&self) -> String {
        match self {
            NotarizedSourceReference::TLSNotarized(reference) => reference.format_source_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, ToSchema)]
pub struct TLSNotarizedReference {
    pub file_name: String,
    pub file_type: SourceFileType,
    pub text_chunking_strategy: TextChunkingStrategy,
}

impl TLSNotarizedReference {
    pub fn format_source_string(&self) -> String {
        format!("{}.{}", self.file_name, self.file_type())
    }

    pub fn file_type(&self) -> SourceFileType {
        self.file_type.clone()
    }
}

impl fmt::Display for TLSNotarizedReference {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "TLS Notarized File Name: {}, File Type: {}",
            self.file_name,
            self.file_type()
        )
    }
}
