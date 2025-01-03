use std::fs;

use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;

use shinkai_message_primitives::schemas::shinkai_fs::ParsedFile;

use crate::shinkai_file_manager::ShinkaiFileManager;
use crate::shinkai_fs_error::ShinkaiFsError;

impl ShinkaiFileManager {
    /// Add a file: writes a file from `data` to a relative path under `base_dir`.
    pub fn write_file_to_fs(dest_path: ShinkaiPath, data: Vec<u8>) -> Result<(), ShinkaiFsError> {
        // Ensure the parent directory exists
        fs::create_dir_all(dest_path.as_path().parent().unwrap())?;

        // Write the data to the destination path
        fs::write(dest_path.as_path(), data)?;

        Ok(())
    }

    /// Remove file: deletes file from filesystem and DB.
    pub fn remove_file(path: ShinkaiPath, sqlite_manager: &SqliteManager) -> Result<(), ShinkaiFsError> {
        eprintln!("remove_file> path: {:?}", path);
        // Check if file exists on filesystem
        if !path.exists() {
            return Err(ShinkaiFsError::FileNotFoundOnFilesystem);
        }

        // Remove from filesystem
        fs::remove_file(path.as_path())?;

        // Update DB
        let rel_path = path.relative_path();
        if let Some(parsed_file) = sqlite_manager.get_parsed_file_by_rel_path(&rel_path)? {
            eprintln!("remove_file> parsed_file: {:?}", parsed_file);
            if let Some(parsed_file_id) = parsed_file.id {
                // Remove associated chunks if they exist
                eprintln!("remove_file> parsed_file_id: {:?}", parsed_file_id);
                if let Ok(chunks) = sqlite_manager.get_chunks_for_parsed_file(parsed_file_id) {
                    eprintln!("remove_file> chunks: {:?}", chunks);
                    for chunk in chunks {
                        if let Some(chunk_id) = chunk.chunk_id {
                            eprintln!("remove_file> chunk_id: {:?}", chunk_id);
                            sqlite_manager.remove_chunk_with_embedding(chunk_id)?;
                        }
                    }
                }
                // Remove the parsed file entry
                sqlite_manager.remove_parsed_file(parsed_file_id)?;
            }
        }

        Ok(())
    }

    /// Create folder: just create a directory on the filesystem.
    /// No DB changes since we don't store directories in DB.
    pub fn create_folder(path: ShinkaiPath) -> Result<(), ShinkaiFsError> {
        fs::create_dir_all(path.as_path())?;
        Ok(())
    }

    /// Remove folder: remove a directory and all its contents from the filesystem.
    /// This does not directly affect the DB, but any files in that folder
    /// should have been removed first. If not, scanning the DB for files
    /// might be necessary.
    pub fn remove_folder(path: ShinkaiPath, sqlite_manager: &SqliteManager) -> Result<(), ShinkaiFsError> {
        eprintln!("remove_folder> path: {:?}", path);
        if !path.exists() {
            eprintln!("remove_folder> path does not exist: {:?}", path);
            return Err(ShinkaiFsError::FolderNotFoundOnFilesystem);
        }

        // Iterate over each file or directory in the directory
        for entry in fs::read_dir(path.as_path())? {
            eprintln!("remove_folder> entry: {:?}", entry);
            let entry = entry?;
            let file_path = ShinkaiPath::from_str(entry.path().to_str().unwrap());

            if file_path.is_file() {
                // Remove the file and its embeddings
                eprintln!("remove_folder> file_path is a file: {:?}", file_path);
                Self::remove_file(file_path, sqlite_manager)?;
            } else if file_path.as_path().is_dir() {
                eprintln!("remove_folder> file_path is a directory: {:?}", file_path);
                // Recursively remove subdirectories
                Self::remove_folder(file_path, sqlite_manager)?;
            }
        }

        // Remove the directory itself
        fs::remove_dir(path.as_path())?;
        Ok(())
    }

