use shinkai_vector_resources::model_type::EmbeddingModelType;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn get_supported_embedding_models(&self) -> Result<Vec<EmbeddingModelType>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT supported_embedding_models FROM shinkai_settings LIMIT 1")?;

        let models = stmt.query_row([], |row| {
            let models: String = row.get(0)?;
            let models: Vec<EmbeddingModelType> = serde_json::from_str(&models).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            Ok(models)
        })?;

        Ok(models)
    }

    pub fn update_supported_embedding_models(&self, models: Vec<EmbeddingModelType>) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO shinkai_settings (id, supported_embedding_models)
                VALUES (1, ?)
                ON CONFLICT (id)
                DO UPDATE SET
                supported_embedding_models = excluded.supported_embedding_models",
        )?;

        let models = serde_json::to_string(&models).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        stmt.execute([models])?;

        Ok(())
    }
}

mod tests {
    use super::*;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
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

        // Verify that shinkai_settings table has only one row
        let conn = manager.get_connection().unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM shinkai_settings").unwrap();

        let count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
