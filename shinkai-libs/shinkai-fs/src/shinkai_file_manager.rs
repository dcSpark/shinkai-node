use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::Serializer;
use serde::{Deserialize, Serialize};
use shinkai_embedding::embedding_generator::EmbeddingGenerator;
use shinkai_message_primitives::schemas::shinkai_fs::{ParsedFile, ShinkaiFileChunk};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;
use utoipa::ToSchema;

use crate::shinkai_fs_error::ShinkaiFsError;
use crate::simple_parser::simple_parser::SimpleParser;

pub struct ShinkaiFileManager;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, ToSchema)]
pub struct FileInfo {
    pub name: String,
    pub is_directory: bool,
    #[serde(serialize_with = "serialize_system_time")]
    pub created_time: Option<SystemTime>,
    #[serde(serialize_with = "serialize_system_time")]
    pub modified_time: Option<SystemTime>,
    pub has_embeddings: bool,
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
        Self::add_file(dest_path.clone(), data)?;

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
        eprintln!("rel_path: {:?}", rel_path);

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
            tags: None,             // TODO: connect this
            total_tokens: None,     // TODO: connect this
            total_characters: None, // TODO: connect this
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

    /// List all files and folders in a directory with additional metadata.
    pub fn list_directory_contents(
        path: ShinkaiPath,
        sqlite_manager: &SqliteManager,
    ) -> Result<Vec<FileInfo>, ShinkaiFsError> {
        let mut contents = Vec::new();
        let mut file_map = HashMap::new();

        // Read directory contents and store in a hash map
        for entry in fs::read_dir(path.as_path())? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let file_name = entry.file_name().into_string().unwrap_or_default();
            file_map.insert(file_name.clone(), metadata.is_dir());

            let file_info = FileInfo {
                name: file_name,
                is_directory: metadata.is_dir(),
                created_time: metadata.created().ok(),
                modified_time: metadata.modified().ok(),
                has_embeddings: false, // Default to false, will update if found in DB
            };
            contents.push(file_info);
        }

        // Use the relative path for querying the database
        let rel_path = path.relative_path();
        let files_with_embeddings = sqlite_manager.get_processed_files_in_directory(&rel_path)?;

        // Create a hash map for files with embeddings
        let embeddings_map: std::collections::HashSet<_> = files_with_embeddings
            .into_iter()
            .map(|file| file.relative_path)
            .collect();

        // Update the contents with embedding information
        for file_info in &mut contents {
            if embeddings_map.contains(&file_info.name) {
                file_info.has_embeddings = true;
            }
        }

        Ok(contents)
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
    use shinkai_embedding::mock_generator::MockGenerator;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_message_primitives::schemas::shinkai_fs::ParsedFile;
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
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);
        let generator = MockGenerator::new(model_type, 384); // 128 is the number of floats in the mock embedding

        (db, dir, ShinkaiPath::from_string(file_path), generator)
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
            if entry.name == "subdir" && entry.is_directory {
                found_subdir = true;
            } else if entry.name == "test_file.txt" && !entry.is_directory {
                found_file = true;
            }
        }

        assert!(found_subdir, "Subdirectory 'subdir' should be found.");
        assert!(found_file, "File 'test_file.txt' should be found.");
    }

    #[test]
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

        // Check that the directory contents are correct
        assert_eq!(contents.len(), 4);

        let mut found_january = false;
        let mut found_february = false;
        let mut found_march = false;
        let mut found_subdir = false;

        for entry in contents {
            if entry.name == "january.txt" && !entry.is_directory {
                found_january = true;
                assert!(entry.has_embeddings, "File 'january.txt' should have embeddings.");
            } else if entry.name == "february.txt" && !entry.is_directory {
                found_february = true;
                assert!(entry.has_embeddings, "File 'february.txt' should have embeddings.");
            } else if entry.name == "march.txt" && !entry.is_directory {
                found_march = true;
                assert!(!entry.has_embeddings, "File 'march.txt' should not have embeddings.");
            } else if entry.name == "subdir" && entry.is_directory {
                found_subdir = true;
            }
        }

        assert!(found_january, "File 'january.txt' should be found.");
        assert!(found_february, "File 'february.txt' should be found.");
        assert!(found_march, "File 'march.txt' should be found.");
        assert!(found_subdir, "Directory 'subdir' should be found.");
    }

    #[tokio::test]
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
        println!("chunks: {:?}", chunks); // Debugging output
        assert!(chunks.len() >= 2, "Expected at least 2 chunks, found {}", chunks.len());

        // Clean up
        dir.close().unwrap();
    }
}
