use crate::agent::job_manager::JobManager;
use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use keyphrases::KeyPhraseExtractor;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::vector_resource::{
    BaseVectorResource, Node, ResultsMode, RetrievedNode, ScoringMode, TraversalMethod, TraversalOption, VRHeader,
};
use std::result::Result::Ok;
use std::sync::Arc;
use tokio::sync::Mutex;

impl JobManager {
    /// Helper method which fetches all local VRs, & directly linked FSItem Vector Resources specified in the given JobScope.
    /// Returns all of them in a single list ready to be used.
    /// Of note, this does not fetch resources inside of folders in the job scope, as those are not fetched in whole,
    /// but instead have a deep vector search performed on them via the VectorFS itself separately.
    pub async fn fetch_job_scope_direct_resources(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        job_scope: &JobScope,
        profile: &ShinkaiName,
    ) -> Result<Vec<BaseVectorResource>, ShinkaiDBError> {
        let mut resources = Vec::new();

        // Add local resources to the list
        for local_entry in &job_scope.local_vrkai {
            resources.push(local_entry.vrkai.resource.clone());
        }

        for fs_item in &job_scope.vector_fs_items {
            let reader = vector_fs
                .new_reader(profile.clone(), fs_item.path.clone(), profile.clone())
                .await?;

            let ret_resource = vector_fs.retrieve_vector_resource(&reader).await?;
            resources.push(ret_resource);
        }

        Ok(resources)
    }

    /// Performs multiple vector searches within the job scope based on extracting keywords from the query text.
    /// Attempts to take at least 1 retrieved node per keyword that is from a VR different than the highest scored node, to encourage wider diversity in results.
    /// Returns the search results and the description/summary text of the VR the highest scored retrieved node is from.
    pub async fn keyword_chained_job_scope_vector_search(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        job_scope: &JobScope,
        query_text: String,
        user_profile: &ShinkaiName,
        generator: RemoteEmbeddingGenerator,
        num_of_results: u64,
    ) -> Result<(Vec<RetrievedNode>, String), ShinkaiDBError> {
        // First perform a standard job scope vector search using the whole query text
        let query = generator.generate_embedding_default(&query_text).await?;
        let mut ret_nodes = JobManager::job_scope_vector_search(
            db.clone(),
            vector_fs.clone(),
            job_scope,
            query,
            query_text.clone(),
            num_of_results,
            user_profile,
            true,
            generator.clone(),
        )
        .await?;

        // Extract the summary text from the most similar
        let summary_node_text = ret_nodes
            .get(0)
            .and_then(|node| node.node.get_text_content().ok())
            .map(|text| text.to_string())
            .unwrap_or_default();

        // Initialize included_vrs vector with the resource header id of the first node, if it exists
        let mut included_vrs = ret_nodes
            .get(0)
            .map(|node| vec![node.resource_header.reference_string()])
            .unwrap_or_else(Vec::new);

        // Extract top 3 keywords from the query_text
        let keywords = Self::extract_keywords_from_text(&query_text, 3);

        // Now we proceed to keyword search chaining logic.
        for keyword in keywords {
            let keyword_query = generator.generate_embedding_default(&keyword).await?;
            let keyword_ret_nodes = JobManager::job_scope_vector_search(
                db.clone(),
                vector_fs.clone(),
                job_scope,
                keyword_query,
                keyword.clone(),
                num_of_results,
                user_profile,
                true,
                generator.clone(),
            )
            .await?;

            // Start looping through the vector search results for this keyword
            let mut keyword_node_inserted = false;
            let mut from_unique_vr = false;
            for keyword_node in keyword_ret_nodes {
                if !ret_nodes
                    .iter()
                    .any(|node| node.node.content == keyword_node.node.content)
                {
                    // If the node is unique and we haven't inserted any keyword node yet
                    if !keyword_node_inserted {
                        // Insert the first node that is not in ret_nodes
                        if ret_nodes.len() >= 3 {
                            ret_nodes.insert(3, keyword_node.clone()); // Insert at the 3rd position
                        } else {
                            ret_nodes.push(keyword_node.clone()); // If less than 3 nodes, just append
                        }
                        keyword_node_inserted = true;

                        // Check if this keyword node is from a unique VR
                        from_unique_vr = !included_vrs.contains(&keyword_node.resource_header.reference_string());
                        // Update the included_vrs
                        included_vrs.push(keyword_node.resource_header.reference_string());

                        // If the first unique node is from a unique VR, no need to continue going through rest of the keyword_nodes
                        if from_unique_vr {
                            break;
                        }
                    } else {
                        // If we're attempting to insert a unique VR node and found one
                        if ret_nodes.len() >= 4 {
                            ret_nodes.insert(4, keyword_node); // Insert at the 4th position if possible
                        } else {
                            ret_nodes.push(keyword_node); // Otherwise, just append
                        }
                        // Once a unique VR node is inserted, no need to continue for this keyword
                        break;
                    }
                }
            }
        }

        // Remove the extra lowest scored search results to ensure the list does not exceed num_of_results
        ret_nodes.truncate(num_of_results as usize);

        Ok((ret_nodes, summary_node_text))
    }

