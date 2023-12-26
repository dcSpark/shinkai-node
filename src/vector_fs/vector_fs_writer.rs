use crate::db::db::ProfileBoundWriteBatch;

use super::{fs_error::VectorFSError, vector_fs::VectorFS, vector_fs_reader::VFSReader};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_search_traversal::{VRHeader, VRPath};

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
}
