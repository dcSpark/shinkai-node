use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_resource::VRPath;
use std::{collections::HashMap, fmt::Write};

use super::{vector_fs_error::VectorFSError, vector_fs_reader::VFSReader};

/// Struct that holds the read/write permissions specified for a specific path in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PathPermission {
    read_permission: ReadPermission,
    write_permission: WritePermission,
    /// Whitelist which specifies per ShinkaiName which perms they have. Checked
    /// if either read or write perms are set to Whitelist, respectively.
    whitelist: HashMap<ShinkaiName, WhitelistPermission>,
}

impl PathPermission {
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
    fs_permissions: HashMap<VRPath, String>,
    /// ShinkaiName of the profile this permissions index is for.
    profile_name: ShinkaiName,
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

    /// Inserts a new path permission into the fs_permissions map. Note, this will overwrite
    /// the old permission. Whitelists of existing path permissions are preserved always in this method,
    /// even when neither read/write are still set as Whitelist.
    pub fn insert_path_permission(
        &mut self,
        path: VRPath,
        read_permission: ReadPermission,
        write_permission: WritePermission,
    ) -> Result<(), VectorFSError> {
        let path_perm = PathPermission {
            read_permission,
            write_permission,
            whitelist: HashMap::new(),
        };
        self.fs_permissions.insert(path.clone(), path_perm.to_json()?);
        Ok(())
    }

    /// Removes a permission from the fs_permissions map.
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
    pub fn validate_read_permission(&self, requester_name: &ShinkaiName, path: &VRPath) -> Result<(), VectorFSError> {
        let mut path = path.clone();

        loop {
            if let Some(path_permission_json) = self.fs_permissions.get(&path) {
                let mut path_permission = PathPermission::from_json(&path_permission_json.clone())?;
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
                        if let Some(whitelist_permission) = path_permission.whitelist.get(requester_name) {
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
    pub fn validate_write_permission(&self, requester_name: &ShinkaiName, path: &VRPath) -> Result<(), VectorFSError> {
        let mut path = path.clone();

        loop {
            if let Some(path_permission_json) = self.fs_permissions.get(&path) {
                let mut path_permission = PathPermission::from_json(&path_permission_json.clone())?;
                match &path_permission.write_permission {
                    // If private, then writing is allowed for the specific profile that owns the VectorFS
                    WritePermission::Private => {
                        if requester_name.get_profile_name() == self.profile_name.get_profile_name() {
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
}
