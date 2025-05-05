use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::Serializer;
use serde::{Deserialize, Serialize};
use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use shinkai_message_primitives::schemas::shinkai_fs::{ParsedFile, ShinkaiFileChunk};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_message_primitives::shinkai_utils::utils::count_tokens_from_message_llama3;
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;
use utoipa::ToSchema;

use crate::shinkai_fs_error::ShinkaiFsError;
use crate::simple_parser::simple_parser::SimpleParser;

pub struct ShinkaiFileManager;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, ToSchema)]
pub struct FileInfo {
    pub path: String,
    pub is_directory: bool,
    #[serde(serialize_with = "serialize_system_time")]
    pub created_time: Option<SystemTime>,
    #[serde(serialize_with = "serialize_system_time")]
    pub modified_time: Option<SystemTime>,
    pub has_embeddings: bool,
    pub children: Option<Vec<FileInfo>>,
    pub size: Option<u64>, // None if directory
    pub name: String,      // e.g. "my_doc.docx"
}

#[derive(PartialEq, Serialize, Deserialize, Clone, ToSchema)]
pub enum FileProcessingMode {
    Auto,
    NoParsing,
    MustParse,
}

impl ShinkaiFileManager {
    /// Save a file to disk and process it for embeddings based on the mode.
    pub async fn save_and_process_file(
        dest_path: ShinkaiPath,
        data: Vec<u8>,
        sqlite_manager: &SqliteManager,
        mode: FileProcessingMode,
        generator: &dyn EmbeddingGenerator,
    ) -> Result<(), ShinkaiFsError> {
        // Save the file to disk
        Self::write_file_to_fs(dest_path.clone(), data)?;

        // Process the file for embeddings if the mode is not NoParsing
        if mode != FileProcessingMode::NoParsing {
            let _ = Self::process_embeddings_for_file(dest_path, sqlite_manager, mode, generator).await;
        }

        Ok(())
    }

    /// Process file: If not in DB, add it. If supported, generate chunks.
    /// If already processed, consider checking if file changed (not implemented here).
    pub async fn process_embeddings_for_file(
        path: ShinkaiPath,
        sqlite_manager: &SqliteManager,
        mode: FileProcessingMode, // TODO: maybe we dont need this?
        generator: &dyn EmbeddingGenerator,
    ) -> Result<(), ShinkaiFsError> {
        if mode == FileProcessingMode::NoParsing {
            return Ok(());
        }

        // Compute the relative path
        let rel_path = path.relative_path();

        // Check if the file is already processed
        if let Some(_parsed_file) = sqlite_manager.get_parsed_file_by_rel_path(&rel_path)? {
            // TODO: check if the file has changed since last processing
            return Ok(());
        }

        // Steps to process a file:
        // 1. Read the file content to ensure accessibility.
        // 2. Divide the file content into manageable chunks.
        // 3. Generate embeddings for each chunk using the specified model.
        // 4. Construct a ParsedFile object and associate it with its chunks.
        // 5. Persist the ParsedFile and its chunks into the database.

        // 1- Parse the file
        let max_node_text_size = generator.model_type().max_input_token_count();
        let mut text_groups = SimpleParser::parse_file(path.clone(), max_node_text_size.try_into().unwrap())?;

        // Generate embeddings for each text group and assign them directly
        for text_group in &mut text_groups {
            let embedding = generator.generate_embedding_default(&text_group.text).await?;
            text_group.embedding = Some(embedding);
        }

        // Calculate total characters from all text groups
        let total_characters = text_groups.iter().map(|group| group.text.chars().count() as i64).sum();

        // Calculate total tokens using llama3 token counting
        let total_tokens = text_groups
            .iter()
            .map(|group| count_tokens_from_message_llama3(&group.text) as i64)
            .sum();

        // Add the parsed file to the database
        let parsed_file = ParsedFile {
            id: None, // Expected. The DB will auto-generate the id.
            relative_path: rel_path.to_string(),
            original_extension: path.extension().map(|s| s.to_string()),
            description: None, // TODO: connect this
            source: None,      // TODO: connect this
            embedding_model_used: Some(generator.model_type().to_string()),
            keywords: None,          // TODO: connect this
            distribution_info: None, // TODO: connect this
            created_time: Some(Self::current_timestamp()),
            tags: None, // TODO: connect this
            total_tokens: Some(total_tokens),
            total_characters: Some(total_characters),
        };

        sqlite_manager.add_parsed_file(&parsed_file)?;

        // Retrieve the parsed file ID
        let parsed_file_id = sqlite_manager
            .get_parsed_file_by_rel_path(&rel_path)?
            .ok_or(ShinkaiFsError::FailedToRetrieveParsedFileID)?
            .id
            .unwrap();

        // Create and add chunks to the database
        for (position, text_group) in text_groups.iter().enumerate() {
            let chunk = ShinkaiFileChunk {
                chunk_id: None,
                parsed_file_id,
                position: position as i64,
                content: text_group.text.clone(),
            };
            sqlite_manager
                .create_chunk_with_embedding(&chunk, Some(&text_group.embedding.as_ref().unwrap().clone()))?;
        }

        Ok(())
    }

    pub fn get_absolute_paths_with_folder(files: Vec<String>, folder: PathBuf) -> Vec<String> {
        files
            .iter()
            .map(|path| {
                let file_path = folder.clone().join(path);
                let full_path = fs::canonicalize(file_path).unwrap_or_default();
                full_path.display().to_string()
            })
            .collect()
    }

