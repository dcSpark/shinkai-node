use std::collections::HashMap;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_search_traversal::VRPath;

/// Enum representing the different types of permissions a VRPath can have.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Permission {
    Private,
    Public,
    Whitelist,
}

/// Struct holding the VectorFS' permissions for a given profile.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PermissionsIndex {
    /// Map from VRPath to its corresponding Permission.
    fs_permissions: HashMap<VRPath, Permission>,
    /// Map from VRPath to a list of ShinkaiNames that are whitelisted for that path.
    whitelist: HashMap<VRPath, HashMap<ShinkaiName, bool>>,
    /// ShinkaiName of the profile this permissions index is for. Used to allow owner of profile access.
    profile_name: ShinkaiName,
}

impl PermissionsIndex {
    /// Creates a new PermissionsIndex.
    pub fn new(profile_name: ShinkaiName) -> Self {
        Self {
            fs_permissions: HashMap::new(),
            whitelist: HashMap::new(),
            profile_name,
        }
    }

    /// Inserts a new permission into the fs_permissions map. Note, this will overwrite
    /// the old permission. If the new overriding permission is not a whitelist, it will delete all whitelisted names
    /// for the given VRPath.
    pub fn insert_fs_permission(&mut self, path: VRPath, permission: Permission) {
        if let Some(old_permission) = self.fs_permissions.insert(path.clone(), permission.clone()) {
            if matches!(old_permission, Permission::Whitelist) && !matches!(permission, Permission::Whitelist) {
                self.whitelist.remove(&path);
            }
        }
    }

    /// Removes a permission from the fs_permissions map.
    /// If the permission was a whitelist, also removes the corresponding entry from the whitelist map.
    pub fn remove_fs_permission(&mut self, path: VRPath) {
        if let Some(permission) = self.fs_permissions.remove(&path) {
            if matches!(permission, Permission::Whitelist) {
                self.whitelist.remove(&path);
            }
        }
    }

    /// Inserts a user to the whitelist for a given path.
    pub fn insert_to_whitelist(&mut self, path: VRPath, name: ShinkaiName) {
        let inner_whitelist = self.whitelist.entry(path).or_insert_with(HashMap::new);
        inner_whitelist.insert(name, true);
    }

    /// Removes a user from the whitelist for a given path.
    pub fn remove_from_whitelist(&mut self, path: VRPath, name: ShinkaiName) {
        if let Some(inner_whitelist) = self.whitelist.get_mut(&path) {
            inner_whitelist.remove(&name);
        }
    }

    /// Validates the permission for a given requester ShinkaiName + Path in the node's VectorFS.
    pub fn validate_permission(&self, requester_name: &ShinkaiName, path: &VRPath) -> bool {
        let mut path = path.clone();

        // TODO: Verify later that in all cases that profile owner from any device/agent as requester_name
        // always includes the profile name (it should in theory). This allows us to ensure VectorFSInternals is always profile-bound/permissioned.
        if let Some(Permission::Private) = self.fs_permissions.get(&path) {
            return requester_name.get_profile_name() == self.profile_name.get_profile_name();
            // return requester_name.get_node_name() == self.profile_name.get_node_name();
        }

        if let Some(Permission::Public) = self.fs_permissions.get(&path) {
            return true;
        }

        // Checks if the specific path is whitelisted for the user. If not, then recursively checks above
        // directories (if they are also whitelisted) to see if the requester has permissions, until a non-whitelisted
        // directory is found, or the permission is found for the requester.
        while let Some(Permission::Whitelist) = self.fs_permissions.get(&path) {
            if let Some(inner_whitelist) = self.whitelist.get(&path) {
                if let Some(&is_whitelisted) = inner_whitelist.get(requester_name) {
                    if is_whitelisted {
                        return true;
                    }
                }
            } else {
                return false;
            }
            path.pop();
        }

        false
    }

    /// Validates the permissions for a list of requester ShinkaiNames for a single Path in the node's VectorFS.
    pub fn validate_permission_multi_requesters(&self, requester_names: &[ShinkaiName], path: &VRPath) -> bool {
        requester_names.iter().all(|name| self.validate_permission(name, path))
    }
}
