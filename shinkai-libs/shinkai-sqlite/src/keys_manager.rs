use rusqlite::params;

use crate::{errors::SqliteManagerError, SqliteManager};

impl SqliteManager {
    pub fn write_symmetric_key(&self, hex_blake3_hash: &str, private_key: &[u8]) -> Result<(), SqliteManagerError> {
        // Write the private key to the database with the public key as the key
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO message_box_symmetric_keys (hex_blake3_hash, symmetric_key) VALUES (?1, ?2)",
        )?;
        stmt.execute(params![hex_blake3_hash, private_key])?;

        Ok(())
    }

    pub fn read_symmetric_key(&self, hex_blake3_hash: &str) -> Result<Vec<u8>, SqliteManagerError> {
        // Read the private key from the database using the public key
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT symmetric_key FROM message_box_symmetric_keys WHERE hex_blake3_hash = ?1")?;
        let mut rows = stmt.query(params![hex_blake3_hash])?;

        if let Some(row) = rows.next()? {
            let symmetric_key: Vec<u8> = row.get(0)?;
            Ok(symmetric_key)
        } else {
            Err(SqliteManagerError::DataNotFound)
        }
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

    #[test]
    fn test_read_write_symmetric_key() {
        let db = setup_test_db();
        let hex_blake3_hash = "1234567890abcdef";
        let symmetric_key = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 0];

        db.write_symmetric_key(hex_blake3_hash, &symmetric_key).unwrap();
        let read_symmetric_key = db.read_symmetric_key(hex_blake3_hash).unwrap();

        assert_eq!(symmetric_key, read_symmetric_key);
    }
}
