use super::{
    vector_fs_permissions::PermissionsIndex,
    vector_fs_types::{LastReadIndex, SubscriptionsIndex},
};
use serde_json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    embeddings::Embedding,
    model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference},
    source::DistributionInfo,
    vector_resource::{MapVectorResource, VRSourceReference, VectorResourceCore},
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorFSInternals {
    pub fs_core_resource: MapVectorResource,
    pub permissions_index: PermissionsIndex,
    pub subscription_index: SubscriptionsIndex,
    pub supported_embedding_models: Vec<EmbeddingModelType>,
    pub last_read_index: LastReadIndex,
}

impl VectorFSInternals {
    pub async fn new(
        node_name: ShinkaiName,
        default_embedding_model_used: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Self {
        let core_resource = MapVectorResource::new(
            "VecFS Core Resource",
            None,
            VRSourceReference::None,
            Embedding::new("", vec![]),
            HashMap::new(),
            HashMap::new(),
            default_embedding_model_used,
            true,
            DistributionInfo::new_empty(),
        );
        Self {
            fs_core_resource: core_resource,
            permissions_index: PermissionsIndex::new(node_name).await,
            subscription_index: SubscriptionsIndex::new_empty(),
            supported_embedding_models,
            last_read_index: LastReadIndex::new_empty(),
        }
    }

    /// IMPORTANT: This creates a barebones empty struct, intended to be used for tests
    /// that do not require a real filled out internals struct.
    pub async fn new_empty() -> Self {
        let node_name = ShinkaiName::from_node_name("@@node1_test.shinkai".to_string()).unwrap();
        let default_embedding_model =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_XS);
        let supported_embedding_models = vec![default_embedding_model.clone()];
        Self::new(node_name, default_embedding_model, supported_embedding_models).await
    }

    /// Returns the default Embedding model used by the profile's VecFS.
    pub fn default_embedding_model(&self) -> EmbeddingModelType {
        self.fs_core_resource.embedding_model_used()
    }

    /// A hard-coded DB key for the profile-wide VectorFSInternals.
    pub fn profile_fs_internals_shinkai_db_key() -> String {
        "profile_vector_fs_internals".to_string()
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }
}
