use super::vector_fs::{self, VectorFS};
use super::vector_fs_error::VectorFSError;
use super::vector_fs_types::{FSEntry, FSFolder, FSItem, FSRoot, LastReadIndex};
use super::vector_fs_writer::VFSWriter;
use crate::db::db_profile_bound::ProfileBoundWriteBatch;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::resource_errors::VRError;
use shinkai_vector_resources::shinkai_time::ShinkaiTime;
use shinkai_vector_resources::source::{SourceFile, SourceFileMap};
use shinkai_vector_resources::vector_resource::{
    BaseVectorResource, NodeContent, RetrievedNode, VRKai, VRPack, VectorResource, VectorResourceCore,
    VectorResourceSearch,
};
use shinkai_vector_resources::{embeddings::Embedding, vector_resource::VRPath};

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
    pub fn new(
        requester_name: ShinkaiName,
        path: VRPath,
        vector_fs: &mut VectorFS,
        profile: ShinkaiName,
    ) -> Result<Self, VectorFSError> {
        let reader = VFSReader {
            requester_name: requester_name.clone(),
            path: path.clone(),
            profile: profile.clone(),
        };

        // Validate that the path exists
        if vector_fs.validate_path_points_to_entry(path.clone(), &profile).is_err() {
            return Err(VectorFSError::NoEntryAtPath(path));
        }

        // Validate read permissions to ensure requester_name has rights
        vector_fs
            .validate_read_access_for_paths(profile.clone(), requester_name.clone(), vec![path.clone()])
            .map_err(|_| {
                VectorFSError::InvalidReaderPermission(requester_name.clone(), profile.clone(), path.clone())
            })?;

        // Once permission verified, saves the datatime both into memory (last_read_index)
        // and into the FSDB as stored logs.
        let fs_internals = vector_fs.get_profile_fs_internals(&profile)?;
        let current_datetime = ShinkaiTime::generate_time_now();
        fs_internals
            .last_read_index
            .update_path_last_read(path.clone(), current_datetime, requester_name.clone());
        let mut write_batch = ProfileBoundWriteBatch::new_vfs_batch(&profile)?;
        vector_fs
            .db
            .wb_add_read_access_log(requester_name, &path, current_datetime, profile, &mut write_batch)?;
        vector_fs.db.write_pb(write_batch)?;

        Ok(reader)
    }

    /// Generates a VFSReader using the same requester_name/profile held in self.
    /// Read permissions are verified before the VFSReader is produced.
    pub fn new_reader_copied_data(&self, path: VRPath, vector_fs: &mut VectorFS) -> Result<VFSReader, VectorFSError> {
        VFSReader::new(self.requester_name.clone(), path, vector_fs, self.profile.clone())
    }

    /// Generates a VFSWriter using the same requester_name/profile held in self.
    /// Write permissions are verified before the VFSWriter is produced.
    pub fn new_writer_copied_data(&self, path: VRPath, vector_fs: &mut VectorFS) -> Result<VFSWriter, VectorFSError> {
        VFSWriter::new(self.requester_name.clone(), path, vector_fs, self.profile.clone())
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
    pub fn retrieve_fs_path_simplified_json(&mut self, reader: &VFSReader) -> Result<String, VectorFSError> {
        let entry = self.retrieve_fs_entry(reader)?;
        return entry.to_json_simplified();
    }

    /// Retrieves the FSEntry for the reader's path in the VectorFS.
    pub fn retrieve_fs_entry(&mut self, reader: &VFSReader) -> Result<FSEntry, VectorFSError> {
        let internals = self.get_profile_fs_internals_read_only(&reader.profile)?;

        // Create FSRoot directly if path is root
        if reader.path.is_empty() {
            let fs_root =
                FSRoot::from_core_vector_resource(internals.fs_core_resource.clone(), &internals.last_read_index)?;
            return Ok(FSEntry::Root(fs_root));
        }

        // Otherwise retrieve the node and process it
        let ret_node = internals.fs_core_resource.retrieve_node_at_path(reader.path.clone())?;
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
    pub fn retrieve_vector_resource(&mut self, reader: &VFSReader) -> Result<BaseVectorResource, VectorFSError> {
        let fs_item = self.retrieve_fs_entry(reader)?.as_item()?;
        self.db.get_resource_by_fs_item(&fs_item, &reader.profile)
    }

    /// Attempts to retrieve the SourceFileMap from inside an FSItem at the path specified in reader. If this path does not currently exist, or
    /// a source_file is not saved at this path, then an error is returned.
    pub fn retrieve_source_file_map(&mut self, reader: &VFSReader) -> Result<SourceFileMap, VectorFSError> {
        let fs_item = self.retrieve_fs_entry(reader)?.as_item()?;
        self.db.get_source_file_map_by_fs_item(&fs_item, &reader.profile)
    }

    /// Attempts to retrieve a VRKai from the path specified in reader (errors if entry at path is not an item).
    pub fn retrieve_vrkai(&mut self, reader: &VFSReader) -> Result<VRKai, VectorFSError> {
        let fs_item = self.retrieve_fs_entry(reader)?.as_item()?;
        let resource = self.db.get_resource_by_fs_item(&fs_item, &reader.profile)?;
        let sfm = self.retrieve_source_file_map(reader).ok();

        Ok(VRKai::from_base_vector_resource(resource, sfm))
    }

    /// Attempts to retrieve a VRPack from the path specified in reader (errors if entry at path is not a folder or root).
    pub fn retrieve_vrpack(&mut self, reader: &VFSReader) -> Result<VRPack, VectorFSError> {
        let fs_entry = self.retrieve_fs_entry(reader)?;
        let mut vrpack = VRPack::new_empty(); // Assuming a constructor for VRPack exists
        let vec_fs_base_path = reader.path.clone();

        // Recursive function to process each entry and populate the VRPack
        fn process_entry(
            entry: &FSEntry,
            vrpack: &mut VRPack,
            current_path: VRPath,
            vector_fs: &mut VectorFS,
            reader: &VFSReader,
            vec_fs_base_path: VRPath,
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
                        )?;
                    }
                }
                FSEntry::Folder(folder) => {
                    println!("\n {}'s child folders: {:?}", folder.path, folder.child_folders);
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
                        )?;
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
                        )?;
                    }
                }
                FSEntry::Item(item) => {
                    // For each item, use retrieve_vrkai to get the VRKai object
                    let item_path = vec_fs_base_path.append_path_cloned(&current_path.push_cloned(item.name.clone()));
                    let item_reader = reader.new_reader_copied_data(item_path, vector_fs)?;
                    match vector_fs.retrieve_vrkai(&item_reader) {
                        Ok(vrkai) => vrpack.insert_vrkai(&vrkai, current_path.clone())?,
                        Err(e) => return Err(e),
                    }
                }
            }
            Ok(())
        }

        // Start processing from the root or folder of the FSEntry
        process_entry(&fs_entry, &mut vrpack, VRPath::root(), self, reader, vec_fs_base_path)?;

        Ok(vrpack)
    }

    /// Attempts to retrieve a VectorResource from inside an FSItem within the folder specified at reader path.
    /// If a VectorResource is not saved at this path, an error will be returned.
    pub fn retrieve_vector_resource_in_folder(
        &mut self,
        reader: &VFSReader,
        item_name: String,
    ) -> Result<BaseVectorResource, VectorFSError> {
        let new_reader = reader.new_reader_copied_data(reader.path.push_cloned(item_name), self)?;
        self.retrieve_vector_resource(&new_reader)
    }

    /// Attempts to retrieve a SourceFileMap from inside an FSItem within the folder specified at reader path.
    /// If this path does not currently exist, or a source_file is not saved at this path,
    /// then an error is returned.
    pub fn retrieve_source_file_map_in_folder(
        &mut self,
        reader: &VFSReader,
        item_name: String,
    ) -> Result<SourceFileMap, VectorFSError> {
        let new_reader = reader.new_reader_copied_data(reader.path.push_cloned(item_name), self)?;
        self.retrieve_source_file_map(&new_reader)
    }

    /// Attempts to retrieve a VRKai from inside an FSItem within the folder specified at reader path.
    /// If a VectorResource is not saved at this path, an error will be returned.
    pub fn retrieve_vrkai_in_folder(&mut self, reader: &VFSReader, item_name: String) -> Result<VRKai, VectorFSError> {
        let new_reader = reader.new_reader_copied_data(reader.path.push_cloned(item_name), self)?;
        self.retrieve_vrkai(&new_reader)
    }

    /// Retrieves a node at a given path from the VectorFS core resource under a profile
    pub fn _retrieve_core_resource_node_at_path(
        &self,
        path: VRPath,
        profile: &ShinkaiName,
    ) -> Result<RetrievedNode, VectorFSError> {
        let internals = self.get_profile_fs_internals_read_only(profile)?;
        internals
            .fs_core_resource
            .retrieve_node_at_path(path.clone())
            .map_err(|_| VectorFSError::NoEntryAtPath(path.clone()))
    }

    /// Validates that the path points to a FSFolder
    pub fn validate_path_points_to_folder(&self, path: VRPath, profile: &ShinkaiName) -> Result<(), VectorFSError> {
        let ret_node = self._retrieve_core_resource_node_at_path(path.clone(), profile)?;

        match ret_node.node.content {
            NodeContent::Resource(_) => Ok(()),
            _ => Err(VectorFSError::PathDoesNotPointAtFolder(path)),
        }
    }

    /// Validates that the path points to a FSItem
    pub fn validate_path_points_to_item(&self, path: VRPath, profile: &ShinkaiName) -> Result<(), VectorFSError> {
        let ret_node = self._retrieve_core_resource_node_at_path(path.clone(), profile)?;

        match ret_node.node.content {
            NodeContent::VRHeader(_) => Ok(()),
            _ => Err(VectorFSError::PathDoesNotPointAtItem(path.clone())),
        }
    }

    /// Validates that the path points to any FSEntry, meaning that something exists at that path. Also returns `Ok()` for root `/`.
    pub fn validate_path_points_to_entry(&self, path: VRPath, profile: &ShinkaiName) -> Result<(), VectorFSError> {
        if path == VRPath::root() {
            return Ok(());
        }
        self._retrieve_core_resource_node_at_path(path, profile).map(|_| ())
    }
}
