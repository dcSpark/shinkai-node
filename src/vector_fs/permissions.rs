use std::collections::HashMap;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_search_traversal::VRPath;

/// Enum representing the different types of permissions a VRPath can have.
pub enum Permission {
    Private,
    Public,
    Whitelist,
}

/// Struct representing the permissions index.
pub struct PermissionsIndex {
    /// Map from VRPath to its corresponding Permission.
    fs_permissions: HashMap<VRPath, Permission>,
    /// Map from VRPath to a list of ShinkaiNames that are whitelisted for that path.
    whitelist: HashMap<VRPath, HashMap<ShinkaiName, bool>>,
    /// ShinkaiName of the current running node. Used to allow access for owner of node.
    node_name: ShinkaiName,
}

impl PermissionsIndex {
    /// Validates the permission for a given requester ShinkaiName + Path in the node's VectorFS.
    pub fn validate_permission(&self, requester_name: &ShinkaiName, path: &VRPath) -> bool {
        let mut path = path.clone();

        if let Some(Permission::Private) = self.fs_permissions.get(&path) {
            return requester_name.get_node_name() == self.node_name.get_node_name();
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
}
