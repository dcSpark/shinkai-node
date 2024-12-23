use std::fs;
use std::io;
use std::path::Path;

use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;

use shinkai_message_primitives::schemas::shinkai_fs::ParsedFile;

use crate::shinkai_file_manager::ShinkaiFileManager;
use crate::shinkai_fs_error::ShinkaiFsError;

impl ShinkaiFileManager {
    /// Add a file: writes a file from `data` to a relative path under `base_dir`.
    pub fn add_file(dest_path: ShinkaiPath, data: Vec<u8>) -> Result<(), ShinkaiFsError> {
        // Ensure the parent directory exists
        fs::create_dir_all(dest_path.as_path().parent().unwrap())?;

        // Write the data to the destination path
        fs::write(dest_path.as_path(), data)?;

        Ok(())
    }

    /// Remove file: deletes file from filesystem and DB.
    pub fn remove_file(
        path: ShinkaiPath,
        base_dir: &Path,
        sqlite_manager: &SqliteManager,
    ) -> Result<(), ShinkaiFsError> {
        // Check if file exists on filesystem
        if !path.exists() {
            return Err(ShinkaiFsError::FileNotFoundOnFilesystem);
        }

        // Remove from filesystem
        fs::remove_file(path.as_path())?;

        // Update DB
        let rel_path = Self::compute_relative_path(&path, base_dir)?;
        if let Some(parsed_file) = sqlite_manager.get_parsed_file_by_rel_path(&rel_path)? {
            if let Some(parsed_file_id) = parsed_file.id {
                sqlite_manager.remove_parsed_file(parsed_file_id)?;
            } else {
                return Err(ShinkaiFsError::FailedToRetrieveParsedFileID);
            }
        } else {
            return Err(ShinkaiFsError::FileNotFoundInDatabase);
        }

        Ok(())
    }

    /// Create folder: just create a directory on the filesystem.
    /// No DB changes since we don't store directories in DB.
    pub fn create_folder(path: ShinkaiPath) -> Result<(), ShinkaiFsError> {
        fs::create_dir_all(path.as_path())?;
        Ok(())
    }

    /// Remove folder: remove a directory from the filesystem.
    /// This does not directly affect the DB, but any files in that folder
    /// should have been removed first. If not, scanning the DB for files
    /// might be necessary.
    pub fn remove_folder(path: ShinkaiPath) -> Result<(), ShinkaiFsError> {
        if !path.exists() {
            return Err(ShinkaiFsError::FolderNotFoundOnFilesystem);
        }

        // Check if the folder is empty
        if fs::read_dir(path.as_path())?.next().is_some() {
            return Err(ShinkaiFsError::FolderNotFoundOnFilesystem);
        }

        fs::remove_dir(path.as_path())?;
        Ok(())
    }

    /// Rename file: rename a file in the filesystem and update `ParsedFile.relative_path` in DB.
    pub fn rename_file(
        old_path: ShinkaiPath,
        new_relative_path: &str,
        base_dir: &Path,
        sqlite_manager: &SqliteManager,
    ) -> Result<(), ShinkaiFsError> {
        // Debugging: Check if the old file exists
        if !old_path.exists() {
            println!("Old file does not exist: {:?}", old_path);
            return Err(ShinkaiFsError::FileNotFoundOnFilesystem);
        }

        let new_path = base_dir.join(new_relative_path);
        // Debugging: Print the new path
        println!("Renaming to new path: {:?}", new_path);

        fs::create_dir_all(new_path.parent().unwrap())?;
        fs::rename(old_path.as_path(), &new_path)?;

        // Update DB
        let old_rel_path = Self::compute_relative_path(&old_path, base_dir)?;
        if let Some(mut parsed_file) = sqlite_manager.get_parsed_file_by_rel_path(&old_rel_path)? {
            parsed_file.relative_path = new_relative_path.to_string();
            sqlite_manager.update_parsed_file(&parsed_file)?;
        } else {
            // File not found in DB is not necessarily an error if we just discovered it,
            // but let's return an error for consistency.
            return Err(ShinkaiFsError::FileNotFoundInDatabase);
        }

        Ok(())
    }

