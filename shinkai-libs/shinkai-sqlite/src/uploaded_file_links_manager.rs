use std::collections::HashMap;

use rusqlite::params;
use shinkai_message_primitives::schemas::file_links::{FileLink, FileMapPath, FolderSubscriptionWithPath};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn write_file_links(
        &self,
        folder_subs_with_path: &FolderSubscriptionWithPath,
        file_links: &HashMap<FileMapPath, FileLink>,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        let metadata = serde_json::to_vec(folder_subs_with_path).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        let file_links = serde_json::to_vec(file_links).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;

        conn.execute(
            "INSERT OR REPLACE INTO uploaded_file_links (path, metadata, file_links) VALUES (?1, ?2, ?3)",
            params![folder_subs_with_path.path, metadata, file_links],
        )?;

        Ok(())
    }

    pub fn read_all_file_links(
        &self,
    ) -> Result<HashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileLink>>, SqliteManagerError> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare("SELECT metadata, file_links FROM uploaded_file_links")?;
        let rows = stmt.query_map([], |row| {
            let metadata: Vec<u8> = row.get(0)?;
            let file_links: Vec<u8> = row.get(1)?;

            let folder_subs_with_path: FolderSubscriptionWithPath = serde_json::from_slice(&metadata).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            let file_links: HashMap<FileMapPath, FileLink> = serde_json::from_slice(&file_links).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            Ok((folder_subs_with_path, file_links))
        })?;

        let mut result = HashMap::new();
        for row in rows {
            let (folder_subs_with_path, file_links) = row?;
            result.insert(folder_subs_with_path, file_links);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use shinkai_message_primitives::schemas::shinkai_subscription_req::{FolderSubscription, PaymentOption};
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    fn test_folder_subscription_with_path(path: &str) -> FolderSubscriptionWithPath {
        FolderSubscriptionWithPath {
            path: path.to_string(),
            folder_subscription: FolderSubscription {
                minimum_token_delegation: Some(100),
                minimum_time_delegated_hours: Some(100),
                monthly_payment: Some(PaymentOption::USD(Decimal::new(1000, 2))), // Represents 10.00
                is_free: false,
                has_web_alternative: Some(true),
                folder_description: "This is a test folder".to_string(),
            },
        }
    }

    fn test_file_links() -> HashMap<FileMapPath, FileLink> {
        let mut file_links = HashMap::new();
        file_links.insert(
            "shinkai_sharing/dummy_file1".to_string(),
            FileLink {
                link: "http://example.com/file1".to_string(),
                last_8_hash: "4aaabb39".to_string(),
                expiration: SystemTime::now() + Duration::new(3600, 0),
                path: "shinkai_sharing/dummy_file1".to_string(),
            },
        );
        file_links.insert(
            "shinkai_sharing/dummy_file2".to_string(),
            FileLink {
                link: "http://example.com/file2".to_string(),
                last_8_hash: "2bbbbb39".to_string(),
                expiration: SystemTime::now() + Duration::new(3600, 0),
                path: "shinkai_sharing/dummy_file2".to_string(),
            },
        );
        file_links
    }

    #[test]
    fn test_write_file_links() {
        let db = setup_test_db();
        let folder_subs_with_path = test_folder_subscription_with_path("test_folder1");
        let file_links = test_file_links();

        let result = db.write_file_links(&folder_subs_with_path, &file_links);
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_all_file_links() {
        let db = setup_test_db();

        // Create multiple folder subscriptions with different paths
        let folder_subs_with_path1 = test_folder_subscription_with_path("test_folder1");
        let folder_subs_with_path2 = test_folder_subscription_with_path("test_folder2");
        let folder_subs_with_path3 = test_folder_subscription_with_path("test_folder3");

        let file_links1 = test_file_links();
        let file_links2 = test_file_links();
        let file_links3 = test_file_links();

        // Write file links for each folder subscription
        db.write_file_links(&folder_subs_with_path1, &file_links1).unwrap();
        db.write_file_links(&folder_subs_with_path2, &file_links2).unwrap();
        db.write_file_links(&folder_subs_with_path3, &file_links3).unwrap();

        // Read all file links from the database
        let result = db.read_all_file_links().unwrap();

        // Check that all folder subscriptions are present
        assert_eq!(result.len(), 3);
        assert_eq!(result.get(&folder_subs_with_path1).unwrap().len(), file_links1.len());
        assert_eq!(result.get(&folder_subs_with_path2).unwrap().len(), file_links2.len());
        assert_eq!(result.get(&folder_subs_with_path3).unwrap().len(), file_links3.len());
    }
}
