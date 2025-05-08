use crate::llm_provider::job_manager::JobManager;
use shinkai_embedding::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;
use shinkai_message_primitives::schemas::shinkai_fs::{ShinkaiFileChunk, ShinkaiFileChunkCollection};
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_message_primitives::shinkai_utils::search_mode::VectorSearchMode;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;
use std::boxed::Box;
use std::collections::HashMap;
use std::collections::HashSet;
use std::result::Result::Ok;
use std::sync::Arc;

impl JobManager {
    /// Helper function to process folders and collect file information
    async fn process_folder_contents(
        folder: &ShinkaiPath,
        sqlite_manager: &SqliteManager,
        parsed_file_ids: &mut Vec<i64>,
        paths_map: &mut HashMap<i64, ShinkaiPath>,
        total_tokens: &mut i64,
        all_files_have_token_count: &mut bool,
    ) -> Result<(), SqliteManagerError> {
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
                let file_path = ShinkaiPath::from_string(file_info.path);
                if let Some(parsed_file) = sqlite_manager.get_parsed_file_by_shinkai_path(&file_path).unwrap() {
                    let file_id = parsed_file.id.unwrap();
                    parsed_file_ids.push(file_id);
                    paths_map.insert(file_id, file_path);

                    // Count tokens while we're here
                    if let Some(file_tokens) = parsed_file.total_tokens {
                        *total_tokens += file_tokens;
                    } else {
                        *all_files_have_token_count = false;
                    }
                }
            } else if file_info.is_directory {
                let sub_folder_path = ShinkaiPath::from_string(file_info.path);
                Box::pin(Self::process_folder_contents(
                    &sub_folder_path,
                    sqlite_manager,
                    parsed_file_ids,
                    paths_map,
                    total_tokens,
                    all_files_have_token_count,
                ))
                .await?;
            }
        }
        Ok(())
    }

    /// Searches all resources in the given job scope and returns the search results.
    pub async fn search_for_chunks_in_resources(
        fs_files_paths: Vec<ShinkaiPath>,
        fs_folder_paths: Vec<ShinkaiPath>,
        job_filenames: Vec<String>,
        job_id: String,
        scope: &MinimalJobScope,
        sqlite_manager: Arc<SqliteManager>,
        query_text: String,
        num_of_top_results: usize,
        max_tokens_in_prompt: usize,
        embedding_generator: RemoteEmbeddingGenerator,
    ) -> Result<ShinkaiFileChunkCollection, SqliteManagerError> {
        let mut parsed_file_ids = Vec::new();
        let mut paths_map = HashMap::new();
        let mut total_tokens: i64 = 0;
        let mut all_files_have_token_count = true;

        let query_embedding = match embedding_generator.generate_embedding_default(&query_text).await {
            Ok(embedding) => embedding,
            Err(e) => {
                return Err(SqliteManagerError::SomeError(e.to_string()));
            }
        };

        // Process fs_files_paths
        for path in &fs_files_paths {
            if let Some(parsed_file) = sqlite_manager.get_parsed_file_by_shinkai_path(path).unwrap() {
                let file_id = parsed_file.id.unwrap();
                parsed_file_ids.push(file_id);
                paths_map.insert(file_id, path.clone());

                if let Some(file_tokens) = parsed_file.total_tokens {
                    total_tokens += file_tokens;
                } else {
                    all_files_have_token_count = false;
                }
            }
        }

        // Process job_filenames
        for filename in &job_filenames {
            let file_path = match ShinkaiFileManager::construct_job_file_path(&job_id, filename, &sqlite_manager) {
                Ok(path) => path,
                Err(_) => continue,
            };

            if let Some(parsed_file) = sqlite_manager.get_parsed_file_by_shinkai_path(&file_path).unwrap() {
                let file_id = parsed_file.id.unwrap();
                parsed_file_ids.push(file_id);
                paths_map.insert(file_id, file_path);

                if let Some(file_tokens) = parsed_file.total_tokens {
                    total_tokens += file_tokens;
                } else {
                    all_files_have_token_count = false;
                }
            }
        }

        // Retrieve each file in the job scope
        for path in &scope.vector_fs_items {
            if let Some(parsed_file) = sqlite_manager.get_parsed_file_by_shinkai_path(path).unwrap() {
                let file_id = parsed_file.id.unwrap();
                parsed_file_ids.push(file_id);
                paths_map.insert(file_id, path.clone());

                // Count tokens while we're here
                if let Some(file_tokens) = parsed_file.total_tokens {
                    total_tokens += file_tokens;
                } else {
                    all_files_have_token_count = false;
                }
            }
        }

        // Process fs_folder_paths
        for folder in &fs_folder_paths {
            Self::process_folder_contents(
                folder,
                &sqlite_manager,
                &mut parsed_file_ids,
                &mut paths_map,
                &mut total_tokens,
                &mut all_files_have_token_count,
            )
            .await?;
        }

        // Retrieve files inside vector_fs_folders
        for folder in &scope.vector_fs_folders {
            Self::process_folder_contents(
                folder,
                &sqlite_manager,
                &mut parsed_file_ids,
                &mut paths_map,
                &mut total_tokens,
                &mut all_files_have_token_count,
            )
            .await?;
        }

        // Determine the vector search mode configured in the job scope.
        let max_tokens_in_prompt =
            if scope.vector_search_mode == VectorSearchMode::FillUpTo25k && max_tokens_in_prompt > 25000 {
                25000
            } else {
                max_tokens_in_prompt
            };

        // If we have token counts for all files and they fit within the limit,
        // we can include all chunks from all files
        if all_files_have_token_count && total_tokens <= max_tokens_in_prompt as i64 {
            let mut all_chunks = Vec::new();
            for file_id in parsed_file_ids {
                let file_chunks = sqlite_manager.get_chunks_for_parsed_file(file_id)?;
                all_chunks.extend(file_chunks);
            }

            return Ok(ShinkaiFileChunkCollection {
                chunks: all_chunks,
                paths: Some(paths_map),
            });
        }

        // Perform a vector search on all parsed files
        let search_results = sqlite_manager.search_chunks(&parsed_file_ids, query_embedding, num_of_top_results)?;

        // If there are no initial results, just return early
        if search_results.is_empty() {
            eprintln!("No initial results found for search");
            return Ok(ShinkaiFileChunkCollection {
                chunks: vec![],
                paths: Some(paths_map),
            });
        }

        // Count the total number of characters in the search results using map-reduce
        let total_characters: usize = search_results
            .iter()
            .map(|(chunk, _distance)| chunk.content.len())
            .sum();

        // Calculate the average chunk size
        let average_chunk_size = total_characters / search_results.len();

        // Calculate the total amount of extra chunks we need to fetch to fill up to max_tokens_in_prompt
        let extra_chunks_needed = (max_tokens_in_prompt - total_characters) / average_chunk_size;

        // Distribute the extra chunks across the search results
        let total_results = search_results.len();
        let chunk_needed_per_result = extra_chunks_needed / total_results;
        let remainder = extra_chunks_needed % total_results;

        // Use a HashSet to avoid duplicate chunks
        let mut expanded_results_set = HashSet::new();
        let mut total_characters = 0;

        // Expand results to fill the context window
        for (i, (chunk, _distance)) in search_results.into_iter().enumerate() {
            // Always include the chunk itself
            let chunk_length = chunk.content.len();
            total_characters += chunk_length;
            expanded_results_set.insert(chunk.clone());

            // Determine how many neighbors to fetch for this chunk
            let this_result_window_size = chunk_needed_per_result + if i < remainder { 1 } else { 0 };

            // Now use that as the "proximity window" for this particular chunk
            let mut neighbors =
                sqlite_manager.get_neighboring_chunks(chunk.parsed_file_id, chunk.position, this_result_window_size)?;

            // Insert neighbors one at a time until we hit max_tokens_in_prompt
            while total_characters < max_tokens_in_prompt {
                if let Some(neighbor) = neighbors.pop() {
                    total_characters += neighbor.content.len();
                    expanded_results_set.insert(neighbor);
                } else {
                    // No more neighbors for this chunk
                    break;
                }
            }

            // If we've filled the window, no need to keep going
            if total_characters >= max_tokens_in_prompt {
                break;
            }
        }

        // Convert HashSet to Vec
        let expanded_results: Vec<ShinkaiFileChunk> = expanded_results_set.into_iter().collect();

        Ok(ShinkaiFileChunkCollection {
            chunks: expanded_results,
            paths: Some(paths_map),
        })
    }
}
