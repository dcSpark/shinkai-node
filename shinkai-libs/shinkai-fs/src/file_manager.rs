use std::fs;
use std::io;
use shinkai_message_primitives::schemas::shinkai_fs::ShinkaiFile;
use shinkai_message_primitives::schemas::shinkai_fs::ShinkaiFileChunk;
use thiserror::Error;

use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;

#[derive(Debug, Error)]
pub enum FileManagerError {
    #[error("IO error occurred: {0}")]
    Io(#[from] io::Error),
    #[error("Database error: {0}")]
    Database(#[from] SqliteManagerError),
    // Add more error variants as needed
}

pub struct FileManager;

impl FileManager {
    pub fn remove_file(path: ShinkaiPath, sqlite_manager: &SqliteManager) -> Result<(), FileManagerError> {
        // Check if the file exists
        if !path.exists() {
            return Err(FileManagerError::Io(io::Error::new(io::ErrorKind::NotFound, "File not found")));
        }

        // Remove the file from the filesystem
        fs::remove_file(path.as_path()).map_err(FileManagerError::Io)?;

        // Use SqliteManager to get the file by path
        if let Some(file) = sqlite_manager.get_file_by_path(path.as_str())? {
            // Use SqliteManager to remove the file by its ID
            sqlite_manager.remove_file(file.file_id)?;
        } else {
            return Err(FileManagerError::Database(SqliteManagerError::DataNotFound));
        }

        Ok(())
    }

    pub fn get_files_in_directory(directory: ShinkaiPath) -> Result<Vec<ShinkaiPath>, FileManagerError> {
        let mut files = Vec::new();

        let entries = fs::read_dir(directory.as_path())?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                files.push(ShinkaiPath::new(&path));
            } else if path.is_dir() {
                // Recursively get files from subdirectories
                let sub_files = FileManager::get_files_in_directory(ShinkaiPath::new(&path))?;
                files.extend(sub_files);
            }
        }

        Ok(files)
    }

    // 1. File Type Detection and Classification
    pub fn is_supported_for_embedding(file: &ShinkaiFile) -> bool {
        matches!(file.mime_type.as_deref(), Some("text/plain") | Some("application/pdf") | Some("application/msword"))
        // Extend as needed
    }

    // 2. Unified File Processing API
    pub fn process_file(file: &ShinkaiFile, sqlite_manager: &SqliteManager) -> Result<(), FileManagerError> {
        if Self::is_supported_for_embedding(file) {
            // Generate chunks and embeddings
            // Store them in the database
        } else {
            // Store file metadata as is
        }
        Ok(())
    }

    // 8. Search and Retrieval Enhancements
    // pub fn search_similar_chunks(&self, embedding: &[f32], top_k: usize) -> Result<Vec<ShinkaiFileChunk>, SqliteManagerError> {
        // Implement ANN search or fallback approach
    // }
}
