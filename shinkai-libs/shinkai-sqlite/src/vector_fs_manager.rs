use rusqlite::params;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    model_type::EmbeddingModelType,
    vector_resource::{MapVectorResource, VectorResourceCore},
};

use crate::{errors::SqliteManagerError, SqliteManager};

impl SqliteManager {
    pub fn save_profile_fs_internals(
        &self,
        profile: &ShinkaiName,
        fs_core_resource: MapVectorResource,
        permissions_index: Vec<u8>,
        subscription_index: Vec<u8>,
        supported_embedding_models: Vec<EmbeddingModelType>,
        last_read_index: Vec<u8>,
    ) -> Result<(), SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;
        let resource_id = fs_core_resource.reference_string();
        self.save_resource(&fs_core_resource.into(), &profile_name)?;

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO vector_fs_internals 
                (profile_name, core_resource_id, permissions_index, subscription_index, supported_embedding_models, last_read_index)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![profile_name, resource_id, permissions_index, subscription_index, serde_json::to_vec(&supported_embedding_models)
            .map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?, last_read_index],
        )?;

        Ok(())
    }

    pub fn get_profile_fs_internals(
        &self,
        profile: &ShinkaiName,
    ) -> Result<(MapVectorResource, Vec<u8>, Vec<u8>, Vec<EmbeddingModelType>, Vec<u8>), SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT core_resource_id, permissions_index, subscription_index, supported_embedding_models, last_read_index FROM vector_fs_internals WHERE profile_name = ?1")?;
        let mut rows = stmt.query(params![profile_name])?;

        let row = rows
            .next()?
            .ok_or(SqliteManagerError::ProfileNotFound(profile_name.to_string()))?;
        let core_resource_id: String = row.get(0)?;
        let permissions_index: Vec<u8> = row.get(1)?;
        let subscription_index: Vec<u8> = row.get(2)?;
        let supported_embedding_models: Vec<EmbeddingModelType> = serde_json::from_slice(&row.get::<_, Vec<u8>>(3)?)?;
        let last_read_index: Vec<u8> = row.get(4)?;

        let core_resource = self.get_resource(&core_resource_id, profile)?;

        Ok((
            core_resource.as_map_resource_cloned()?,
            permissions_index,
            subscription_index,
            supported_embedding_models,
            last_read_index,
        ))
    }
}
