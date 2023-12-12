use serde_json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    map_resource::MapVectorResource, model_type::EmbeddingModelType, vector_search_traversal::VRPath,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorFSInternals {
    pub file_system_resource: MapVectorResource,
    pub identity_permissions_index: HashMap<ShinkaiName, Vec<VRPath>>,
    pub metadata_key_index: HashMap<String, Vec<VRPath>>,
    pub data_tag_index: HashMap<String, Vec<VRPath>>,
    pub subscription_index: HashMap<VRPath, Vec<ShinkaiName>>,
    pub default_embedding_model: EmbeddingModelType,
    pub supported_embedding_models: Vec<EmbeddingModelType>,
}

impl VectorFSInternals {
    pub fn new(
        file_system_resource: MapVectorResource,
        identity_permissions_index: HashMap<ShinkaiName, Vec<VRPath>>,
        metadata_key_index: HashMap<String, Vec<VRPath>>,
        data_tag_index: HashMap<String, Vec<VRPath>>,
        subscription_index: HashMap<VRPath, Vec<ShinkaiName>>,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Self {
        Self {
            file_system_resource,
            identity_permissions_index,
            metadata_key_index,
            data_tag_index,
            subscription_index,
            default_embedding_model,
            supported_embedding_models,
        }
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }
}
