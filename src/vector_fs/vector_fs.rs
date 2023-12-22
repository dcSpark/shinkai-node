use super::fs_internals::VectorFSInternals;
use super::{db::fs_db::VectorFSDB, fs_error::VectorFSError};
use rocksdb::Error;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    embeddings::Embedding, map_resource::MapVectorResource, model_type::EmbeddingModelType, source::VRSource,
};
use std::collections::HashMap;

/// Struct that wraps all functionality of the VectorFS.
/// Of note, internals_map holds a hashmap of the VectorFSInternals
/// for all profiles on the node.
pub struct VectorFS {
    internals_map: HashMap<ShinkaiName, VectorFSInternals>,
    db: VectorFSDB,
}

impl VectorFS {
    /// Initializes the VectorFS struct. If no existing VectorFS exists in the VectorFSDB, then initializes
    /// from scratch. Otherwise reads from the FSDB.
    /// Requires supplying list of profiles setup in the node for profile_list.
    pub fn new(
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
        profile_list: Vec<ShinkaiName>,
        db_path: &str,
    ) -> Result<Self, VectorFSError> {
        let fs_db = VectorFSDB::new(db_path)?;

        // For each profile, initialize the internals in the DB if needed, and read them into the internals_map
        let mut internals_map = HashMap::new();
        for profile in profile_list {
            fs_db.init_profile_fs_internals(
                &profile,
                default_embedding_model.clone(),
                supported_embedding_models.clone(),
            )?;
            let internals = fs_db.get_profile_fs_internals(&profile)?;
            internals_map.insert(profile, internals);
        }

        Ok(Self {
            internals_map,
            db: fs_db,
        })
    }

    /// IMPORTANT: Only to be used when writing tests that do not use the VectorFS.
    /// Simply creates a barebones struct to be used to satisfy required types.
    pub fn new_empty() -> Self {
        Self {
            internals_map: HashMap::new(),
            db: VectorFSDB::new_empty(),
        }
    }

    /// Attempts to fetch a mutable reference to the profile VectorFSInternals (from memory)
    /// in the internals_map.
    pub fn get_profile_fs_internals(&mut self, profile: &ShinkaiName) -> Result<&mut VectorFSInternals, VectorFSError> {
        self.internals_map
            .get_mut(profile)
            .ok_or_else(|| VectorFSError::ProfileNameNonExistent(profile.to_string()))
    }
}