    // /// Rename folder: rename a directory in the filesystem and update all `ParsedFile.relative_path`
    // /// entries that are inside this folder.
    // pub fn rename_folder(
    //     old_path: ShinkaiPath,
    //     new_relative_path: &str,
    //     base_dir: &Path,
    //     sqlite_manager: &SqliteManager
    // ) -> Result<(), FileManagerError> {
    //     if !old_path.exists() {
    //         return Err(FileManagerError::FolderNotFoundOnFilesystem);
    //     }

    //     let new_path = base_dir.join(new_relative_path);
    //     fs::create_dir_all(new_path.parent().unwrap())?;
    //     fs::rename(old_path.as_path(), &new_path)?;

    //     // Update DB for all parsed_files under old_path
    //     let old_rel_path = Self::compute_relative_path(&old_path, base_dir)?;
    //     // Ensure old_rel_path always ends with a slash to match prefixes correctly
    //     let old_prefix = if !old_rel_path.ends_with('/') {
    //         format!("{}/", old_rel_path)
    //     } else {
    //         old_rel_path
    //     };

    //     let new_prefix = if !new_relative_path.ends_with('/') {
    //         format!("{}/", new_relative_path)
    //     } else {
    //         new_relative_path.to_string()
    //     };

    //     let all_files = sqlite_manager.get_all_parsed_files()?;
    //     for mut pf in all_files {
    //         if pf.relative_path.starts_with(&old_prefix) {
    //             let remainder = &pf.relative_path[old_prefix.len()..];
    //             pf.relative_path = format!("{}{}", new_prefix, remainder);
    //             sqlite_manager.update_parsed_file(&pf)?;
    //         }
    //     }

    //     Ok(())
    // }

    /// Move file: effectively the same as renaming a file to a new directory.
    pub fn move_file(
        old_path: ShinkaiPath,
        new_relative_path: &str,
        base_dir: &Path,
        sqlite_manager: &SqliteManager,
    ) -> Result<(), ShinkaiFsError> {
        Self::rename_file(old_path, new_relative_path, base_dir, sqlite_manager)
    }

    // /// Move folder: like rename_folder, but the new folder can be somewhere else entirely in the directory tree.
    // pub fn move_folder(
    //     old_path: ShinkaiPath,
    //     new_relative_path: &str,
    //     base_dir: &Path,
    //     sqlite_manager: &SqliteManager
    // ) -> Result<(), FileManagerError> {
    //     // This is essentially the same operation as rename_folder if the only difference is the path.
    //     Self::rename_folder(old_path, new_relative_path, base_dir, sqlite_manager)
    // }

    // /// Scan a folder: recursively discover all files in a directory, and `process_file` them.
    // /// Files that have not been seen before are added, changed files are re-processed, and
    // /// removed files should be cleaned up (if desired).
    // pub fn scan_folder(
    //     directory: ShinkaiPath,
    //     base_dir: &Path,
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

