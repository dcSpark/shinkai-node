use std::fs;
use std::path::Path;
use std::time::SystemTime;
use std::collections::HashMap;

use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::SqliteManager;

use shinkai_message_primitives::schemas::shinkai_fs::ParsedFile;

use crate::shinkai_fs_error::ShinkaiFsError;
use crate::file_parser::ShinkaiFileParser;
use crate::embedding_generator::EmbeddingGenerator;

pub struct ShinkaiFileManager;

#[derive(Debug)]
pub struct FileInfo {
    pub name: String,
    pub is_directory: bool,
    pub created_time: Option<SystemTime>,
    pub modified_time: Option<SystemTime>,
    pub has_embeddings: bool,
}

pub enum FileProcessingMode {
    Auto,
    NoParsing,
    MustParse,
}

impl ShinkaiFileManager {
    /// Process file: If not in DB, add it. If supported, generate chunks.
    /// If already processed, consider checking if file changed (not implemented here).
    pub async fn process_file(
        path: ShinkaiPath,
        base_dir: &Path,
        sqlite_manager: &SqliteManager,
        mode: FileProcessingMode,
        generator: &dyn EmbeddingGenerator,
    ) -> Result<(), ShinkaiFsError> {
        let rel_path = Self::compute_relative_path(&path, base_dir)?;
        let parsed_file = if let Some(pf) = sqlite_manager.get_parsed_file_by_rel_path(&rel_path)? {
            pf
        } else {
            let original_extension = path
                .as_path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|s| s.to_string());

            let pf = ParsedFile {
                id: 0,
                relative_path: rel_path.clone(),
                original_extension,
                description: None,
                source: None,
                embedding_model_used: None,
                keywords: None,
                distribution_info: None,
                created_time: Some(Self::current_timestamp()),
                tags: None,
                total_tokens: None,
                total_characters: None,
            };
            sqlite_manager.add_parsed_file(&pf)?;
            sqlite_manager.get_parsed_file_by_rel_path(&rel_path)?.unwrap()
        };

        match mode {
            FileProcessingMode::Auto => {
                // Implement logic for Auto mode
                let file_buffer = fs::read(path.as_path())?;
                let text_groups = ShinkaiFileParser::process_file_into_text_groups(
                    file_buffer,
                    rel_path.clone(),
                    1024, // Example max_node_text_size
                    VRSourceReference::from_file(&rel_path, TextChunkingStrategy::V1)?,
                ).await?;
                // Further processing...
            }
            FileProcessingMode::NoParsing => {
                // NoParsing mode: Skip parsing logic
                // You might still want to update metadata or perform other tasks
            }
            FileProcessingMode::MustParse => {
                // Implement logic for MustParse mode
            }
        }

        // TODO: Implement embedding checking with sqlite_manager

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
        let rel_path = Self::compute_relative_path(&path, path.as_path())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::fs::{self, File};
    use std::io::Write;
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
            id,
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
    fn test_list_directory_contents() {
        let db = setup_test_db();

        // Create a temporary directory
        let dir = tempdir().unwrap();
        let dir_path = ShinkaiPath::from_string(dir.path().to_string_lossy().to_string());

        // Create a subdirectory and a file inside the temporary directory
        let subdir_path = dir.path().join("subdir");
        fs::create_dir(&subdir_path).unwrap();

        let file_path = dir.path().join("test_file.txt");
        let mut file = File::create(&file_path).unwrap();
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
        let db = setup_test_db();

        // Initialize the database tables
        let conn = db.get_connection().unwrap();
        SqliteManager::initialize_filesystem_tables(&conn).unwrap();

        // Add parsed files with different relative paths
        let pf1 = create_test_parsed_file(1, "january.txt");
        let pf2 = create_test_parsed_file(2, "february.txt");
        db.add_parsed_file(&pf1).unwrap();
        db.add_parsed_file(&pf2).unwrap();

        // Create a temporary directory
        let dir = tempdir().unwrap();
        let dir_path = ShinkaiPath::from_string(dir.path().to_string_lossy().to_string());

        // Create files in the temporary directory to match the database entries
        let file_path1 = dir.path().join("january.txt");
        let mut file1 = File::create(&file_path1).unwrap();
        writeln!(file1, "January report").unwrap();

        let file_path2 = dir.path().join("february.txt");
        let mut file2 = File::create(&file_path2).unwrap();
        writeln!(file2, "February report").unwrap();

        // Create a file that is not in the database
        let file_path3 = dir.path().join("march.txt");
        let mut file3 = File::create(&file_path3).unwrap();
        writeln!(file3, "March report").unwrap();

        // Create a subdirectory
        let subdir_path = dir.path().join("subdir");
        fs::create_dir(&subdir_path).unwrap();

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
}
