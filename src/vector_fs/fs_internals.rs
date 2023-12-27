use crate::tools::js_toolkit_executor::DEFAULT_LOCAL_TOOLKIT_EXECUTOR_PORT;

use super::permissions::PermissionsIndex;
use serde_json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    embeddings::Embedding,
    map_resource::MapVectorResource,
    model_type::{EmbeddingModelType, TextEmbeddingsInference},
    vector_resource::VectorResource,
    vector_search_traversal::{VRPath, VRSource},
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorFSInternals {
    pub fs_core_resource: MapVectorResource,
    pub permissions_index: PermissionsIndex,
    pub subscription_index: HashMap<VRPath, Vec<ShinkaiName>>,
    pub supported_embedding_models: Vec<EmbeddingModelType>,
}

impl VectorFSInternals {
    pub fn new(
        node_name: ShinkaiName,
        default_embedding_model_used: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Self {
        let core_resource = MapVectorResource::new(
            "VecFS Core Resource",
            None,
            VRSource::None,
            Embedding::new(&String::new(), vec![]),
            HashMap::new(),
            HashMap::new(),
            default_embedding_model_used,
        );
        Self {
            fs_core_resource: core_resource,
            permissions_index: PermissionsIndex::new(node_name),
            subscription_index: HashMap::new(),
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

    /// Returns the default Embedding model used by the profile's VecFS.
    pub fn default_embedding_model(&self) -> EmbeddingModelType {
        self.fs_core_resource.embedding_model_used()
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
