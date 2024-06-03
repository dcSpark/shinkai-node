use super::vector_fs::VectorFS;
use super::vector_fs_error::VectorFSError;
use super::vector_fs_types::{FSEntry, FSFolder, FSItem, FSRoot};
use super::vector_fs_writer::VFSWriter;
use crate::db::db_profile_bound::ProfileBoundWriteBatch;
use async_recursion::async_recursion;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::shinkai_time::ShinkaiTime;
use shinkai_vector_resources::source::SourceFileMap;
use shinkai_vector_resources::vector_resource::{
    BaseVectorResource, NodeContent, RetrievedNode, VRKai, VRPack, VectorResourceCore,
};
use shinkai_vector_resources::vector_resource::{VRPath, VectorResourceSearch};

/// A struct that represents having access rights to read the VectorFS under a profile/at a specific path.
/// If a VFSReader struct is constructed, that means the `requester_name` has passed
/// permissions validation and is thus allowed to read `path`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VFSReader {
    pub requester_name: ShinkaiName,
    pub path: VRPath,
    pub profile: ShinkaiName,
}

impl VFSReader {
    /// Creates a new VFSReader if the `requester_name` passes read permission validation check.
    pub async fn new(
        requester_name: ShinkaiName,
        path: VRPath,
        vector_fs: &VectorFS,
        profile: ShinkaiName,
    ) -> Result<Self, VectorFSError> {
        let reader = VFSReader {
            requester_name: requester_name.clone(),
            path: path.clone(),
            profile: profile.clone(),
        };

        // Validate profile ShinkaiName has an actual profile inside
        if profile.extract_profile().is_err() {
            return Err(VectorFSError::ProfileNameNonExistent(profile.to_string()));
        }

        // Validate that the path exists
        if vector_fs
            .validate_path_points_to_entry(path.clone(), &profile)
            .await
            .is_err()
        {
            return Err(VectorFSError::NoEntryAtPath(path));
        }

        // Validate read permissions to ensure requester_name has rights
        vector_fs
            .validate_read_access_for_paths(profile.clone(), requester_name.clone(), vec![path.clone()])
            .await
            .map_err(|_| {
                VectorFSError::InvalidReaderPermission(requester_name.clone(), profile.clone(), path.clone())
            })?;

        // Once permission verified, saves the datatime both into memory (last_read_index)
        // and into the FSDB as stored logs.
        let current_datetime = ShinkaiTime::generate_time_now();
        // Update the last read path and time
        vector_fs
            .update_last_read_path(&profile, path.clone(), current_datetime, requester_name.clone())
            .await?;

        let mut write_batch = ProfileBoundWriteBatch::new_vfs_batch(&profile)?;
        vector_fs
            .db
            .wb_add_read_access_log(requester_name, &path, current_datetime, profile, &mut write_batch)?;
        vector_fs.db.write_pb(write_batch)?;

        Ok(reader)
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

    /// Serialize the PathPermission struct into a JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize a JSON string into a PathPermission struct
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl VectorFS {
    /// Retrieves a simplified JSON String representation of the FSEntry at the reader's path in the VectorFS.
    /// This is the representation that should be sent to frontends to visualize the VectorFS.
    pub async fn retrieve_fs_path_simplified_json(&self, reader: &VFSReader) -> Result<String, VectorFSError> {
        let entry = self.retrieve_fs_entry(reader).await?;
        entry.to_json_simplified()
    }

    /// Retrieves a simplified JSON Value representation of the FSEntry at the reader's path in the VectorFS.
    /// This is the representation that should be sent to frontends to visualize the VectorFS.
    pub async fn retrieve_fs_path_simplified_json_value(&self, reader: &VFSReader) -> Result<Value, VectorFSError> {
        let entry = self.retrieve_fs_entry(reader).await?;
        entry.to_json_simplified_value()
    }

    /// Retrieves a minimal JSON Value representation of the FSEntry at the reader's path in the VectorFS.
    /// This is a very minimalistic representation that should be sent to frontends to visualize the VectorFS.
    pub async fn retrieve_fs_path_minimal_json_value(&self, reader: &VFSReader) -> Result<Value, VectorFSError> {
        let entry = self.retrieve_fs_entry(reader).await?;
        entry.to_json_minimal_value()
    }

    /// Retrieves the FSEntry for the reader's path in the VectorFS.
    pub async fn retrieve_fs_entry(&self, reader: &VFSReader) -> Result<FSEntry, VectorFSError> {
        let internals = self.get_profile_fs_internals_cloned(&reader.profile).await?;

        // Create FSRoot directly if path is root
        if reader.path.is_empty() {
            let fs_root =
                FSRoot::from_core_vector_resource(internals.fs_core_resource.clone(), &internals.last_read_index)?;
            return Ok(FSEntry::Root(fs_root));
        }

        // Otherwise retrieve the node and process it
        let ret_node = internals
            .fs_core_resource
            .retrieve_node_at_path(reader.path.clone(), None)?;
        match ret_node.node.content {
            NodeContent::Resource(_) => {
                let fs_folder = FSFolder::from_vector_resource_node(
                    ret_node.node.clone(),
                    reader.path.clone(),
                    &internals.last_read_index,
                )?;
                Ok(FSEntry::Folder(fs_folder))
            }
            NodeContent::VRHeader(_) => {
                let fs_item =
                    FSItem::from_vr_header_node(ret_node.node, reader.path.clone(), &internals.last_read_index)?;
                Ok(FSEntry::Item(fs_item))
            }
            _ => Ok(Err(VRError::InvalidNodeType(ret_node.node.id))?),
        }
    }

    /// Attempts to retrieve a VectorResource from inside an FSItem at the path specified in reader. If an FSItem/VectorResource is not saved
    /// at this path, an error will be returned.
    pub async fn retrieve_vector_resource(&self, reader: &VFSReader) -> Result<BaseVectorResource, VectorFSError> {
        let fs_item = self.retrieve_fs_entry(reader).await?.as_item()?;
        self.db.get_resource_by_fs_item(&fs_item, &reader.profile)
    }

    /// Attempts to retrieve the SourceFileMap from inside an FSItem at the path specified in reader. If this path does not currently exist, or
    /// a source_file is not saved at this path, then an error is returned.
    pub async fn retrieve_source_file_map(&self, reader: &VFSReader) -> Result<SourceFileMap, VectorFSError> {
        let fs_item = self.retrieve_fs_entry(reader).await?.as_item()?;
        self.db.get_source_file_map_by_fs_item(&fs_item, &reader.profile)
    }

    /// Attempts to retrieve a VRKai from the path specified in reader (errors if entry at path is not an item).
    pub async fn retrieve_vrkai(&self, reader: &VFSReader) -> Result<VRKai, VectorFSError> {
        let fs_item = self.retrieve_fs_entry(reader).await?.as_item()?;
        let resource = self.db.get_resource_by_fs_item(&fs_item, &reader.profile)?;
        let sfm = self.retrieve_source_file_map(reader).await.ok();

        Ok(VRKai::new(resource, sfm))
    }

    /// Attempts to retrieve a VRPack from the path specified in reader (errors if entry at path is not a folder or root).
    pub async fn retrieve_vrpack(&self, reader: &VFSReader) -> Result<VRPack, VectorFSError> {
        let fs_entry = self.retrieve_fs_entry(reader).await?;
        let vec_fs_base_path_parent = reader.path.pop_cloned();
        let default_root_name = format!("{}-root", reader.profile.to_string());
        let folder_name = &reader.path.last_path_id().unwrap_or(default_root_name);
        let mut vrpack = VRPack::new_empty(folder_name);
        let mut folder_merkle_hash_map = std::collections::HashMap::new();

        // Recursive function to process each entry and populate the VRPack
        #[async_recursion]
        async fn process_entry(
            entry: &FSEntry,
            vrpack: &mut VRPack,
            current_path: VRPath,
            vector_fs: &VectorFS,
            reader: &VFSReader,
            vec_fs_base_path: VRPath,
            folder_merkle_hash_map: &mut std::collections::HashMap<VRPath, String>,
        ) -> Result<(), VectorFSError> {
            match entry {
                FSEntry::Root(folder) => {
                    for child in &folder.child_folders {
                        let entry = FSEntry::Folder(child.clone());
                        process_entry(
                            &entry,
                            vrpack,
                            current_path.clone(),
                            vector_fs,
                            reader,
                            vec_fs_base_path.clone(),
                            folder_merkle_hash_map,
                        )
                        .await?;
                    }
                }
                FSEntry::Folder(folder) => {
                    let inner_path = current_path.push_cloned(folder.name.clone());
                    vrpack.create_folder(&folder.name, current_path.clone())?;
                    for child in &folder.child_folders {
                        let entry = FSEntry::Folder(child.clone());
                        process_entry(
                            &entry,
                            vrpack,
                            inner_path.clone(),
                            vector_fs,
                            reader,
                            vec_fs_base_path.clone(),
                            folder_merkle_hash_map,
                        )
                        .await?;
                    }
                    for child in &folder.child_items {
                        let entry = FSEntry::Item(child.clone());
                        process_entry(
                            &entry,
                            vrpack,
                            inner_path.clone(),
                            vector_fs,
                            reader,
                            vec_fs_base_path.clone(),
                            folder_merkle_hash_map,
                        )
                        .await?;
                    }

                    folder_merkle_hash_map.insert(inner_path.clone(), folder.merkle_hash.to_string());
                }
                FSEntry::Item(item) => {
                    // For each item, use retrieve_vrkai to get the VRKai object
                    let item_path = vec_fs_base_path.append_path_cloned(&current_path.push_cloned(item.name.clone()));
                    let item_reader = reader.new_reader_copied_data(item_path, vector_fs).await?;
                    match vector_fs.retrieve_vrkai(&item_reader).await {
                        Ok(vrkai) => vrpack.insert_vrkai(&vrkai, current_path.clone(), false)?,
                        Err(e) => return Err(e),
                    }
                }
            }
            Ok(())
        }

        // Start processing from the root or folder of the FSEntry
        process_entry(
            &fs_entry,
            &mut vrpack,
            VRPath::root(),
            self,
            reader,
            vec_fs_base_path_parent,
            &mut folder_merkle_hash_map,
        )
        .await?;

        // Traverse through the sorted list and call the set merkle hash method on all
        let mut kv_pairs: Vec<(&VRPath, &String)> = folder_merkle_hash_map.iter().collect();
        kv_pairs.sort_by(|a, b| b.0.path_ids.len().cmp(&a.0.path_ids.len()));
        for (path, merkle_hash) in kv_pairs {
            vrpack._set_folder_merkle_hash(path.clone(), merkle_hash.clone())?;
        }

        vrpack.resource.as_trait_object_mut().update_merkle_root()?;

        Ok(vrpack)
    }

    /// Attempts to retrieve a VectorResource from inside an FSItem within the folder specified at reader path.
    /// If a VectorResource is not saved at this path, an error will be returned.
    pub async fn retrieve_vector_resource_in_folder(
        &self,
        reader: &VFSReader,
        item_name: String,
    ) -> Result<BaseVectorResource, VectorFSError> {
        let new_reader = reader
            .new_reader_copied_data(reader.path.push_cloned(item_name), self)
            .await?;
        self.retrieve_vector_resource(&new_reader).await
    }

    /// Attempts to retrieve a SourceFileMap from inside an FSItem within the folder specified at reader path.
    /// If this path does not currently exist, or a source_file is not saved at this path,
    /// then an error is returned.
    pub async fn retrieve_source_file_map_in_folder(
        &self,
        reader: &VFSReader,
        item_name: String,
    ) -> Result<SourceFileMap, VectorFSError> {
        let new_reader = reader
            .new_reader_copied_data(reader.path.push_cloned(item_name), self)
            .await?;
        self.retrieve_source_file_map(&new_reader).await
    }

    /// Attempts to retrieve a VRKai from inside an FSItem within the folder specified at reader path.
    /// If a VectorResource is not saved at this path, an error will be returned.
    pub async fn retrieve_vrkai_in_folder(
        &self,
        reader: &VFSReader,
        item_name: String,
    ) -> Result<VRKai, VectorFSError> {
        let new_reader = reader
            .new_reader_copied_data(reader.path.push_cloned(item_name), self)
            .await?;
        self.retrieve_vrkai(&new_reader).await
    }

    /// Retrieves a node at a given path from the VectorFS core resource under a profile
    pub async fn _retrieve_core_resource_node_at_path(
        &self,
        path: VRPath,
        profile: &ShinkaiName,
    ) -> Result<RetrievedNode, VectorFSError> {
        let internals = self.get_profile_fs_internals_cloned(profile).await?;
        internals
            .fs_core_resource
            .retrieve_node_at_path(path.clone(), None)
            .map_err(|_| VectorFSError::NoEntryAtPath(path.clone()))
    }

    /// Validates that the path points to a FSFolder
    pub async fn validate_path_points_to_folder(
        &self,
        path: VRPath,
        profile: &ShinkaiName,
    ) -> Result<(), VectorFSError> {
        let ret_node = self._retrieve_core_resource_node_at_path(path.clone(), profile).await?;

        match ret_node.node.content {
            NodeContent::Resource(_) => Ok(()),
            _ => Err(VectorFSError::PathDoesNotPointAtFolder(path)),
        }
    }

    /// Validates that the path points to a FSItem
    pub async fn validate_path_points_to_item(&self, path: VRPath, profile: &ShinkaiName) -> Result<(), VectorFSError> {
        let ret_node = self._retrieve_core_resource_node_at_path(path.clone(), profile).await?;

        match ret_node.node.content {
            NodeContent::VRHeader(_) => Ok(()),
            _ => Err(VectorFSError::PathDoesNotPointAtItem(path.clone())),
        }
    }

    /// Validates that the path points to any FSEntry, meaning that something exists at that path. Also returns `Ok()` for root `/`.
    pub async fn validate_path_points_to_entry(
        &self,
        path: VRPath,
        profile: &ShinkaiName,
    ) -> Result<(), VectorFSError> {
        if path == VRPath::root() {
            return Ok(());
        }
        self._retrieve_core_resource_node_at_path(path, profile)
            .await
            .map(|_| ())
    }

    /// Generates 2 RetrievedNodes which contain either the description + 2nd node, or the first two nodes if no description is available.
    ///  Sets their score to `1.0` with empty retrieval path & id. This is intended for job vector searches to prepend the intro text about relevant VRs.
    /// Only works on OrderedVectorResources, errors otherwise.
    pub async fn _internal_get_vr_intro_ret_nodes(
        &self,
        reader: &VFSReader,
    ) -> Result<Vec<RetrievedNode>, VectorFSError> {
        let vr = self.retrieve_vector_resource(&reader).await?;
        Ok(vr.as_trait_object().generate_intro_ret_nodes()?)
    }

    /// Checks if the folder at the specified path is empty.
    pub async fn is_folder_empty(&self, reader: &VFSReader) -> Result<bool, VectorFSError> {
        let fs_entry = self.retrieve_fs_entry(reader).await?;

        match fs_entry {
            FSEntry::Folder(folder) => {
                // A folder is considered empty if it has no child folders and no child items.
                Ok(folder.child_folders.is_empty() && folder.child_items.is_empty())
            }
            FSEntry::Root(root) => {
                // Similarly, a root is considered empty if it has no child folders.
                Ok(root.child_folders.is_empty())
            }
            _ => Err(VectorFSError::PathDoesNotPointAtFolder(reader.path.clone())),
        }
    }

    /// Returns the number of folders under the path specified in the VectorFS.
    pub async fn count_number_of_folders_under_path(
        &self,
        path: VRPath,
        profile: &ShinkaiName,
    ) -> Result<usize, VectorFSError> {
        let internals = self.get_profile_fs_internals_cloned(profile).await?;
        let folder_count = internals
            .fs_core_resource
            .retrieve_resource_nodes_exhaustive(Some(path.clone()))
            .len();
        Ok(folder_count)
    }

    /// Returns the number of items under the path specified in the VectorFS.
    pub async fn count_number_of_items_under_path(
        &self,
        path: VRPath,
        profile: &ShinkaiName,
    ) -> Result<usize, VectorFSError> {
        let internals = self.get_profile_fs_internals_cloned(profile).await?;
        let count = internals
            .fs_core_resource
            .retrieve_vrheader_nodes_exhaustive(Some(path.clone()))
            .len();
        Ok(count)
    }

    /// Returns all VRHeaderNodes under the path specified in the VectorFS, recursively (aka. any depth).
    /// These represent the VRs in the VectorFS.
    pub async fn retrieve_all_vr_header_nodes_underneath_folder(
        &self,
        reader: VFSReader,
    ) -> Result<Vec<RetrievedNode>, VectorFSError> {
        let internals = self.get_profile_fs_internals_cloned(&reader.profile).await?;
        let vrheader_nodes = internals
            .fs_core_resource
            .retrieve_vrheader_nodes_exhaustive(Some(reader.path.clone()));

        Ok(vrheader_nodes)
    }

    /// Returns all VectorFS paths of items underneath the path specified (any depth underneath).
    pub async fn retrieve_all_item_paths_underneath_folder(
        &self,
        reader: VFSReader,
    ) -> Result<Vec<VRPath>, VectorFSError> {
        let vrheader_nodes_all_depths = self.retrieve_all_vr_header_nodes_underneath_folder(reader).await?;

        let paths = vrheader_nodes_all_depths
            .iter()
            .map(|ret_node| ret_node.retrieval_path.clone())
            .collect::<Vec<VRPath>>();

        Ok(paths)
    }
}
