use super::vector_fs_permissions::PathPermission;
use super::vector_fs_types::{FSEntry, FSFolder, FSItem};
use super::{vector_fs::VectorFS, vector_fs_error::VectorFSError, vector_fs_reader::VFSReader};
use crate::db::db_profile_bound::ProfileBoundWriteBatch;
use crate::vector_fs::vector_fs_permissions::{ReadPermission, WritePermission};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::shinkai_time::ShinkaiTime;
use shinkai_vector_resources::source::SourceFileMap;
use shinkai_vector_resources::vector_resource::VectorResourceCore;
use shinkai_vector_resources::vector_resource::{NodeContent, RetrievedNode, SourceFileType, VRKai, VRPack};
use shinkai_vector_resources::{
    embeddings::Embedding,
    vector_resource::{BaseVectorResource, MapVectorResource, Node, VRHeader, VRPath, VRSourceReference},
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// A struct that represents having rights to write to the VectorFS under a profile/at a specific path.
/// If a VFSWriter struct is constructed, that means the `requester_name` has passed
/// permissions validation and is thus allowed to write to `path`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VFSWriter {
    pub requester_name: ShinkaiName,
    pub path: VRPath,
    pub profile: ShinkaiName,
}

impl VFSWriter {
    /// Creates a new VFSWriter if the `requester_name` passes write permission validation check.
    pub async fn new(
        requester_name: ShinkaiName,
        path: VRPath,
        vector_fs: &VectorFS,
        profile: ShinkaiName,
    ) -> Result<Self, VectorFSError> {
        let writer = VFSWriter {
            requester_name: requester_name.clone(),
            path: path.clone(),
            profile: profile.clone(),
        };

        // Validate profile ShinkaiName has an actual profile inside
        if profile.extract_profile().is_err() {
            return Err(VectorFSError::ProfileNameNonExistent(profile.to_string()));
        }

        // Validate write permissions to ensure requester_name has rights
        vector_fs
            .validate_write_access_for_paths(profile.clone(), requester_name.clone(), vec![path.clone()])
            .await
            .map_err(|_| {
                VectorFSError::InvalidWriterPermission(requester_name.clone(), profile.clone(), path.clone())
            })?;

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
    pub async fn new_reader_copied_data(&self, path: VRPath, vector_fs: &VectorFS) -> Result<VFSReader, VectorFSError> {
        VFSReader::new(self.requester_name.clone(), path, vector_fs, self.profile.clone()).await
    }

    /// Generates a VFSWriter using the same requester_name/profile held in self.
    /// Write permissions are verified before the VFSWriter is produced.
    pub async fn new_writer_copied_data(&self, path: VRPath, vector_fs: &VectorFS) -> Result<VFSWriter, VectorFSError> {
        VFSWriter::new(self.requester_name.clone(), path, vector_fs, self.profile.clone()).await
    }

    /// Generates a new empty ProfileBoundWiteBatch using the profile in the Writer
    fn new_write_batch(&self) -> Result<ProfileBoundWriteBatch, VectorFSError> {
        ProfileBoundWriteBatch::new_vfs_batch(&self.profile)
    }
}

impl VectorFS {
    /// Copies the FSFolder from the writer's path into being held underneath the destination_path.
    pub async fn copy_folder(&self, writer: &VFSWriter, destination_path: VRPath) -> Result<FSFolder, VectorFSError> {
        let write_batch = writer.new_write_batch()?;
        let (write_batch, new_folder) = self
            .internal_wb_copy_folder(writer, destination_path, write_batch, false)
            .await?;
        self.db.write_pb(write_batch)?;
        Ok(new_folder)
    }

    /// Internal method to copy the FSFolder from the writer's path into being held underneath the destination_path.
    fn internal_wb_copy_folder<'a>(
        &'a self,
        writer: &'a VFSWriter,
        destination_path: VRPath,
        mut write_batch: ProfileBoundWriteBatch,
        is_recursive_call: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(ProfileBoundWriteBatch, FSFolder), VectorFSError>> + Send + 'a>> {
        Box::pin(async move {
            let current_datetime = ShinkaiTime::generate_time_now();
            let destination_writer = writer.new_writer_copied_data(destination_path.clone(), self).await?;

            // Ensure paths are valid before proceeding
            self.validate_path_points_to_folder(writer.path.clone(), &writer.profile)
                .await?;
            if &destination_path != &VRPath::root() {
                self.validate_path_points_to_folder(destination_path.clone(), &writer.profile)
                    .await?;
            }
            let destination_child_path = destination_path.push_cloned(writer.path.last_path_id()?);
            if self
                .validate_path_points_to_entry(destination_child_path.clone(), &writer.profile)
                .await
                .is_ok()
            {
                return Err(VectorFSError::CannotOverwriteFSEntry(destination_child_path.clone()));
            }

            // Get the existing folder
            let (folder_ret_node, embedding) = self._get_node_from_core_resource(writer).await?;
            let metadata = folder_ret_node.node.metadata.clone();
            let mut folder_resource = folder_ret_node.node.get_vector_resource_content()?.clone();
            // Backup tag index, remove nodes/embeddings, and then reapply tag index
            let cloned_tag_index = folder_resource.as_trait_object().get_data_tag_index().clone();
            let nodes_embeddings = folder_resource.as_trait_object_mut().remove_root_nodes()?;
            folder_resource
                .as_trait_object_mut()
                .set_data_tag_index(cloned_tag_index);

            // We insert the emptied folder resource into the destination path, and copy permissions
            self._add_existing_vr_to_core_resource(
                &destination_writer,
                folder_resource,
                embedding,
                metadata,
                current_datetime,
            )
            .await?;
            {
                let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
                internals
                    .permissions_index
                    .copy_path_permission(writer.path.clone(), destination_path.clone())
                    .await?;
                self._update_fs_internals(writer.profile.clone(), internals).await?;
            }

            // Determine and copy permissions from the parent of the new copied folder
            let parent_path = destination_path.parent_path();
            let (read_permission, write_permission) = if parent_path == VRPath::root() {
                (ReadPermission::Private, WritePermission::Private)
            } else {
                let parent_permissions = self
                    .get_profile_fs_internals_cloned(&writer.profile)
                    .await?
                    .permissions_index
                    .get_path_permission(&parent_path)
                    .await
                    .unwrap_or(PathPermission {
                        read_permission: ReadPermission::Private,
                        write_permission: WritePermission::Private,
                        whitelist: HashMap::new(),
                    });
                (parent_permissions.read_permission, parent_permissions.write_permission)
            };

            // Set permissions for the new copied folder
            {
                let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
                internals
                    .permissions_index
                    .insert_path_permission(destination_child_path.clone(), read_permission, write_permission)
                    .await?;
                self._update_fs_internals(writer.profile.clone(), internals).await?;
            }

            // Now we copy each of the folder's original child folders/items (nodes) and add them to their destination path
            for (node, _) in nodes_embeddings {
                let origin_writer = writer
                    .new_writer_copied_data(writer.path.push_cloned(node.id.clone()), self)
                    .await?;
                let dest_path = destination_child_path.clone();
                match node.content {
                    NodeContent::Resource(_) => {
                        let (batch, _) = self
                            .internal_wb_copy_folder(&origin_writer, dest_path, write_batch, true)
                            .await?;
                        write_batch = batch;
                    }
                    NodeContent::VRHeader(_) => {
                        let (batch, _) = self.wb_copy_item(&origin_writer, dest_path, write_batch).await?;
                        write_batch = batch;
                    }
                    _ => continue,
                }
            }

            // Only commit updating the fs internals once at the top level, efficiency improvement
            if !is_recursive_call {
                let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
                self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
            }

            // Fetch the new FSFolder after everything has been copied over in fs internals
            let reader = destination_writer
                .new_reader_copied_data(destination_child_path.clone(), self)
                .await?;
            let fs_entry = self.retrieve_fs_entry(&reader).await?;

            match fs_entry {
                FSEntry::Folder(new_folder) => Ok((write_batch, new_folder)),
                _ => Err(VectorFSError::PathDoesNotPointAtFolder(destination_child_path)),
            }
        })
    }

