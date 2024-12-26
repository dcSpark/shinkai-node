use crate::llm_provider::job_manager::JobManager;
use keyphrases::KeyPhraseExtractor;
use shinkai_embedding::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;
use shinkai_message_primitives::schemas::shinkai_fs::{ShinkaiFileChunk, ShinkaiFileChunkCollection};
use shinkai_message_primitives::shinkai_utils::job_scope::MinimalJobScope;
use shinkai_message_primitives::shinkai_utils::search_mode::VectorSearchMode;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;
use std::collections::HashMap;
use std::collections::HashSet;
use std::result::Result::Ok;
use std::sync::Arc;

impl JobManager {
    // /// Performs multiple proximity vector searches within the job scope based on extracting keywords from the query text.
    // /// Attempts to take at least 1 proximity group per keyword that is from a VR different than the highest scored node, to encourage wider diversity in results.
    // /// Returns the search results and the description/summary text of the VR the highest scored retrieved node is from.
    // #[allow(clippy::too_many_arguments)]
    // pub async fn keyword_chained_job_scope_vector_search(
    //     db: Arc<SqliteManager>,
    //     job_scope: &MinimalJobScope,
    //     query_text: String,
    //     user_profile: &ShinkaiName,
    //     generator: RemoteEmbeddingGenerator,
    //     num_of_top_results: u64,
    //     max_tokens_in_prompt: usize,
    // ) -> Result<(Vec<ShinkaiFileChunkCollection>, Option<String>), SqliteManagerError> {
    //     let mut master_intro_hashmap: HashMap<String, Vec<ShinkaiFileChunkCollection>> = HashMap::new();
    //     // First perform a standard job scope vector search using the whole query text
    //     let query = generator.generate_embedding_default(&query_text).await?;
    //     let (mut ret_groups, intro_hashmap) = JobManager::internal_job_scope_vector_search_groups(
    //         db.clone(),
    //         job_scope,
    //         query,
    //         query_text.clone(),
    //         num_of_top_results,
    //         user_profile,
    //         true,
    //         generator.clone(),
    //         max_tokens_in_prompt,
    //     )
    //     .await?;
    //     // Insert the contents of intro_hashmap into master_intro_hashmap
    //     for (key, value) in intro_hashmap {
    //         master_intro_hashmap.entry(key).or_insert(value);
    //     }

    //     // Initialize included_vrs vector with the resource header id of the first node from each ret_node_group, if it exists
    //     let mut included_vrs: Vec<String> = Vec::new();
    //     for ret_node_group in ret_groups.iter() {
    //         if let Some(first_node) = ret_node_group.first() {
    //             included_vrs.push(first_node.resource_header.reference_string());
    //         }
    //     }

    //     // Extract top 3 keywords from the query_text
    //     let keywords = Self::extract_keywords_from_text(&query_text, 0);

    //     // Now we proceed to keyword search chaining logic.
    //     for keyword in keywords {
    //         let keyword_query = generator.generate_embedding_default(&keyword).await?;
    //         let (keyword_ret_nodes_groups, keyword_intro_hashmap) =
    //             JobManager::internal_job_scope_vector_search_groups(
    //                 db.clone(),
    //                 job_scope,
    //                 keyword_query,
    //                 keyword.clone(),
    //                 num_of_top_results,
    //                 user_profile,
    //                 true,
    //                 generator.clone(),
    //                 max_tokens_in_prompt,
    //             )
    //             .await?;

    //         // Insert the contents into master_intro_hashmap
    //         for (key, value) in keyword_intro_hashmap {
    //             master_intro_hashmap.entry(key).or_insert(value);
    //         }

