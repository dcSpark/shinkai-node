use super::fs_internals::VectorFSInternals;
use crate::db::db::ProfileBoundWriteBatch;
use rocksdb::{
    AsColumnFamilyRef, ColumnFamily, ColumnFamilyDescriptor, DBCommon, DBIteratorWithThreadMode, Error, IteratorMode,
    Options, SingleThreaded, WriteBatch, DB,
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    embeddings::Embedding, map_resource::MapVectorResource, model_type::EmbeddingModelType, source::VRSource,
    vector_search_traversal::VRPath,
};
use std::collections::HashMap;
use std::path::Path;

pub enum FSTopic {
    VectorResources,
    FileSystem,
}

impl FSTopic {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VectorResources => "resources",
            Self::FileSystem => "filesystem",
        }
    }
}

pub struct VectorFS {
    pub internals: VectorFSInternals,
    pub db: DB,
    pub db_path: String,
}

impl VectorFS {
    pub fn new(
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
        db_path: &str,
    ) -> Result<Self, Error> {
        let db = Self::setup_db(db_path)?;
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
            db: db,
            db_path: db_path.to_string(),
        })
    }

    fn setup_db(db_path: &str) -> Result<DB, Error> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        // if we want to enable compression
        // db_opts.set_compression_type(DBCompressionType::Lz4);

        let cf_names = if Path::new(db_path).exists() {
            // If the database file exists, get the list of column families from the database
            DB::list_cf(&db_opts, db_path)?
        } else {
            // If the database file does not exist, use the default list of column families
            vec![
                FSTopic::VectorResources.as_str().to_string(),
                FSTopic::FileSystem.as_str().to_string(),
            ]
        };

        let mut cfs = vec![];
        for cf_name in &cf_names {
            let mut cf_opts = Options::default();
            cf_opts.create_if_missing(true);
            cf_opts.create_missing_column_families(true);
            let cf_desc = ColumnFamilyDescriptor::new(cf_name.to_string(), cf_opts);
            cfs.push(cf_desc);
        }

        let db = DB::open_cf_descriptors(&db_opts, db_path, cfs)?;
        Ok(db)
    }
}
