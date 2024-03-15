use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_resource::VRPath;
use std::{collections::HashMap, fmt::Write};

use super::{
    vector_fs::VectorFS, vector_fs_error::VectorFSError, vector_fs_reader::VFSReader, vector_fs_writer::VFSWriter,
};

/// Struct that holds the read/write permissions specified for a specific path in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PathPermission {
    pub read_permission: ReadPermission,
    pub write_permission: WritePermission,
    /// Whitelist which specifies per ShinkaiName which perms they have. Checked
    /// if either read or write perms are set to Whitelist, respectively.
    pub whitelist: HashMap<ShinkaiName, WhitelistPermission>,
}

impl PathPermission {
    /// Get the whitelist permission for a given ShinkaiName.
    /// If the ShinkaiName is not found (ie. profile's name), checks if permission exists for its node/global name instead.
    pub fn get_whitelist_permission(&self, requester_name: &ShinkaiName) -> Option<&WhitelistPermission> {
        let node_name = ShinkaiName::from_node_name(requester_name.node_name.clone()).ok()?;
        self.whitelist
            .get(requester_name)
            .or_else(|| self.whitelist.get(&node_name))
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

/// Enum representing the different types of read permissions a VRPath can have.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReadPermission {
    /// Only your profile has access
    Private,
    /// One or more specific profiles on your node
    NodeProfiles(Vec<ShinkaiName>),
    /// Specific identities on the Shinkai Network have access
    Whitelist,
    /// Anybody on the Shinkai Network has access
    Public,
}

/// Enum representing the different types of write permissions a VRPath can have.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WritePermission {
    /// Only your profile has access
    Private,
    /// One or more specific profiles on your node
    NodeProfiles(Vec<ShinkaiName>),
    /// Specific identities on the Shinkai Network have access
    Whitelist,
}

/// Enum describing what kind of permission for a specific path that a user has
/// on the whitelist
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WhitelistPermission {
    Read,
    Write,
    ReadWrite,
}

/// Struct holding the VectorFS' permissions for a given profile.
/// Note we store the PathPermissions as json strings internally to support efficient
/// permission checking during VectorFS vector searches.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermissionsIndex {
    /// Map which defines the kind of read and write permission per path in the VectorFS
    pub fs_permissions: HashMap<VRPath, String>,
    /// ShinkaiName of the profile this permissions index is for.
    pub profile_name: ShinkaiName,
}

impl PermissionsIndex {
    /// Creates a new PermissionsIndex struct
    pub fn new(profile_name: ShinkaiName) -> Self {
        let mut index = Self {
            fs_permissions: HashMap::new(),
            profile_name,
        };
        // Set permissions for the FS root to be private by default (only for profile owner).
        // This unwrap is safe due to hard coded values.
        index
            .insert_path_permission(VRPath::new(), ReadPermission::Private, WritePermission::Private)
            .unwrap();
        index
    }

    /// Creates a new PermissionsIndex using an input hashmap and profile.
    pub fn from_hashmap(profile_name: ShinkaiName, json_permissions: HashMap<VRPath, String>) -> Self {
        let mut index = Self {
            fs_permissions: json_permissions,
            profile_name,
        };

        index
    }

    /// Prepares a copy of the internal permissions hashmap to be used in a Vector Search, by appending
    /// a json serialized reader at a hardcoded key. which is very unlikely to be used normally.
    pub fn export_permissions_hashmap_with_reader(&self, reader: &VFSReader) -> HashMap<VRPath, String> {
        // Convert values to json
        let mut hashmap: HashMap<VRPath, String> = self.fs_permissions.clone();

        // Add reader at a hard-coded path that can't be used by the VecFS normally
        if let Ok(json) = reader.to_json() {
            hashmap.insert(Self::vfs_reader_unique_path(), json);
        }

        hashmap
    }

    /// A hard-coded path that isn't likely to be used by the VecFS normally for permissions ever.
    pub fn vfs_reader_unique_path() -> VRPath {
        let mut path = VRPath::new();
        path.push("9529".to_string());
        path.push("|do-not_use".to_string());
        path.push("7482".to_string());
        path
    }

