use std::collections::HashMap;

use super::vector_fs_permissions::PermissionsIndex;
use super::{vector_fs::VectorFS, vector_fs_error::VectorFSError, vector_fs_reader::VFSReader};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::embeddings::MAX_EMBEDDING_STRING_SIZE;
use shinkai_vector_resources::vector_resource::{LimitTraversalMode, Node};
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
    fn permissions_validation_func(_: &Node, path: &VRPath, hashmap: HashMap<VRPath, String>) -> bool {
        // Check if the hashmap contains the key path
        if !hashmap.contains_key(path) {
            return false;
        }

        let reader = VFSReader::from_json(hashmap.get(&PermissionsIndex::vfs_reader_unique_path()).unwrap()).unwrap();
        let perm_index = PermissionsIndex::convert_from_json_values(reader.profile.clone(), hashmap).unwrap();
        perm_index.validate_read_permission(&reader.requester_name, path)
    }

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
        let stringified_permissions_map = internals
            .permissions_index
            .convert_fs_permissions_to_json_values(reader);

        let traversal_options = vec![TraversalOption::SetTraversalLimiting(
            LimitTraversalMode::LimitTraversalByValidationWithMap((
                Self::permissions_validation_func,
                stringified_permissions_map,
            )),
        )];
        // Vector search without hierarchical scoring because "folders" have no content/real embedding
        // + using our permissions validation function.
        let results = internals.fs_core_resource.vector_search_customized(
            query,
            num_of_results,
            TraversalMethod::Exhaustive,
            &traversal_options,
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
