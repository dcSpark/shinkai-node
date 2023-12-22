use super::permissions::PermissionsIndex;
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
    pub permissions_index: PermissionsIndex,
    pub subscription_index: HashMap<VRPath, Vec<ShinkaiName>>,
    pub default_embedding_model: EmbeddingModelType,
    pub supported_embedding_models: Vec<EmbeddingModelType>,
}

impl VectorFSInternals {
    pub fn new(
        node_name: ShinkaiName,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Self {
        Self {
            file_system_core_resource: MapVectorResource::new_empty(
                "VecFS Core Resource",
                None,
                VRSource::None,
                "core",
            ),
            permissions_index: PermissionsIndex::new(node_name),
            subscription_index: HashMap::new(),
            default_embedding_model,
            supported_embedding_models,
        }
    }

    /// IMPORTANT: This creates a barebones empty struct, intended to be used for tests
    /// that do not require a real filled out internals struct.
    pub fn new_empty() -> Self {
        let node_name = ShinkaiName::from_node_name("@@node1_test.shinkai".to_string()).unwrap();
        let default_embedding_model =
            EmbeddingModelType::TextEmbeddingsInference(TextEmbeddingsInference::AllMiniLML6v2);
        let supported_embedding_models = vec![default_embedding_model.clone()];
        Self::new(node_name, default_embedding_model, supported_embedding_models)
    }

    /// A hard-coded DB key for the profile-wide VectorFSInternals.
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