    /// Retrieves the PathPermission for a given path.
    pub fn get_path_permission(&self, path: &VRPath) -> Result<PathPermission, VectorFSError> {
        self.fs_permissions
            .get(path)
            .map(|json| PathPermission::from_json(json))
            .transpose()?
            .ok_or_else(|| VectorFSError::NoPermissionEntryAtPath(path.clone()))
    }

    /// Inserts a path permission into the fs_permissions map. Note, this will overwrite the old read/write permissions
    /// for the path if they exist. The Whitelist for the path are preserved always in this method,
    /// even when neither read/write are still set as Whitelist.
    pub fn insert_path_permission(
        &mut self,
        path: VRPath,
        read_permission: ReadPermission,
        write_permission: WritePermission,
    ) -> Result<(), VectorFSError> {
        let whitelist = self
            .fs_permissions
            .get(&path)
            .and_then(|json| PathPermission::from_json(json).ok())
            .map_or_else(HashMap::new, |perm| perm.whitelist);

        let path_perm = PathPermission {
            read_permission,
            write_permission,
            whitelist,
        };
        self.fs_permissions.insert(path.clone(), path_perm.to_json()?);
        Ok(())
    }

    /// Copies the path permissions from the origin_path to the destination_path.
    /// Note, this will overwrite any old permission at the destination_path.
    pub fn copy_path_permission(&mut self, origin_path: VRPath, destination_path: VRPath) -> Result<(), VectorFSError> {
        if let Some(origin_permission_json) = self.fs_permissions.get(&origin_path) {
            let origin_permission = PathPermission::from_json(&origin_permission_json.clone())?;
            self.fs_permissions
                .insert(destination_path, origin_permission.to_json()?);
            Ok(())
        } else {
            Err(VectorFSError::NoPermissionEntryAtPath(origin_path))
        }
    }

    /// Internal method which removes a permission from the fs_permissions map.
    /// Should only be used by VectorFS when deleting FSEntries entirely.
    pub fn remove_path_permission(&mut self, path: VRPath) {
        self.fs_permissions.remove(&path);
    }

    /// Inserts the WhitelistPermission for a ShinkaiName to the whitelist for a given path.
    pub fn insert_to_whitelist(
        &mut self,
        path: VRPath,
        name: ShinkaiName,
        whitelist_perm: WhitelistPermission,
    ) -> Result<(), VectorFSError> {
        if let Some(mut path_permission_json) = self.fs_permissions.get_mut(&path) {
            let mut path_permission = PathPermission::from_json(&path_permission_json.clone())?;
            path_permission.whitelist.insert(name, whitelist_perm);
            *path_permission_json = path_permission.to_json()?;
        }
        Ok(())
    }

    /// Removes a ShinkaiName from the whitelist for a given path.
    pub fn remove_from_whitelist(&mut self, path: VRPath, name: ShinkaiName) -> Result<(), VectorFSError> {
        if let Some(mut path_permission_json) = self.fs_permissions.get_mut(&path) {
            let mut path_permission = PathPermission::from_json(&path_permission_json.clone())?;
            path_permission.whitelist.remove(&name);
            *path_permission_json = path_permission.to_json()?;
        }
        Ok(())
    }

