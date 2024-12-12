use rusqlite::params;

use crate::{errors::SqliteManagerError, SqliteManager};

impl SqliteManager {
    pub fn add_file_to_files_message_inbox(
        &self,
        file_inbox_name: String,
        file_name: String,
        file_content: Vec<u8>,
    ) -> Result<(), SqliteManagerError> {
        let file_inboxes_path = self.get_file_inboxes_path();
        let inbox_dir_name = Self::get_inbox_directory_name(&file_inbox_name);
        let file_path = file_inboxes_path.join(&inbox_dir_name).join(&file_name);

        // Store the file content in the inboxes directory
        std::fs::create_dir_all(file_path.parent().unwrap()).map_err(|_| SqliteManagerError::FailedFetchingValue)?;
        std::fs::write(file_path, file_content).map_err(|_| SqliteManagerError::FailedFetchingValue)?;

        // Store inboxes metadata in the database
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO file_inboxes (file_inbox_name, file_name) VALUES (?1, ?2)",
            params![file_inbox_name, file_name],
        )?;

        Ok(())
    }

    pub fn get_all_files_from_inbox(
        &self,
        file_inbox_name: String,
    ) -> Result<Vec<(String, Vec<u8>)>, SqliteManagerError> {
        let file_inboxes_path = self.get_file_inboxes_path();
        let inbox_dir_name = Self::get_inbox_directory_name(&file_inbox_name);
        let inbox_path = file_inboxes_path.join(&inbox_dir_name);

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT file_name FROM file_inboxes WHERE file_inbox_name = ?1")?;
        let file_names = stmt.query_map(params![file_inbox_name], |row| row.get::<_, String>(0))?;

        let mut files = Vec::new();
        for file_name in file_names {
            let file_name = file_name?;
            let file_path = inbox_path.join(&file_name);
            let file_content = std::fs::read(file_path).map_err(|_| SqliteManagerError::FailedFetchingValue)?;
            files.push((file_name, file_content));
        }

        Ok(files)
    }

    pub fn get_all_filenames_from_inbox(&self, file_inbox_name: String) -> Result<Vec<String>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT file_name FROM file_inboxes WHERE file_inbox_name = ?1")?;
        let file_names = stmt.query_map(params![file_inbox_name], |row| row.get::<_, String>(0))?;

        let mut files = Vec::new();
        for file_name in file_names {
            files.push(file_name?);
        }

        Ok(files)
    }

    pub fn remove_inbox(&self, file_inbox_name: &str) -> Result<(), SqliteManagerError> {
        let file_inboxes_path = self.get_file_inboxes_path();
        let inbox_dir_name = Self::get_inbox_directory_name(&file_inbox_name);
        let inbox_path = file_inboxes_path.join(&inbox_dir_name);

        std::fs::remove_dir_all(inbox_path).map_err(|_| SqliteManagerError::FailedFetchingValue)?;

        let conn = self.get_connection()?;
        conn.execute(
            "DELETE FROM file_inboxes WHERE file_inbox_name = ?1",
            params![file_inbox_name],
        )?;

        Ok(())
    }

    pub fn get_file_from_inbox(
        &self,
        file_inbox_name: String,
        file_name: String,
    ) -> Result<Vec<u8>, SqliteManagerError> {
        let file_inboxes_path = self.get_file_inboxes_path();
        let inbox_dir_name = Self::get_inbox_directory_name(&file_inbox_name);
        let file_path = file_inboxes_path.join(&inbox_dir_name).join(&file_name);

        std::fs::read(file_path).map_err(|_| SqliteManagerError::FailedFetchingValue)
    }

    fn get_file_inboxes_path(&self) -> std::path::PathBuf {
        match std::env::var("NODE_STORAGE_PATH").ok() {
            Some(path) => std::path::PathBuf::from(path).join("files"),
            None => std::path::PathBuf::from("files"),
        }
    }

    fn get_inbox_directory_name(name: &str) -> String {
        let sanitized_dir = name.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
        format!("inbox_{}", sanitized_dir)
    }
}

#[cfg(test)]
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

        std::env::set_var("NODE_STORAGE_PATH", db_path.parent().unwrap());

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[test]
    fn test_files_message_inbox() {
        let db = setup_test_db();
        let hex_blake3_hash = "1234567890abcdef".to_string();
        let file_name1 = "test_file.txt".to_string();
        let file_content1 = b"test content".to_vec();
        let file_name2 = "test_file2.txt".to_string();
        let file_content2 = b"test content2".to_vec();

        db.add_file_to_files_message_inbox(hex_blake3_hash.clone(), file_name1.clone(), file_content1.clone())
            .unwrap();

        db.add_file_to_files_message_inbox(hex_blake3_hash.clone(), file_name2.clone(), file_content2.clone())
            .unwrap();

        let files = db.get_all_files_from_inbox(hex_blake3_hash.clone()).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, file_name1);
        assert_eq!(files[0].1, file_content1);
        assert_eq!(files[1].0, file_name2);
        assert_eq!(files[1].1, file_content2);

        let file_names = db.get_all_filenames_from_inbox(hex_blake3_hash.clone()).unwrap();
        assert_eq!(file_names.len(), 2);
        assert_eq!(file_names[0], file_name1);
        assert_eq!(file_names[1], file_name2);

        let file_content = db.get_file_from_inbox(hex_blake3_hash.clone(), file_name1).unwrap();
        assert_eq!(file_content, file_content1);

        db.remove_inbox(&hex_blake3_hash).unwrap();

        let files = db.get_all_files_from_inbox(hex_blake3_hash).unwrap();
        assert_eq!(files.len(), 0);
    }
}
