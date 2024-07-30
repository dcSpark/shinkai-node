use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    data_tags::DataTagIndex, embeddings::Embedding, metadata_index::MetadataIndex, shinkai_time::ShinkaiTime,
    source::SourceFileMap,
};

/// A GraphRAG resource which embeds a parquet file and metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphRagResource {
    pub name: String,
    pub description: Option<String>,
    pub parquet: Vec<u8>,
    pub data_tag_index: DataTagIndex,
    pub metadata_index: MetadataIndex,
    pub sfm: Option<SourceFileMap>,
}

impl GraphRagResource {
    pub fn new(name: String, description: Option<String>, parquet: Vec<u8>) -> Self {
        Self {
            name,
            description,
            parquet,
            data_tag_index: DataTagIndex::new(),
            metadata_index: MetadataIndex::new(),
            sfm: None,
        }
    }
}

/// Represents a pack of GraphRAG resources which is the output of the GraphRAG indexing pipeline
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphRagPack {
    pub name: String,
    pub resources: Vec<GraphRagResource>,
    // TODO: verify if it has the same structure as VR embeddings
    pub embeddings: Vec<Embedding>,
    pub metadata: HashMap<String, String>,
}