    /// Validates the permission for a given requester ShinkaiName + Path in the node's VectorFS.
    /// If it returns Ok(()), then permission has passed.
    pub fn validate_read_access(&self, requester_name: &ShinkaiName, path: &VRPath) -> Result<(), VectorFSError> {
        let mut path = path.clone();

        loop {
            if let Some(path_permission_json) = self.fs_permissions.get(&path) {
                let path_permission = PathPermission::from_json(&path_permission_json.clone())?;

                // Global profile owner check
                if requester_name.get_profile_name() == self.profile_name.get_profile_name() {
                    return Ok(());
                }

                // Otherwise check specific permission
                match &path_permission.read_permission {
                    // If Public, then reading is always allowed
                    ReadPermission::Public => return Ok(()),
                    // If private, then reading is allowed for the specific profile that owns the VectorFS
                    ReadPermission::Private => {
                        if requester_name.get_profile_name() == self.profile_name.get_profile_name() {
                            return Ok(());
                        } else {
                            return Err(VectorFSError::InvalidReadPermission(
                                requester_name.clone(),
                                path.clone(),
                            ));
                        }
                    }
                    // If node profiles permission, then reading is allowed to specified profiles in the same node
                    ReadPermission::NodeProfiles(profiles) => {
                        if profiles.iter().any(|profile| {
                            profile.node_name == self.profile_name.node_name
                                && requester_name.profile_name == profile.profile_name
                        }) {
                            return Ok(());
                        } else {
                            return Err(VectorFSError::InvalidReadPermission(
                                requester_name.clone(),
                                path.clone(),
                            ));
                        }
                    }
                    // If Whitelist, checks if the current path permission has the WhitelistPermission for the user. If not, then recursively checks above
                    // directories (if they are also whitelisted) to see if the WhitelistPermission can be found there, until a non-whitelisted
                    // directory is found (returns false), or the WhitelistPermission is found for the requester.
                    ReadPermission::Whitelist => {
                        if let Some(whitelist_permission) = path_permission.get_whitelist_permission(requester_name) {
                            if matches!(
                                whitelist_permission,
                                WhitelistPermission::Read | WhitelistPermission::ReadWrite
                            ) {
                                return Ok(());
                            } else {
                                return Err(VectorFSError::InvalidReadPermission(
                                    requester_name.clone(),
                                    path.clone(),
                                ));
                            }
                        }
                    }
                }
            }
            // If we've gone through the whole path and no WhitelistPermission is found, then return false
            if path.pop().is_none() {
                return Err(VectorFSError::InvalidReadPermission(
                    requester_name.clone(),
                    path.clone(),
                ));
            }
        }
    }

    /// Validates the permission for a given requester ShinkaiName + Path in the node's VectorFS.
    /// If it returns Ok(()), then permission has passed.
    pub fn validate_write_access(&self, requester_name: &ShinkaiName, path: &VRPath) -> Result<(), VectorFSError> {
        let mut path = path.clone();

        loop {
            if let Some(path_permission_json) = self.fs_permissions.get(&path) {
                let path_permission = PathPermission::from_json(&path_permission_json.clone())?;

                // Global profile owner check
                if requester_name.get_profile_name() == self.profile_name.get_profile_name() {
                    return Ok(());
                }

                // Otherwise check specific permission
                match &path_permission.write_permission {
                    // If private, then writing is allowed for the specific profile that owns the VectorFS
                    WritePermission::Private => {
                        if requester_name.get_profile_name() == self.profile_name.get_profile_name() {
                            return Ok(());
                        } else {
                            return Err(VectorFSError::InvalidWritePermission(
                                requester_name.clone(),
                                path.clone(),
                            ));
                        }
                    }
                    // If node profiles permission, then writing is allowed to specified profiles in the same node
                    WritePermission::NodeProfiles(profiles) => {
                        if profiles.iter().any(|profile| {
                            profile.node_name == self.profile_name.node_name
                                && requester_name.profile_name == profile.profile_name
                        }) {
                        } else {
                            return Err(VectorFSError::InvalidWritePermission(
                                requester_name.clone(),
                                path.clone(),
                            ));
                        }
                    }
                    // If Whitelist, checks if the current path permission has the WhitelistPermission for the user. If not, then recursively checks above
                    // directories (if they are also whitelisted) to see if the WhitelistPermission can be found there, until a non-whitelisted
                    // directory is found (returns false), or the WhitelistPermission is found for the requester.
                    WritePermission::Whitelist => {
                        if let Some(whitelist_permission) = path_permission.whitelist.get(requester_name) {
                            if matches!(
                                whitelist_permission,
                                WhitelistPermission::Write | WhitelistPermission::ReadWrite
                            ) {
                            } else {
                                return Err(VectorFSError::InvalidWritePermission(
                                    requester_name.clone(),
                                    path.clone(),
                                ));
                            }
                        }
                    }
                }
            }
            // If we've gone through the whole path and no WhitelistPermission is found, then return false
            if path.pop().is_none() {
                return Err(VectorFSError::InvalidWritePermission(
                    requester_name.clone(),
                    path.clone(),
                ));
            }
        }
    }

