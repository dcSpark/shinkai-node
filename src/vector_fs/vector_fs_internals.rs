use super::{
    vector_fs_error::VectorFSError, vector_fs_permissions::PermissionsIndex, vector_fs_types::SubscriptionsIndex,
};
use crate::tools::js_toolkit_executor::DEFAULT_LOCAL_TOOLKIT_EXECUTOR_PORT;
use chrono::{DateTime, Utc};
use serde_json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    embeddings::Embedding,
    model_type::{EmbeddingModelType, TextEmbeddingsInference},
    resource_errors::VRError,
    vector_resource::{
        BaseVectorResource, MapVectorResource, NodeContent, VRHeader, VRPath, VRSource, VectorResource,
        VectorResourceCore,
    },
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VectorFSInternals {
    pub fs_core_resource: MapVectorResource,
    pub permissions_index: PermissionsIndex,
    pub subscription_index: SubscriptionsIndex,
    pub supported_embedding_models: Vec<EmbeddingModelType>,
    pub last_read_index: HashMap<VRPath, (DateTime<Utc>, ShinkaiName)>,
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
            subscription_index: SubscriptionsIndex::new_empty(),
            supported_embedding_models,
            last_read_index: HashMap::new(),
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

    // /// Creates a new folder in this VectorFSInternals. Returns the full path to the folder.
    // pub fn create_new_folder(
    //     &mut self,
    //     name: String,
    //     path: VRPath,
    // ) -> Result<VRPath, VectorFSError> {
    //     if path.path_ids.is_empty() {
    //         Err(VRError::InvalidVRPath(path.clone()))?;
    //     }

    //     // Create a new empty map resource to act as a new folder
    //     let new_folder_resource = BaseVectorResource::Map(MapVectorResource::new_empty(&name, None, VRSource::None));
    //     // TODO: Check if the default empty embedding works with vector searches across the FS, or if we need to specify a filled out
    //     // Embedding here to have scoring/traversal work at all, even for exhaustive.
    //     let embedding = new_folder_resource.as_trait_object().resource_embedding();

    //     // Fetch the first node directly, then iterate through the rest
    //     let mut node = self.fs_core_resource.get_node(path.path_ids[0].clone())?;
    //     for id in path.path_ids.iter().skip(1) {
    //         match node.content {
    //             NodeContent::Resource(ref mut resource) => {
    //                 if let Some(last) = path.path_ids.last() {
    //                     if id == last {
    //                         if let Ok(map) = resource.as_map_resource() {
    //                             map.insert_vector_resource_node(
    //                                // process the name into underscore/lowercase ,
    //                                 new_folder_resource.clone(),
    //                                 metadata.clone(),
    //                                 embedding,
    //                             );
    //                             return Ok(());
    //                         }
    //                     } else {
    //                         node = resource.as_trait_object().get_node(id.clone())?;
    //                     }
    //                 }
    //             }
    //             _ => {
    //                 Err(VRError::InvalidVRPath(path.clone()))?;
    //             }
    //         }
    //     }
    //     Err(VRError::InvalidVRPath(path.clone()))?
    // }
}

/// Struct that abstracts away the MapVectorResource interface for folders in the VectorFSInternals
struct Folder {}
