use super::vector_fs_types::{DistributionOrigin, FSFolder, FSItem};
use super::{vector_fs::VectorFS, vector_fs_error::VectorFSError, vector_fs_reader::VFSReader};
use crate::db::db::ProfileBoundWriteBatch;
use crate::vector_fs::vector_fs_permissions::{ReadPermission, WhitelistPermission, WritePermission};
use chrono::{DateTime, Utc};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::shinkai_time::ShinkaiTime;
use shinkai_vector_resources::source::SourceFileMap;
use shinkai_vector_resources::vector_resource::{NodeContent, RetrievedNode, SourceFileType};
use shinkai_vector_resources::{
    embeddings::Embedding,
    source::SourceFile,
    vector_resource::{BaseVectorResource, MapVectorResource, Node, VRHeader, VRPath, VRSource, VectorResourceCore},
};
use std::collections::HashMap;
use std::process::ExitStatus;

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
        if fs_internals
            .permissions_index
            .validate_read_permission(&requester_name, &path)
            .is_err()
        {
            return Err(VectorFSError::InvalidWriterPermission(requester_name, profile, path));
        }

        // Once permission verified, saves the datatime into the FSDB as stored logs.
        let current_datetime = ShinkaiTime::generate_time_now();
        let mut write_batch = ProfileBoundWriteBatch::new_vfs_batch(&profile)?;
        vector_fs
            .db
            .wb_add_write_access_log(requester_name, &path, current_datetime, profile, &mut write_batch)?;
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

    /// Generates a new empty ProfileBoundWiteBatch using the profile in the Writer
    fn new_write_batch(&self) -> Result<ProfileBoundWriteBatch, VectorFSError> {
        ProfileBoundWriteBatch::new_vfs_batch(&self.profile)
    }
}

impl VectorFS {
    /// Saves a Vector Resource and optional SourceFile underneath the FSFolder at the specified path.
    /// If a VR with the same name already exists underneath the current path, then overwrites it.
    /// Currently does not support saving into VecFS root.
    pub fn create_new_folder(&mut self, writer: &VFSWriter, folder_name: &str) -> Result<FSFolder, VectorFSError> {
        // Create a new MapVectorResource which represents a folder
        let current_datetime = ShinkaiTime::generate_time_now();
        let new_vr = BaseVectorResource::Map(MapVectorResource::new_empty(folder_name, None, VRSource::None, true));
        let embedding = Embedding::new("", vec![]); // Empty embedding as folders do not score in VecFS search

        // Setup default metadata for new folder node
        let mut metadata = HashMap::new();
        metadata.insert(FSFolder::last_modified_key(), current_datetime.to_rfc3339());

        // Add the folder into the internals
        let new_folder =
            self._add_existing_vr_to_core_resource(writer, new_vr, embedding, Some(metadata), current_datetime)?;
        let new_folder_path = new_folder.path.clone();

        // Add private read/write permission for the folder path
        {
            let internals = self._get_profile_fs_internals(&writer.profile)?;
            internals.permissions_index.insert_path_permission(
                new_folder_path,
                ReadPermission::Private,
                WritePermission::Private,
            )?;
        }

        // Save the FSInternals into the FSDB
        let internals = self._get_profile_fs_internals_read_only(&writer.profile)?;
        let mut write_batch = writer.new_write_batch()?;
        self.db.wb_save_profile_fs_internals(internals, &mut write_batch)?;
        self.db.write_pb(write_batch)?;

        return Ok(new_folder);
    }

