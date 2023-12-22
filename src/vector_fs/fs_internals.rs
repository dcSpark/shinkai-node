use serde_json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    map_resource::MapVectorResource,
    model_type::{EmbeddingModelType, TextEmbeddingsInference},
    vector_search_traversal::{VRPath, VRSource},
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorFSInternals {
    pub file_system_core_resource: MapVectorResource,
    pub identity_permissions_index: HashMap<ShinkaiName, Vec<VRPath>>,
    pub subscription_index: HashMap<VRPath, Vec<ShinkaiName>>,
    pub default_embedding_model: EmbeddingModelType,
    pub supported_embedding_models: Vec<EmbeddingModelType>,
}

impl VectorFSInternals {
    pub fn new(
        file_system_core_resource: MapVectorResource,
        identity_permissions_index: HashMap<ShinkaiName, Vec<VRPath>>,
        subscription_index: HashMap<VRPath, Vec<ShinkaiName>>,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Self {
        Self {
            file_system_core_resource,
            identity_permissions_index,
            subscription_index,
            default_embedding_model,
            supported_embedding_models,
        }
    }

    /// IMPORTANT: This creates a barebones empty struct, intended to be used for tests
    /// that do not require a real filled out internals struct.
    pub fn new_empty() -> Self {
        Self {
            file_system_core_resource: MapVectorResource::new_empty("", None, VRSource::None, ""),
            identity_permissions_index: HashMap::new(),
            subscription_index: HashMap::new(),
            default_embedding_model: EmbeddingModelType::TextEmbeddingsInference(
                TextEmbeddingsInference::AllMiniLML6v2,
            ),
            supported_embedding_models: Vec::new(),
        }
    }

    /// A hard-coded DB key for the profile-wide VectorFSInternals
    /// Nothing else is allowed to use this shinkai db key (this is enforced
    /// automatically because all resources have a two-part key/different topic)
    pub fn profile_fs_internals_shinkai_db_key() -> String {
        "profile_vec_fs_internals".to_string()
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }
}
