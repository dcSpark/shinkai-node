use super::{fs_error::VectorFSError, vector_fs::VectorFS, vector_fs_reader::VFSReader};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::{
    embeddings::Embedding,
    vector_resource::{VectorResource, VectorResourceCore},
    vector_search_traversal::{RetrievedNode, TraversalMethod, TraversalOption, VRPath},
};

impl<'a> VFSReader<'a> {
    /// Performs a vector search into the VectorFS at a specific path,
    /// returning the retrieved VRHeader nodes.
    pub fn vector_search_headers(
        &self,
        query: Embedding,
        num_of_results: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedNode>, VectorFSError> {
        let internals = self.vector_fs._get_profile_fs_internals_read_only(profile)?;
        // Vector search without hierarchical scoring because "folders" have no content/real embedding
        let results = internals.fs_core_resource.vector_search_customized(
            query,
            num_of_results,
            TraversalMethod::Exhaustive,
            &vec![],
            Some(self.path.clone()),
        );

        Ok(results)
    }
}
