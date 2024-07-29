use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{data_tags::DataTagIndex, metadata_index::MetadataIndex, shinkai_time::ShinkaiTime, vector_resource::Node};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GraphRagResource {
    name: String,
    description: Option<String>,
    resource_id: String,
    nodes: Vec<Node>,
    edges: Vec<GraphEdge>,
    data_tag_index: DataTagIndex,
    created_datetime: DateTime<Utc>,
    last_written_datetime: DateTime<Utc>,
    metadata_index: MetadataIndex,
    merkle_root: Option<String>,
}

/// Represents a Vector Resource Graph Edge which holds a connection between graph nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub weight: f32,
}

impl GraphRagResource {
    pub fn new(name: String, description: Option<String>, nodes: Vec<Node>, edges: Vec<GraphEdge>) -> Self {
        let current_time = ShinkaiTime::generate_time_now();
        let resource_id = Uuid::new_v4().to_string();

        Self {
            name,
            description,
            resource_id,
            nodes,
            edges,
            data_tag_index: DataTagIndex::new(),
            created_datetime: current_time,
            last_written_datetime: current_time,
            metadata_index: MetadataIndex::new(),
            merkle_root: None,
        }
    }

    pub fn new_empty(name: String, description: Option<String>) -> Self {
        Self::new(name, description, Vec::new(), Vec::new())
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn get_nodes(&self) -> &Vec<Node> {
        &self.nodes
    }

    pub fn get_edges(&self) -> &Vec<GraphEdge> {
        &self.edges
    }
}
