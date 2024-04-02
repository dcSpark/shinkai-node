use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NodeHealthStatus {
    pub is_pristine: bool,
    pub node_name: String,
    pub status: String,
    pub version: String,
}
