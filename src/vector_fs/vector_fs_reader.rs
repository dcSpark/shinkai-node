use super::vector_fs::VectorFS;
use super::vector_fs_error::VectorFSError;
use super::vector_fs_types::{FSEntry, FSFolder, FSItem};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::source::SourceFile;
use shinkai_vector_resources::vector_resource::{
    BaseVectorResource, NodeContent, VectorResource, VectorResourceCore, VectorResourceSearch,
};
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

    /// Retrieves the FSEntry for the path in the VectorFS. If path is root `/`, then returns a
    /// FSFolder that matches the FS root structure.
    pub fn retrieve_fs_entry(&self) -> Result<FSEntry, VectorFSError> {
        let internals = self.vector_fs._get_profile_fs_internals_read_only(&self.profile)?;

        // Create FSFolder directly if path is root
        if self.path.is_empty() {
            let core_resource = BaseVectorResource::Map(internals.fs_core_resource.clone());
            let fs_folder = FSFolder::from_vector_resource(core_resource, self.path.clone())?;
            return Ok(FSEntry::Folder(fs_folder));
        }

        // Otherwise retrieve the node and process it
        let ret_node = internals.fs_core_resource.retrieve_node_at_path(self.path.clone())?;
        match ret_node.node.content {
            NodeContent::Resource(_) => {
                let fs_folder = FSFolder::from_vector_resource_node(ret_node.node.clone(), self.path.clone())?;
                Ok(FSEntry::Folder(fs_folder))
            }
            NodeContent::VRHeader(_) => {
                let fs_item = FSItem::from_vr_header_node(ret_node.node, self.path.clone())?;
                Ok(FSEntry::Item(fs_item))
            }
            _ => Ok(Err(VRError::InvalidNodeType(ret_node.node.id))?),
        }
    }

    /// Attempts to retrieve a VectorResource at the path. If a VectorResource is not saved
    /// at this path, an error will be returned.
    pub fn retrieve_vector_resource(&self) -> Result<BaseVectorResource, VectorFSError> {
        let fs_item = self.retrieve_fs_entry()?.as_item()?;
        self.vector_fs.db.get_resource_by_fs_item(&fs_item, &self.profile)
    }

    /// Attempts to retrieve a SourceFile at the path. If this path does not currently exist, or
    /// a source_file is not saved at this path, then an error is returned.
    pub fn retrieve_source_file(&self) -> Result<SourceFile, VectorFSError> {
        let fs_item = self.retrieve_fs_entry()?.as_item()?;
        self.vector_fs.db.get_source_file_by_fs_item(&fs_item, &self.profile)
    }

    /// Attempts to retrieve a VectorResource and its SourceFile at the path. If either is not available
    /// an error will be returned.
    pub fn retrieve_vr_and_source_file(&self) -> Result<(BaseVectorResource, SourceFile), VectorFSError> {
        let vr = self.retrieve_vector_resource()?;
        let sf = self.retrieve_source_file()?;
        Ok((vr, sf))
    }
}
