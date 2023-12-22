use std::collections::HashMap;

use super::super::{fs_error::VectorFSError, fs_internals::VectorFSInternals};
use super::fs_db::{FSTopic, VectorFSDB};
use serde_json::from_str;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::map_resource::MapVectorResource;
use shinkai_vector_resources::model_type::EmbeddingModelType;
use shinkai_vector_resources::vector_search_traversal::VRSource;

impl VectorFSDB {
    /// Saves the supplied VectorFS core resource
    fn save_profile_fs_internals(
        &self,
        fs_internals: &VectorFSInternals,
        profile: &ShinkaiName,
    ) -> Result<(), VectorFSError> {
        let (bytes, cf) = self._prepare_profile_fs_internals(fs_internals)?;
        self.put_cf_pb(
            cf,
            &VectorFSInternals::profile_fs_internals_shinkai_db_key(),
            bytes,
            profile,
        )?;

        Ok(())
    }

    /// Prepares the `VectorFSInternals` for saving into the DB
    fn _prepare_profile_fs_internals(
        &self,
        fs_internals: &VectorFSInternals,
    ) -> Result<(Vec<u8>, &rocksdb::ColumnFamily), VectorFSError> {
        // Convert JSON to bytes for storage
        let json = fs_internals.to_json()?;
        let bytes = json.as_bytes().to_vec();
        let cf = self.get_cf_handle(FSTopic::FileSystem)?;

        Ok((bytes, cf))
    }

    /// Fetches the profile's `VectorFSInternals` from the DB
    pub fn get_profile_fs_internals(&self, profile: &ShinkaiName) -> Result<VectorFSInternals, VectorFSError> {
        println!("Profile: {:?}", profile.to_string());
        let bytes = self.get_cf_pb(
            FSTopic::VectorResources,
            &VectorFSInternals::profile_fs_internals_shinkai_db_key(),
            profile,
        )?;
        let json_str = std::str::from_utf8(&bytes)?;
        let fs_internals: VectorFSInternals = from_str(json_str)?;

        Ok(fs_internals)
    }

    /// Creates and saves the profile's VectorFSInternals if one does not exist in the DB.
    pub fn init_profile_fs_internals(
        &self,
        profile: &ShinkaiName,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Result<(), VectorFSError> {
        if let Err(_) = self.get_profile_fs_internals(profile) {
            let fs_internals = VectorFSInternals::new(
                MapVectorResource::new_empty("VecFS Core Resource", None, VRSource::None, "core"),
                HashMap::new(),
                HashMap::new(),
                default_embedding_model,
                supported_embedding_models,
            );

            self.save_profile_fs_internals(&fs_internals, profile)?;
        }
        Ok(())
    }
}
