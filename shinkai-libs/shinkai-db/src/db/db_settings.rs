use shinkai_vector_resources::model_type::EmbeddingModelType;

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

impl ShinkaiDB {
    /// Gets the default embedding model.
    pub fn get_default_embedding_model(&self) -> Result<EmbeddingModelType, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"settings_default_embedding_model";

        match self.db.get_cf(cf, key)? {
            Some(value) => {
                let model: EmbeddingModelType = serde_json::from_slice(&value)?;
                Ok(model)
            }
            None => Err(ShinkaiDBError::DataNotFound), // Handle the case where the setting does not exist
        }
    }

    /// Updates the default embedding model.
    pub fn update_default_embedding_model(&self, model: EmbeddingModelType) -> Result<(), ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"settings_default_embedding_model";
        let value = serde_json::to_vec(&model)?;

        self.db.put_cf(cf, key, value)?;
        Ok(())
    }

    /// Gets the supported embedding models.
    pub fn get_supported_embedding_models(&self) -> Result<Vec<EmbeddingModelType>, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"settings_supported_embedding_models";

        match self.db.get_cf(cf, key)? {
            Some(value) => {
                let models: Vec<EmbeddingModelType> = serde_json::from_slice(&value)?;
                Ok(models)
            }
            None => Err(ShinkaiDBError::DataNotFound), // Handle the case where the setting does not exist
        }
    }

    /// Updates the supported embedding models.
    pub fn update_supported_embedding_models(&self, models: Vec<EmbeddingModelType>) -> Result<(), ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"settings_supported_embedding_models";
        let value = serde_json::to_vec(&models)?;

        self.db.put_cf(cf, key, value)?;
        Ok(())
    }
}