    /// Extracts top N keywords from the given text.
    fn extract_keywords_from_text(text: &str, num_keywords: usize) -> Vec<String> {
        // Create a new KeyPhraseExtractor with a maximum of num_keywords keywords
        let extractor = KeyPhraseExtractor::new(text, num_keywords);

        // Get the keywords and their scores
        let keywords = extractor.get_keywords();

        // Return only the keywords, discarding the scores
        keywords.into_iter().map(|(_score, keyword)| keyword).collect()
    }

    /// Perform a vector search on all local & VectorFS-held Vector Resources specified in the JobScope.
    /// If include_description is true then adds the description of the highest scored Vector Resource as an auto-included
    /// RetrievedNode at the front of the returned list.
    pub async fn job_scope_vector_search(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        job_scope: &JobScope,
        query: Embedding,
        query_text: String,
        num_of_top_results: u64,
        profile: &ShinkaiName,
        include_description: bool,
        generator: RemoteEmbeddingGenerator,
    ) -> Result<Vec<RetrievedNode>, ShinkaiDBError> {
        // TODO: Make this dynamic based on LLM context window length
        let proximity_window_size = 1;
        let total_num_of_results = (num_of_top_results * proximity_window_size * 2) + num_of_top_results;
        let mut retrieved_node_groups = Vec::new();

        // Setup vars used across searches
        let deep_traversal_options = vec![
            TraversalOption::SetScoringMode(ScoringMode::HierarchicalAverageScoring),
            TraversalOption::SetResultsMode(ResultsMode::ProximitySearch(proximity_window_size, num_of_top_results)),
        ];
        let num_of_resources_to_search_into = 50;

        // VRPack deep vector search
        for entry in &job_scope.local_vrpack {
            let mut vr_pack_results = entry
                .vrpack
                .dynamic_deep_vector_search_customized(
                    query_text.clone(),
                    num_of_resources_to_search_into,
                    &vec![],
                    None,
                    total_num_of_results,
                    TraversalMethod::Exhaustive,
                    &deep_traversal_options,
                    generator.clone(),
                )
                .await?;

            let mut groups = RetrievedNode::group_proximity_results(&mut vr_pack_results)?;
            retrieved_node_groups.append(&mut groups);
        }

        // Folder deep vector search
        {
            // let mut vec_fs = vector_fs;
            for folder in &job_scope.vector_fs_folders {
                let reader = vector_fs
                    .new_reader(profile.clone(), folder.path.clone(), profile.clone())
                    .await?;

                let mut results = vector_fs
                    .deep_vector_search_customized(
                        &reader,
                        query_text.clone(),
                        num_of_resources_to_search_into,
                        total_num_of_results,
                        deep_traversal_options.clone(),
                    )
                    .await?
                    .iter()
                    .map(|fs_node| fs_node.resource_retrieved_node.clone())
                    .collect();

                let mut groups = RetrievedNode::group_proximity_results(&mut results)?;

                retrieved_node_groups.append(&mut groups);
            }
        }

        // Fetch rest of VRs directly
        let resources = JobManager::fetch_job_scope_direct_resources(db, vector_fs, job_scope, profile).await?;
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("Num of resources fetched: {}", resources.len()),
        );

        // Perform vector search on all direct resources
        for resource in &resources {
            let mut results = resource.as_trait_object().vector_search_customized(
                query.clone(),
                total_num_of_results,
                TraversalMethod::Exhaustive,
                &deep_traversal_options,
                None,
            );

            let mut groups = RetrievedNode::group_proximity_results(&mut results)?;

            retrieved_node_groups.append(&mut groups);
        }

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("Num of node groups retrieved: {}", retrieved_node_groups.len()),
        );

        // Sort the retrieved node groups by score
        let sorted_retrieved_node_groups =
            RetrievedNode::sort_by_score_groups(&retrieved_node_groups, total_num_of_results);
        let sorted_retrieved_nodes = sorted_retrieved_node_groups.into_iter().flatten().collect::<Vec<_>>();
        let updated_nodes =
            JobManager::include_description_retrieved_node(include_description, sorted_retrieved_nodes, &resources)
                .await;

        Ok(updated_nodes)
    }

    /// If include_description is true then adds the description of the Vector Resource
    /// that the top scored retrieved node is from, by prepending a fake RetrievedNode
    /// with the description inside. Removes the lowest scored node to preserve list length.
    async fn include_description_retrieved_node(
        include_description: bool,
        sorted_retrieved_nodes: Vec<RetrievedNode>,
        resources: &[BaseVectorResource],
    ) -> Vec<RetrievedNode> {
        let mut new_nodes = sorted_retrieved_nodes.clone();

        if include_description && !sorted_retrieved_nodes.is_empty() {
            let resource_header = sorted_retrieved_nodes[0].resource_header.clone();

            // Iterate through resources until we find one with a matching resource reference string
            for resource in resources {
                if resource.as_trait_object().generate_resource_header().reference_string()
                    == resource_header.reference_string()
                {
                    if let Some(description) = resource.as_trait_object().description() {
                        let description_node = RetrievedNode::new(
                            Node::new_text(String::new(), description.to_string(), None, &vec![]),
                            1.0 as f32,
                            resource_header,
                            sorted_retrieved_nodes[0].retrieval_path.clone(),
                        );
                        new_nodes.insert(0, description_node);
                        new_nodes.pop(); // Remove the last element to maintain the same length
                    }
                    break;
                }
            }
        }

        new_nodes
    }
}
