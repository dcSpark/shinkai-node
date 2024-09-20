use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use serde_json;
use shinkai_message_primitives::schemas::file_links::{FileLink, FileMapPath, FolderSubscriptionWithPath};
use std::collections::HashMap;

impl ShinkaiDB {
    /// Writes file links to the database.
    pub fn write_file_links(
        &self,
        folder_subs_with_path: &FolderSubscriptionWithPath,
        file_links: &HashMap<FileMapPath, FileLink>,
    ) -> Result<(), ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Key for file links
        let file_links_key_str = format!(
            "files_upload_placeholder_value_to_match_prefix_{}",
            folder_subs_with_path.path
        );
        let file_links_key = file_links_key_str.as_bytes();
        let file_links_value = serde_json::to_vec(file_links)?;

        // Key for metadata
        let metadata_key_str = format!(
            "files_metadata_placeholder_to_matchvalueprefix_{}",
            folder_subs_with_path.path
        );
        let metadata_key = metadata_key_str.as_bytes();
        let metadata_value = serde_json::to_vec(folder_subs_with_path)?;

        self.db.put_cf(cf, file_links_key, file_links_value)?;
        self.db.put_cf(cf, metadata_key, metadata_value)?;
        Ok(())
    }

    /// Reads file links from the database.
    pub fn read_file_links(
        &self,
        folder_subs_with_path: &FolderSubscriptionWithPath,
    ) -> Result<Option<HashMap<FileMapPath, FileLink>>, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let file_links_key_str = format!(
            "files_upload_placeholder_value_to_match_prefix_{}",
            folder_subs_with_path.path
        );
        let file_links_key = file_links_key_str.as_bytes();

        match self.db.get_cf(cf, file_links_key)? {
            Some(value) => {
                let file_links: HashMap<FileMapPath, FileLink> = serde_json::from_slice(&value)?;
                Ok(Some(file_links))
            }
            None => Ok(None),
        }
    }

    /// Reads all file links from the database.
    pub fn read_all_file_links(
        &self,
    ) -> Result<HashMap<FolderSubscriptionWithPath, HashMap<FileMapPath, FileLink>>, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let mut all_file_links = HashMap::new();
        let file_links_prefix = "files_upload_placeholder_value_to_match_prefix_";
        let metadata_prefix = "files_metadata_placeholder_to_matchvalueprefix_";

        let iter = self.db.prefix_iterator_cf(cf, file_links_prefix.as_bytes());
        for item in iter {
            let (key, value) = item?;
            if let Ok(key_str) = String::from_utf8(key.to_vec()) {
                if key_str.starts_with(file_links_prefix) {
                    let path = &key_str[file_links_prefix.len()..];
                    let metadata_key_str = format!("{}{}", metadata_prefix, path);
                    let metadata_key = metadata_key_str.as_bytes();

                    if let Some(metadata_value) = self.db.get_cf(cf, metadata_key)? {
                        let folder_subs_with_path: FolderSubscriptionWithPath =
                            serde_json::from_slice(&metadata_value)?;
                        let file_links: HashMap<FileMapPath, FileLink> = serde_json::from_slice(&value)?;
                        all_file_links.insert(folder_subs_with_path, file_links);
                    }
                }
            }
        }

        Ok(all_file_links)
    }

    /// Deletes file links from the database.
    pub fn delete_file_links(&self, folder_subs_with_path: &FolderSubscriptionWithPath) -> Result<(), ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Key for file links
        let file_links_key_str = format!(
            "files_upload_placeholder_value_to_match_prefix_{}",
            folder_subs_with_path.path
        );
        let file_links_key = file_links_key_str.as_bytes();

        // Key for metadata
        let metadata_key_str = format!(
            "files_metadata_placeholder_to_matchvalueprefix_{}",
            folder_subs_with_path.path
        );
        let metadata_key = metadata_key_str.as_bytes();

        self.db.delete_cf(cf, file_links_key)?;
        self.db.delete_cf(cf, metadata_key)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use shinkai_message_primitives::schemas::shinkai_subscription_req::{FolderSubscription, PaymentOption};
    use shinkai_vector_resources::utils::hash_string;
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    fn setup() -> ShinkaiDB {
        let path = Path::new("db_tests/");
        let _ = fs::remove_dir_all(path);

        let node1_db_path = format!("db_tests/{}", hash_string("churrasco italiano"));
        ShinkaiDB::new(node1_db_path.as_str()).unwrap()
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
        let db = setup();
        let folder_subs_with_path = test_folder_subscription_with_path("test_folder1");
        let file_links = test_file_links();

        let result = db.write_file_links(&folder_subs_with_path, &file_links);
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_file_links() {
        let db = setup();
        let folder_subs_with_path = test_folder_subscription_with_path("test_folder1");
        let file_links = test_file_links();

        db.write_file_links(&folder_subs_with_path, &file_links).unwrap();
        let result = db.read_file_links(&folder_subs_with_path).unwrap();

        assert!(result.is_some());
        let read_file_links = result.unwrap();
        assert_eq!(read_file_links.len(), file_links.len());
    }

    #[test]
    fn test_read_all_file_links() {
        let db = setup();

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

    #[test]
    fn test_delete_file_links() {
        let db = setup();
        let folder_subs_with_path = test_folder_subscription_with_path("test_folder1");
        let file_links = test_file_links();

        db.write_file_links(&folder_subs_with_path, &file_links).unwrap();
        db.delete_file_links(&folder_subs_with_path).unwrap();
        let result = db.read_file_links(&folder_subs_with_path).unwrap();

        assert!(result.is_none());
    }
}
