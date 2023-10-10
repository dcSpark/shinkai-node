use crate::agent::job_manager::AgentManager;
use crate::db::db_errors::ShinkaiDBError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::vector_resource_types::RetrievedDataChunk;
use std::result::Result::Ok;

impl AgentManager {
    /// Helper method which fetches all local & DB-held Vector Resources specified in the given JobScope
    /// and returns all of them in a single list ready to be used.
    pub async fn fetch_job_scope_resources(
        &self,
        job_scope: &JobScope,
        profile: &ShinkaiName,
    ) -> Result<Vec<BaseVectorResource>, ShinkaiDBError> {
        let mut resources = Vec::new();

        // Add local resources to the list
        for local_entry in &job_scope.local {
            resources.push(local_entry.resource.clone());
        }

        // Fetch DB resources and add them to the list
        let db = self.db.lock().await;
        for db_entry in &job_scope.database {
            let resource = db.get_resource_by_pointer(&db_entry.resource_pointer, profile)?;
            resources.push(resource);
        }

        std::mem::drop(db);

        Ok(resources)
    }

    /// Perform a vector search on all local & DB-held Vector Resources specified in the JobScope.
    pub async fn job_scope_vector_search(
        &self,
        job_scope: &JobScope,
        query: Embedding,
        num_of_results: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let resources = self.fetch_job_scope_resources(job_scope, profile).await?;
        println!("Num of resources fetched: {}", resources.len());

        // Perform vector search on all resources
        let mut retrieved_chunks = Vec::new();
        for resource in resources {
            let results = resource.as_trait_object().vector_search(query.clone(), num_of_results);
            retrieved_chunks.extend(results);
        }

        println!("Num of chunks retrieved: {}", retrieved_chunks.len());

        // Sort the retrieved chunks by score before returning
        let sorted_retrieved_chunks = RetrievedDataChunk::sort_by_score(&retrieved_chunks, num_of_results);

        Ok(sorted_retrieved_chunks)
    }

    /// Perform a syntactic vector search on all local & DB-held Vector Resources specified in the JobScope.
    pub async fn job_scope_syntactic_vector_search(
        &self,
        job_scope: &JobScope,
        query: Embedding,
        num_of_results: u64,
        profile: &ShinkaiName,
        data_tag_names: &Vec<String>,
    ) -> Result<Vec<RetrievedDataChunk>, ShinkaiDBError> {
        let resources = self.fetch_job_scope_resources(job_scope, profile).await?;

        // Perform syntactic vector search on all resources
        let mut retrieved_chunks = Vec::new();
        for resource in resources {
            let results =
                resource
                    .as_trait_object()
                    .syntactic_vector_search(query.clone(), num_of_results, data_tag_names);
            retrieved_chunks.extend(results);
        }

        // Sort the retrieved chunks by score before returning
        let sorted_retrieved_chunks = RetrievedDataChunk::sort_by_score(&retrieved_chunks, num_of_results);

        Ok(sorted_retrieved_chunks)
    }
}
