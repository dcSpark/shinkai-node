use super::{vector_fs::VectorFS, vector_fs_error::VectorFSError, vector_fs_reader::VFSReader};
use crate::db::db::ProfileBoundWriteBatch;
use chrono::{DateTime, Utc};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::shinkai_time::ShinkaiTime;
use shinkai_vector_resources::vector_resource::{NodeContent, RetrievedNode};
use shinkai_vector_resources::{
    embeddings::Embedding,
    source::SourceFile,
    vector_resource::{BaseVectorResource, MapVectorResource, Node, VRHeader, VRPath, VRSource, VectorResourceCore},
};
use std::collections::HashMap;

/// A struct that represents having rights to write to the VectorFS under a profile/at a specific path.
/// If a VFSWriter struct is constructed, that means the `requester_name` has passed
/// permissions validation and is thus allowed to write to `path`.
pub struct VFSWriter {
    pub requester_name: ShinkaiName,
    pub path: VRPath,
    pub profile: ShinkaiName,
}

impl VFSWriter {
    /// Creates a new VFSWriter if the `requester_name` passes read permission validation check.
    pub fn new(
        requester_name: ShinkaiName,
        path: VRPath,
        vector_fs: &mut VectorFS,
        profile: ShinkaiName,
    ) -> Result<Self, VectorFSError> {
        let writer = VFSWriter {
            requester_name: requester_name.clone(),
            path: path.clone(),
            profile: profile.clone(),
        };

        // Validate write permissions to ensure requester_name has rights
        let fs_internals = vector_fs._get_profile_fs_internals_read_only(&profile)?;
        if !fs_internals
            .permissions_index
            .validate_read_permission(&requester_name, &path)
        {
            return Err(VectorFSError::InvalidWriterPermission(requester_name, profile, path));
        }

        // Once permission verified, saves the datatime into the FSDB as stored logs.
        let current_datetime = ShinkaiTime::generate_time_now();
        let mut write_batch = ProfileBoundWriteBatch::new_vfs_batch(&profile)?;
        vector_fs
            .db
            .wb_add_write_access_log(requester_name, &path, current_datetime, profile, &mut write_batch);
        vector_fs.db.write_pb(write_batch)?;

        Ok(writer)
    }

    /// Generates a VFSReader using the same requester_name/profile held in self.
    /// Read permissions are verified before the VFSReader is produced.
    pub fn _new_reader_copied_data(&self, path: VRPath, vector_fs: &mut VectorFS) -> Result<VFSReader, VectorFSError> {
        VFSReader::new(self.requester_name.clone(), path, vector_fs, self.profile.clone())
    }

    /// Generates a VFSWriter using the same requester_name/profile held in self.
    /// Write permissions are verified before the VFSWriter is produced.
    pub fn _new_writer_copied_data(&self, path: VRPath, vector_fs: &mut VectorFS) -> Result<VFSWriter, VectorFSError> {
        VFSWriter::new(self.requester_name.clone(), path, vector_fs, self.profile.clone())
    }
}

