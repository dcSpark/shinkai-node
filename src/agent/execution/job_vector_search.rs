use crate::agent::job_manager::JobManager;
use crate::db::ShinkaiDB;
use crate::db::db_errors::ShinkaiDBError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::vector_resource_types::{DataChunk, RetrievedDataChunk, VectorResourcePointer};
use tokio::sync::Mutex;
use std::result::Result::Ok;
use std::sync::Arc;

impl JobManager {
    /// Helper method which fetches all local & DB-held Vector Resources specified in the given JobScope
    /// and returns all of them in a single list ready to be used.
    pub async fn fetch_job_scope_resources(
        db: Arc<Mutex<ShinkaiDB>>,
        job_scope: &JobScope,
        profile: &ShinkaiName,
    ) -> Result<Vec<BaseVectorResource>, ShinkaiDBError> {
        let mut resources = Vec::new();

        // Add local resources to the list
        for local_entry in &job_scope.local {
            resources.push(local_entry.resource.clone());
        }

        // Fetch DB resources and add them to the list
        let db = db.lock().await;
        for db_entry in &job_scope.database {
            let resource = db.get_resource_by_pointer(&db_entry.resource_pointer, profile)?;
            resources.push(resource);
        }

        std::mem::drop(db);

        Ok(resources)
    }

    /// Perform a vector search on all local & DB-held Vector Resources specified in the JobScope.
    /// If include_description is true then adds the description of the Vector Resource as an auto-included
    /// RetrievedDataChunk at the front of the returned list.
    pub async fn job_scope_vector_search(
        db: Arc<Mutex<ShinkaiDB>>,
        job_scope: &JobScope,
        query: Embedding,
        num_of_results: u64,
        profile: &ShinkaiName,
        include_description: bool,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let resources = JobManager::fetch_job_scope_resources(db, job_scope, profile).await?;
        println!("Num of resources fetched: {}", resources.len());

        // Perform vector search on all resources
        let mut retrieved_chunks = Vec::new();
        for resource in &resources {
            let results = resource.as_trait_object().vector_search(query.clone(), num_of_results);
            retrieved_chunks.extend(results);
        }

        println!("Num of chunks retrieved: {}", retrieved_chunks.len());

        // Sort the retrieved chunks by score before returning
        let sorted_retrieved_chunks = RetrievedDataChunk::sort_by_score(&retrieved_chunks, num_of_results);
        let updated_chunks = JobManager::include_description_retrieved_chunk(include_description, sorted_retrieved_chunks, &resources)
            .await;

        Ok(updated_chunks)
    }

    /// Perform a syntactic vector search on all local & DB-held Vector Resources specified in the JobScope.
    /// If include_description is true then adds the description of the Vector Resource as an auto-included
    /// RetrievedDataChunk at the front of the returned list.
    pub async fn job_scope_syntactic_vector_search(
        db: Arc<Mutex<ShinkaiDB>>,
        job_scope: &JobScope,
        query: Embedding,
        num_of_results: u64,
        profile: &ShinkaiName,
        data_tag_names: &Vec<String>,
        include_description: bool,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let resources = JobManager::fetch_job_scope_resources(db, job_scope, profile).await?;

        // Perform syntactic vector search on all resources
        let mut retrieved_chunks = Vec::new();
        for resource in &resources {
            let results =
                resource
                    .as_trait_object()
                    .syntactic_vector_search(query.clone(), num_of_results, data_tag_names);
            retrieved_chunks.extend(results);
        }

        // Sort the retrieved chunks by score before returning
        let sorted_retrieved_chunks = RetrievedDataChunk::sort_by_score(&retrieved_chunks, num_of_results);
        let updated_chunks = JobManager::include_description_retrieved_chunk(include_description, sorted_retrieved_chunks, &resources)
            .await;

        Ok(updated_chunks)
    }

    /// If include_description is true then adds the description of the Vector Resource
    /// that the top scored retrieved chunk is from, by prepending a fake RetrievedDataChunk
    /// with the description inside. Removes the lowest scored chunk to preserve list length.
    async fn include_description_retrieved_chunk(
        include_description: bool,
        sorted_retrieved_chunks: Vec<RetrievedDataChunk>,
        resources: &[BaseVectorResource],
    ) -> Vec<RetrievedDataChunk> {
        let mut new_chunks = sorted_retrieved_chunks.clone();

        if include_description && !sorted_retrieved_chunks.is_empty() {
            let pointer = sorted_retrieved_chunks[0].resource_pointer.clone();

            // Iterate through resources until we find one with a matching resource pointer
            for resource in resources {
                if resource.as_trait_object().get_resource_pointer() == pointer {
                    if let Some(description) = resource.as_trait_object().description() {
                        let description_chunk = RetrievedDataChunk::new(
                            DataChunk::new(String::new(), &description, None, &vec![]),
                            1.0 as f32,
                            pointer,
                            sorted_retrieved_chunks[0].retrieval_path.clone(),
                        );
                        new_chunks.insert(0, description_chunk);
                        new_chunks.pop(); // Remove the last element to maintain the same length
                    }
                    break;
                }
            }
        }

        new_chunks
    }
}