    pub fn get_absolute_path_for_additional_files(
        files: Vec<ShinkaiPath>,
        folders: Vec<ShinkaiPath>,
    ) -> Result<Vec<String>, ShinkaiFsError> {
        let mut all_files = Vec::new();
        all_files.extend(files.iter().map(|file| {
            fs::canonicalize(file.path.clone())
                .unwrap_or_default()
                .display()
                .to_string()
        }));

        // Recursively get all files from folders using a helper function
        fn get_files_recursive(path: &PathBuf) -> Vec<String> {
            let mut files = Vec::new();
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    if let Ok(path) = entry.path().canonicalize() {
                        if path.is_file() {
                            files.push(format!("{:?}", path.to_string_lossy()));
                        } else if path.is_dir() {
                            // Recursively get files from subdirectories
                            files.extend(get_files_recursive(&path));
                        }
                    }
                }
            }
            files
        }

        // Process each folder recursively
        for folder in folders.iter() {
            all_files.extend(get_files_recursive(&folder.path));
        }

        all_files.extend(folders.iter().map(|folder| format!("{:?}", folder.path.clone())));
        Ok(all_files)
    }

    pub fn get_absolute_path_for_job_scope(
        sqlite_manager: &SqliteManager,
        job_id: &str,
    ) -> Result<Vec<String>, SqliteManagerError> {
        let scope_files = Self::get_all_files_and_folders_for_job_scope(sqlite_manager, job_id);
        let job_files = Self::get_all_files_and_folders_for_job(job_id, sqlite_manager);

        // Skipping erros as folders might not exist, and this is OK.
        let scope_files = scope_files.unwrap_or_default();
        let job_files = job_files.unwrap_or_default();

        let mut all_files = Vec::new();
        all_files.extend(scope_files);
        all_files.extend(job_files);

        let base_path = ShinkaiPath::base_path();
        let files = all_files
            .into_iter()
            .map(|file| {
                let p = base_path.clone().join(file.path);
                fs::canonicalize(p).unwrap_or_default().display().to_string()
            })
            .collect();

        Ok(files)
    }

    fn get_all_files_and_folders_for_job_scope(
        sqlite_manager: &SqliteManager,
        job_id: &str,
    ) -> Result<Vec<FileInfo>, SqliteManagerError> {
        let job = sqlite_manager.get_job(&job_id)?;
        let job_scope = job.scope;
        let files = job_scope.vector_fs_items;
        let files = files
            .into_iter()
            .map(|shinkai_path| {
                let file_info = FileInfo {
                    path: shinkai_path.relative_path().to_string(),
                    is_directory: false,
                    created_time: None,
                    modified_time: None,
                    has_embeddings: false,
                    children: None,
                    size: None,
                    name: shinkai_path.filename().unwrap_or_default().to_string(),
                };
                return file_info;
            })
            .collect::<Vec<FileInfo>>();

        let folder_files = job_scope
            .vector_fs_folders
            .iter()
            .flat_map(|folder| Self::list_directory_contents(folder.clone(), sqlite_manager).unwrap_or_default())
            .collect::<Vec<FileInfo>>();

        let mut all_files = Vec::new();
        all_files.extend(files);
        all_files.extend(folder_files);

        Ok(all_files)
    }

    pub fn get_all_files_and_folders_for_job(
        job_id: &str,
        sqlite_manager: &SqliteManager,
    ) -> Result<Vec<FileInfo>, ShinkaiFsError> {
        // Get the job folder name using the SqliteManager
        let folder_path = sqlite_manager.get_job_folder_name(job_id)?;

        // Use the existing list_directory_contents method to get the files and folders
        Self::list_directory_contents(folder_path, sqlite_manager)
    }

    /// Single-level listing only (no recursion).
    pub fn list_directory_contents(
        path: ShinkaiPath,
        sqlite_manager: &SqliteManager,
    ) -> Result<Vec<FileInfo>, ShinkaiFsError> {
        Self::gather_directory_contents(&path, sqlite_manager, /*current_depth=*/ 0, /*max_depth=*/ 0)
    }

    /// Recursively list files/folders up to `max_depth`.
    pub fn list_directory_contents_with_depth(
        path: ShinkaiPath,
        sqlite_manager: &SqliteManager,
        max_depth: usize,
    ) -> Result<Vec<FileInfo>, ShinkaiFsError> {
        Self::gather_directory_contents(&path, sqlite_manager, /*current_depth=*/ 0, max_depth)
    }

    /// Private helper that does the actual directory reading.
    /// - `current_depth` starts at 0
    /// - If `current_depth < max_depth`, we recurse into subdirectories
    fn gather_directory_contents(
        path: &ShinkaiPath,
        sqlite_manager: &SqliteManager,
        current_depth: usize,
        max_depth: usize,
    ) -> Result<Vec<FileInfo>, ShinkaiFsError> {
        let mut contents = Vec::new();
        let rel_path = path.relative_path();

        for entry in walkdir::WalkDir::new(path.as_path())
            .max_depth(1)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            // Skip the root directory itself (depth 0)
            if entry.depth() == 0 {
                continue;
            }
            
            let file_name = entry.file_name().to_str().unwrap_or_default();
            let shinkai_path = ShinkaiPath::new(&format!("{}/{}", rel_path, file_name));

            let mut file_info = FileInfo {
                path: shinkai_path.relative_path().to_string(),
                is_directory: entry.file_type().is_dir(),
                created_time: entry.metadata().ok().and_then(|m| m.created().ok()),
                modified_time: entry.metadata().ok().and_then(|m| m.modified().ok()),
                has_embeddings: false,
                children: None,
                size: if entry.file_type().is_file() { Some(entry.metadata().unwrap().len()) } else { None },
                name: file_name.to_string(),
            };

            // If it's a directory and we can still go deeper, recurse
            if file_info.is_directory && current_depth < max_depth {
                file_info.children = Some(Self::gather_directory_contents(
                    &shinkai_path,
                    sqlite_manager,
                    current_depth + 1,
                    max_depth,
                )?);
            } else if !file_info.is_directory {
                // Lookup embeddings directly in the DB
                let maybe_parsed_file = sqlite_manager.get_parsed_file_by_rel_path(&file_info.path)?;
                file_info.has_embeddings = maybe_parsed_file.is_some();
            }

            contents.push(file_info);
        }
        Ok(contents)
    }

    /// Constructs the full path for a file within a job folder.
    pub fn construct_job_file_path(
        job_id: &str,
        file_name: &str,
        sqlite_manager: &SqliteManager,
    ) -> Result<ShinkaiPath, ShinkaiFsError> {
        // Get the job folder path
        let folder_path = sqlite_manager.get_and_create_job_folder(job_id)?;

        // Get the relative path from the job folder to avoid double base path
        let relative_path = folder_path.relative_path();

        // Construct the relative path for the file
        let file_relative_path = format!("{}/{}", relative_path, file_name);

        // Create a new ShinkaiPath from the relative path
        Ok(ShinkaiPath::from_string(file_relative_path))
    }

    /// Save a file to a job-specific directory and process it for embeddings.
    /// This function determines the job folder path, constructs the file path,
    /// and then saves and processes the file using the specified mode and generator.
    pub async fn save_and_process_file_with_jobid(
        job_id: &str,
        file_name: String,
        data: Vec<u8>,
        sqlite_manager: &SqliteManager,
        mode: FileProcessingMode,
        generator: &dyn EmbeddingGenerator,
    ) -> Result<ShinkaiPath, ShinkaiFsError> {
        // Use the new construct_job_file_path function
        let shinkai_path = Self::construct_job_file_path(job_id, &file_name, sqlite_manager)?;

        // Use the existing save_and_process_file function to save and process the file
        Self::save_and_process_file(shinkai_path.clone(), data, sqlite_manager, mode, generator).await?;

        Ok(shinkai_path)
    }

    /// Get the content of a file based on a ShinkaiPath.
    pub fn get_file_content(path: ShinkaiPath) -> Result<Vec<u8>, ShinkaiFsError> {
        let content = fs::read(path.as_path())
            .map_err(|_| ShinkaiFsError::FailedToReadFile(path.as_path().to_string_lossy().to_string()))?;
        Ok(content)
    }

    /// Search files based on partial text content and return matching FileInfo entries.
    /// This performs a case-insensitive search through file contents.
    pub fn search_files_by_content(
        base_path: ShinkaiPath,
        search_text: &str,
        sqlite_manager: &SqliteManager,
    ) -> Result<Vec<FileInfo>, ShinkaiFsError> {
        let mut matching_files = Vec::new();
        let base_dir = base_path.as_path();

        // Walk through the directory recursively
        for entry in walkdir::WalkDir::new(base_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                // Try to read the file content
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    // Perform case-insensitive search
                    if content.to_lowercase().contains(&search_text.to_lowercase()) {
                        // Get file metadata
                        if let Ok(metadata) = entry.metadata() {
                            // Convert the path to a relative path from the base directory
                            if let Ok(relative_path) = entry.path().strip_prefix(base_dir) {
                                let relative_path_str = relative_path.to_string_lossy().to_string();
                                
                                // Check if the file has embeddings
                                let has_embeddings = sqlite_manager
                                    .get_parsed_file_by_rel_path(&relative_path_str)
                                    .map_or(false, |pf| pf.is_some());

                                let file_info = FileInfo {
                                    path: relative_path_str,
                                    is_directory: false,
                                    created_time: metadata.created().ok(),
                                    modified_time: metadata.modified().ok(),
                                    has_embeddings,
                                    children: None,
                                    size: Some(metadata.len()),
                                    name: entry.file_name().to_string_lossy().to_string(),
                                };
                                matching_files.push(file_info);
                            }
                        }
                    }
                } else {
                    println!("Failed to read file: {:?}", entry.path());
                }
            }
        }
        Ok(matching_files)
    }

    /// Search files based on their names and return matching FileInfo entries.
    /// This performs a case-insensitive search of filenames.
    pub fn search_files_by_name(
        base_path: ShinkaiPath,
        search_text: &str,
        sqlite_manager: &SqliteManager,
    ) -> Result<Vec<FileInfo>, ShinkaiFsError> {
        let mut matching_files = Vec::new();
        let base_dir = base_path.as_path();
        let search_text_lower = search_text.to_lowercase();

        // Walk through the directory recursively
        for entry in walkdir::WalkDir::new(base_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            // Skip the root directory itself (depth 0)
            if entry.depth() == 0 {
                continue;
            }
            
            // Get the file name and check if it matches the search criteria
            let file_name = entry.file_name().to_string_lossy().to_string();
            
            // Perform case-insensitive search on the filename
            if file_name.to_lowercase().contains(&search_text_lower) {
                // Get file metadata
                if let Ok(metadata) = entry.metadata() {
                    // Convert the path to a relative path from the base directory
                    if let Ok(relative_path) = entry.path().strip_prefix(base_dir) {
                        let relative_path_str = relative_path.to_string_lossy().to_string();
                        
                        // Check if the file has embeddings (only if it's a file)
                        let has_embeddings = if !entry.file_type().is_dir() {
                            sqlite_manager
                                .get_parsed_file_by_rel_path(&relative_path_str)
                                .map_or(false, |pf| pf.is_some())
                        } else {
                            false
                        };

                        let file_info = FileInfo {
                            path: relative_path_str,
                            is_directory: entry.file_type().is_dir(),
                            created_time: metadata.created().ok(),
                            modified_time: metadata.modified().ok(),
                            has_embeddings,
                            children: None,
                            size: if entry.file_type().is_file() { 
                                Some(metadata.len()) 
                            } else { 
                                None 
                            },
                            name: file_name,
                        };
                        matching_files.push(file_info);
                    }
                }
            }
        }

        Ok(matching_files)
    }
    
    /// Search files based on both their names and content, returning combined results
    /// with duplicates removed. This performs a case-insensitive search.
    pub fn search_files_by_name_and_content(
        base_path: ShinkaiPath,
        search_text: &str,
        sqlite_manager: &SqliteManager,
    ) -> Result<Vec<FileInfo>, ShinkaiFsError> {
        // Get results from both search methods
        let name_results = Self::search_files_by_name(base_path.clone(), search_text, sqlite_manager)?;
        let content_results = Self::search_files_by_content(base_path, search_text, sqlite_manager)?;
        
        // Use a HashMap to keep track of unique paths
        let mut unique_results = std::collections::HashMap::new();
        
        // Add all name search results
        for file_info in name_results {
            unique_results.insert(file_info.path.clone(), file_info);
        }
        
        // Add content search results, not replacing existing entries
        for file_info in content_results {
            unique_results.entry(file_info.path.clone()).or_insert(file_info);
        }
        
        // Convert HashMap values into a Vec
        let combined_results = unique_results.into_values().collect();
        
        Ok(combined_results)
    }
}

