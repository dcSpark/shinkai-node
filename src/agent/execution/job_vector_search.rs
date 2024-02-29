use crate::agent::job_manager::JobManager;
use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::vector_fs::vector_fs::VectorFS;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::embeddings::Embedding;
use shinkai_vector_resources::vector_resource::{BaseVectorResource, Node, RetrievedNode, VRHeader};
use std::result::Result::Ok;
use std::sync::Arc;
use tokio::sync::Mutex;

impl JobManager {
    /// Helper method which fetches all local VRs & directly linked FSItem Vector Resources specified in the given JobScope.
    /// Returns all of them in a single list ready to be used.
    /// Of note, this does not fetch resources inside of folders in the job scope, as those are not fetched in whole,
    /// but instead have a deep vector search performed on them via the VectorFS itself separately.
    pub async fn fetch_job_scope_direct_resources(
        db: Arc<Mutex<ShinkaiDB>>,
        vector_fs: Arc<Mutex<VectorFS>>,
        job_scope: &JobScope,
        profile: &ShinkaiName,
    ) -> Result<Vec<BaseVectorResource>, ShinkaiDBError> {
        let mut resources = Vec::new();

        // Add local resources to the list
        for local_entry in &job_scope.local {
            resources.push(local_entry.vrkai.resource.clone());
        }

        let mut vec_fs = vector_fs.lock().await;
        for fs_item in &job_scope.vector_fs_items {
            let reader = vec_fs
                .new_reader(profile.clone(), fs_item.path.clone(), profile.clone())
                .unwrap();

            let ret_resource = vec_fs.retrieve_vector_resource(&reader)?;
            resources.push(ret_resource);
        }

        Ok(resources)
    }

    /// Perform a vector search on all local & VectorFS-held Vector Resources specified in the JobScope.
    /// If include_description is true then adds the description of the Vector Resource as an auto-included
    /// RetrievedNode at the front of the returned list.
    pub async fn job_scope_vector_search(
        db: Arc<Mutex<ShinkaiDB>>,
        vector_fs: Arc<Mutex<VectorFS>>,
        job_scope: &JobScope,
        query: Embedding,
        num_of_results: u64,
        profile: &ShinkaiName,
        include_description: bool,
    ) -> Result<Vec<RetrievedNode>, ShinkaiDBError> {
        // TODO: perform deep vector search on folders and only manual search here in the VRs directly
        let resources = JobManager::fetch_job_scope_direct_resources(db, vector_fs, job_scope, profile).await?;
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("Num of resources fetched: {}", resources.len()),
        );

        // Perform vector search on all resources
        let mut retrieved_nodes = Vec::new();
        for resource in &resources {
            let results = resource.as_trait_object().vector_search(query.clone(), num_of_results);
            retrieved_nodes.extend(results);
        }

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("Num of nodes retrieved: {}", retrieved_nodes.len()),
        );

        // TODO: Add the results from the deep search in the folders
        // Sort the retrieved nodes by score before returning
        let sorted_retrieved_nodes = RetrievedNode::sort_by_score(&retrieved_nodes, num_of_results);
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
