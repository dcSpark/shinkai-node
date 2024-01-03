use super::{vector_fs::VectorFS, vector_fs_error::VectorFSError, vector_fs_reader::VFSReader};
use crate::db::db::ProfileBoundWriteBatch;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_resource::NodeContent;
use shinkai_vector_resources::{
    embeddings::Embedding,
    source::SourceFile,
    vector_resource::{BaseVectorResource, MapVectorResource, Node, VRHeader, VRPath, VRSource, VectorResourceCore},
};
use std::collections::HashMap;

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

    /// Internal method used to add a VRHeader into the core resource of a profile's VectorFS internals in memory.
    fn _add_vr_header_to_core_resource(
        &mut self,
        vr_header: VRHeader,
        metadata: HashMap<String, String>,
    ) -> Result<(), VectorFSError> {
        let mut internals = self.vector_fs._get_profile_fs_internals(&self.profile)?;

        // If an embedding exists on the VR, and it is generated using the same embedding model
        if let Some(embedding) = vr_header.resource_embedding.clone() {
            if vr_header.resource_embedding_model_used == internals.default_embedding_model() {
                internals.fs_core_resource.insert_vr_header_node_at_path(
                    self.path.clone(),
                    vr_header.resource_name.clone(),
                    vr_header,
                    Some(metadata),
                    embedding.clone(),
                )?;
                Ok(())
            } else {
                return Err(VectorFSError::EmbeddingModelTypeMismatch(
                    vr_header.resource_embedding_model_used,
                    internals.default_embedding_model(),
                ));
            }
        } else {
            return Err(VectorFSError::EmbeddingMissingInResource(vr_header.resource_name));
        }
    }

    /// Internal method used to add a new MapVectorResource into the core resource of a profile's VectorFS internals in memory.
    fn _add_new_vr_to_core_resource(
        &mut self,
        new_vr_name: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<(), VectorFSError> {
        let internals = self.vector_fs._get_profile_fs_internals(&self.profile)?;

        // Create a new MapVectorResource which represents a folder
        let new_vr = BaseVectorResource::Map(MapVectorResource::new_empty(new_vr_name, None, VRSource::None));
        let node = Node::new_vector_resource(new_vr_name.to_string(), &new_vr, metadata);
        let embedding = Embedding::new("", vec![]); // Empty embedding as folders do not score in VecFS search

        // Insert the new MapVectorResource into the current path with the name as the id
        internals
            .fs_core_resource
            .insert_node_at_path(self.path.clone(), new_vr_name.to_string(), node, embedding)?;

        Ok(())
    }

    /// Internal method used to remove a child node of the current path, given its id. Applies only in memory.
    /// This only works if path is a folder and node_id is either an item or folder underneath, and node_id points
    /// to a valid node.
    fn _remove_child_node_from_core_resource(&mut self, node_id: String) -> Result<(), VectorFSError> {
        let internals = self.vector_fs._get_profile_fs_internals(&self.profile)?;
        let path = self.path.push_cloned(node_id);
        internals.fs_core_resource.remove_node_at_path(path)?;

        Ok(())
    }

    /// Internal method used to remove the node at current path. Applies only in memory.
    /// Errors if no node exists at path.
    fn _remove_node_from_core_resource(&mut self) -> Result<(), VectorFSError> {
        let internals = self.vector_fs._get_profile_fs_internals(&self.profile)?;
        internals.fs_core_resource.remove_node_at_path(self.path.clone())?;

        Ok(())
    }

    /// Saves a Vector Resource and optional SourceFile underneath the current path.
    /// If a VR with the same name already exists underneath the current path, then overwrites the existing VR.
    pub fn save_vector_resource(&mut self, resource: BaseVectorResource, source_file: Option<SourceFile>) {
        let batch = ProfileBoundWriteBatch::new(&self.profile);
        let mut resource = resource;
        let resource_name = resource.as_trait_object().name();
        let internals = self.vector_fs._get_profile_fs_internals(&self.profile)?;
        let node_path = self.path.push_cloned(resource_name.to_string());

        // Ensure path of self points at a folder before proceeding
        self.validate_path_points_to_folder(self.path)?;
        // If an existing FSFolder is already saved at the node path, return error.
        if let Ok(_) = self.validate_path_points_to_folder(node_path) {
            return Err(VectorFSError::CannotOverwriteFolderWithResource(node_path));
        }
        // If an existing FSItem is already saved at the node path, delete it.
        if let Ok(_) = self.validate_path_points_to_item(node_path) {
            // TODO: Delete resource & source file in DB.
        }
        // Check if an existing VR is saved in the FSDB with the same reference string, then re-generate id of the current resource.
        if let Ok(_) = self
            .vector_fs
            .db
            .get_resource(&resource.as_trait_object().reference_string(), &self.profile)
        {
            resource.as_trait_object().generate_and_update_resource_id();
        }

        // Now all validation checks/setup have passed, move forward with saving header/resource/source file
    }

    /// Validates that the path points to a FSFolder
    pub fn validate_path_points_to_folder(&self, path: VRPath) -> Result<(), VectorFSError> {
        let internals = self.vector_fs._get_profile_fs_internals_read_only(&self.profile)?;
        let ret_node = internals.fs_core_resource.retrieve_node_at_path(path)?;

        match ret_node.node.content {
            NodeContent::Resource(_) => Ok(()),
            _ => Err(VectorFSError::InvalidNodeType(ret_node.node.id)),
        }
    }

    /// Validates that the path points to a FSItem
    pub fn validate_path_points_to_item(&self, path: VRPath) -> Result<(), VectorFSError> {
        let internals = self.vector_fs._get_profile_fs_internals_read_only(&self.profile)?;
        let ret_node = internals.fs_core_resource.retrieve_node_at_path(path)?;

        match ret_node.node.content {
            NodeContent::VRHeader(_) => Ok(()),
            _ => Err(VectorFSError::InvalidNodeType(ret_node.node.id)),
        }
    }
}