    //         // Start looping through the vector search results for this keyword
    //         let mut keyword_node_inserted = false;
    //         for group in keyword_ret_nodes_groups.iter() {
    //             if let Some(first_group_node) = group.first() {
    //                 if !ret_groups.iter().any(|ret_group| group == ret_group) {
    //                     // If the node is unique and we haven't inserted any keyword node yet
    //                     if !keyword_node_inserted {
    //                         // Insert the first node that is not in ret_nodes
    //                         if ret_groups.len() >= 3 {
    //                             ret_groups.insert(3, group.clone()); // Insert at the 3rd position
    //                         } else {
    //                             ret_groups.push(group.clone()); // If less than 3 nodes, just append
    //                         }
    //                         keyword_node_inserted = true;

    //                         // Check if this keyword node is from a unique VR
    //                         let from_unique_vr =
    //                             !included_vrs.contains(&first_group_node.resource_header.reference_string());
    //                         // Update the included_vrs
    //                         included_vrs.push(first_group_node.resource_header.reference_string());

    //                         // If the first unique node is from a unique VR, no need to continue going through rest of the keyword_nodes
    //                         if from_unique_vr {
    //                             break;
    //                         }
    //                     } else {
    //                         // If we're attempting to insert a unique VR node and found one
    //                         if ret_groups.len() >= 4 {
    //                             ret_groups.insert(4, group.clone()); // Insert at the 4th position if possible
    //                         } else {
    //                             ret_groups.push(group.clone()); // Otherwise, just append
    //                         }
    //                         // Once a unique VR node is inserted, no need to continue for this keyword
    //                         break;
    //                     }
    //                 }
    //             }
    //         }
    //     }

    //     // For the top N groups, fetch their VRs' intros and include them at the front of the list
    //     // We do this by iterating in reverse order (ex. 5th, 4th, 3rd, 2nd, 1st), so highest scored VR intro will be at the top.
    //     let num_groups = Self::determine_num_groups_for_intro_fetch(max_tokens_in_prompt);
    //     let mut final_nodes = Vec::new();
    //     let mut added_intros = HashMap::new();
    //     let mut first_intro_text = None;

    //     for group in ret_groups.iter().take(num_groups).rev() {
    //         // Take the first 5 groups and reverse the order
    //         if let Some(first_node) = group.first() {
    //             // Take the first node of the group
    //             if let Some(intro_text_nodes) = master_intro_hashmap.get(&first_node.resource_header.reference_string())
    //             {
    //                 if !added_intros.contains_key(&first_node.resource_header.reference_string()) {
    //                     // Add the intro nodes, and the ref string to added_intros
    //                     for intro_node in intro_text_nodes.iter() {
    //                         final_nodes.push(intro_node.clone());
    //                         added_intros.insert(first_node.resource_header.reference_string(), true);
    //                     }
    //                 }
    //                 if first_intro_text.is_none() {
    //                     if let Some(intro_text_node) = intro_text_nodes.first() {
    //                         if let Ok(intro_text) = intro_text_node.node.get_text_content() {
    //                             first_intro_text = Some(intro_text.to_string());
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //     }

    //     // Now go through the groups and add the actual result nodes, skipping any that already are added up front from the intros
    //     for group in ret_groups.iter() {
    //         for node in group.iter() {
    //             if !final_nodes
    //                 .iter()
    //                 .take(10)
    //                 .any(|result_node| result_node.node.content == node.node.content)
    //             {
    //                 final_nodes.push(node.clone());
    //             }
    //         }
    //     }

    //     // println!(
    //     //     "\n\n\nDone Vector Searching: {}\n------------------------------------------------",
    //     //     query_text
    //     // );

    //     // for node in &final_nodes {
    //     //     eprintln!("{:?} - {:?}\n", node.score as f32, node.format_for_prompt(3500));
    //     // }

    //     Ok((final_nodes, first_intro_text))
    // }

    // //     /// Determines the number of grouped proximity retrieved nodes to check for intro fetching
    // //     fn determine_num_groups_for_intro_fetch(max_tokens_in_prompt: usize) -> usize {
    // //         if max_tokens_in_prompt < 5000 {
    // //             5
    // //         } else if max_tokens_in_prompt < 33000 {
    // //             6
    // //         } else {
    // //             7
    // //         }
    // //     }

