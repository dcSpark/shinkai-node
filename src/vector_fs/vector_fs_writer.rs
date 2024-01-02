use super::{vector_fs::VectorFS, vector_fs_error::VectorFSError, vector_fs_reader::VFSReader};
use crate::db::db::ProfileBoundWriteBatch;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_resource::{VRHeader, VRPath};

/// A struct that allows performing write actions on the VectorFS under a profile/at a specific path.
/// If a VFSWriter struct is constructed, that means the `requester_name` has passed
/// permissions validation and is thus allowed to write to `path`.
pub struct VFSWriter<'a> {
    pub requester_name: ShinkaiName,
    pub path: VRPath,
    pub vector_fs: &'a mut VectorFS,
    pub profile: ShinkaiName,
}

impl<'a> VFSWriter<'a> {
    /// Creates a new VFSWriter if the `requester_name` passes read permission validation check.
    pub fn new(
        requester_name: ShinkaiName,
        path: VRPath,
        vector_fs: &'a mut VectorFS,
        profile: ShinkaiName,
    ) -> Result<Self, VectorFSError> {
        let writer = VFSWriter {
            requester_name: requester_name.clone(),
            path: path.clone(),
            vector_fs,
            profile: profile.clone(),
        };

        // Validate read permissions
        let fs_internals = writer.vector_fs._get_profile_fs_internals_read_only(&profile)?;
        if !fs_internals
            .permissions_index
            .validate_read_permission(&requester_name, &path)
        {
            return Err(VectorFSError::InvalidWriterPermission(requester_name, profile, path));
        }

        Ok(writer)
    }

    /// Generates a VFSReader using data held in VFSWriter. This is an internal method to improve ease of use of
    /// generating a Reader for specific write operations which may need it.
    pub fn _reader(&'a self, path: VRPath, profile: ShinkaiName) -> Result<VFSReader<'a>, VectorFSError> {
        VFSReader::new(self.requester_name.clone(), path, self.vector_fs, profile)
    }

    // /// Internal method used to add a VRHeader into the core resource of a profile's VectorFS internals in memory.
    // pub fn _memory_add_vr_header_to_core_resource(&mut self, vr_header: VRHeader) -> Result<(), VectorFSError> {
    //     let mut internals = self.vector_fs._get_profile_fs_internals(&self.profile)?;

    //     if let Some(embedding) = vr_header.resource_embedding {
    //         if vr_header.resource_embedding_model_used == internals.default_embedding_model() {
    //             // save to source resource in internals
    //             Ok(())
    //         } else {
    //             return Err(VectorFSError::EmbeddingModelTypeMismatch(
    //                 vr_header.resource_embedding_model_used,
    //                 internals.default_embedding_model(),
    //             ));
    //         }
    //     } else {
    //         return Err(VectorFSError::EmbeddingMissingInResource);
    //     }
    // }

    // /// Saves a Vector Resource into the VectorFS. If a VR with the same id already exists and writing to the
    // /// same path, then overwrites the existing VR. If same id, but writing to a different path, then a new id
    // ///  is generated/set for the input VR and it is saved separately in the fs_db.
    // /// If an source_file is provided then it likewise follows the same update logic just like the VR.
    // pub fn save_vector_resource(&mut self, resource: BaseVectorResource) {
    //    }
}