    /// Compute a relative path given a file path and a base directory.
    pub fn compute_relative_path(file_path: &ShinkaiPath, base_dir: &Path) -> Result<String, ShinkaiFsError> {
        let abs_file_path = file_path.as_path().canonicalize()?;
        let abs_base_dir = base_dir.canonicalize()?;

        let rel = abs_file_path
            .strip_prefix(&abs_base_dir)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "File is not under the base directory"))?;

        Ok(rel.to_string_lossy().to_string())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Read;
    use tempfile::tempdir;

    #[test]
    fn test_remove_empty_folder() {
        let dir = tempdir().unwrap();
        let path = ShinkaiPath::from_string(dir.path().to_string_lossy().to_string());

        // Create an empty folder
        fs::create_dir_all(path.as_path()).unwrap();

        // Attempt to remove the empty folder
        assert!(ShinkaiFileManager::remove_folder(path.clone()).is_ok());

        // Ensure the folder is removed
        assert!(!path.exists());
    }

    #[test]
    fn test_remove_non_empty_folder() {
        let dir = tempdir().unwrap();
        let path = ShinkaiPath::from_string(dir.path().to_string_lossy().to_string());

        // Create a folder and add a file inside it
        fs::create_dir_all(path.as_path()).unwrap();
        let file_path = path.as_path().join("test_file.txt");
        File::create(&file_path).unwrap();

        // Attempt to remove the non-empty folder
        assert!(ShinkaiFileManager::remove_folder(path.clone()).is_err());

        // Ensure the folder still exists
        assert!(path.exists());
    }

    #[test]
    fn test_add_file() {
        let dir = tempdir().unwrap();
        let path = ShinkaiPath::from_string(dir.path().join("test_file.txt").to_string_lossy().to_string());
        let data = b"Hello, Shinkai!".to_vec();

        // Add the file
        assert!(ShinkaiFileManager::add_file(path.clone(), data.clone()).is_ok());

        // Verify the file exists and contains the correct data
        let mut file = File::open(path.as_path()).unwrap();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, data);
    }

    // #[test]
    // fn test_rename_file() {
    //     let dir = tempdir().unwrap();
    //     let old_path = ShinkaiPath::from_string(dir.path().join("old_file.txt").to_string_lossy().to_string());
    //     let new_relative_path = "new_file.txt";
    //     let base_dir = dir.path();
    //     let data = b"Hello, Shinkai!".to_vec();

    //     // Create the original file
    //     ShinkaiFileManager::add_file(old_path.clone(), data.clone()).unwrap();

    //     // Debugging: Verify the file was created
    //     assert!(old_path.exists(), "The file should exist on the filesystem after creation.");

    //     // Setup the test database
    //     let sqlite_manager = setup_test_db();

    //     // Add the file to the database
    //     let rel_path = ShinkaiFileManager::compute_relative_path(&old_path, base_dir).unwrap();
    //     let parsed_file = ParsedFile {
    //         id: 0,
    //         relative_path: rel_path.clone(),
    //         original_extension: Some("txt".to_string()),
    //         description: None,
    //         source: None,
    //         embedding_model_used: None,
    //         keywords: None,
    //         distribution_info: None,
    //         created_time: Some(ShinkaiFileManager::current_timestamp()),
    //         tags: None,
    //         total_tokens: None,
    //         total_characters: None,
    //     };
    //     sqlite_manager.add_parsed_file(&parsed_file).unwrap();

    //     // Debugging: Check if the file is in the database
    //     let db_file = sqlite_manager.get_parsed_file_by_rel_path(&rel_path).unwrap();
    //     assert!(db_file.is_some(), "The file should exist in the database before renaming.");

    //     // Debugging: Print paths for verification
    //     println!("Old path: {:?}", old_path);
    //     println!("New path: {:?}", base_dir.join(new_relative_path));

    //     // Rename the file
    //     let rename_result = ShinkaiFileManager::rename_file(old_path.clone(), new_relative_path, base_dir, &sqlite_manager);
    //     assert!(rename_result.is_ok(), "Renaming the file should succeed: {:?}", rename_result);

    //     // Verify the old file does not exist and the new file does
    //     let new_path = base_dir.join(new_relative_path);
    //     assert!(!old_path.exists(), "The old file should not exist after renaming.");
    //     assert!(new_path.exists(), "The new file should exist after renaming.");

    //     // Verify the new file contains the correct data
    //     let mut file = File::open(new_path).unwrap();
    //     let mut contents = Vec::new();
    //     file.read_to_end(&mut contents).unwrap();
    //     assert_eq!(contents, data);
    // }
}