    /// Finds all paths that have one of the specified type of read permissions, starting from a given path.
    pub fn find_paths_with_read_permissions(
        &self,
        starting_path: VRPath,
        read_permissions_to_find: Vec<ReadPermission>,
    ) -> Result<Vec<(VRPath, ReadPermission)>, VectorFSError> {
        let mut paths_with_permissions = Vec::new();

        // Iterate through the fs_permissions hashmap
        for (path, permission_json) in self.fs_permissions.iter() {
            // Check if the current path is a descendant of the starting path
            if starting_path.is_ancestor_path(path) {
                match PathPermission::from_json(permission_json) {
                    Ok(path_permission) => {
                        if read_permissions_to_find.contains(&path_permission.read_permission) {
                            paths_with_permissions.push((path.clone(), path_permission.read_permission.clone()));
                        }
                    }
                    Err(_) => (),
                }
            }
        }

        Ok(paths_with_permissions)
    }

    /// Finds all paths that have one of the specified type of write permissions, starting from a given path.
    pub fn find_paths_with_write_permissions(
        &self,
        starting_path: VRPath,
        write_permissions_to_find: Vec<WritePermission>,
    ) -> Result<Vec<(VRPath, WritePermission)>, VectorFSError> {
        let mut paths_with_permissions = Vec::new();

        // Iterate through the fs_permissions hashmap
        for (path, permission_json) in self.fs_permissions.iter() {
            // Check if the current path is a descendant of the starting path
            if starting_path.is_ancestor_path(path) {
                match PathPermission::from_json(permission_json) {
                    Ok(path_permission) => {
                        if write_permissions_to_find.contains(&path_permission.write_permission) {
                            paths_with_permissions.push((path.clone(), path_permission.write_permission.clone()));
                        }
                    }
                    Err(_) => (),
                }
            }
        }

        Ok(paths_with_permissions)
    }
}

impl VectorFS {
    /// Validates read access for a given `ShinkaiName` across multiple `VRPath`s in a profile's VectorFS.
    /// Returns `Ok(())` if all paths are valid for reading by the given name, or an error indicating the first one that it found which did not pass.
    pub fn validate_read_access_for_paths(
        &self,
        profile_name: ShinkaiName,
        name_to_check: ShinkaiName,
        paths: Vec<VRPath>,
    ) -> Result<(), VectorFSError> {
        for path in paths {
            let fs_internals = self.get_profile_fs_internals_read_only(&profile_name)?;
            if fs_internals
                .permissions_index
                .validate_read_access(&name_to_check, &path)
                .is_err()
            {
                return Err(VectorFSError::InvalidReadPermission(name_to_check, path));
            }
        }
        Ok(())
    }

    /// Validates write access for a given `ShinkaiName` across multiple `VRPath`s in a profile's VectorFS.
    /// Returns `Ok(())` if all paths are valid for writing by the given name, or an error indicating the first one that it found which did not pass.
    pub fn validate_write_access_for_paths(
        &self,
        profile_name: ShinkaiName,
        name_to_check: ShinkaiName,
        paths: Vec<VRPath>,
    ) -> Result<(), VectorFSError> {
        for path in paths {
            let fs_internals = self.get_profile_fs_internals_read_only(&profile_name)?;
            if fs_internals
                .permissions_index
                .validate_write_access(&name_to_check, &path)
                .is_err()
            {
                return Err(VectorFSError::InvalidWritePermission(name_to_check, path));
            }
        }
        Ok(())
    }

    /// Retrieves the PathPermission for each path in a list, returning a list of tuples containing the VRPath and its corresponding PathPermission.
    pub fn get_path_permission_for_paths(
        &self,
        profile_name: ShinkaiName,
        paths: Vec<VRPath>,
    ) -> Result<Vec<(VRPath, PathPermission)>, VectorFSError> {
        let mut path_permissions = Vec::new();

        for path in paths {
            let fs_internals = self.get_profile_fs_internals_read_only(&profile_name)?;
            match fs_internals.permissions_index.get_path_permission(&path) {
                Ok(permission) => path_permissions.push((path, permission)),
                Err(e) => return Err(e),
            }
        }

        Ok(path_permissions)
    }

