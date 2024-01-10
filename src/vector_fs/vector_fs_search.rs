use super::{vector_fs::VectorFS, vector_fs_error::VectorFSError, vector_fs_reader::VFSReader};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::embeddings::MAX_EMBEDDING_STRING_SIZE;
use shinkai_vector_resources::{
    embeddings::Embedding,
    vector_resource::{
        RetrievedNode, TraversalMethod, TraversalOption, VRPath, VectorResource, VectorResourceCore,
        VectorResourceSearch,
    },
};

// TODO:
// Add a new VectorResource traversal option which is something like `ApplyNodeValidationBeforeTraversing`.
// Have it validate the validation function passes true before traversing into a node, or else if false skip over the node.
//
// Then use this as the default vector search (wrap in a local method) for the VectorFS, where we use a closure
// with the fs permissions, and have it validate that the user has read rights for the node validation function.
//
impl VectorFS {
    /// Performs a vector search into the VectorFS at a specific path,
    /// returning the retrieved VRHeader nodes.
    pub fn vector_search_headers(
        &self,
        reader: &VFSReader,
        query: Embedding,
        num_of_results: u64,
        profile: &ShinkaiName,
    ) -> Result<Vec<RetrievedNode>, VectorFSError> {
        let internals = self._get_profile_fs_internals_read_only(profile)?;
        // Vector search without hierarchical scoring because "folders" have no content/real embedding
        let results = internals.fs_core_resource.vector_search_customized(
            query,
            num_of_results,
            TraversalMethod::Exhaustive,
            &vec![],
            Some(reader.path.clone()),
        );

        Ok(results)
    }

    /// Generates an Embedding for the input query to be used in a Vector Search in the VecFS.
    /// This automatically uses the correct default embedding model for the given profile.
    pub async fn generate_query_embedding(
        &self,
        input_query: String,
        profile: &ShinkaiName,
    ) -> Result<Embedding, VectorFSError> {
        let generator = self._get_embedding_generator(profile)?;
        Ok(generator
            .generate_embedding_shorten_input_default(&input_query, MAX_EMBEDDING_STRING_SIZE as u64) // TODO: remove the hard-coding of embedding string size
            .await?)
    }
}