    // /// Extracts top N keywords from the given text.
    // fn extract_keywords_from_text(text: &str, num_keywords: usize) -> Vec<String> {
    //     // Create a new KeyPhraseExtractor with a maximum of num_keywords keywords
    //     let extractor = KeyPhraseExtractor::new(text, num_keywords);

    //     // Get the keywords and their scores
    //     let keywords = extractor.get_keywords();

    //     // Return only the keywords, discarding the scores
    //     keywords.into_iter().map(|(_score, keyword)| keyword).collect()
    // }

    // //TODOs:
    // // - Potentially check the top 10 group result VR, and if they were a pdf or docx, then include first 1-2 nodes of the pdf/docx to always have title/authors available
    // //
    // /// Perform a proximity vector search on all local & VectorFS-held Vector Resources specified in the JobScope.
    // /// Returns the proximity groups of retrieved nodes.
    // #[allow(clippy::too_many_arguments)]
    // async fn internal_job_scope_vector_search_groups(
    //     _db: Arc<SqliteManager>,
    //     job_scope: &MinimalJobScope,
    //     query: Vec<f32>,
    //     query_text: String,
    //     num_of_top_results: u64,
    //     profile: &ShinkaiName,
    //     _include_description: bool,
    //     generator: RemoteEmbeddingGenerator,
    //     max_tokens_in_prompt: usize,
    // ) -> Result<
    //     (
    //         Vec<Vec<ShinkaiFileChunkCollection>>,
    //         HashMap<String, Vec<ShinkaiFileChunkCollection>>,
    //     ),
    //     SqliteManagerError,
    // > {
    //     // Determine the vector search mode configured in the job scope.
    //     // Limit the maximum tokens to 25k (~ 10 pages of PDF) if the context window is greater than that.
    //     // If the length is < 25k, pass the entire context.
    //     // If the length is > 25k, pass the first page of the document and fill up to 25k tokens of context window.
    //     let max_tokens_in_prompt =
    //         if job_scope.vector_search_mode.contains(&VectorSearchMode::FillUpTo25k) && max_tokens_in_prompt > 25000 {
    //             25000
    //         } else {
    //             max_tokens_in_prompt
    //         };

    //     let average_out_deep_search_scores = true;
    //     let proximity_window_size = Self::determine_proximity_window_size(max_tokens_in_prompt);
    //     let total_num_of_results = (num_of_top_results * proximity_window_size * 2) + num_of_top_results;
    //     // Holds the intro text for each VR, where only the ones that have results with top scores will be used
    //     let mut intro_hashmap: HashMap<String, Vec<RetrievedNode>> = HashMap::new();

    //     // Setup vars used across searches
    //     let deep_traversal_options = vec![
    //         TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring),
    //         TraversalOption::SetResultsMode(ResultsMode::ProximitySearch(proximity_window_size, num_of_top_results)),
    //     ];
    //     let num_of_resources_to_search_into = 50;
    //     let mut retrieved_node_groups = Vec::new();

    //     // VRPack deep vector search
    //     for entry in &job_scope.local_vrpack {
    //         let vr_pack_results = entry
    //             .vrpack
    //             .dynamic_deep_vector_search_with_vrkai_path_customized(
    //                 query_text.clone(),
    //                 num_of_resources_to_search_into,
    //                 &vec![],
    //                 None,
    //                 total_num_of_results,
    //                 TraversalMethod::Exhaustive,
    //                 &deep_traversal_options,
    //                 generator.clone(),
    //                 average_out_deep_search_scores,
    //                 job_scope.vector_search_mode.clone(),
    //             )
    //             .await?;

