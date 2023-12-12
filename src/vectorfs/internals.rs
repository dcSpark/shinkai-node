use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    map_resource::MapVectorResource, model_type::EmbeddingModelType, vector_search_traversal::VRPath,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorFSInternals {
    file_system_resource: MapVectorResource,
    identity_permissions_index: HashMap<ShinkaiName, Vec<VRPath>>,
    metadata_key_index: HashMap<String, Vec<VRPath>>,
    data_tag_index: HashMap<String, Vec<VRPath>>,
    /// List of users who are subscribed to a specific VRPath. When the node (or any node below)
    /// a VRPath update, then all subscriber's nodes are notified
    subscription_index: HashMap<VRPath, Vec<ShinkaiName>>,
    /// Embedding model used in the file_system_resource/by default for generating all VectorResources
    default_embedding_model: EmbeddingModelType,
    /// Currently only supports a single embedding model in the Shinkai Node. In the future will
    /// support multiple.
    supported_embedding_models: Vec<EmbeddingModelType>,
}
