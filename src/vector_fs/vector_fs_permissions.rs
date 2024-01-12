use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_resource::VRPath;
use std::{collections::HashMap, fmt::Write};

use super::vector_fs_reader::VFSReader;

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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermissionsIndex {
    /// Map which defines the kind of read and write permission per path in the VectorFS
    fs_permissions: HashMap<VRPath, PathPermission>,
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
        // Set permissions for the FS root to be private by default (only for profile owner)
        index.insert_path_permission(VRPath::new(), ReadPermission::Private, WritePermission::Private);
        index
    }

    /// Converts internal permissions map into a more generic form where all values are Strings. Also encodes
    /// the input reader at very unlikely to be used path, to be parsed/read later (used in Vector Searches).
    pub fn convert_fs_permissions_to_json_values(&self, reader: &VFSReader) -> HashMap<VRPath, String> {
        // Convert values to json
        let mut hashmap: HashMap<VRPath, String> = self
            .fs_permissions
            .iter()
            .filter_map(|(vrpath, path_permission)| {
                if let Ok(path_permission_json) = path_permission.to_json() {
                    Some((vrpath.clone(), path_permission_json))
                } else {
                    None
                }
            })
            .collect();

        // Add reader at a hard-coded path that can't be used by the VecFS normally
        if let Ok(json) = reader.to_json() {
            hashmap.insert(Self::vfs_reader_unique_path(), json);
        }

        hashmap
    }

    /// Creates a new PermissionsIndex using an input hashmap where the values are encoded as json Strings.
    pub fn convert_from_json_values(
        profile_name: ShinkaiName,
        json_permissions: HashMap<VRPath, String>,
    ) -> Result<Self, serde_json::Error> {
        let mut index = Self {
            fs_permissions: HashMap::new(),
            profile_name,
        };

        for (vrpath, json) in json_permissions {
            if vrpath != Self::vfs_reader_unique_path() {
                let path_permission = PathPermission::from_json(&json)?;
                index.fs_permissions.insert(vrpath, path_permission);
            }
        }

        Ok(index)
    }

    /// A hard-coded path that isn't likely to be used by the VecFS normally for permissions ever.
    pub fn vfs_reader_unique_path() -> VRPath {
        let mut path = VRPath::new();
        path.push("9529".to_string());
        path.push("31008".to_string());
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
    ) {
        let path_perm = PathPermission {
            read_permission,
            write_permission,
            whitelist: HashMap::new(),
        };
        self.fs_permissions.insert(path.clone(), path_perm);
    }

    /// Removes a permission from the fs_permissions map.
    pub fn remove_path_permission(&mut self, path: VRPath) {
        self.fs_permissions.remove(&path);
    }

    /// Inserts the WhitelistPermission for a ShinkaiName to the whitelist for a given path.
    pub fn insert_to_whitelist(&mut self, path: VRPath, name: ShinkaiName, whitelist_perm: WhitelistPermission) {
        if let Some(path_permission) = self.fs_permissions.get_mut(&path) {
            path_permission.whitelist.insert(name, whitelist_perm);
        }
    }

    /// Removes a ShinkaiName from the whitelist for a given path.
    pub fn remove_from_whitelist(&mut self, path: VRPath, name: ShinkaiName) {
        if let Some(path_permission) = self.fs_permissions.get_mut(&path) {
            path_permission.whitelist.remove(&name);
        }
    }

    /// Validates the permission for a given requester ShinkaiName + Path in the node's VectorFS.
    pub fn validate_read_permission(&self, requester_name: &ShinkaiName, path: &VRPath) -> bool {
        let mut path = path.clone();

        loop {
            if let Some(path_permission) = self.fs_permissions.get(&path) {
                match &path_permission.read_permission {
                    // If Public, then reading is always allowed
                    ReadPermission::Public => return true,
                    // If private, then reading is allowed for the specific profile that owns the VectorFS
                    ReadPermission::Private => {
                        return requester_name.get_profile_name() == self.profile_name.get_profile_name()
                    }
                    // If node profiles permission, then reading is allowed to specified profiles in the same node
                    ReadPermission::NodeProfiles(profiles) => {
                        return profiles.iter().any(|profile| {
                            profile.node_name == self.profile_name.node_name
                                && requester_name.profile_name == profile.profile_name
                        })
                    }
                    // If Whitelist, checks if the current path permission has the WhitelistPermission for the user. If not, then recursively checks above
                    // directories (if they are also whitelisted) to see if the WhitelistPermission can be found there, until a non-whitelisted
                    // directory is found (returns false), or the WhitelistPermission is found for the requester.
                    ReadPermission::Whitelist => {
                        if let Some(whitelist_permission) = path_permission.whitelist.get(requester_name) {
                            return matches!(
                                whitelist_permission,
                                WhitelistPermission::Read | WhitelistPermission::ReadWrite
                            );
                        }
                    }
                }
            }
            // If we've gone through the whole path and no WhitelistPermission is found, then return false
            if path.pop().is_none() {
                return false;
            }
        }
    }

    /// Validates the permission for a given requester ShinkaiName + Path in the node's VectorFS.
    pub fn validate_write_permission(&self, requester_name: &ShinkaiName, path: &VRPath) -> bool {
        let mut path = path.clone();

        loop {
            if let Some(path_permission) = self.fs_permissions.get(&path) {
                match &path_permission.write_permission {
                    // If private, then writing is allowed for the specific profile that owns the VectorFS
                    WritePermission::Private => {
                        return requester_name.get_profile_name() == self.profile_name.get_profile_name()
                    }
                    // If node profiles permission, then writing is allowed to specified profiles in the same node
                    WritePermission::NodeProfiles(profiles) => {
                        return profiles.iter().any(|profile| {
                            profile.node_name == self.profile_name.node_name
                                && requester_name.profile_name == profile.profile_name
                        })
                    }
                    // If Whitelist, checks if the current path permission has the WhitelistPermission for the user. If not, then recursively checks above
                    // directories (if they are also whitelisted) to see if the WhitelistPermission can be found there, until a non-whitelisted
                    // directory is found (returns false), or the WhitelistPermission is found for the requester.
                    WritePermission::Whitelist => {
                        if let Some(whitelist_permission) = path_permission.whitelist.get(requester_name) {
                            return matches!(
                                whitelist_permission,
                                WhitelistPermission::Write | WhitelistPermission::ReadWrite
                            );
                        }
                    }
                }
            }
            // If we've gone through the whole path and no WhitelistPermission is found, then return false
            if path.pop().is_none() {
                return false;
            }
        }
    }
}