    //         // Fetch the VRKai intros and add them to hashmap
    //         let mut bare_results = vec![];
    //         for (ret_node, path) in vr_pack_results {
    //             let ref_string = ret_node.resource_header.reference_string();
    //             if let std::collections::hash_map::Entry::Vacant(e) = intro_hashmap.entry(ref_string) {
    //                 if let Ok(intro_nodes) = entry.vrpack.get_vrkai_intro_ret_nodes(path.clone()) {
    //                     e.insert(intro_nodes);
    //                 }
    //             }
    //             bare_results.push(ret_node);
    //         }

    //         let mut groups = RetrievedNode::group_proximity_results(&bare_results)?;
    //         retrieved_node_groups.append(&mut groups);
    //     }

    //     // Folder deep vector search
    //     {
    //         for folder in &job_scope.vector_fs_folders {
    //             let reader = vector_fs
    //                 .new_reader(profile.clone(), folder.path.clone(), profile.clone())
    //                 .await
    //                 .map_err(|e: VectorFSError| SqliteManagerError::SomeError(format!("VectorFS error: {}", e)))?;

    //             let results = vector_fs
    //                 .deep_vector_search_customized(
    //                     &reader,
    //                     query_text.clone(),
    //                     num_of_resources_to_search_into,
    //                     total_num_of_results,
    //                     deep_traversal_options.clone(),
    //                     average_out_deep_search_scores,
    //                     job_scope.vector_search_mode.clone(),
    //                 )
    //                 .await
    //                 .map_err(|e: VectorFSError| SqliteManagerError::SomeError(format!("VectorFS error: {}", e)))?;

    //             // Fetch the intros
    //             let mut bare_results = vec![];
    //             for result in results {
    //                 let ret_node = result.resource_retrieved_node.clone();
    //                 let ref_string = ret_node.resource_header.reference_string();
    //                 if let std::collections::hash_map::Entry::Vacant(e) = intro_hashmap.entry(ref_string) {
    //                     let result_reader = reader
    //                         .new_reader_copied_data(result.fs_item_path().clone(), &vector_fs)
    //                         .await
    //                         .map_err(|e: VectorFSError| {
    //                             SqliteManagerError::SomeError(format!("VectorFS error: {}", e))
    //                         })?;

    //                     if let Ok(intro_nodes) = vector_fs._internal_get_vr_intro_ret_nodes(&result_reader).await {
    //                         e.insert(intro_nodes);
    //                     }
    //                 }
    //                 bare_results.push(ret_node);
    //             }

    //             let mut groups = RetrievedNode::group_proximity_results(&mut bare_results)?;

    //             retrieved_node_groups.append(&mut groups);
    //         }
    //     }

    //     // Fetch rest of VRs directly
    //     let resources = JobManager::fetch_job_scope_direct_resources(vector_fs, job_scope, profile).await?;
    //     shinkai_log(
    //         ShinkaiLogOption::JobExecution,
    //         ShinkaiLogLevel::Info,
    //         &format!("Num of resources fetched: {}", resources.len()),
    //     );

    //     // Perform vector search on all direct resources
    //     for resource in &resources {
    //         // First get the resource embedding, and score vs the query
    //         let resource_embedding = resource.as_trait_object().resource_embedding();
    //         let resource_score = query.score_similarity(resource_embedding);

    //         // Do the search
    //         let mut results = resource.as_trait_object().vector_search_customized(
    //             query.clone(),
    //             total_num_of_results,
    //             TraversalMethod::Exhaustive,
    //             &deep_traversal_options,
    //             None,
    //             job_scope.vector_search_mode.clone(),
    //         );

    //         // Average out the node scores together with the resource_score
    //         if average_out_deep_search_scores {
    //             for ret_node in &mut results {
    //                 ret_node.score = deep_search_scores_average_out(
    //                     None,
    //                     resource_score,
    //                     resource.as_trait_object().description().unwrap_or("").to_string(),
    //                     ret_node.score,
    //                     ret_node.node.get_text_content().unwrap_or("").to_string(),
    //                 );
    //             }
    //         }