impl VectorFS {
    /// Internal method used to add a VRHeader into the core resource of a profile's VectorFS internals in memory.
    fn _add_vr_header_to_core_resource(
        &mut self,
        writer: &VFSWriter,
        vr_header: VRHeader,
        metadata: HashMap<String, String>,
    ) -> Result<(), VectorFSError> {
        let internals = self._get_profile_fs_internals(&writer.profile)?;

        // If an embedding exists on the VR, and it is generated using the same embedding model
        if let Some(embedding) = vr_header.resource_embedding.clone() {
            if vr_header.resource_embedding_model_used == internals.default_embedding_model() {
                internals.fs_core_resource.insert_vr_header_node_at_path(
                    writer.path.clone(),
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
        writer: &VFSWriter,
        new_vr_name: &str,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<(), VectorFSError> {
        let internals = self._get_profile_fs_internals(&writer.profile)?;

        // Create a new MapVectorResource which represents a folder
        let new_vr = BaseVectorResource::Map(MapVectorResource::new_empty(new_vr_name, None, VRSource::None));
        let node = Node::new_vector_resource(new_vr_name.to_string(), &new_vr, metadata);
        let embedding = Embedding::new("", vec![]); // Empty embedding as folders do not score in VecFS search

        // Insert the new MapVectorResource into the current path with the name as the id
        internals.fs_core_resource.insert_node_at_path(
            writer.path.clone(),
            new_vr_name.to_string(),
            node,
            embedding,
        )?;

        Ok(())
    }

    /// Internal method used to remove a child node of the current path, given its id. Applies only in memory.
    /// This only works if path is a folder and node_id is either an item or folder underneath, and node_id points
    /// to a valid node.
    fn _remove_child_node_from_core_resource(
        &mut self,
        writer: &VFSWriter,
        node_id: String,
    ) -> Result<(), VectorFSError> {
        let internals = self._get_profile_fs_internals(&writer.profile)?;
        let path = writer.path.push_cloned(node_id);
        internals.fs_core_resource.remove_node_at_path(path)?;

        Ok(())
    }

    /// Internal method used to remove the node at current path. Applies only in memory.
    /// Errors if no node exists at path.
    fn _remove_node_from_core_resource(&mut self, writer: &VFSWriter) -> Result<(), VectorFSError> {
        let internals = self._get_profile_fs_internals(&writer.profile)?;
        internals.fs_core_resource.remove_node_at_path(writer.path.clone())?;

        Ok(())
    }

    /// Saves a Vector Resource and optional SourceFile underneath the FSFolder at the specified path.
    /// If a VR with the same name already exists underneath the current path, then overwrites it.
    pub fn folder_save_vector_resource(
        &mut self,
        writer: &VFSWriter,
        resource: BaseVectorResource,
        source_file: Option<SourceFile>,
    ) -> Result<(), VectorFSError> {
        let batch = ProfileBoundWriteBatch::new(&writer.profile);
        let mut resource = resource;
        let resource_name = resource.as_trait_object().name();
        let internals = self._get_profile_fs_internals(&writer.profile)?;
        let node_path = writer.path.push_cloned(resource_name.to_string());
        let mut resource_exists_at_path = false;

        // Ensure path of writer points at a folder before proceeding
        self._validate_path_points_to_folder(writer.path.clone(), &writer.profile)?;
        // If an existing FSFolder is already saved at the node path, return error.
        if let Ok(_) = self._validate_path_points_to_folder(node_path.clone(), &writer.profile) {
            return Err(VectorFSError::CannotOverwriteFolder(node_path.clone()));
        }
        if let Ok(_) = self._validate_path_points_to_item(node_path.clone(), &writer.profile) {
            resource_exists_at_path = true;
        }
        // Check if an existing VR is saved in the FSDB with the same reference string, then if so re-generate id of the current resource.
        if let Ok(_) = self
            .db
            .get_resource(&resource.as_trait_object().reference_string(), &writer.profile)
        {
            resource.as_trait_object_mut().generate_and_update_resource_id();
        }
        // Now all validation checks/setup have passed, move forward with saving header/resource/source file

        // Don't forget to update the folder's last_modified in metadata if resource_exists_at_path == false (meaning new item added)
        // And the VR node's last_saved needs to be updated too in its metadata.
        Ok(())
    }

    /// Retrieves a node at a given path from the VectorFS core resource under a profile
    pub fn _retrieve_core_resource_node_at_path(
        &self,
        path: VRPath,
        profile: &ShinkaiName,
    ) -> Result<RetrievedNode, VectorFSError> {
        let internals = self._get_profile_fs_internals_read_only(profile)?;
        internals
            .fs_core_resource
            .retrieve_node_at_path(path.clone())
            .map_err(|_| VectorFSError::NoEntryAtPath(path.clone()))
    }

    // /// Updates the SourceFile attached to a Vector Resource (FSItem) underneath the current path.
    // /// If no VR (FSItem) with the same name already exists underneath the current path, then errors.
    // pub fn update_source_file(&mut self, resource: BaseVectorResource) {}
}
