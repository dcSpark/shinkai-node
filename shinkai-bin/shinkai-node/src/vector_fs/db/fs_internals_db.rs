use crate::db::db_profile_bound::ProfileBoundWriteBatch;

use super::super::{vector_fs_error::VectorFSError, vector_fs_internals::VectorFSInternals};
use super::fs_db::{FSTopic, VectorFSDB};
use serde_json::from_str;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::model_type::EmbeddingModelType;

impl VectorFSDB {
    /// Commits saving the supplied VectorFS Internals to the write batch
    pub fn wb_save_profile_fs_internals(
        &self,
        fs_internals: &VectorFSInternals,
        batch: &mut ProfileBoundWriteBatch,
    ) -> Result<(), VectorFSError> {
        let (bytes, cf) = self._prepare_profile_fs_internals(fs_internals)?;
        batch.pb_put_cf(cf, &VectorFSInternals::profile_fs_internals_shinkai_db_key(), bytes);

        Ok(())
    }

    /// Saves the supplied VectorFS Internals
    pub fn save_profile_fs_internals(
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
    ) -> Result<(Vec<u8>, &str), VectorFSError> {
        // Convert JSON to bytes for storage
        let json = fs_internals.to_json()?;
        // eprintln!("json: {:?}", json);
        let bytes = json.as_bytes().to_vec();
        let cf = FSTopic::FileSystem.as_str();

        Ok((bytes, cf))
    }

    /// Fetches the profile's `VectorFSInternals` from the DB
    pub fn get_profile_fs_internals(&self, profile: &ShinkaiName) -> Result<VectorFSInternals, VectorFSError> {
        let bytes = self.get_cf_pb(
            FSTopic::FileSystem,
            &VectorFSInternals::profile_fs_internals_shinkai_db_key(),
            profile,
        )?;
        let json_str = std::str::from_utf8(&bytes)?;
        let fs_internals: VectorFSInternals = from_str(json_str)?;

        Ok(fs_internals)
    }

    /// Creates and saves the profile's VectorFSInternals if one does not exist in the DB.
    pub async fn init_profile_fs_internals(
        &self,
        profile: &ShinkaiName,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
    ) -> Result<(), VectorFSError> {
        if let Err(_) = self.get_profile_fs_internals(profile) {
            // Extract just the node name from the profile name
            let fs_internals =
                VectorFSInternals::new(profile.clone(), default_embedding_model, supported_embedding_models).await;

            self.save_profile_fs_internals(&fs_internals, profile)?;
        }
        Ok(())
    }
}