    /// Deletes the folder at writer's path, including all items and subfolders within.
    pub async fn delete_folder(&self, writer: &VFSWriter) -> Result<(), VectorFSError> {
        let mut write_batch = writer.new_write_batch()?;
        write_batch = self.internal_wb_delete_folder(writer, write_batch, false).await?;
        self.db.write_pb(write_batch)?;
        Ok(())
    }

    /// Deletes the folder at writer's path, including all items and subfolders within, using a write batch.
    pub async fn wb_delete_folder(
        &self,
        writer: &VFSWriter,
        write_batch: ProfileBoundWriteBatch,
    ) -> Result<ProfileBoundWriteBatch, VectorFSError> {
        self.internal_wb_delete_folder(writer, write_batch, false).await
    }

    /// Internal method that deletes the folder at writer's path, including all items and subfolders within, using a write batch.
    fn internal_wb_delete_folder<'a>(
        &'a self,
        writer: &'a VFSWriter,
        mut write_batch: ProfileBoundWriteBatch,
        is_recursive_call: bool,
    ) -> Pin<Box<dyn Future<Output = Result<ProfileBoundWriteBatch, VectorFSError>> + Send + 'a>> {
        Box::pin(async move {
            self.validate_path_points_to_folder(writer.path.clone(), &writer.profile)
                .await?;

            // Read the folder node first without removing it
            let (folder_node, _) = self._get_node_from_core_resource(writer).await?;
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            let folder =
                FSFolder::from_vector_resource_node(folder_node.node, writer.path.clone(), &internals.last_read_index)?;

            // Iterate over items in the folder and delete each
            for item in folder.child_items {
                let item_writer = VFSWriter {
                    requester_name: writer.requester_name.clone(),
                    path: writer.path.push_cloned(item.name.clone()),
                    profile: writer.profile.clone(),
                };
                write_batch = self.wb_delete_item(&item_writer, write_batch).await?;
            }

            // Recursively delete subfolders
            for subfolder in folder.child_folders {
                let folder_writer = VFSWriter {
                    requester_name: writer.requester_name.clone(),
                    path: writer.path.push_cloned(subfolder.name.clone()),
                    profile: writer.profile.clone(),
                };
                write_batch = self
                    .internal_wb_delete_folder(&folder_writer, write_batch, true)
                    .await?;
            }

            // Now remove the folder node from the core resource and remove its path permissions
            let (_removed_folder_node, _) = self._remove_node_from_core_resource(writer).await?;
            {
                let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
                internals
                    .permissions_index
                    .remove_path_permission(folder.path.clone())
                    .await;
                self._update_fs_internals(writer.profile.clone(), internals).await?;
            }

            // Only commit updating the fs internals once at the top level, efficiency improvement
            // TODO: Efficiency, have each recursive call return the list of folder/item paths to delete in the permissions index, and do it all just once here
            if !is_recursive_call {
                let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
                self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
            }

            Ok(write_batch)
        })
    }

    /// Deletes the FSItem at the writer's path.
    pub async fn delete_item(&self, writer: &VFSWriter) -> Result<(), VectorFSError> {
        let mut write_batch = writer.new_write_batch()?;
        write_batch = self.wb_delete_item(writer, write_batch).await?;
        self.db.write_pb(write_batch)?;
        Ok(())
    }

    /// Deletes the item at writer's path, within a write batch.
    pub async fn wb_delete_item(
        &self,
        writer: &VFSWriter,
        mut write_batch: ProfileBoundWriteBatch,
    ) -> Result<ProfileBoundWriteBatch, VectorFSError> {
        self.validate_path_points_to_item(writer.path.clone(), &writer.profile)
            .await?;
        let (item_node, _) = self._remove_node_from_core_resource(writer).await?;
        let ref_string = item_node.get_vr_header_content()?.reference_string();

        let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
        internals
            .permissions_index
            .remove_path_permission(writer.path.clone())
            .await;
        self._update_fs_internals(writer.profile.clone(), internals.clone())
            .await?;
        self.db.wb_delete_resource(&ref_string, &mut write_batch)?;
        self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
        Ok(write_batch)
    }

    /// Copies the FSItem from the writer's path into being held underneath the destination_path.
    /// Does not support copying into VecFS root.
    pub async fn copy_item(&self, writer: &VFSWriter, destination_path: VRPath) -> Result<FSItem, VectorFSError> {
        let write_batch = writer.new_write_batch()?;
        let (write_batch, new_item) = self.wb_copy_item(writer, destination_path, write_batch).await?;
        self.db.write_pb(write_batch)?;
        Ok(new_item)
    }

    /// Copy the FSItem from the writer's path into being held underneath the destination_path.
    /// Does not support copying into VecFS root.
    pub async fn wb_copy_item(
        &self,
        writer: &VFSWriter,
        destination_path: VRPath,
        mut write_batch: ProfileBoundWriteBatch,
    ) -> Result<(ProfileBoundWriteBatch, FSItem), VectorFSError> {
        let current_datetime = ShinkaiTime::generate_time_now();
        let destination_writer = writer.new_writer_copied_data(destination_path.clone(), self).await?;

        // Ensure paths are valid before proceeding
        self.validate_path_points_to_item(writer.path.clone(), &writer.profile)
            .await?;
        self.validate_path_points_to_folder(destination_path.clone(), &writer.profile)
            .await?;
        let destination_child_path = destination_path.push_cloned(writer.path.last_path_id()?);
        if self
            .validate_path_points_to_entry(destination_child_path.clone(), &writer.profile)
            .await
            .is_ok()
        {
            return Err(VectorFSError::CannotOverwriteFSEntry(destination_child_path.clone()));
        }

        // Get the existing item
        let (item_ret_node, _) = self._get_node_from_core_resource(writer).await?;
        let item_metadata = item_ret_node.node.metadata;
        let mut source_file_map = None;
        let source_file_map_is_saved = item_metadata
            .as_ref()
            .and_then(|metadata| metadata.get(&FSItem::source_file_map_last_saved_metadata_key()))
            .map_or(false, |_| true);

        // Fetch the VR and SFM from the DB
        let reader = writer.new_reader_copied_data(writer.path.clone(), self).await?;
        if source_file_map_is_saved {
            source_file_map = Some(self.retrieve_source_file_map(&reader).await?);
        }
        let mut vector_resource = self.retrieve_vector_resource(&reader).await?;
        // Generate a new VR id for the resource, and generate a new header
        vector_resource.as_trait_object_mut().generate_and_update_resource_id();
        let header = vector_resource.as_trait_object().generate_resource_header();
        let source_db_key = header.reference_string();

        // Save the copied item w/new resource id into the new destination w/permissions
        let new_item = self
            ._add_vr_header_to_core_resource(&destination_writer, header, item_metadata, current_datetime, false)
            .await?;

        // Determine and set permissions based on the parent of the destination path
        let parent_path = destination_path.parent_path();
        let (read_permission, write_permission) = if parent_path == VRPath::root() {
            (ReadPermission::Private, WritePermission::Private)
        } else {
            let parent_permissions = self
                .get_profile_fs_internals_cloned(&writer.profile)
                .await?
                .permissions_index
                .get_path_permission(&parent_path)
                .await
                .unwrap_or(PathPermission {
                    read_permission: ReadPermission::Private,
                    write_permission: WritePermission::Private,
                    whitelist: HashMap::new(),
                });
            (parent_permissions.read_permission, parent_permissions.write_permission)
        };

        // Set permissions for the new copied item
        {
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            internals
                .permissions_index
                .insert_path_permission(new_item.path.clone(), read_permission, write_permission)
                .await?;
            self._update_fs_internals(writer.profile.clone(), internals).await?;
        }

        // Save fs internals, new VR, and new SFM to the DB
        if let Some(sfm) = source_file_map {
            self.db
                .wb_save_source_file_map(&sfm, &source_db_key, &mut write_batch)?;
        }
        self.db.wb_save_resource(&vector_resource, &mut write_batch)?;
        let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
        self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;

        Ok((write_batch, new_item))
    }

    /// Moves the FSItem from the writer's path into being held underneath the destination_path.
    /// Does not support moving into VecFS root.
    pub async fn move_item(&self, writer: &VFSWriter, destination_path: VRPath) -> Result<FSItem, VectorFSError> {
        let current_datetime = ShinkaiTime::generate_time_now();
        let destination_writer = writer.new_writer_copied_data(destination_path.clone(), self).await?;

        // Ensure paths are valid before proceeding
        self.validate_path_points_to_item(writer.path.clone(), &writer.profile)
            .await?;
        self.validate_path_points_to_folder(destination_path.clone(), &writer.profile)
            .await?;
        let destination_child_path = destination_path.push_cloned(writer.path.last_path_id()?);
        if self
            .validate_path_points_to_entry(destination_child_path.clone(), &writer.profile)
            .await
            .is_ok()
        {
            return Err(VectorFSError::CannotOverwriteFSEntry(destination_child_path.clone()));
        }

        // If the item was moved successfully in memory, then commit to the DB
        let move_result = self
            ._internal_move_item(writer, &destination_writer, current_datetime, destination_path)
            .await;
        if let Ok(new_item) = move_result {
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            let mut write_batch = writer.new_write_batch()?;
            self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
            self.db.write_pb(write_batch)?;
            Ok(new_item)
        }
        // Else if it was not successful in memory, reload fs internals from db to revert changes and return error
        else {
            self.revert_internals_to_last_db_save(&writer.profile, &writer.profile)
                .await?;
            move_result
        }
    }

    /// Internal method which moves the item at writer's path into destination_writer's path (in memory only)
    async fn _internal_move_item(
        &self,
        writer: &VFSWriter,
        destination_writer: &VFSWriter,
        current_datetime: DateTime<Utc>,
        destination_path: VRPath,
    ) -> Result<FSItem, VectorFSError> {
        // Remove the existing item
        let (item_node, _) = self._remove_node_from_core_resource(writer).await?;
        let header = item_node.get_vr_header_content()?.clone();
        let item_metadata = item_node.metadata;
        // And save the item into the new destination w/permissions
        let new_item = self
            ._add_vr_header_to_core_resource(destination_writer, header, item_metadata, current_datetime, false)
            .await?;

        // Determine and set permissions based on the parent of the destination path
        let (read_permission, write_permission) = if destination_path == VRPath::root() {
            (ReadPermission::Private, WritePermission::Private)
        } else {
            let parent_permissions = self
                .get_profile_fs_internals_cloned(&writer.profile)
                .await?
                .permissions_index
                .get_path_permission(&destination_path)
                .await
                .unwrap_or(PathPermission {
                    read_permission: ReadPermission::Private,
                    write_permission: WritePermission::Private,
                    whitelist: HashMap::new(),
                });
            (parent_permissions.read_permission, parent_permissions.write_permission)
        };

        // Set permissions for the new moved item
        {
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            internals
                .permissions_index
                .insert_path_permission(new_item.path.clone(), read_permission, write_permission)
                .await?;
            // Remove the original item's permissions
            internals
                .permissions_index
                .remove_path_permission(writer.path.clone())
                .await;
            self._update_fs_internals(writer.profile.clone(), internals).await?;
        }
        Ok(new_item)
    }

    /// Moves the FSFolder from the writer's path into being held underneath the destination_path.
    /// Supports moving into VecFS root.
    pub async fn move_folder(&self, writer: &VFSWriter, destination_path: VRPath) -> Result<FSFolder, VectorFSError> {
        let current_datetime = ShinkaiTime::generate_time_now();
        let destination_writer = writer.new_writer_copied_data(destination_path.clone(), self).await?;

        // Ensure paths are valid before proceeding
        self.validate_path_points_to_folder(writer.path.clone(), &writer.profile)
            .await?;
        if &destination_path != &VRPath::root() {
            self.validate_path_points_to_folder(destination_path.clone(), &writer.profile)
                .await?;
        }

        let destination_child_path = destination_path.push_cloned(writer.path.last_path_id()?);
        if self
            .validate_path_points_to_entry(destination_child_path.clone(), &writer.profile)
            .await
            .is_ok()
        {
            return Err(VectorFSError::CannotOverwriteFSEntry(destination_child_path.clone()));
        }

        // Make sure we don't partially copy the folder into itself before failing
        if writer.path.is_descendant_path(&destination_child_path) {
            return Err(VectorFSError::CannotMoveFolderIntoItself(writer.path.clone()));
        }

        // If the folder was moved successfully in memory, then commit to the DB
        let move_result = self
            .internal_move_folder(writer, &destination_writer, current_datetime, destination_path)
            .await;
        if let Ok(new_folder) = move_result {
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            let mut write_batch = writer.new_write_batch()?;
            self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
            self.db.write_pb(write_batch)?;
            Ok(new_folder)
        }
        // Else if it was not successful in memory, reload fs internals from db to revert changes and return error
        else {
            self.revert_internals_to_last_db_save(&writer.profile, &writer.profile)
                .await?;
            Ok(move_result?)
        }
    }

    /// Internal method which moves the folder at writer's path into destination_writer's path (in memory only)
    async fn internal_move_folder(
        &self,
        writer: &VFSWriter,
        destination_writer: &VFSWriter,
        current_datetime: DateTime<Utc>,
        destination_path: VRPath,
    ) -> Result<FSFolder, VectorFSError> {
        // Copy the folder to the new destination
        let new_folder = self
            .internal_copy_folder(writer, destination_writer, current_datetime)
            .await?;

        // Determine and set permissions based on the parent of the destination path
        let (read_permission, write_permission) = if destination_path == VRPath::root() {
            (ReadPermission::Private, WritePermission::Private)
        } else {
            let parent_permissions = self
                .get_profile_fs_internals_cloned(&writer.profile)
                .await?
                .permissions_index
                .get_path_permission(&destination_path)
                .await
                .unwrap_or(PathPermission {
                    read_permission: ReadPermission::Private,
                    write_permission: WritePermission::Private,
                    whitelist: HashMap::new(),
                });
            (parent_permissions.read_permission, parent_permissions.write_permission)
        };

        // Set permissions for the new moved folder
        {
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            internals
                .permissions_index
                .insert_path_permission(new_folder.path.clone(), read_permission, write_permission)
                .await?;
            // Remove the original folder's permissions
            internals
                .permissions_index
                .remove_path_permission(writer.path.clone())
                .await;
            self._update_fs_internals(writer.profile.clone(), internals).await?;
        }

        // Remove the existing folder
        self._remove_node_from_core_resource(writer).await?;

        Ok(new_folder)
    }

    /// Internal method which copies the folder at writer's path into destination_writer's path (in memory only)
    async fn internal_copy_folder(
        &self,
        writer: &VFSWriter,
        destination_writer: &VFSWriter,
        current_datetime: DateTime<Utc>,
    ) -> Result<FSFolder, VectorFSError> {
        // Get the existing folder
        let (folder_node, folder_embedding) = self._get_node_from_core_resource(writer).await?;
        let folder_resource = folder_node.node.get_vector_resource_content()?.clone();
        let folder_metadata = folder_node.node.metadata;

        // Save the folder into the new destination w/permissions
        let new_folder = self
            ._add_existing_vr_to_core_resource(
                destination_writer,
                folder_resource,
                folder_embedding,
                folder_metadata,
                current_datetime,
            )
            .await?;
        {
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            internals
                .permissions_index
                .copy_path_permission(writer.path.clone(), new_folder.path.clone())
                .await?;
            self._update_fs_internals(writer.profile.clone(), internals).await?;
        }
        Ok(new_folder)
    }

    /// Automatically creates new FSFolders along the given path that do not exist, including the final path id (aka. don't supply an FSItem's path, use its parent path).
    /// Returns a Vec<VRPath> containing the paths of the newly created folders.
    pub async fn create_new_folder_auto(&self, writer: &VFSWriter, path: VRPath) -> Result<Vec<VRPath>, VectorFSError> {
        let mut current_path = VRPath::root();
        let mut created_folders = Vec::new();
        for segment in path.path_ids {
            current_path.push(segment.clone());
            if self
                .validate_path_points_to_entry(current_path.clone(), &writer.profile)
                .await
                .is_err()
            {
                let new_writer = writer.new_writer_copied_data(current_path.pop_cloned(), self).await?;
                self.create_new_folder(&new_writer, &segment).await?;
                created_folders.push(current_path.clone());
            }
        }
        Ok(created_folders)
    }

    /// Creates a new FSFolder underneath the writer's path. Errors if the path in `writer` does not exist.
    pub async fn create_new_folder(
        &self,
        writer: &VFSWriter,
        new_folder_name: &str,
    ) -> Result<FSFolder, VectorFSError> {
        // Create a new MapVectorResource which represents a folder
        let current_datetime = ShinkaiTime::generate_time_now();
        let new_vr = BaseVectorResource::Map(MapVectorResource::new_empty(
            new_folder_name,
            None,
            VRSourceReference::None,
            true,
        ));
        let embedding = Embedding::new_empty(); // Empty embedding as folders do not score in VecFS search

        // Setup default metadata for new folder node
        let mut metadata = HashMap::new();
        metadata.insert(FSFolder::last_modified_key(), current_datetime.to_rfc3339());

        // Call the new method to save the existing folder
        self.internal_save_folder(writer, new_vr, embedding, Some(metadata), current_datetime)
            .await
    }

    /// Internal method which saves a FSFolder into the writer's path.
    async fn internal_save_folder(
        &self,
        writer: &VFSWriter,
        new_vr: BaseVectorResource,
        embedding: Embedding,
        metadata: Option<HashMap<String, String>>,
        current_datetime: DateTime<Utc>,
    ) -> Result<FSFolder, VectorFSError> {
        // Add the folder into the internals
        let new_folder = self
            ._add_existing_vr_to_core_resource(writer, new_vr, embedding, metadata, current_datetime)
            .await?;
        let new_folder_path = new_folder.path.clone();

        // Determine permissions based on whether the parent folder is root
        let read_permission;
        let write_permission;
        if writer.path == VRPath::root() {
            read_permission = ReadPermission::Private;
            write_permission = WritePermission::Private;
        } else {
            // Read the permissions of the parent folder
            let parent_permissions = self
                .get_profile_fs_internals_cloned(&writer.profile)
                .await?
                .permissions_index
                .get_path_permission(&writer.path)
                .await
                .unwrap_or(PathPermission {
                    read_permission: ReadPermission::Private,
                    write_permission: WritePermission::Private,
                    whitelist: HashMap::new(),
                });
            read_permission = parent_permissions.read_permission;
            write_permission = parent_permissions.write_permission;
        }

        // Add read/write permission for the folder path
        {
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            internals
                .permissions_index
                .insert_path_permission(new_folder_path, read_permission, write_permission)
                .await?;
            self._update_fs_internals(writer.profile.clone(), internals).await?;
        }

        // Save the FSInternals into the FSDB
        let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
        let mut write_batch = writer.new_write_batch()?;
        self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
        self.db.write_pb(write_batch)?;

        Ok(new_folder)
    }

    /// Updates the permissions of a folder, all its subfolders, and subitems recursively.
    pub fn update_permissions_recursively<'a>(
        &'a self,
        writer: &'a VFSWriter,
        read_permission: ReadPermission,
        write_permission: WritePermission,
    ) -> Pin<Box<dyn Future<Output = Result<(), VectorFSError>> + Send + 'a>> {
        Box::pin(async move {
            // Ensure the path points to a folder before proceeding
            self.validate_path_points_to_folder(writer.path.clone(), &writer.profile)
                .await?;

            // Retrieve the folder node
            let (folder_node, _) = self._get_node_from_core_resource(writer).await?;
            let mut folder_resource = folder_node.node.get_vector_resource_content()?.clone();

            // Update permissions for the current folder
            {
                let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
                internals
                    .permissions_index
                    .insert_path_permission(writer.path.clone(), read_permission.clone(), write_permission.clone())
                    .await?;
                self._update_fs_internals(writer.profile.clone(), internals).await?;
            }

            // Recursively update permissions for all child nodes
            if let NodeContent::Resource(_) = folder_node.node.content {
                let nodes_embeddings = folder_resource.as_trait_object_mut().remove_root_nodes()?;
                for (node, _) in nodes_embeddings {
                    let child_writer = writer
                        .new_writer_copied_data(writer.path.push_cloned(node.id.clone()), self)
                        .await?;
                    match node.content {
                        NodeContent::Resource(_res) => {
                            self.update_permissions_recursively(
                                &child_writer,
                                read_permission.clone(),
                                write_permission.clone(),
                            )
                            .await?;
                        }
                        NodeContent::VRHeader(_) => {
                            let internals = self.get_profile_fs_internals_cloned(&child_writer.profile).await?;
                            internals
                                .permissions_index
                                .insert_path_permission(
                                    child_writer.path.clone(),
                                    read_permission.clone(),
                                    write_permission.clone(),
                                )
                                .await?;
                            self._update_fs_internals(writer.profile.clone(), internals).await?;
                        }
                        _ => continue,
                    }
                }
            }

            // Save the FSInternals into the FSDB
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            let mut write_batch = writer.new_write_batch()?;
            self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
            self.db.write_pb(write_batch)?;

            Ok(())
        })
    }

    /// Extracts the VRPack into the VectorFS underneath the folder specified in the writer's path. Uses the VRPack's name
    /// as the folder name which everything gets extracted into.
    pub async fn extract_vrpack_in_folder(&self, writer: &VFSWriter, vrpack: VRPack) -> Result<(), VectorFSError> {
        // Construct the base path for the VRPack extraction
        let base_path = writer.path.clone();

        // Check if an entry already exists at vec_fs_base_path
        let unpack_destination_path = base_path.push_cloned(vrpack.name.to_string());
        if self
            .validate_path_points_to_entry(unpack_destination_path.clone(), &writer.profile)
            .await
            .is_ok()
        {
            return Err(VectorFSError::CannotOverwriteFSEntry(unpack_destination_path.clone()));
        }

        let vrkais_with_paths = vrpack.unpack_all_vrkais()?;
        let mut folder_merkle_hash_map: HashMap<VRPath, String> = HashMap::new();

        for (vrkai, path) in vrkais_with_paths {
            let parent_folder_path = base_path.append_path_cloned(&path.parent_path());
            let parent_folder_writer = writer.new_writer_copied_data(parent_folder_path.clone(), self).await?;
            // Create the folders
            let new_folders = self
                .create_new_folder_auto(&parent_folder_writer, parent_folder_path.clone())
                .await?;
            // Save the VRKai in its final location
            self.save_vrkai_in_folder(&parent_folder_writer, vrkai).await?;

            // Now update the folder merkle hash map
            for full_folder_path in new_folders {
                // Remove the base path from the full folder path
                let mut folder_path = full_folder_path.clone();
                for _ in base_path.path_ids.iter() {
                    folder_path.front_pop();
                }
                // Skip if empty
                if folder_path.path_ids.is_empty() {
                    continue;
                }
                if folder_merkle_hash_map.get(&folder_path).is_none() {
                    let merkle_hash = vrpack.get_folder_merkle_hash(folder_path.clone())?;
                    folder_merkle_hash_map.insert(full_folder_path, merkle_hash);
                }
            }
        }

        // Sets the Merkle hashes for a collection of folders.
        self._set_folders_merkle_hashes(
            writer,
            folder_merkle_hash_map
                .iter()
                .map(|(path, hash)| (path.clone(), hash.clone()))
                .collect(),
            base_path.clone(),
        )
        .await?;

        Ok(())
    }

    /// Internal method which sets the merkle hashes of folders in the VectorFS
    /// without updating any other folder merkle hashes during setting.
    /// Then once done setting all merkle hashes, updates the merkle hashes of all folders
    /// from base_path and upwards.
    async fn _set_folders_merkle_hashes(
        &self,
        writer: &VFSWriter,
        folder_paths_with_hashes: Vec<(VRPath, String)>,
        base_path: VRPath,
    ) -> Result<(), VectorFSError> {
        {
            // Fetch the profile's file system internals
            let mut internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;

            // Iterate over each folder path and its corresponding Merkle hash
            for (folder_path, merkle_hash) in folder_paths_with_hashes {
                // Use the core resource to set the Merkle hash at the specified path
                internals
                    .fs_core_resource
                    ._set_resource_merkle_hash_at_path(folder_path, merkle_hash)
                    .map_err(VectorFSError::from)?; // Convert VRError to VectorFSError as needed
            }

            // Once done with merkle hash updates, update the merkle hashes of all folders from base_path and upwards.
            internals
                .fs_core_resource
                ._update_resource_merkle_hash_at_path(base_path, true)?;
            self._update_fs_internals(writer.profile.clone(), internals).await?;
        }

        // Finally saving the profile fs internals
        let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
        let mut write_batch = writer.new_write_batch()?;
        self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
        self.db.write_pb(write_batch)?;
        Ok(())
    }

    /// Saves a VRKai into an FSItem at the exact path specified in writer (ie. `.../{parent_folder}/resource_name`)
    /// If a FSItem already exists underneath the writer's path, then updates(overwrites) it.
    /// Does not support saving into VecFS root.
    pub async fn save_vrkai(&self, writer: &VFSWriter, vrkai: VRKai) -> Result<FSItem, VectorFSError> {
        let parent_folder_path = writer.path.pop_cloned();
        let new_writer = writer.new_writer_copied_data(parent_folder_path.clone(), self).await?;
        self.save_vrkai_in_folder(&new_writer, vrkai).await
    }

    /// Saves a VRKai into an FSItem, underneath the FSFolder at the writer's path.
    /// If a FSItem with the same name (as the VR) already exists underneath the current path, then updates(overwrites) it.
    /// Does not support saving into VecFS root.
    pub async fn save_vrkai_in_folder(&self, writer: &VFSWriter, vrkai: VRKai) -> Result<FSItem, VectorFSError> {
        self.save_vector_resource_in_folder(writer, vrkai.resource, vrkai.sfm)
            .await
    }

    /// Saves a Vector Resource and optional SourceFile into an FSItem at the exact path specified in writer (ie. `.../{parent_folder}/resource_name`)
    /// If a FSItem already exists underneath the writer's path, then updates(overwrites) it.
    /// Does not support saving into VecFS root.
    pub async fn save_vector_resource(
        &self,
        writer: &VFSWriter,
        resource: BaseVectorResource,
        source_file_map: Option<SourceFileMap>,
    ) -> Result<FSItem, VectorFSError> {
        let parent_folder_path = writer.path.pop_cloned();
        let new_writer = writer.new_writer_copied_data(parent_folder_path.clone(), self).await?;
        self.save_vector_resource_in_folder(&new_writer, resource, source_file_map)
            .await
    }

    /// Saves a Vector Resource and optional SourceFile into an FSItem, underneath the FSFolder at the writer's path.
    /// If a FSItem with the same name (as the VR) already exists underneath the current path, then updates(overwrites) it.
    /// Does not support saving into VecFS root.
    pub async fn save_vector_resource_in_folder(
        &self,
        writer: &VFSWriter,
        resource: BaseVectorResource,
        source_file_map: Option<SourceFileMap>,
    ) -> Result<FSItem, VectorFSError> {
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
            // Ensure path of writer points at a folder before proceeding
            self.validate_path_points_to_folder(writer.path.clone(), &writer.profile)
                .await?;
            // If an existing FSFolder is already saved at the node path, return error.
            if let Ok(_) = self
                .validate_path_points_to_folder(node_path.clone(), &writer.profile)
                .await
            {
                return Err(VectorFSError::CannotOverwriteFolder(node_path.clone()));
            }
            // If an existing FSItem is saved at the node path
            let mut existing_vr_ref = None;
            if let Ok(_) = self
                .validate_path_points_to_item(node_path.clone(), &writer.profile)
                .await
            {
                if let Ok(ret_node) = self
                    ._retrieve_core_resource_node_at_path(node_path.clone(), &writer.profile)
                    .await
                {
                    node_metadata.clone_from(&ret_node.node.metadata);
                    node_at_path_already_exists = true;
                    if let Ok(vr_header) = ret_node.node.get_vr_header_content() {
                        existing_vr_ref = Some(vr_header.reference_string());
                    }
                }
            }

            // Check if an existing VR is saved in the FSDB with the same reference string. If so, re-generate id of the current resource.
            if existing_vr_ref != Some(resource.as_trait_object().reference_string()) {
                if let Ok(r) = self
                    .db
                    .get_resource(&resource.as_trait_object().reference_string(), &writer.profile)
                {
                    resource.as_trait_object_mut().generate_and_update_resource_id();
                }
            }

            // Now all validation checks/setup have passed, move forward with saving header/resource/source file
            let current_datetime = ShinkaiTime::generate_time_now();
            // Update the metadata keys of the FSItem node
            let mut node_metadata = node_metadata.unwrap_or_else(HashMap::new);
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
            // Update vr_size key in metadata
            let vr_size = resource.as_trait_object().encoded_size()?;
            node_metadata.insert(FSItem::vr_size_metadata_key(), vr_size.to_string());

            // Now after updating the metadata, finally save the VRHeader Node into the core vector resource
            {
                new_item = Some(
                    self._add_vr_header_to_core_resource(
                        writer,
                        vr_header,
                        Some(node_metadata),
                        current_datetime,
                        !node_at_path_already_exists,
                    )
                    .await?,
                );
            }
        }

        // Now that we've inserted the the new item into the fs internals core VR proceed forward
        if let Some(item) = new_item {
            // Determine permissions based on whether the parent folder is root
            // This shouldn't be possible now but just to play it safe
            let (read_permission, write_permission) = if writer.path == VRPath::root() {
                (ReadPermission::Private, WritePermission::Private)
            } else {
                // Read the permissions of the parent folder
                let parent_permissions = self
                    .get_profile_fs_internals_cloned(&writer.profile)
                    .await?
                    .permissions_index
                    .get_path_permission(&writer.path)
                    .await
                    .unwrap_or(PathPermission {
                        read_permission: ReadPermission::Private,
                        write_permission: WritePermission::Private,
                        whitelist: HashMap::new(),
                    });
                (parent_permissions.read_permission, parent_permissions.write_permission)
            };

            // Add read/write permission for the new item path
            {
                let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
                internals
                    .permissions_index
                    .insert_path_permission(item.path.clone(), read_permission, write_permission)
                    .await?;
                self._update_fs_internals(writer.profile.clone(), internals).await?;
            }

            // Finally saving the resource, the source file (if it was provided), and the FSInternals into the FSDB
            let mut write_batch = writer.new_write_batch()?;
            if let Some(sfm) = source_file_map {
                self.db
                    .wb_save_source_file_map(&sfm, &source_db_key, &mut write_batch)?;
            }
            self.db.wb_save_resource(&resource, &mut write_batch)?;
            let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
            self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
            self.db.write_pb(write_batch)?;

            Ok(item)
        } else {
            Err(VectorFSError::NoEntryAtPath(node_path))
        }
    }

    /// Updates the SourceFileMap of the FSItem at the writer's path.
    /// If no FSItem with the same name already exists underneath the current path, then errors.
    pub async fn update_source_file_map(
        &self,
        writer: &VFSWriter,
        source_file_map: SourceFileMap,
    ) -> Result<FSItem, VectorFSError> {
        let mut source_db_key = String::new();
        let mut node_metadata = None;
        let mut vr_header = None;
        let mut new_item = None;

        {
            // If an existing FSFolder is already saved at the node path, return error.
            if let Ok(_) = self
                .validate_path_points_to_folder(writer.path.clone(), &writer.profile)
                .await
            {
                return Err(VectorFSError::CannotOverwriteFolder(writer.path.clone()));
            }
            // If an existing FSItem is saved at the node path
            if let Ok(_) = self
                .validate_path_points_to_item(writer.path.clone(), &writer.profile)
                .await
            {
                if let Ok(ret_node) = self
                    ._retrieve_core_resource_node_at_path(writer.path.clone(), &writer.profile)
                    .await
                {
                    if let Ok(header) = ret_node.node.get_vr_header_content() {
                        node_metadata.clone_from(&ret_node.node.metadata);
                        vr_header = Some(header.clone());
                        source_db_key = header.reference_string();
                    } else {
                        return Err(VectorFSError::InvalidFSEntryType(writer.path.to_string()));
                    }
                }
            }

            // Now all validation checks/setup have passed, move forward with saving header/source file map
            let current_datetime = ShinkaiTime::generate_time_now();
            // Update the metadata keys of the FSItem node
            let mut node_metadata = node_metadata.unwrap_or_else(HashMap::new);
            node_metadata.insert(
                FSItem::source_file_map_last_saved_metadata_key(),
                current_datetime.to_rfc3339(),
            );
            let sfm_size = source_file_map.encoded_size()?;
            node_metadata.insert(FSItem::source_file_map_size_metadata_key(), sfm_size.to_string());

            // Now after updating the metadata, finally save the VRHeader Node into the core vector resource
            let vr_header = vr_header.ok_or(VectorFSError::InvalidFSEntryType(writer.path.to_string()))?;
            {
                new_item = Some(
                    self._add_vr_header_to_core_resource(
                        writer,
                        vr_header,
                        Some(node_metadata),
                        current_datetime,
                        false,
                    )
                    .await?,
                );
            }
        }

        // Finally saving the the source file map and the FSInternals into the FSDB
        let mut write_batch = writer.new_write_batch()?;
        self.db
            .wb_save_source_file_map(&source_file_map, &source_db_key, &mut write_batch)?;
        let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
        self.db.wb_save_profile_fs_internals(&internals, &mut write_batch)?;
        self.db.write_pb(write_batch)?;

        if let Some(item) = new_item {
            Ok(item)
        } else {
            Err(VectorFSError::NoEntryAtPath(writer.path.clone()))
        }
    }

    /// Updates the description of the Vector Resource in the FSItem at the writer's path.
    pub async fn update_item_resource_description(
        &self,
        writer: &VFSWriter,
        description: String,
    ) -> Result<FSItem, VectorFSError> {
        // Fetch the VR and SFM from the DB
        let reader = writer.new_reader_copied_data(writer.path.clone(), self).await?;
        let mut vector_resource = self.retrieve_vector_resource(&reader).await?;
        vector_resource.as_trait_object_mut().set_description(Some(description));

        // Now save the VR
        Self::save_vector_resource(self, writer, vector_resource, None).await
    }

    /// Internal method used to add a VRHeader into the core resource of a profile's VectorFS internals in memory.
    async fn _add_vr_header_to_core_resource(
        &self,
        writer: &VFSWriter,
        vr_header: VRHeader,
        metadata: Option<HashMap<String, String>>,
        current_datetime: DateTime<Utc>,
        adding_new_item_to_fs: bool,
    ) -> Result<FSItem, VectorFSError> {
        let new_node_path = writer.path.push_cloned(vr_header.resource_name.clone());

        // Mutator method for inserting the VR header and updating the last_modified metadata of parent folder
        let mut mutator = |node: &mut Node, _embedding: &mut Embedding| -> Result<(), VRError> {
            // If adding a new FSItem update last_modified key to be current date time. If overwriting existing item,
            // or moving an item, then  can skip.
            if adding_new_item_to_fs {
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
            resource.as_trait_object_mut().insert_node_dt_specified(
                vr_header.resource_name.clone(),
                new_vr_header_node,
                new_node_embedding,
                Some(current_datetime),
                true,
            )?;

            // Update the resource's keywords. If no keywords, copy all, else random replace a few
            if resource.as_trait_object_mut().keywords().keyword_list.is_empty() {
                resource
                    .as_trait_object_mut()
                    .keywords_mut()
                    .set_keywords(vr_header.resource_keywords.keyword_list.clone())
            } else {
                resource
                    .as_trait_object_mut()
                    .keywords_mut()
                    .random_replace_keywords(5, vr_header.resource_keywords.keyword_list.clone())
            }
            Ok(())
        };

        // If an embedding exists on the VR, and it is generated using the same embedding model
        if vr_header.resource_embedding.clone().is_some() {
            // Acquire a write lock on internals_map to ensure thread-safe access
            let mut internals_map = self.internals_map.write().await;
            let internals = internals_map
                .get_mut(&writer.profile)
                .ok_or_else(|| VectorFSError::ProfileNameNonExistent(writer.profile.to_string()))?;

            if vr_header.resource_embedding_model_used == internals.default_embedding_model()
                || internals
                    .supported_embedding_models
                    .contains(&vr_header.resource_embedding_model_used)
            {
                internals
                    .fs_core_resource
                    .mutate_node_at_path(writer.path.clone(), &mut mutator, true)?;
                // Update last read of the new FSItem
                internals.last_read_index.update_path_last_read(
                    new_node_path.clone(),
                    current_datetime,
                    writer.requester_name.clone(),
                );

                let retrieved_node = internals
                    .fs_core_resource
                    .retrieve_node_at_path(new_node_path.clone(), None)?;
                let new_item = FSItem::from_vr_header_node(
                    retrieved_node.node,
                    new_node_path.clone(),
                    &internals.last_read_index,
                )?;
                Ok(new_item)
            } else {
                // TODO: If the embedding model does not match, instead of error, regenerate the resource's embedding
                // using the default embedding model and add it to the VRHeader in the FSItem. At the same time implement dynamic vector searching in VecFS to support this.
                Err(VectorFSError::EmbeddingModelTypeMismatch(
                    vr_header.resource_embedding_model_used,
                    internals.default_embedding_model(),
                ))
            }
        } else {
            Err(VectorFSError::EmbeddingMissingInResource(vr_header.resource_name))
        }
    }

    /// Internal method used to add an existing VectorResource into the core resource of a profile's VectorFS internals in memory.
    /// Aka, add a folder into the VectorFS under the given path.
    async fn _add_existing_vr_to_core_resource(
        &self,
        writer: &VFSWriter,
        resource: BaseVectorResource,
        embedding: Embedding,
        metadata: Option<HashMap<String, String>>,
        current_datetime: DateTime<Utc>,
    ) -> Result<FSFolder, VectorFSError> {
        let resource_name = resource.as_trait_object().name().to_string();
        let resource_keywords = resource.as_trait_object().keywords().keyword_list.clone();
        let new_node_path = writer.path.push_cloned(resource_name.clone());

        // Check the path points to a folder
        if &writer.path != &VRPath::root() {
            self.validate_path_points_to_folder(writer.path.clone(), &writer.profile)
                .await?;
        }
        // Check if anything exists at the new node's path and error if so (cannot overwrite an existing FSEntry)
        if let Ok(_) = self
            .validate_path_points_to_entry(new_node_path.clone(), &writer.profile)
            .await
        {
            return Err(VectorFSError::EntryAlreadyExistsAtPath(new_node_path));
        }

        // Acquire a write lock on internals_map to ensure thread-safe access
        let mut internals_map = self.internals_map.write().await;
        let internals = internals_map
            .get_mut(&writer.profile)
            .ok_or_else(|| VectorFSError::ProfileNameNonExistent(writer.profile.to_string()))?;

        // Check if parent is root, if so then direct insert into root and return, else proceed
        if writer.path.is_empty() {
            let new_node = Node::new_vector_resource(resource_name.clone(), &resource, metadata.clone());
            internals.fs_core_resource.insert_node_dt_specified(
                resource_name.clone(),
                new_node.clone(),
                embedding.clone(),
                None,
                true,
            )?;
            // Update last read of the new FSFolder
            internals.last_read_index.update_path_last_read(
                new_node_path.clone(),
                current_datetime,
                writer.requester_name.clone(),
            );

            let folder = FSFolder::from_vector_resource_node(new_node, new_node_path, &internals.last_read_index)?;
            return Ok(folder);
        }
        drop(internals_map);

        // Mutator method for inserting the VR and updating the last_modified metadata of parent folder
        let mut mutator = |node: &mut Node, _: &mut Embedding| -> Result<(), VRError> {
            // Update last_modified key of the parent folder
            node.metadata
                .as_mut()
                .map(|m| m.insert(FSFolder::last_modified_key(), current_datetime.to_rfc3339()));
            // Create the new folder child node and insert it
            let new_node = Node::new_vector_resource(resource_name.clone(), &resource, metadata.clone());
            let parent_resource = node.get_vector_resource_content_mut()?;
            parent_resource.as_trait_object_mut().insert_node_dt_specified(
                resource_name.clone(),
                new_node,
                embedding.clone(),
                None,
                true,
            )?;

            // If new resource has keywords, and none in target copy all, else random replace a few
            if !resource_keywords.is_empty() {
                if parent_resource.as_trait_object_mut().keywords().keyword_list.is_empty() {
                    parent_resource
                        .as_trait_object_mut()
                        .keywords_mut()
                        .set_keywords(resource_keywords.clone())
                } else {
                    parent_resource
                        .as_trait_object_mut()
                        .keywords_mut()
                        .random_replace_keywords(5, resource_keywords.clone())
                }
            }

            Ok(())
        };

        // Acquire a write lock on internals_map to ensure thread-safe access
        let mut internals_map = self.internals_map.write().await;
        let internals = internals_map
            .get_mut(&writer.profile)
            .ok_or_else(|| VectorFSError::ProfileNameNonExistent(writer.profile.to_string()))?;
        internals
            .fs_core_resource
            .mutate_node_at_path(writer.path.clone(), &mut mutator, true)?;
        // Update last read of the new FSFolder
        internals.last_read_index.update_path_last_read(
            new_node_path.clone(),
            current_datetime,
            writer.requester_name.clone(),
        );

        let retrieved_node = internals
            .fs_core_resource
            .retrieve_node_at_path(new_node_path.clone(), None)?;
        let folder =
            FSFolder::from_vector_resource_node(retrieved_node.node, new_node_path, &internals.last_read_index)?;

        Ok(folder)
    }

    /// Internal method used to remove a child node underneath the writer's path, given its id. Applies only in memory.
    /// This only works if path is a folder/root and node_id is either an item or folder underneath, and node_id points
    /// to a valid node.
    async fn _remove_child_node_from_core_resource(
        &self,
        writer: &VFSWriter,
        node_id: String,
    ) -> Result<(Node, Embedding), VectorFSError> {
        let mut internals_map = self.internals_map.write().await;
        let internals = internals_map
            .get_mut(&writer.profile)
            .ok_or_else(|| VectorFSError::ProfileNameNonExistent(writer.profile.to_string()))?;

        let path = writer.path.push_cloned(node_id);
        Ok(internals.fs_core_resource.remove_node_at_path(path, true)?)
    }

    /// Internal method used to remove the node at path. Applies only in memory.
    /// Errors if no node exists at path.
    async fn _remove_node_from_core_resource(&self, writer: &VFSWriter) -> Result<(Node, Embedding), VectorFSError> {
        let mut internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
        let result = internals
            .fs_core_resource
            .remove_node_at_path(writer.path.clone(), true)?;
        self._update_fs_internals(writer.profile.clone(), internals).await?;
        Ok(result)
    }

    /// Internal method used to get a child node underneath the writer's path, given its id. Applies only in memory.
    /// This only works if path is a folder and node_id is either an item or folder underneath, and node_id points
    /// to a valid node.
    async fn _get_child_node_from_core_resource(
        &self,
        writer: &VFSWriter,
        node_id: String,
    ) -> Result<(RetrievedNode, Embedding), VectorFSError> {
        let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
        let path = writer.path.push_cloned(node_id);
        let result = internals
            .fs_core_resource
            .retrieve_node_and_embedding_at_path(path, None)?;
        Ok(result)
    }

    /// Internal method used to get the node at writer's path. Applies only in memory.
    /// Errors if no node exists at path.
    async fn _get_node_from_core_resource(
        &self,
        writer: &VFSWriter,
    ) -> Result<(RetrievedNode, Embedding), VectorFSError> {
        let internals = self.get_profile_fs_internals_cloned(&writer.profile).await?;
        let result = internals
            .fs_core_resource
            .retrieve_node_and_embedding_at_path(writer.path.clone(), None)?;
        Ok(result)
    }
}
