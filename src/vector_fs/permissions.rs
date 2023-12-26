use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_search_traversal::VRPath;
use std::{collections::HashMap, fmt::Write};

/// Struct that holds the read/write permissions specified for a specific path in the VectorFS
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PathPermission {
    read_permission: ReadPermission,
    write_permission: WritePermission,
    /// Whitelist which specifies per ShinkaiName which perms they have. Checked
    /// if either read or write perms are set to Whitelist, respectively.
    whitelist: HashMap<ShinkaiName, WhitelistPermission>,
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
    /// Creates a new PermissionsIndex.
    pub fn new(profile_name: ShinkaiName) -> Self {
        Self {
            fs_permissions: HashMap::new(),
            profile_name,
        }
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