    /// Saves a Vector Resource and optional SourceFile underneath the FSFolder at the specified path.
    /// If a VR with the same name already exists underneath the current path, then updates(overwrites) it.
    /// Does not support saving into VecFS root.
    pub fn save_vector_resource_in_folder(
        &mut self,
        writer: &VFSWriter,
        resource: BaseVectorResource,
        source_file_map: Option<SourceFileMap>,
        distribution_origin: DistributionOrigin,
    ) -> Result<FSItem, VectorFSError> {
        let batch = ProfileBoundWriteBatch::new(&writer.profile);
        let mut resource = resource;
        let vr_header = resource.as_trait_object().generate_resource_header();
        let source_db_key = vr_header.reference_string();
        let resource_name = SourceFileType::clean_string_of_extension(resource.as_trait_object().name());
        resource.as_trait_object_mut().set_name(resource_name.clone());
        let node_path = writer.path.push_cloned(resource_name.to_string());
        let mut node_metadata = None;
        let mut node_at_path_already_exists = false;
        let mut new_item = None;

        {
            let internals = self._get_profile_fs_internals(&writer.profile)?;

            // Ensure path of writer points at a folder before proceeding
            self._validate_path_points_to_folder(writer.path.clone(), &writer.profile)?;
            // If an existing FSFolder is already saved at the node path, return error.
            if let Ok(_) = self._validate_path_points_to_folder(node_path.clone(), &writer.profile) {
                return Err(VectorFSError::CannotOverwriteFolder(node_path.clone()));
            }
            // If an existing FSItem is saved at the node path
            if let Ok(_) = self._validate_path_points_to_item(node_path.clone(), &writer.profile) {
                if let Ok(ret_node) = self._retrieve_core_resource_node_at_path(node_path.clone(), &writer.profile) {
                    node_metadata = ret_node.node.metadata.clone();
                    node_at_path_already_exists = true;
                }
            }
            // Check if an existing VR is saved in the FSDB with the same reference string. If so, re-generate id of the current resource.
            if let Ok(_) = self
                .db
                .get_resource(&resource.as_trait_object().reference_string(), &writer.profile)
            {
                resource.as_trait_object_mut().generate_and_update_resource_id();
            }

            // Now all validation checks/setup have passed, move forward with saving header/resource/source file
            let current_datetime = ShinkaiTime::generate_time_now();
            // Update the metadata keys of the FSItem node
            let mut node_metadata = node_metadata.unwrap_or_else(|| HashMap::new());
            node_metadata.insert(FSItem::vr_last_saved_metadata_key(), current_datetime.to_rfc3339());
            if let Some(sfm) = &source_file_map {
                // Last Saved SFM
                node_metadata.insert(
                    FSItem::source_file_map_last_saved_metadata_key(),
                    current_datetime.to_rfc3339(),
                );
                // SFM Size
                let sfm_size = sfm.encoded_size()?;
                node_metadata.insert(FSItem::source_file_map_size_metadata_key(), sfm_size.to_string());
            }
            // Update distribution_origin key in metadata
            node_metadata.insert(
                FSItem::distribution_origin_metadata_key(),
                distribution_origin.to_json()?,
            );
            // Update vr_size key in metadata
            let vr_size = resource.as_trait_object().encoded_size()?;
            node_metadata.insert(FSItem::vr_size_metadata_key(), vr_size.to_string());

            // Now after updating the metadata, finally save the VRHeader Node into the core vector resource
            {
                new_item = Some(self._add_vr_header_to_core_resource(
                    writer,
                    vr_header,
                    Some(node_metadata),
                    current_datetime,
                    node_at_path_already_exists,
                )?);
            }
        }

        // Finally saving the resource, the source file (if it was provided), and the FSInternals into the FSDB
        let mut write_batch = writer.new_write_batch()?;
        if let Some(sfm) = source_file_map {
            self.db
                .wb_save_source_file_map(&sfm, &source_db_key, &mut write_batch)?;
        }
        self.db.wb_save_resource(&resource, &mut write_batch)?;
        let internals = self._get_profile_fs_internals_read_only(&writer.profile)?;
        self.db.wb_save_profile_fs_internals(internals, &mut write_batch)?;
        self.db.write_pb(write_batch)?;

        if let Some(item) = new_item {
            Ok(item)
        } else {
            Err(VectorFSError::NoEntryAtPath(node_path))
        }
    }

    // /// Updates the SourceFile attached to a Vector Resource (FSItem) underneath the current path.
    // /// If no VR (FSItem) with the same name already exists underneath the current path, then errors.
    // pub fn update_source_file(&mut self,) {

    // Don't forget to update the sourcefile map last saved metadata key
    // if source_file_map.is_some() {
    //     node_metadata.insert(
    //         FSItem::source_file_map_last_saved_metadata_key(),
    //         current_datetime.to_rfc3339(),
    //     );
    // }

    // }

    // /// Attempts to update the FSItem at the VFSWriter's path by mutating it's SourceFileMap (if available)
    // /// by attaching the `original_creation_datetime` to the SourceFile at  `source_file_map_path` in the map.
    // pub fn update_source_file_original_creation_datetime(
    //     &mut self,
    //     writer: &VFSWriter,
    //     source_file_map_path: VRPath,
    //     original_creation_datetime: DateTime<Utc>,
    // ) -> Result<(), VectorFSError> {
    // }