// Custom serializer for SystemTime to ISO8601
fn serialize_system_time<S>(time: &Option<SystemTime>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(system_time) = time {
        let datetime: DateTime<Utc> = (*system_time).into();
        return serializer.serialize_some(&datetime.to_rfc3339());
    }
    serializer.serialize_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use shinkai_embedding::mock_generator::MockGenerator;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_message_primitives::schemas::shinkai_fs::ParsedFile;
    use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use tempfile::{tempdir, NamedTempFile};

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        eprintln!("db_path: {:?}", db_path);

        // Delete the database file if it exists
        if db_path.exists() {
            std::fs::remove_file(&db_path).unwrap_or_else(|e| {
                eprintln!("Failed to delete existing database file: {}", e);
            });
        }

        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    fn create_test_parsed_file(id: i64, relative_path: &str) -> ParsedFile {
        let pf_relative_path = SqliteManager::normalize_path(relative_path);
        ParsedFile {
            id: Some(id),
            relative_path: pf_relative_path.to_string(),
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

    // Helper function to set up a test environment
    fn setup_test_environment() -> (SqliteManager, tempfile::TempDir, ShinkaiPath, MockGenerator) {
        let db = setup_test_db();

        // Initialize the database tables
        let conn = db.get_connection().unwrap();
        SqliteManager::initialize_filesystem_tables(&conn).unwrap();

        // Create a temporary directory and file path
        let dir = tempdir().unwrap();
        let file_path = "test_file.txt".to_string();

        // Set the environment variable to the temporary directory path
        std::env::set_var("NODE_STORAGE_PATH", dir.path().to_string_lossy().to_string());

        let vr_path = ShinkaiPath::from_base_path();
        eprintln!("vr_path: {:?}", vr_path.as_path());

        // Check if the directory exists, and create it if it doesn't
        if !Path::new(&vr_path.as_path()).exists() {
            let _ = fs::create_dir_all(&vr_path.as_path()).map_err(|e| {
                eprintln!("Failed to create directory {}: {}", vr_path.as_path().display(), e);
            });
        }

        // Create a mock embedding generator
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM);
        let generator = MockGenerator::new(model_type, 384); // 128 is the number of floats in the mock embedding

        (db, dir, ShinkaiPath::from_string(file_path), generator)
    }

    fn create_new_job(db: &SqliteManager, job_id: String, agent_id: String, scope: MinimalJobScope) {
        match db.create_new_job(job_id, agent_id, scope, false, None, None) {
            Ok(_) => (),
            Err(e) => panic!("Failed to create a new job: {}", e),
        }
    }

    // Helper function to write large content to a file
    fn write_large_content(file: &mut File) {
        let large_content = [
            "This is the first part of the test file. It contains some initial text to start the file processing. ",
            "Here is the second part of the test file. It adds more content to ensure the file is large enough. ",
            "Finally, this is the third part of the test file. It completes the content needed for multiple chunks. ",
            "Additional content to ensure the file is sufficiently large for testing purposes. This should help in generating multiple chunks. ",
            "More content to further increase the size of the file. This should definitely ensure multiple chunks are created. ",
            "Even more content to make sure we exceed the threshold for chunking. This is important for testing the chunking logic. ",
            "Continuing to add content to ensure the file is large enough. This should be more than sufficient for the test. ",
            "Final addition of content to make sure we have enough text. This should cover all bases for the chunking test."
        ].join("");
        writeln!(file, "{}", large_content).unwrap();
    }

    #[test]
    #[serial]
    fn test_list_directory_contents() {
        let (db, _dir, _shinkai_path, _generator) = setup_test_environment();

        // Create a temporary directory
        let dir_path = ShinkaiPath::from_base_path();

        // Create a subdirectory and a file inside the temporary directory
        let subdir_path = ShinkaiPath::from_string("subdir".to_string());
        fs::create_dir(&subdir_path.path).unwrap();

        let file_path = ShinkaiPath::from_string("test_file.txt".to_string());
        let mut file = File::create(&file_path.path).unwrap();
        writeln!(file, "Hello, Shinkai!").unwrap();

        // Call the function to list directory contents
        let contents = ShinkaiFileManager::list_directory_contents(dir_path, &db).unwrap();

        // Check that the directory contents are correct
        assert_eq!(contents.len(), 2);

        let mut found_subdir = false;
        let mut found_file = false;

        for entry in contents {
            if entry.path == "subdir" && entry.is_directory {
                found_subdir = true;
            } else if entry.path == "test_file.txt" && !entry.is_directory {
                found_file = true;
            }
        }

        assert!(found_subdir, "Subdirectory 'subdir' should be found.");
        assert!(found_file, "File 'test_file.txt' should be found.");
    }

    #[test]
    #[serial]
    fn test_list_directory_contents_with_db_entries() {
        let (db, _dir, _shinkai_path, _generator) = setup_test_environment();

        // Initialize the database tables
        let conn = db.get_connection().unwrap();
        SqliteManager::initialize_filesystem_tables(&conn).unwrap();

        let pf1_path = ShinkaiPath::from_string("january.txt".to_string());
        let pf2_path = ShinkaiPath::from_string("february.txt".to_string());

        // Add parsed files with different relative paths
        let pf1 = create_test_parsed_file(1, &pf1_path.relative_path());
        let pf2 = create_test_parsed_file(2, &pf2_path.relative_path());
        db.add_parsed_file(&pf1).unwrap();
        db.add_parsed_file(&pf2).unwrap();

        // Create a temporary directory
        let dir_path = ShinkaiPath::from_base_path();

        // Create files in the temporary directory to match the database entries
        let mut file1 = File::create(&pf1_path.as_path()).unwrap();
        writeln!(file1, "January report").unwrap();

        let mut file2 = File::create(&pf2_path.as_path()).unwrap();
        writeln!(file2, "February report").unwrap();

        // Create a file that is not in the database
        let pf3_path = ShinkaiPath::from_string("march.txt".to_string());
        let mut file3 = File::create(&pf3_path.as_path()).unwrap();
        writeln!(file3, "March report").unwrap();

        // Create a subdirectory
        let subdir_path = ShinkaiPath::from_string("subdir".to_string());
        fs::create_dir(&subdir_path.as_path()).unwrap();

        // Call the function to list directory contents
        let contents = ShinkaiFileManager::list_directory_contents(dir_path, &db).unwrap();
        eprintln!("contents: {:?}", contents);

        // Check that the directory contents are correct
        assert_eq!(contents.len(), 4);

        let mut found_january = false;
        let mut found_february = false;
        let mut found_march = false;
        let mut found_subdir = false;

        for entry in contents {
            if entry.path == "january.txt" && !entry.is_directory {
                found_january = true;
                assert!(entry.has_embeddings, "File 'january.txt' should have embeddings.");
            } else if entry.path == "february.txt" && !entry.is_directory {
                found_february = true;
                assert!(entry.has_embeddings, "File 'february.txt' should have embeddings.");
            } else if entry.path == "march.txt" && !entry.is_directory {
                found_march = true;
                assert!(!entry.has_embeddings, "File 'march.txt' should not have embeddings.");
            } else if entry.path == "subdir" && entry.is_directory {
                found_subdir = true;
            }
        }

        assert!(found_january, "File 'january.txt' should be found.");
        assert!(found_february, "File 'february.txt' should be found.");
        assert!(found_march, "File 'march.txt' should be found.");
        assert!(found_subdir, "Directory 'subdir' should be found.");
    }

    #[tokio::test]
    #[serial]
    async fn test_process_file() {
        let (db, dir, shinkai_path, generator) = setup_test_environment();

        // Create and write to the file
        let mut file = File::create(shinkai_path.as_path()).unwrap();
        write_large_content(&mut file);

        // Call the process_embeddings_for_file function
        let result = ShinkaiFileManager::process_embeddings_for_file(
            shinkai_path.clone(),
            &db,
            FileProcessingMode::Auto,
            &generator,
        )
        .await;

        // Assert the result is Ok
        assert!(result.is_ok());

        // Verify the file is added to the database
        let parsed_file = db.get_parsed_file_by_rel_path("test_file.txt").unwrap();
        assert!(parsed_file.is_some());

        // Verify the chunks are added to the database
        let parsed_file_id = parsed_file.unwrap().id.unwrap();
        let chunks = db.get_chunks_for_parsed_file(parsed_file_id).unwrap();
        println!("chunks: {:?}", chunks); // Debugging output
        assert!(chunks.len() >= 2, "Expected at least 2 chunks, found {}", chunks.len());

        // Clean up
        dir.close().unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_save_and_process_file() {
        let (db, dir, shinkai_path, generator) = setup_test_environment();

        // Prepare the data to be written
        let mut file = File::create(shinkai_path.as_path()).unwrap();
        write_large_content(&mut file);
        let data = std::fs::read(shinkai_path.as_path()).unwrap();

        // Call the save_and_process_file function
        let result = ShinkaiFileManager::save_and_process_file(
            shinkai_path.clone(),
            data,
            &db,
            FileProcessingMode::Auto,
            &generator,
        )
        .await;

        // Assert the result is Ok
        assert!(result.is_ok());

        // Verify the file is added to the database
        let parsed_file = db.get_parsed_file_by_rel_path("test_file.txt").unwrap();
        assert!(parsed_file.is_some());

        // Verify the chunks are added to the database
        let parsed_file_id = parsed_file.unwrap().id.unwrap();
        let chunks = db.get_chunks_for_parsed_file(parsed_file_id).unwrap();
        assert!(chunks.len() >= 2, "Expected at least 2 chunks, found {}", chunks.len());

        // Clean up
        dir.close().unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_create_job_and_upload_file() {
        let (db, _dir, _shinkai_path, generator) = setup_test_environment();

        let job_id = "test_job".to_string();
        let job_inbox = "job_inbox::test_job::false".to_string();
        let agent_id = "agent_test".to_string();
        let scope = MinimalJobScope::default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        // Update the smart inbox name
        let new_inbox_name = "Updated Inbox Name";
        db.unsafe_update_smart_inbox_name(&job_inbox, new_inbox_name).unwrap();

        // Get and create the job folder
        let folder_path = db.get_and_create_job_folder(&job_id).unwrap();

        // Prepare the data to be written
        let file_name = "test_file.txt";
        let file_content = b"Hello, Shinkai!".to_vec();
        let file_path = folder_path.as_path().join(file_name);
        let file_path = ShinkaiPath::from_string(file_path.to_string_lossy().to_string());

        // Use save_and_process_file to save and process the file
        let result = ShinkaiFileManager::save_and_process_file(
            file_path.clone(),
            file_content,
            &db,
            FileProcessingMode::Auto,
            &generator,
        )
        .await;

        // Assert the result is Ok
        assert!(result.is_ok());

        // Verify the file is added to the database
        let folder_and_filename = file_path.relative_path();

        let parsed_file = db.get_parsed_file_by_rel_path(&folder_and_filename).unwrap();
        assert!(parsed_file.is_some());

        // List directory contents and check the file is listed
        let contents = ShinkaiFileManager::list_directory_contents(folder_path, &db).unwrap();
        eprintln!("conents: {:?}", contents);
        let file_names: Vec<String> = contents.iter().map(|info| info.path.clone()).collect();
        assert!(
            file_names.iter().any(|path| path.contains(&file_name)),
            "File '{}' should be listed in the directory contents.",
            file_name
        );
    }

    #[test]
    #[serial]
    fn test_get_file_content() {
        let (_db, _dir, _shinkai_path, _generator) = setup_test_environment();

        // Create a specific file path within the temporary directory
        let file_name = "test_file.txt";
        let file_path = ShinkaiPath::from_string(file_name.to_string());

        // Create and write to the file
        let mut file = File::create(file_path.as_path()).unwrap();
        writeln!(file, "Hello, Shinkai!").unwrap();

        // Call the get_file_content function
        let content = ShinkaiFileManager::get_file_content(file_path.clone());

        // Assert the content is as expected
        assert!(content.is_ok());
        assert_eq!(content.unwrap(), b"Hello, Shinkai!\n".to_vec());
    }

    #[test]
    #[serial]
    fn test_construct_job_file_path() {
        let db = setup_test_db();
        let job_id = "test_job";
        let agent_id = "agent_test";
        let scope = MinimalJobScope::default();

        // Create a new job in the database
        db.create_new_job(job_id.to_string(), agent_id.to_string(), scope, false, None, None)
            .expect("Failed to create a new job");

        // Call the construct_job_file_path function
        let file_name = "test_file.txt";
        let result = ShinkaiFileManager::construct_job_file_path(job_id, file_name, &db);
        eprintln!("result: {:?}", result);

        // Assert the result is Ok
        assert!(result.is_ok());

        // Verify the constructed path
        let shinkai_path = result.unwrap();
        let expected_folder_path = db.get_and_create_job_folder(job_id).unwrap();
        let expected_path = expected_folder_path.as_path().join(file_name);
        assert_eq!(
            shinkai_path.as_path().to_string_lossy(),
            expected_path.to_string_lossy()
        );
    }

    #[test]
    #[serial]
    fn test_list_directory_contents_with_depth() {
        let (db, _dir, _shinkai_path, _generator) = setup_test_environment();

        // Create a temporary directory structure
        let base_dir = ShinkaiPath::from_base_path();
        let level1_dir = base_dir.as_path().join("level1");
        let level2_dir = level1_dir.join("level2");
        let level3_dir = level2_dir.join("level3");

        fs::create_dir_all(&level3_dir.as_path()).unwrap();

        // Create files at different levels
        let file1_path = level1_dir.join("file1.txt");
        let file2_path = level2_dir.join("file2.txt");
        let file3_path = level3_dir.join("file3.txt");

        File::create(&file1_path.as_path()).unwrap();
        File::create(&file2_path.as_path()).unwrap();
        File::create(&file3_path.as_path()).unwrap();

        // Add parsed files with embeddings to the database
        let pf1 = create_test_parsed_file(
            1,
            &ShinkaiPath::from_string("level1/file1.txt".to_string()).relative_path(),
        );
        let pf2 = create_test_parsed_file(
            2,
            &ShinkaiPath::from_string("level1/level2/file2.txt".to_string()).relative_path(),
        );
        db.add_parsed_file(&pf1).unwrap();
        db.add_parsed_file(&pf2).unwrap();

        // Call the function to list directory contents with depth 3
        let contents = ShinkaiFileManager::list_directory_contents_with_depth(base_dir, &db, 3).unwrap();
        eprintln!("contents: {:?}", contents);

        // Check that the directory contents are correct
        assert_eq!(contents.len(), 1); // Only one top-level directory

        let level1_info = &contents[0];
        assert_eq!(level1_info.path, "level1");
        assert!(level1_info.is_directory);
        assert!(level1_info.children.is_some());

        let level2_contents = level1_info.children.as_ref().unwrap();
        assert_eq!(level2_contents.len(), 2); // One directory and one file

        let file1_path = os_path::OsPath::from("level1/file1.txt").to_string();
        let file1_info = level2_contents.iter().find(|info| info.path == file1_path).unwrap();
        assert!(!file1_info.is_directory);
        assert!(
            file1_info.has_embeddings,
            "File 'level1/file1.txt' should have embeddings."
        );

        let level2_path = os_path::OsPath::from("level1/level2").to_string();
        let level2_info = level2_contents.iter().find(|info| info.path == level2_path).unwrap();
        assert!(level2_info.is_directory);
        assert!(level2_info.children.is_some());

        let level3_contents = level2_info.children.as_ref().unwrap();
        assert_eq!(level3_contents.len(), 2); // One directory and one file

        let file2_path = os_path::OsPath::from("level1/level2/file2.txt").to_string();
        let file2_info = level3_contents.iter().find(|info| info.path == file2_path).unwrap();
        assert!(!file2_info.is_directory);
        assert!(
            file2_info.has_embeddings,
            "File 'level1/level2/file2.txt' should have embeddings."
        );

        let level3_path = os_path::OsPath::from("level1/level2/level3").to_string();
        let level3_info = level3_contents.iter().find(|info| info.path == level3_path).unwrap();
        assert!(level3_info.is_directory);
        assert!(level3_info.children.is_some());

        let level3_files = level3_info.children.as_ref().unwrap();
        assert_eq!(level3_files.len(), 1); // Only one file

        let file3_path = os_path::OsPath::from("level1/level2/level3/file3.txt").to_string();
        let file3_info = level3_files.iter().find(|info| info.path == file3_path).unwrap();
        assert!(!file3_info.is_directory);
        assert!(
            !file3_info.has_embeddings,
            "File 'level1/level2/level3/file3.txt' should not have embeddings."
        );
    }

    #[test]
    #[serial]
    fn test_search_files_by_content() {
        let (db, _dir, _shinkai_path, _generator) = setup_test_environment();
        let base_path = ShinkaiPath::from_base_path();

        // Create test directory structure and files with content
        let test_files = vec![
            ("docs/reports/2024/january.txt", "January 2024 monthly report", 1),
            ("docs/reports/2024/february.txt", "February 2024 quarterly update", 2),
            ("docs/other/2024/march.txt", "March 2024 meeting notes", 3),
            ("projects/report-2024.md", "2024 Project Status Report", 4),
            ("misc/notes.txt", "Random notes from 2024 meetings", 5),
        ];

        // Create directories and files
        for (path, content, id) in &test_files {
            // Create directory structure
            if let Some(parent) = Path::new(path).parent() {
                fs::create_dir_all(base_path.as_path().join(parent)).unwrap();
            }

            // Create and write to file
            let mut file = File::create(base_path.as_path().join(path)).unwrap();
            writeln!(file, "{}", content).unwrap();

            // Add file to database with embeddings
            let pf = create_test_parsed_file(*id, path);
            db.add_parsed_file(&pf).unwrap();
        }

        // Test exact content match
        let results = ShinkaiFileManager::search_files_by_content(base_path.clone(), "January 2024", &db).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "docs/reports/2024/january.txt");
        assert!(results[0].has_embeddings);

        // Test partial content match
        let results = ShinkaiFileManager::search_files_by_content(base_path.clone(), "2024", &db).unwrap();
        assert_eq!(results.len(), 5); // Should find all files as they all contain "2024"

        // Test case insensitive match
        let results = ShinkaiFileManager::search_files_by_content(base_path.clone(), "REPORT", &db).unwrap();
        assert_eq!(results.len(), 2); // Should find files containing "report" regardless of case

        // Test content spanning multiple files
        let results = ShinkaiFileManager::search_files_by_content(base_path.clone(), "meeting", &db).unwrap();
        assert_eq!(results.len(), 2); // Should find both files containing "meeting"

        // Test no matches
        let results = ShinkaiFileManager::search_files_by_content(base_path.clone(), "nonexistent", &db).unwrap();
        assert_eq!(results.len(), 0);

        // Verify file metadata
        let results = ShinkaiFileManager::search_files_by_content(base_path.clone(), "2024", &db).unwrap();
        for file_info in results {
            assert!(!file_info.is_directory);
            assert!(file_info.size.is_some());
            assert!(file_info.has_embeddings);
            assert!(file_info.created_time.is_some());
            assert!(file_info.modified_time.is_some());
        }
    }

    #[test]
    #[serial]
    fn test_search_files_by_name() {
        let (db, _dir, _shinkai_path, _generator) = setup_test_environment();
        let base_path = ShinkaiPath::from_base_path();

        // Create test directory structure and files with content
        let test_files = vec![
            ("docs/reports/2024/january.txt", "January 2024 monthly report", 1),
            ("docs/reports/2024/february.txt", "February 2024 quarterly update", 2),
            ("docs/other/2024/march.txt", "March 2024 meeting notes", 3),
            ("projects/report-2024.md", "2024 Project Status Report", 4),
            ("misc/notes.txt", "Random notes from 2024 meetings", 5),
        ];

        // Create directories and files
        for (path, content, id) in &test_files {
            // Create directory structure
            if let Some(parent) = Path::new(path).parent() {
                fs::create_dir_all(base_path.as_path().join(parent)).unwrap();
            }

            // Create and write to file
            let mut file = File::create(base_path.as_path().join(path)).unwrap();
            writeln!(file, "{}", content).unwrap();

            // Add file to database with embeddings
            let pf = create_test_parsed_file(*id, path);
            db.add_parsed_file(&pf).unwrap();
        }

        // Create a directory that should match name searches
        let report_dir_path = base_path.as_path().join("reports-2024");
        fs::create_dir(&report_dir_path).unwrap();

        // Test exact name match
        let results = ShinkaiFileManager::search_files_by_name(base_path.clone(), "january.txt", &db).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "docs/reports/2024/january.txt");
        assert!(results[0].has_embeddings);

        // Test partial name match
        let results = ShinkaiFileManager::search_files_by_name(base_path.clone(), "2024", &db).unwrap();
        assert_eq!(results.len(), 4); // Should find all files whose name contains "2024"

        // Test case insensitive match
        let results = ShinkaiFileManager::search_files_by_name(base_path.clone(), "REPORT", &db).unwrap();
        assert_eq!(results.len(), 3); // Should find files containing "report" regardless of case

        // Test name spanning multiple files
        let results = ShinkaiFileManager::search_files_by_name(base_path.clone(), "meeting", &db).unwrap();
        assert_eq!(results.len(), 0); // Should not find any files containing "meeting" because is searching now only by name

        // Test no matches
        let results = ShinkaiFileManager::search_files_by_name(base_path.clone(), "nonexistent", &db).unwrap();
        assert_eq!(results.len(), 0);

        // Verify file and directory metadata
        let results = ShinkaiFileManager::search_files_by_name(base_path.clone(), "2024", &db).unwrap();
        for file_info in results {
            // Check appropriate properties based on whether it's a file or directory
            if file_info.is_directory {
                assert!(file_info.size.is_none(), "Directories should have no size");
                assert!(!file_info.has_embeddings, "Directories should not have embeddings");
            } else {
                assert!(file_info.size.is_some(), "Files should have size");
                // Only check for embeddings on files we explicitly added to the database
                let has_expected_embeddings = test_files.iter().any(|(path, _, _)| file_info.path == *path);
                if has_expected_embeddings {
                    assert!(file_info.has_embeddings, "Expected file should have embeddings");
                }
            }
            assert!(file_info.created_time.is_some(), "All entries should have created_time");
            assert!(file_info.modified_time.is_some(), "All entries should have modified_time");
        }
    }

    #[test]
    #[serial]
    fn test_search_files_by_name_and_content() {
        let (db, _dir, _shinkai_path, _generator) = setup_test_environment();
        let base_path = ShinkaiPath::from_base_path();

        // Create test directory structure and files with content
        let test_files = vec![
            ("docs/reports/2024/january.txt", "January 2024 monthly report", 1),
            ("docs/reports/2024/february.txt", "February 2024 quarterly update", 2),
            ("docs/other/2024/march.txt", "March 2024 meeting notes", 3),
            ("projects/report-2024.md", "2024 Project Status Report", 4),
            ("misc/notes.txt", "Random notes from 2024 meetings", 5),
        ];

        // Create directories and files
        for (path, content, id) in &test_files {
            // Create directory structure
            if let Some(parent) = Path::new(path).parent() {
                fs::create_dir_all(base_path.as_path().join(parent)).unwrap();
            }

            // Create and write to file
            let mut file = File::create(base_path.as_path().join(path)).unwrap();
            writeln!(file, "{}", content).unwrap();

            // Add file to database with embeddings
            let pf = create_test_parsed_file(*id, path);
            db.add_parsed_file(&pf).unwrap();
        }

        // Create a directory that should match name searches
        let report_dir_path = base_path.as_path().join("reports-2024");
        fs::create_dir(&report_dir_path).unwrap();

        // Create a file that matches by content but not name
        let content_match_path = "docs/content-only.txt";
        if let Some(parent) = Path::new(content_match_path).parent() {
            fs::create_dir_all(base_path.as_path().join(parent)).unwrap();
        }
        let mut file = File::create(base_path.as_path().join(content_match_path)).unwrap();
        writeln!(file, "This file contains meeting notes but no matching filename").unwrap();
        let pf = create_test_parsed_file(6, content_match_path);
        db.add_parsed_file(&pf).unwrap();

        // Test simple by-name match 
        let name_results = ShinkaiFileManager::search_files_by_name(base_path.clone(), "january.txt", &db).unwrap();
        assert_eq!(name_results.len(), 1);
        assert_eq!(name_results[0].path, "docs/reports/2024/january.txt");

        // Test simple by-content match
        let content_results = ShinkaiFileManager::search_files_by_content(base_path.clone(), "monthly report", &db).unwrap();
        assert_eq!(content_results.len(), 1);
        assert_eq!(content_results[0].path, "docs/reports/2024/january.txt");

        // Test combined search - exact match by name
        let combined_results = ShinkaiFileManager::search_files_by_name_and_content(
            base_path.clone(), "january.txt", &db
        ).unwrap();
        assert_eq!(combined_results.len(), 1);
        assert_eq!(combined_results[0].path, "docs/reports/2024/january.txt");

        // Test combined search - exact match by content
        let combined_results = ShinkaiFileManager::search_files_by_name_and_content(
            base_path.clone(), "quarterly update", &db
        ).unwrap();
        assert_eq!(combined_results.len(), 1);
        assert_eq!(combined_results[0].path, "docs/reports/2024/february.txt");

        // Test combined search - matches both name and content
        let combined_results = ShinkaiFileManager::search_files_by_name_and_content(
            base_path.clone(), "2024", &db
        ).unwrap();
        
        // Should find:
        // - 4 files with "2024" in their name
        // - The "reports-2024" directory  
        // - All 5 files containing "2024" in their content
        // With duplicates removed, so counting unique paths
        assert!(combined_results.len() >= 6, 
            "Expected at least 6 results (4 name-matching files + directory + content-only match), found {}",
            combined_results.len());

        // Test content-only match
        let combined_results = ShinkaiFileManager::search_files_by_name_and_content(
            base_path.clone(), "meeting", &db
        ).unwrap();
        
        // Should find:
        // - Zero files with "meeting" in name 
        // - At least 2 files containing "meeting" in content
        assert!(combined_results.len() >= 2, 
            "Expected at least 2 results for content-only search, found {}", 
            combined_results.len());
        
        // Make sure we find the content-only file
        let content_only_match = combined_results.iter()
            .find(|info| info.path == content_match_path);
        assert!(content_only_match.is_some(), "Should find the content-only match file");

        // Verify file and directory metadata
        for file_info in combined_results {
            // Check appropriate properties based on whether it's a file or directory
            if file_info.is_directory {
                assert!(file_info.size.is_none(), "Directories should have no size");
                assert!(!file_info.has_embeddings, "Directories should not have embeddings");
            } else {
                assert!(file_info.size.is_some(), "Files should have size");
                // Only check for embeddings on files we explicitly added to the database
                let has_expected_embeddings = test_files.iter().any(|(path, _, _)| file_info.path == *path) ||
                                               file_info.path == content_match_path;
                if has_expected_embeddings {
                    assert!(file_info.has_embeddings, "Expected file should have embeddings");
                }
            }
            assert!(file_info.created_time.is_some(), "All entries should have created_time");
            assert!(file_info.modified_time.is_some(), "All entries should have modified_time");
        }
    }
}