    /// Finds all paths that have one of the specified types of read permissions, starting from the path in the given VFSReader.
    pub fn find_paths_with_read_permissions(
        &self,
        reader: &VFSReader,
        read_permissions_to_find: Vec<ReadPermission>,
    ) -> Result<Vec<(VRPath, ReadPermission)>, VectorFSError> {
        let fs_internals = self.get_profile_fs_internals_read_only(&reader.profile)?;
        fs_internals
            .permissions_index
            .find_paths_with_read_permissions(reader.path.clone(), read_permissions_to_find)
    }

    /// Finds all paths that have one of the specified types of write permissions, starting from the path in the given VFSReader.
    pub fn find_paths_with_write_permissions(
        &self,
        reader: &VFSReader,
        write_permissions_to_find: Vec<WritePermission>,
    ) -> Result<Vec<(VRPath, WritePermission)>, VectorFSError> {
        let fs_internals = self.get_profile_fs_internals_read_only(&reader.profile)?;
        fs_internals
            .permissions_index
            .find_paths_with_write_permissions(reader.path.clone(), write_permissions_to_find)
    }

    /// Sets the read/write permissions for the FSEntry at the writer's path (overwrites).
    /// This action is only allowed to be performed by the profile owner.
    /// No remove_path_permission is implemented, as all FSEntries must have a path permission.
    pub fn set_path_permission(
        &mut self,
        writer: &VFSWriter,
        read_permission: ReadPermission,
        write_permission: WritePermission,
    ) -> Result<(), VectorFSError> {
        if let Some(fs_internals) = self.internals_map.get_mut(&writer.profile) {
            if writer.requester_name == writer.profile {
                fs_internals.permissions_index.insert_path_permission(
                    writer.path.clone(),
                    read_permission,
                    write_permission,
                )?;
                self.db.save_profile_fs_internals(fs_internals, &writer.profile)?;
            } else {
                return Err(VectorFSError::InvalidWritePermission(
                    writer.requester_name.clone(),
                    writer.path.clone(),
                ));
            }
        }
        Ok(())
    }

    /// Inserts a ShinkaiName into the Whitelist permissions list for the FSEntry at the writer's path (overwrites).
    /// This action is only allowed to be performed by the profile owner.
    pub fn set_whitelist_permission(
        &mut self,
        writer: &VFSWriter,
        name_to_whitelist: ShinkaiName,
        whitelist_perm: WhitelistPermission,
    ) -> Result<(), VectorFSError> {
        if let Some(fs_internals) = self.internals_map.get_mut(&writer.profile) {
            if writer.requester_name == writer.profile {
                fs_internals.permissions_index.insert_to_whitelist(
                    writer.path.clone(),
                    name_to_whitelist,
                    whitelist_perm,
                )?;
                self.db.save_profile_fs_internals(fs_internals, &writer.profile)?;
            } else {
                return Err(VectorFSError::InvalidWritePermission(
                    writer.requester_name.clone(),
                    writer.path.clone(),
                ));
            }
        }
        Ok(())
    }

    /// Removes a ShinkaiName from the Whitelist permissions list for the FSEntry at the writer's path.
    /// This action is only allowed to be performed by the profile owner.
    pub fn remove_whitelist_permission(
        &mut self,
        writer: &VFSWriter,
        name_to_remove: ShinkaiName,
    ) -> Result<(), VectorFSError> {
        if let Some(fs_internals) = self.internals_map.get_mut(&writer.profile) {
            if writer.requester_name == writer.profile {
                fs_internals
                    .permissions_index
                    .remove_from_whitelist(writer.path.clone(), name_to_remove)?;
                self.db.save_profile_fs_internals(fs_internals, &writer.profile)?;
            } else {
                return Err(VectorFSError::InvalidWritePermission(
                    writer.requester_name.clone(),
                    writer.path.clone(),
                ));
            }
        }
        Ok(())
    }
}