    /// Rename file: rename a file in the filesystem and update `ParsedFile.relative_path` in DB.
    pub fn rename_file(
        old_path: ShinkaiPath,
        new_path: ShinkaiPath,
        sqlite_manager: &SqliteManager,
    ) -> Result<(), ShinkaiFsError> {
        // Debugging: Check if the old file exists
        if !old_path.exists() {
            println!("Old file does not exist: {:?}", old_path);
            return Err(ShinkaiFsError::FileNotFoundOnFilesystem);
        }

        let new_rel_path = new_path.relative_path();
        // Debugging: Print the new path
        eprintln!("Renaming to new path: {:?}", new_rel_path);

        // Check if the parent directory of the new path exists
        let parent_dir = new_path.as_path().parent().unwrap();
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)?;
        }

        fs::rename(old_path.as_path(), &new_path.as_path())?;

        // Update DB
        let old_rel_path = old_path.relative_path();
        if let Some(mut parsed_file) = sqlite_manager.get_parsed_file_by_rel_path(&old_rel_path)? {
            parsed_file.relative_path = new_path.relative_path().to_string();
            sqlite_manager.update_parsed_file(&parsed_file)?;
        } else {
            // File not found in DB is not necessarily an error, it just means that it doesn't have embeddings.
            eprintln!(
                "Rename File not found in DB: {:?} (it just doesn't have embeddings)",
                old_path
            );
        }

        Ok(())
    }

    /// Move file: effectively the same as renaming a file to a new directory.
    pub fn move_file(
        old_path: ShinkaiPath,
        new_path: ShinkaiPath,
        sqlite_manager: &SqliteManager,
    ) -> Result<(), ShinkaiFsError> {
        Self::rename_file(old_path, new_path, sqlite_manager)
    }

    /// Move folder: like rename_folder, but the new folder can be somewhere else entirely in the directory tree.
    pub fn move_folder(
        old_path: ShinkaiPath,
        new_path: ShinkaiPath,
        sqlite_manager: &SqliteManager,
    ) -> Result<(), ShinkaiFsError> {
        // Check if the old folder exists
        if !old_path.exists() {
            println!("Old folder does not exist: {:?}", old_path);
            return Err(ShinkaiFsError::FolderNotFoundOnFilesystem);
        }

        let new_rel_path = new_path.relative_path();
        // Debugging: Print the new path
        eprintln!("Moving to new path: {:?}", new_rel_path);

        // Check if the parent directory of the new path exists
        let parent_dir = new_path.as_path().parent().unwrap();
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)?;
        }

        fs::rename(old_path.as_path(), &new_path.as_path())?;

        // Update DB for all parsed_files under old_path
        let old_rel_path = old_path.relative_path();
        let all_files = sqlite_manager.get_parsed_files_by_prefix(&old_rel_path)?;
        for mut pf in all_files {
            let remainder = &pf.relative_path[old_rel_path.len()..];
            pf.relative_path = format!("{}{}", new_rel_path, remainder);
            sqlite_manager.update_parsed_file(&pf)?;
        }

        Ok(())
    }

    // /// Scan a folder: recursively discover all files in a directory, and `process_file` them.
    // /// Files that have not been seen before are added, changed files are re-processed, and
    // /// removed files should be cleaned up (if desired).
    // pub fn scan_folder(
    //     directory: ShinkaiPath,
    //     sqlite_manager: &SqliteManager
    // ) -> Result<(), FileManagerError> {
    //     if !directory.exists() {
    //         return Err(FileManagerError::FolderNotFoundOnFilesystem);
    //     }

    //     let files = Self::get_files_in_directory(directory)?;
    //     for file_path in files {
    //         Self::process_file(file_path, base_dir, sqlite_manager)?;
    //     }

    //     // Optionally, remove entries from DB that no longer exist on filesystem by comparing DB entries with filesystem.
    //     // This step is optional and depends on your desired behavior.

    //     Ok(())
    // }

    /// Check if file is supported for embedding (placeholder).
    pub fn is_supported_for_embedding(parsed_file: &ParsedFile) -> bool {
        match parsed_file.original_extension.as_deref() {
            Some("txt") | Some("pdf") | Some("doc") => true,
            _ => false,
        }
    }

    /// Returns the current UNIX timestamp (in seconds).
    pub fn current_timestamp() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let start = SystemTime::now();
        let since_epoch = start.duration_since(UNIX_EPOCH).unwrap();
        since_epoch.as_secs() as i64
    }

    /// Splits text into chunks of approximately `chunk_size` characters.
    pub fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
        text.chars()
            .collect::<Vec<char>>()
            .chunks(chunk_size)
            .map(|c| c.iter().collect())
            .collect()
    }

    /// Copy file: copies a file from `input_path` to `destination_path`.
    /// `destination_path` is the directory where the file should be copied.
    pub fn copy_file(input_path: ShinkaiPath, destination_path: ShinkaiPath) -> Result<(), ShinkaiFsError> {
        // Ensure the parent directory of the destination path exists
        fs::create_dir_all(destination_path.as_path())?;

        // Derive the file name from the input path
        let file_name = input_path.as_path().file_name().unwrap();

        // Construct the full destination path
        let full_destination_path = destination_path.as_path().join(file_name);

        // Copy the file
        fs::copy(input_path.as_path(), full_destination_path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::shinkai_file_manager::FileProcessingMode;

    use super::*;
    use serial_test::serial;
    use shinkai_embedding::mock_generator::MockGenerator;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_message_primitives::schemas::shinkai_fs::ShinkaiFileChunk;
    use std::fs::{self, File};
    use std::io::Read;
    use std::path::PathBuf;
    use tempfile::{tempdir, NamedTempFile};

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    fn create_test_parsed_file(id: i64, relative_path: &str) -> ParsedFile {
        ParsedFile {
            id: Some(id),
            relative_path: relative_path.to_string(),
            original_extension: None,
            description: None,
            source: None,
            embedding_model_used: None,
            keywords: None,
            distribution_info: None,
            created_time: None,
            tags: None,
            total_tokens: None,
            total_characters: None,
        }
    }

    #[test]
    #[serial]
    fn test_remove_empty_folder() {
        let dir = tempdir().unwrap();
        let path = ShinkaiPath::from_string(dir.path().to_string_lossy().to_string());

        // Create an empty folder
        fs::create_dir_all(path.as_path()).unwrap();

        // Setup the test database
        let sqlite_manager = setup_test_db();

        // Attempt to remove the empty folder
        assert!(ShinkaiFileManager::remove_folder(path.clone(), &sqlite_manager).is_ok());

        // Ensure the folder is removed
        assert!(!path.exists());
    }

    #[test]
    #[serial]
    fn test_remove_non_empty_folder() {
        let dir = tempdir().unwrap();

        // Set the environment variable to the temporary directory path
        std::env::set_var("NODE_STORAGE_PATH", dir.path().to_string_lossy().to_string());

        let folder_path = ShinkaiPath::new("test_folder");
        fs::create_dir_all(folder_path.as_path()).unwrap();

        let file_path = ShinkaiPath::new("test_folder/test_file.txt");
        File::create(file_path.as_path()).unwrap();

        // let base_path = ShinkaiPath::from_base_path();
        // eprintln!("base_path: {:?}", base_path);

        // Setup the test database
        let sqlite_manager = setup_test_db();

        // Add a parsed file and chunks to the database
        let parsed_file = create_test_parsed_file(1, "test_folder/test_file.txt");
        sqlite_manager.add_parsed_file(&parsed_file).unwrap();

        let chunk = ShinkaiFileChunk {
            chunk_id: None,
            parsed_file_id: parsed_file.id.unwrap(),
            position: 1,
            content: "This is a test chunk.".to_string(),
        };
        sqlite_manager.create_chunk_with_embedding(&chunk, None).unwrap();

        // Attempt to remove the non-empty folder
        assert!(ShinkaiFileManager::remove_folder(folder_path.clone(), &sqlite_manager).is_ok());

        // Ensure the folder is removed
        assert!(!folder_path.exists());

        // Verify the file and its chunks are removed from the database
        let chunks = sqlite_manager.get_chunks_for_parsed_file(parsed_file.id.unwrap());

        assert!(
            chunks.unwrap().is_empty(),
            "Chunks should be removed from the database."
        );
    }

    #[test]
    #[serial]
    fn test_add_file() {
        let dir = tempdir().unwrap();
        let path = ShinkaiPath::from_string(dir.path().join("test_file.txt").to_string_lossy().to_string());
        let data = b"Hello, Shinkai!".to_vec();

        // Add the file
        assert!(ShinkaiFileManager::write_file_to_fs(path.clone(), data.clone()).is_ok());

        // Verify the file exists and contains the correct data
        let mut file = File::open(path.as_path()).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, data);
    }

    #[test]
    #[serial]
    fn test_rename_file_without_embeddings() {
        let dir = tempdir().unwrap();

        // Set the environment variable to the temporary directory path
        std::env::set_var("NODE_STORAGE_PATH", dir.path().to_string_lossy().to_string());

        let old_path = ShinkaiPath::from_string("old_file.txt".to_string());
        let new_path = ShinkaiPath::from_string("new_file.txt".to_string());

        let data = b"Hello, Shinkai!".to_vec();

        // Create the original file
        ShinkaiFileManager::write_file_to_fs(old_path.clone(), data.clone()).unwrap();

        // Setup the test database
        let sqlite_manager = setup_test_db();

        // List directory contents
        let contents =
            ShinkaiFileManager::list_directory_contents(ShinkaiPath::from_base_path(), &sqlite_manager).unwrap();
        eprintln!("contents: {:?}", contents);

        // Verify the file is listed
        let mut found_file = false;
        for entry in contents {
            if entry.path == "old_file.txt" && !entry.is_directory {
                found_file = true;
                assert!(!entry.has_embeddings, "File 'old_file.txt' should not have embeddings.");
            }
        }

        assert!(found_file, "File 'old_file.txt' should be found in the directory.");

        // Rename the file
        let rename_result = ShinkaiFileManager::rename_file(old_path.clone(), new_path.clone(), &sqlite_manager);
        assert!(
            rename_result.is_ok(),
            "Renaming the file should succeed: {:?}",
            rename_result
        );

        // Verify the old file does not exist and the new file does
        assert!(!old_path.exists(), "The old file should not exist after renaming.");
        assert!(new_path.exists(), "The new file should exist after renaming.");
    }

    #[tokio::test]
    #[serial]
    async fn test_rename_file_with_embeddings() {
        let dir = tempdir().unwrap();

        // Set the environment variable to the temporary directory path
        std::env::set_var("NODE_STORAGE_PATH", dir.path().to_string_lossy().to_string());

        let old_path = ShinkaiPath::from_string("old_file.txt".to_string());
        let new_path = ShinkaiPath::from_string("new_file.txt".to_string());

        let data = b"Hello, Shinkai!".to_vec();

        // Create the original file
        ShinkaiFileManager::write_file_to_fs(old_path.clone(), data.clone()).unwrap();

        // Setup the test database
        let sqlite_manager = setup_test_db();

        // List directory contents
        let contents =
            ShinkaiFileManager::list_directory_contents(ShinkaiPath::from_base_path(), &sqlite_manager).unwrap();
        eprintln!("contents: {:?}", contents);

        // Verify the file is listed
        let mut found_file = false;
        for entry in contents {
            if entry.path == "old_file.txt" && !entry.is_directory {
                found_file = true;
                assert!(!entry.has_embeddings, "File 'old_file.txt' should not have embeddings.");
            }
        }

        assert!(found_file, "File 'old_file.txt' should be found in the directory.");

        let mock_generator = MockGenerator::new(
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M),
            10,
        );

        // Add embeddings to the file
        let _ = ShinkaiFileManager::process_embeddings_for_file(
            old_path.clone(),
            &sqlite_manager,
            FileProcessingMode::Auto,
            &mock_generator,
        )
        .await;

        // Rename the file
        let rename_result = ShinkaiFileManager::rename_file(old_path.clone(), new_path.clone(), &sqlite_manager);
        assert!(
            rename_result.is_ok(),
            "Renaming the file should succeed: {:?}",
            rename_result
        );

        // Verify the old file does not exist and the new file does
        assert!(!old_path.exists(), "The old file should not exist after renaming.");
        assert!(new_path.exists(), "The new file should exist after renaming.");

        let results = sqlite_manager.debug_get_all_parsed_files();
        eprintln!("results: {:?}", results);

        // Check that the file path with the embeddings were updated in the db
        if let Some(parsed_file) = sqlite_manager
            .get_parsed_file_by_rel_path(&new_path.relative_path())
            .unwrap()
        {
            assert_eq!(
                parsed_file.relative_path,
                new_path.relative_path(),
                "The relative path in the database should be updated to the new path."
            );
        } else {
            panic!("The file should be found in the database with the updated path.");
        }
    }

    #[test]
    #[serial]
    fn test_copy_file() {
        let dir = tempdir().unwrap();

        // Set the environment variable to the temporary directory path
        std::env::set_var("NODE_STORAGE_PATH", dir.path().to_string_lossy().to_string());

        let input_path = ShinkaiPath::from_string("input_file.txt".to_string());
        let destination_dir = ShinkaiPath::from_string("destination_dir".to_string());
        let data = b"Hello, Shinkai!".to_vec();

        // Add the input file
        assert!(ShinkaiFileManager::write_file_to_fs(input_path.clone(), data.clone()).is_ok());

        // Create the destination directory
        assert!(ShinkaiFileManager::create_folder(destination_dir.clone()).is_ok());

        // Copy the file
        assert!(ShinkaiFileManager::copy_file(input_path.clone(), destination_dir.clone()).is_ok());

        // Verify the destination file exists and contains the correct data
        let destination_file_path = destination_dir.as_path().join("input_file.txt");
        let mut file = File::open(destination_file_path).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, data);
    }

    #[test]
    #[serial]
    fn test_remove_file_and_folder() {
        let dir = tempdir().unwrap();

        // Set the environment variable to the temporary directory path
        std::env::set_var("NODE_STORAGE_PATH", dir.path().to_string_lossy().to_string());

        let file_path = ShinkaiPath::from_string("test_file.txt".to_string());
        let folder_path = ShinkaiPath::from_string("test_folder".to_string());
        let data = b"Hello, Shinkai!".to_vec();

        // Setup the test database
        let sqlite_manager = setup_test_db();

        // Add the file
        assert!(ShinkaiFileManager::write_file_to_fs(file_path.clone(), data.clone()).is_ok());

        // Add a parsed file and chunks to the database
        let parsed_file = create_test_parsed_file(1, "test_file.txt");
        sqlite_manager.add_parsed_file(&parsed_file).unwrap();

        let chunk = ShinkaiFileChunk {
            chunk_id: None,
            parsed_file_id: parsed_file.id.unwrap(),
            position: 1,
            content: "This is a test chunk.".to_string(),
        };
        sqlite_manager.create_chunk_with_embedding(&chunk, None).unwrap();

        let chunks = sqlite_manager.get_chunks_for_parsed_file(parsed_file.id.unwrap());
        eprintln!("chunks before delete: {:?}", chunks);

        // Remove the file
        assert!(ShinkaiFileManager::remove_file(file_path.clone(), &sqlite_manager).is_ok());

        // Verify the file and its chunks are removed
        assert!(!file_path.exists(), "The file should not exist after removal.");
        let chunks = sqlite_manager.get_chunks_for_parsed_file(parsed_file.id.unwrap());
        eprintln!("chunks after delete: {:?}", chunks);
        assert!(
            chunks.unwrap().is_empty(),
            "Chunks should be removed from the database."
        );

        // Create a folder
        assert!(ShinkaiFileManager::create_folder(folder_path.clone()).is_ok());

        // Remove the folder
        assert!(ShinkaiFileManager::remove_folder(folder_path.clone(), &sqlite_manager).is_ok());

        // Verify the folder is removed
        assert!(!folder_path.exists(), "The folder should not exist after removal.");
    }

    #[test]
    #[serial]
    fn test_move_folder() {
        let dir = tempdir().unwrap();
        let base_dir = dir.path();

        // Set the environment variable to the temporary directory path
        std::env::set_var("NODE_STORAGE_PATH", base_dir.to_string_lossy().to_string());

        let folder_path = ShinkaiPath::from_string("test_folder".to_string());
        let new_folder_path = ShinkaiPath::from_string("new_test_folder".to_string());

        let file1_path = ShinkaiPath::from_string("test_folder/file1.txt".to_string());
        let file2_path = ShinkaiPath::from_string("test_folder/file2.txt".to_string());

        let data1 = b"File 1 content".to_vec();
        let data2 = b"File 2 content".to_vec();

        // Setup the test database
        let sqlite_manager = setup_test_db();

        // Create the folder and add files
        assert!(ShinkaiFileManager::create_folder(folder_path.clone()).is_ok());
        assert!(ShinkaiFileManager::write_file_to_fs(file1_path.clone(), data1.clone()).is_ok());
        assert!(ShinkaiFileManager::write_file_to_fs(file2_path.clone(), data2.clone()).is_ok());

        // Add parsed files to the database
        let parsed_file1 = create_test_parsed_file(1, "test_folder/file1.txt");
        let parsed_file2 = create_test_parsed_file(2, "test_folder/file2.txt");
        sqlite_manager.add_parsed_file(&parsed_file1).unwrap();
        sqlite_manager.add_parsed_file(&parsed_file2).unwrap();

        // Move the folder
        assert!(ShinkaiFileManager::move_folder(folder_path.clone(), new_folder_path.clone(), &sqlite_manager).is_ok());

        // Verify the files have been moved in the filesystem
        let new_file1_path = ShinkaiPath::from_string("new_test_folder/file1.txt".to_string());
        let new_file2_path = ShinkaiPath::from_string("new_test_folder/file2.txt".to_string());

        assert!(new_file1_path.exists(), "File 1 should exist in the new location.");
        assert!(new_file2_path.exists(), "File 2 should exist in the new location.");

        // Verify the files have been moved in the database
        let updated_file1 = sqlite_manager
            .get_parsed_file_by_rel_path("new_test_folder/file1.txt")
            .unwrap();
        let updated_file2 = sqlite_manager
            .get_parsed_file_by_rel_path("new_test_folder/file2.txt")
            .unwrap();

        assert!(updated_file1.is_some(), "File 1 should be updated in the database.");
        assert!(updated_file2.is_some(), "File 2 should be updated in the database.");
    }

    #[test]
    #[serial]
    fn test_remove_folder_with_subfolder_and_embeddings() {
        let dir = tempdir().unwrap();
        let base_dir = dir.path();

        // Set the environment variable to the temporary directory path
        std::env::set_var("NODE_STORAGE_PATH", base_dir.to_string_lossy().to_string());

        let main_folder_path = ShinkaiPath::from_string("main_folder".to_string());
        let subfolder_path = ShinkaiPath::from_string("main_folder/subfolder".to_string());

        let file1_path = ShinkaiPath::from_string("main_folder/file1.txt".to_string());
        let file2_path = ShinkaiPath::from_string("main_folder/subfolder/file2.txt".to_string());

        let data1 = b"File 1 content".to_vec();
        let data2 = b"File 2 content".to_vec();

        // Setup the test database
        let sqlite_manager = setup_test_db();

        // Create the main folder, subfolder, and add files
        assert!(ShinkaiFileManager::create_folder(main_folder_path.clone()).is_ok());
        assert!(ShinkaiFileManager::create_folder(subfolder_path.clone()).is_ok());
        assert!(ShinkaiFileManager::write_file_to_fs(file1_path.clone(), data1.clone()).is_ok());
        assert!(ShinkaiFileManager::write_file_to_fs(file2_path.clone(), data2.clone()).is_ok());

        // Add parsed files and chunks to the database
        let parsed_file1 = create_test_parsed_file(1, "main_folder/file1.txt");
        let parsed_file2 = create_test_parsed_file(2, "main_folder/subfolder/file2.txt");
        sqlite_manager.add_parsed_file(&parsed_file1).unwrap();
        sqlite_manager.add_parsed_file(&parsed_file2).unwrap();

        let chunk1 = ShinkaiFileChunk {
            chunk_id: None,
            parsed_file_id: parsed_file1.id.unwrap(),
            position: 1,
            content: "This is a test chunk for file 1.".to_string(),
        };
        sqlite_manager.create_chunk_with_embedding(&chunk1, None).unwrap();

        let chunk2 = ShinkaiFileChunk {
            chunk_id: None,
            parsed_file_id: parsed_file2.id.unwrap(),
            position: 1,
            content: "This is a test chunk for file 2.".to_string(),
        };
        sqlite_manager.create_chunk_with_embedding(&chunk2, None).unwrap();

        // Remove the main folder
        assert!(ShinkaiFileManager::remove_folder(main_folder_path.clone(), &sqlite_manager).is_ok());

        // Verify the main folder and subfolder are removed
        assert!(!main_folder_path.exists(), "The main folder should not exist after removal.");
        assert!(!subfolder_path.exists(), "The subfolder should not exist after removal.");

        // Verify the files and their chunks are removed from the database
        let chunks1 = sqlite_manager.get_chunks_for_parsed_file(parsed_file1.id.unwrap());
        assert!(
            chunks1.unwrap().is_empty(),
            "Chunks for file 1 should be removed from the database."
        );

        let chunks2 = sqlite_manager.get_chunks_for_parsed_file(parsed_file2.id.unwrap());
        assert!(
            chunks2.unwrap().is_empty(),
            "Chunks for file 2 should be removed from the database."
        );
    }
}