    /// Internal method used to add a VRHeader into the core resource of a profile's VectorFS internals in memory.
    fn _add_vr_header_to_core_resource(
        &mut self,
        writer: &VFSWriter,
        vr_header: VRHeader,
        metadata: Option<HashMap<String, String>>,
        current_datetime: DateTime<Utc>,
        node_at_path_already_exists: bool,
    ) -> Result<FSItem, VectorFSError> {
        let internals = self._get_profile_fs_internals(&writer.profile)?;
        let new_node_path = writer.path.push_cloned(vr_header.resource_name.clone());

        // Mutator method for inserting the VR header and updating the last_modified metadata of parent folder
        let mut mutator = |node: &mut Node, embedding: &mut Embedding| -> Result<(), VRError> {
            // If no existing node is stored with the same id, then this is adding a new node so update last_modified key
            if !node_at_path_already_exists {
                node.metadata
                    .as_mut()
                    .map(|m| m.insert(FSFolder::last_modified_key(), current_datetime.to_rfc3339()));
            }
            // Setup the new node & insert it
            let node_id = vr_header.resource_name.clone();
            let resource = node.get_vector_resource_content_mut()?;
            let new_vr_header_node = Node::new_vr_header(node_id, &vr_header, metadata.clone(), &vec![]);
            let new_node_embedding = vr_header
                .resource_embedding
                .clone()
                .ok_or(VRError::NoEmbeddingProvided)?;
            resource.as_trait_object_mut().insert_node(
                vr_header.resource_name.clone(),
                new_vr_header_node,
                new_node_embedding,
                Some(current_datetime),
            )?;
            Ok(())
        };

        // If an embedding exists on the VR, and it is generated using the same embedding model
        if let Some(_) = vr_header.resource_embedding.clone() {
            if vr_header.resource_embedding_model_used == internals.default_embedding_model() {
                internals
                    .fs_core_resource
                    .mutate_node_at_path(writer.path.clone(), &mut mutator)?;
                // Update last read of the new FSItem
                internals.last_read_index.update_path_last_read(
                    new_node_path.clone(),
                    current_datetime,
                    writer.requester_name.clone(),
                );

                let retrieved_node = internals
                    .fs_core_resource
                    .retrieve_node_at_path(new_node_path.clone())?;
                let new_item = FSItem::from_vr_header_node(
                    retrieved_node.node,
                    new_node_path.clone(),
                    &internals.last_read_index,
                )?;
                Ok(new_item)
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

    /// Internal method used to add an existing VectorResource into the core resource of a profile's VectorFS internals in memory.
    /// Aka, add a folder into the VectorFS under the given path.
    fn _add_existing_vr_to_core_resource(
        &mut self,
        writer: &VFSWriter,
        resource: BaseVectorResource,
        embedding: Embedding,
        metadata: Option<HashMap<String, String>>,
        current_datetime: DateTime<Utc>,
    ) -> Result<FSFolder, VectorFSError> {
        let resource_name = resource.as_trait_object().name().to_string();
        let new_node_path = writer.path.push_cloned(resource_name.clone());
        // Check if anything exists at the new node's path and error if so (cannot overwrite an existing FSEntry)
        if let Ok(_) = self._validate_path_points_to_entry(new_node_path.clone(), &writer.profile) {
            return Err(VectorFSError::EntryAlreadyExistsAtPath(new_node_path));
        }

        // Fetch FSInternals
        let internals = self._get_profile_fs_internals(&writer.profile)?;

        // Check if parent is root, if so then direct insert into root and return, else proceed
        if writer.path.is_empty() {
            let new_node = Node::new_vector_resource(resource_name.clone(), &resource, metadata.clone());
            internals
                .fs_core_resource
                .insert_node(resource_name.clone(), new_node.clone(), embedding.clone(), None)?;
            // Update last read of the new FSFolder
            internals.last_read_index.update_path_last_read(
                new_node_path.clone(),
                current_datetime,
                writer.requester_name.clone(),
            );

            let folder = FSFolder::from_vector_resource_node(new_node, new_node_path, &internals.last_read_index)?;
            return Ok(folder);
        }

        // Mutator method for inserting the VR and updating the last_modified metadata of parent folder
        let mut mutator = |node: &mut Node, _: &mut Embedding| -> Result<(), VRError> {
            // Update last_modified key of the parent folder
            node.metadata
                .as_mut()
                .map(|m| m.insert(FSFolder::last_modified_key(), current_datetime.to_rfc3339()));
            // Create the new folder child node and insert it
            let new_node = Node::new_vector_resource(resource_name.clone(), &resource, metadata.clone());
            let resource = node.get_vector_resource_content_mut()?;
            resource
                .as_trait_object_mut()
                .insert_node(resource_name.clone(), new_node, embedding.clone(), None)?;
            Ok(())
        };

        internals
            .fs_core_resource
            .mutate_node_at_path(writer.path.clone(), &mut mutator)?;
        // Update last read of the new FSFolder
        internals.last_read_index.update_path_last_read(
            new_node_path.clone(),
            current_datetime,
            writer.requester_name.clone(),
        );

        let retrieved_node = internals
            .fs_core_resource
            .retrieve_node_at_path(new_node_path.clone())?;
        let folder =
            FSFolder::from_vector_resource_node(retrieved_node.node, new_node_path, &internals.last_read_index)?;

        Ok(folder)
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
}
