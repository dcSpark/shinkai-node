use crate::llm_provider::job_manager::JobManager;
use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;
use shinkai_message_primitives::schemas::shinkai_fs::ShinkaiFileChunkCollection;
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;
use std::result::Result::Ok;
use std::collections::HashMap;

impl JobManager {
    /// Retrieves all resources in the given job scope and returns them as a vector of ShinkaiFileChunkCollection.
    pub async fn retrieve_all_resources_in_job_scope(
        scope: &MinimalJobScope,
        sqlite_manager: &SqliteManager,
    ) -> Result<Vec<ShinkaiFileChunkCollection>, SqliteManagerError> {
        let mut collections = Vec::new();

        // Retrieve each file in the job scope
        for path in &scope.vector_fs_items {
            if let Some(collection) = JobManager::retrieve_file_chunks(path, sqlite_manager).await? {
                collections.push(collection);
            }
        }

        // Retrieve files inside vector_fs_folders
        for folder in &scope.vector_fs_folders {
            let files = match ShinkaiFileManager::list_directory_contents(folder.clone(), sqlite_manager) {
                Ok(files) => files,
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Error,
                        &format!("Error listing directory contents: {:?}", e),
                    );
                    return Err(SqliteManagerError::SomeError(format!("ShinkaiFsError: {:?}", e)));
                }
            };

            for file_info in files {
                if !file_info.is_directory && file_info.has_embeddings {
                    let file_path = ShinkaiPath::from_string(file_info.name);
                    if let Some(collection) = JobManager::retrieve_file_chunks(&file_path, sqlite_manager).await? {
                        collections.push(collection);
                    }
                }
            }
        }

        Ok(collections)
    }

    /// Static function to retrieve file chunks for a given path.
    pub async fn retrieve_file_chunks(
        path: &ShinkaiPath,
        sqlite_manager: &SqliteManager,
    ) -> Result<Option<ShinkaiFileChunkCollection>, SqliteManagerError> {
        match sqlite_manager.get_parsed_file_by_shinkai_path(path) {
            Ok(Some(parsed_file)) if parsed_file.embedding_model_used.is_some() => {
                let chunks = sqlite_manager.get_chunks_for_parsed_file(parsed_file.id.unwrap())?;
                let mut paths_map = HashMap::new();
                paths_map.insert(parsed_file.id.unwrap(), path.clone());
                Ok(Some(ShinkaiFileChunkCollection { chunks, paths: Some(paths_map) }))
            }
            Ok(Some(_)) => {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Info,
                    &format!("File has no embeddings: {}", path),
                );
                Ok(None)
            }
            Ok(None) => {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    &format!("File not found in database: {}", path),
                );
                Ok(None)
            }
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    &format!("Error retrieving file from database: {} with error: {:?}", path, e),
                );
                Err(e)
            }
        }
    }
}

// TODO: implement tests under a cfg. 