use serde_json::Value;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn save_wallet_manager(&self, wallet_data: &Value) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO shinkai_wallet (id, wallet_data)
                VALUES (1, ?)
                ON CONFLICT (id)
                DO UPDATE SET
                wallet_data = excluded.wallet_data",
        )?;

        let wallet_data = serde_json::to_vec(&wallet_data).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        stmt.execute([wallet_data])?;

        Ok(())
    }

    pub fn read_wallet_manager(&self) -> Result<Value, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT wallet_data FROM shinkai_wallet LIMIT 1")?;

        let wallet_data = stmt
            .query_row([], |row| {
                let wallet_data: Vec<u8> = row.get(0)?;
                let wallet_data: Value = serde_json::from_slice(&wallet_data).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;

                Ok(wallet_data)
            })
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => SqliteManagerError::WalletManagerNotFound,
                _ => SqliteManagerError::from(e),
            })?;

        Ok(wallet_data)
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
    async fn test_save_and_read_wallet_manager() {
        let manager = setup_test_db();
        let wallet_data = serde_json::json!({
            "payment_wallet": "payment_wallet_data",
            "receiving_wallet": "receiving_wallet_data",
        });

        // Insert wallet data
        let result = manager.save_wallet_manager(&wallet_data);
        assert!(result.is_ok());

        let wallet_data = manager.read_wallet_manager().unwrap();
        assert_eq!(wallet_data, wallet_data);

        // Update wallet data
        let wallet_data = serde_json::json!({
            "payment_wallet": "updated_payment_wallet_data",
            "receiving_wallet": "updated_receiving_wallet_data",
        });

        let result = manager.save_wallet_manager(&wallet_data);
        assert!(result.is_ok());

        let wallet_data = manager.read_wallet_manager().unwrap();
        assert_eq!(wallet_data, wallet_data);

        // Verify that shinkai_wallet table has only one row
        let conn = manager.get_connection().unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM shinkai_wallet").unwrap();

        let count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