    //         // Fetch the intros
    //         let mut bare_results = vec![];
    //         for ret_node in results {
    //             let ref_string = ret_node.resource_header.reference_string();
    //             if let std::collections::hash_map::Entry::Vacant(e) = intro_hashmap.entry(ref_string) {
    //                 if let Ok(intro_nodes) = resource.as_trait_object().generate_intro_ret_nodes() {
    //                     e.insert(intro_nodes);
    //                 }
    //             }
    //             bare_results.push(ret_node);
    //         }

    //         let mut groups = RetrievedNode::group_proximity_results(&mut bare_results)?;
    //         retrieved_node_groups.append(&mut groups);
    //     }

    //     shinkai_log(
    //         ShinkaiLogOption::JobExecution,
    //         ShinkaiLogLevel::Info,
    //         &format!("Num of node groups retrieved: {}", retrieved_node_groups.len()),
    //     );

    //     // Sort the retrieved node groups by score, and generate a description if any direct VRs available
    //     let sorted_retrieved_node_groups =
    //         RetrievedNode::sort_by_score_groups(&retrieved_node_groups, total_num_of_results);

    //     Ok((sorted_retrieved_node_groups, intro_hashmap))
    // }

    // /// Determines the proximity window size based on the max tokens supported by the model
    // fn determine_proximity_window_size(max_tokens_in_prompt: usize) -> u64 {
    //     if max_tokens_in_prompt < 5000 {
    //         1
    //     } else if max_tokens_in_prompt < 33000 {
    //         2
    //     } else {
    //         3
    //     }
    // }

    /// Searches all resources in the given job scope and returns the search results.
    pub async fn search_all_resources_in_job_scope(
        scope: &MinimalJobScope,
        sqlite_manager: Arc<SqliteManager>,
        query_text: String,
        num_of_top_results: usize,
        max_tokens_in_prompt: usize,
        embedding_generator: RemoteEmbeddingGenerator,
    ) -> Result<ShinkaiFileChunkCollection, SqliteManagerError> {
        let mut parsed_file_ids = Vec::new();
        let mut paths_map = HashMap::new();
        
        let query_embedding = match embedding_generator.generate_embedding_default(&query_text).await {
            Ok(embedding) => embedding,
            Err(e) => {
                return Err(SqliteManagerError::SomeError(e.to_string()));
            }
        };

        // Retrieve each file in the job scope
        for path in &scope.vector_fs_items {
            if let Some(parsed_file) = sqlite_manager.get_parsed_file_by_shinkai_path(path).unwrap() {
                let file_id = parsed_file.id.unwrap();
                parsed_file_ids.push(file_id);
                paths_map.insert(file_id, path.clone());
            }
        }

        // Retrieve files inside vector_fs_folders
        for folder in &scope.vector_fs_folders {
            let files = match ShinkaiFileManager::list_directory_contents(folder.clone(), &sqlite_manager) {
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
                    }
                }
            }
        }

        // Determine the vector search mode configured in the job scope.
        let max_tokens_in_prompt =
            if scope.vector_search_mode.contains(&VectorSearchMode::FillUpTo25k) && max_tokens_in_prompt > 25000 {
                25000
            } else {
                max_tokens_in_prompt
            };

        // Perform a vector search on all parsed files
        let search_results = sqlite_manager.search_chunks(&parsed_file_ids, query_embedding, num_of_top_results)?;

        // Count the total number of characters in the search results using map-reduce
        let total_characters: usize = search_results
            .iter()
            .map(|(chunk, _distance)| chunk.content.len())
            .sum();

        // Calculate the average chunk size
        let average_chunk_size = total_characters / search_results.len();

        // Calculate the total amount of extra chunks we need to fetch to fill up to max_tokens_in_prompt
        let extra_chunks_needed = (max_tokens_in_prompt - total_characters) / average_chunk_size;

        // If there are no initial results, just return early
        if search_results.is_empty() {
            return Ok(ShinkaiFileChunkCollection {
                chunks: vec![],
                paths: Some(paths_map),
            });
        }

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
