use super::vector_fs::VectorFS;
use super::vector_fs_error::VectorFSError;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator};
use shinkai_vector_resources::{embeddings::Embedding, vector_resource::VRPath};

/// A struct that allows performing read actions on the VectorFS under a profile/at a specific path.
/// If a VFSReader struct is constructed, that means the `requester_name` has passed
/// permissions validation and is thus allowed to read `path`.
pub struct VFSReader<'a> {
    pub requester_name: ShinkaiName,
    pub path: VRPath,
    pub vector_fs: &'a VectorFS,
    pub profile: ShinkaiName,
}

impl<'a> VFSReader<'a> {
    /// Creates a new VFSReader if the `requester_name` passes read permission validation check.
    pub fn new(
        requester_name: ShinkaiName,
        path: VRPath,
        vector_fs: &'a VectorFS,
        profile: ShinkaiName,
    ) -> Result<Self, VectorFSError> {
        let reader = VFSReader {
            requester_name: requester_name.clone(),
            path: path.clone(),
            vector_fs,
            profile: profile.clone(),
        };

        // Validate read permissions
        let fs_internals = reader.vector_fs._get_profile_fs_internals_read_only(&profile)?;
        if !fs_internals
            .permissions_index
            .validate_read_permission(&requester_name, &path)
        {
            return Err(VectorFSError::InvalidReaderPermission(requester_name, profile, path));
        }

        Ok(reader)
    }

    /// Generates an Embedding for the input query to be used in a Vector Search in the VecFS.
    /// This automatically uses the correct default embedding model for the given profile.
    pub async fn generate_query_embedding(
        &self,
        input_query: String,
        profile: &ShinkaiName,
    ) -> Result<Embedding, VectorFSError> {
        let generator = self.vector_fs._get_embedding_generator(profile)?;
        Ok(generator.generate_embedding_default(&input_query).await?)
    }
}
