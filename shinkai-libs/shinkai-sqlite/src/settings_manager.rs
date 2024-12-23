use shinkai_embedding::model_type::EmbeddingModelType;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn get_supported_embedding_models(&self) -> Result<Vec<EmbeddingModelType>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT value FROM shinkai_settings WHERE key = 'supported_embedding_models'")?;

        let models = stmt
            .query_row([], |row| {
                let models: String = row.get(0)?;
                let models: Vec<EmbeddingModelType> = serde_json::from_str(&models).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;

                Ok(models)
            })
            .map_err(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    SqliteManagerError::DataNotFound
                } else {
                    SqliteManagerError::DatabaseError(e)
                }
            })?;

        Ok(models)
    }

    pub fn update_supported_embedding_models(&self, models: Vec<EmbeddingModelType>) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare("INSERT OR REPLACE INTO shinkai_settings (key, value) VALUES ('supported_embedding_models', ?)")?;

        let models = serde_json::to_string(&models).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        stmt.execute([models])?;

        Ok(())
    }

    pub fn set_api_v2_key(&self, key: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("INSERT OR REPLACE INTO shinkai_settings (key, value) VALUES ('api_v2_key', ?)")?;
        stmt.execute([key])?;

        Ok(())
    }

    pub fn read_api_v2_key(&self) -> Result<Option<String>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let stmt = conn.prepare("SELECT value FROM shinkai_settings WHERE key = 'api_v2_key'");

        let key = stmt?.query_row([], |row| row.get(0)).ok();

        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_update_and_get_supported_embedding_models() {
        let manager = setup_test_db();
        let models = vec![EmbeddingModelType::OllamaTextEmbeddingsInference(
            OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M,
        )];

        // Insert the models
        let result = manager.update_supported_embedding_models(models.clone());
        assert!(result.is_ok());

        let updated_models = manager.get_supported_embedding_models().unwrap();
        assert_eq!(models, updated_models);

        let new_models = vec![
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M),
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::JinaEmbeddingsV2BaseEs),
        ];

        // Update the models
        let result = manager.update_supported_embedding_models(new_models.clone());
        assert!(result.is_ok());

        let updated_models = manager.get_supported_embedding_models().unwrap();
        assert_eq!(new_models, updated_models);
    }

    #[tokio::test]
    async fn test_set_and_read_api_v2_key() {
        let manager = setup_test_db();
        let key = "test_key";

        // Insert the key
        let result = manager.set_api_v2_key(key);
        assert!(result.is_ok());

        let read_key = manager.read_api_v2_key().unwrap();
        assert_eq!(Some(key.to_string()), read_key);

        // Update the key
        let new_key = "new_test_key";
        let result = manager.set_api_v2_key(new_key);
        assert!(result.is_ok());

        let read_key = manager.read_api_v2_key().unwrap();
        assert_eq!(Some(new_key.to_string()), read_key);
    }
}
