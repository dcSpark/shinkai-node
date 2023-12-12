use super::fs_db::VectorFSDB;
use super::fs_internals::VectorFSInternals;
use rocksdb::Error;
use shinkai_vector_resources::{
    embeddings::Embedding, map_resource::MapVectorResource, model_type::EmbeddingModelType, source::VRSource,
};
use std::collections::HashMap;

pub struct VectorFS {
    pub internals: VectorFSInternals,
    pub db: VectorFSDB,
}

impl VectorFS {
    pub fn new(
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
        db_path: &str,
    ) -> Result<Self, Error> {
        let db = VectorFSDB::new(db_path)?;
        Ok(Self {
            internals: VectorFSInternals {
                file_system_resource: MapVectorResource::new(
                    "default_name",
                    Some("default_description"),
                    VRSource::None,
                    "default_resource_id",
                    Embedding::new("", vec![]),
                    HashMap::new(),
                    HashMap::new(),
                    default_embedding_model.clone(),
                ),
                identity_permissions_index: HashMap::new(),
                metadata_key_index: HashMap::new(),
                data_tag_index: HashMap::new(),
                subscription_index: HashMap::new(),
                default_embedding_model,
                supported_embedding_models,
            },
            db,
        })
    }
}
