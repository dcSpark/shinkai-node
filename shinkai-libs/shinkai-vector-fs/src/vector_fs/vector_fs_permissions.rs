use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_resource::{VRPath, VectorResourceSearch};
use std::{collections::HashMap, thread, time::Duration};
use tokio::sync::{RwLock, RwLockReadGuard};

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
#[derive(Debug)]
pub struct PermissionsIndex {
    /// Map which defines the kind of read and write permission per path in the VectorFS
    pub fs_permissions: RwLock<HashMap<VRPath, String>>,
    /// ShinkaiName of the profile this permissions index is for.
    pub profile_name: ShinkaiName,
}

impl Clone for PermissionsIndex {
    fn clone(&self) -> Self {
        loop {
            match self.fs_permissions.try_read() {
                Ok(fs_permissions_guard) => {
                    let cloned_fs_permissions = fs_permissions_guard.clone();
                    drop(fs_permissions_guard); // Explicitly drop the guard to release the lock
                    return PermissionsIndex {
                        fs_permissions: RwLock::new(cloned_fs_permissions),
                        profile_name: self.profile_name.clone(),
                    };
                }
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
        }
    }
}

impl Serialize for PermissionsIndex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        loop {
            match self.fs_permissions.try_read() {
                Ok(fs_permissions_guard) => {
                    let data = json!({
                        "fs_permissions": *fs_permissions_guard,
                        "profile_name": self.profile_name,
                    });
                    return data.serialize(serializer);
                }
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
        }
    }
}

#[derive(Deserialize)]
struct PermissionsIndexHelper {
    fs_permissions: HashMap<VRPath, String>,
    profile_name: ShinkaiName,
}

impl<'de> Deserialize<'de> for PermissionsIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize into the helper struct
        let helper = PermissionsIndexHelper::deserialize(deserializer)?;
        // Construct PermissionsIndex from the helper
        Ok(PermissionsIndex {
            fs_permissions: RwLock::new(helper.fs_permissions),
            profile_name: helper.profile_name,
        })
    }
}

impl PartialEq for PermissionsIndex {
    fn eq(&self, other: &Self) -> bool {
        // First, check if the profile names are equal. If not, return false immediately.
        if self.profile_name != other.profile_name {
            return false;
        }

        // Attempt to acquire read locks on both self and other fs_permissions.
        // Use a loop for retrying every 2ms indefinitely until successful.
        let self_fs_permissions = loop {
            match self.fs_permissions.try_read() {
                Ok(lock) => break lock,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
            }
        };

        let other_fs_permissions = loop {
            match other.fs_permissions.try_read() {
                Ok(lock) => break lock,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
            }
        };

        // Now that we have both locks, we can compare the HashMaps directly.
        *self_fs_permissions == *other_fs_permissions
    }
}

impl PermissionsIndex {
    /// Creates a new PermissionsIndex struct
    pub async fn new(profile_name: ShinkaiName) -> Self {
        let index = Self {
            fs_permissions: RwLock::new(HashMap::new()),
            profile_name,
        };
        // Set permissions for the FS root to be private by default (only for profile owner).
        // This unwrap is safe due to hard coded values.
        index
            .insert_path_permission(VRPath::new(), ReadPermission::Private, WritePermission::Private)
            .await
            .unwrap();
        index
    }

    /// Creates a new PermissionsIndex using an input hashmap and profile.
    pub fn from_hashmap(profile_name: ShinkaiName, json_permissions: HashMap<VRPath, String>) -> Self {
        Self {
            fs_permissions: RwLock::new(json_permissions),
            profile_name,
        }
    }

    // /// We can't use serde:Serialize because of the RwLock, so we need to manually serialize the struct.
    // pub async fn serialize_async(&self) -> JsonResult<String> {
    //     // Acquire the lock asynchronously
    //     let fs_permissions = self.fs_permissions.read().await;

    //     // Directly construct a serde_json::Value that represents the PermissionsIndex data
    //     let to_serialize = json!({
    //         "fs_permissions": *fs_permissions,
    //         "profile_name": self.profile_name,
    //     });

    //     // Serialize the serde_json::Value to a String
    //     serde_json::to_string(&to_serialize)
    // }

    /// Prepares a copy of the internal permissions hashmap to be used in a Vector Search, by appending
    /// a json serialized reader at a hardcoded key. which is very unlikely to be used normally.
    pub async fn export_permissions_hashmap_with_reader(&self, reader: &VFSReader) -> HashMap<VRPath, String> {
        // Asynchronously acquire a read lock and then clone the HashMap
        let mut hashmap = self.fs_permissions.read().await.clone();

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
    pub async fn get_path_permission(&self, path: &VRPath) -> Result<PathPermission, VectorFSError> {
        let permissions_map: RwLockReadGuard<HashMap<VRPath, String>> = self.fs_permissions.read().await;

        permissions_map
            .get(path)
            .map(|json| PathPermission::from_json(json))
            .transpose()?
            .ok_or_else(|| VectorFSError::NoPermissionEntryAtPath(path.clone()))
    }

    /// Inserts a path permission into the fs_permissions map. Note, this will overwrite the old read/write permissions
    /// for the path if they exist. The Whitelist for the path are preserved always in this method,
    /// even when neither read/write are still set as Whitelist.
    pub async fn insert_path_permission(
        &self,
        path: VRPath,
        read_permission: ReadPermission,
        write_permission: WritePermission,
    ) -> Result<(), VectorFSError> {
        // Acquire a read lock to access the current permissions
        let mut fs_permissions = self.fs_permissions.write().await;
        let whitelist = fs_permissions
            .get(&path)
            .and_then(|json| PathPermission::from_json(json).ok())
            .map_or_else(HashMap::new, |perm| perm.whitelist);

        let path_perm = PathPermission {
            read_permission,
            write_permission,
            whitelist,
        };
        fs_permissions.insert(path.clone(), path_perm.to_json()?);
        Ok(())
    }

    /// Copies the path permissions from the origin_path to the destination_path.
    /// Note, this will overwrite any old permission at the destination_path.
    pub async fn copy_path_permission(
        &self,
        origin_path: VRPath,
        destination_path: VRPath,
    ) -> Result<(), VectorFSError> {
        // Clone the origin permission JSON string while holding a read lock
        let origin_permission_json = {
            let fs_permissions = self.fs_permissions.read().await;
            fs_permissions.get(&origin_path).cloned()
        };

        // Now that the read lock is dropped, proceed with acquiring a write lock
        if let Some(origin_permission_json) = origin_permission_json {
            let origin_permission = PathPermission::from_json(&origin_permission_json)?;
            let mut fs_permissions_write = self.fs_permissions.write().await;
            fs_permissions_write.insert(destination_path, origin_permission.to_json()?);
            Ok(())
        } else {
            Err(VectorFSError::NoPermissionEntryAtPath(origin_path))
        }
    }

    /// Internal method which removes a permission from the fs_permissions map.
    /// Should only be used by VectorFS when deleting FSEntries entirely.
    pub async fn remove_path_permission(&self, path: VRPath) {
        let mut fs_permissions = self.fs_permissions.write().await;
        fs_permissions.remove(&path);
    }

    /// Inserts the WhitelistPermission for a ShinkaiName to the whitelist for a given path.
    pub async fn insert_to_whitelist(
        &self,
        path: VRPath,
        name: ShinkaiName,
        whitelist_perm: WhitelistPermission,
    ) -> Result<(), VectorFSError> {
        // Acquire a write lock to modify the permissions
        let mut fs_permissions = self.fs_permissions.write().await;

        // Check if the path exists and clone the JSON string if it does
        if let Some(path_permission_json) = fs_permissions.get(&path).cloned() {
            // Deserialize the JSON string into a PathPermission object
            let mut path_permission = PathPermission::from_json(&path_permission_json)?;

            // Insert the new whitelist permission
            path_permission.whitelist.insert(name, whitelist_perm);

            // Serialize the updated PathPermission object back into a JSON string
            let updated_json = path_permission.to_json()?;

            // Update the entry in the map
            fs_permissions.insert(path, updated_json);
        }
        Ok(())
    }

    /// Removes a ShinkaiName from the whitelist for a given path.
    pub async fn remove_from_whitelist(&self, path: VRPath, name: ShinkaiName) -> Result<(), VectorFSError> {
        // Acquire a write lock to modify the permissions
        let mut fs_permissions = self.fs_permissions.write().await;

        // Check if the path exists and clone the JSON string if it does
        if let Some(path_permission_json) = fs_permissions.get(&path).cloned() {
            // Deserialize the JSON string into a PathPermission object
            let mut path_permission = PathPermission::from_json(&path_permission_json)?;

            // Remove the ShinkaiName from the whitelist
            path_permission.whitelist.remove(&name);

            // Serialize the updated PathPermission object back into a JSON string
            let updated_json = path_permission.to_json()?;

            // Update the entry in the map
            fs_permissions.insert(path, updated_json);
        }
        Ok(())
    }

    /// Validates the permission for a given requester ShinkaiName + Path in the node's VectorFS.
    /// If it returns Ok(()), then permission has passed.
    pub fn validate_read_access(&self, requester_name: &ShinkaiName, path: &VRPath) -> Result<(), VectorFSError> {
        let mut path = path.clone();

        loop {
            // Acquire a read lock to access the permissions
            match self.fs_permissions.try_read() {
                Ok(fs_permissions) => {
                    {
                        let path_permission_json = fs_permissions.get(&path).cloned();
                        // Explicitly drop the lock here
                        drop(fs_permissions);

                        if let Some(json) = path_permission_json {
                            let path_permission = PathPermission::from_json(&json)?;

                            // Global profile owner check
                            if requester_name.get_profile_name_string() == self.profile_name.get_profile_name_string() {
                                return Ok(());
                            }

                            // Otherwise check specific permission
                            match &path_permission.read_permission {
                                // If Public, then reading is always allowed
                                ReadPermission::Public => return Ok(()),
                                // If private, then reading is allowed for the specific profile that owns the VectorFS
                                ReadPermission::Private => {
                                    if requester_name.get_profile_name_string()
                                        == self.profile_name.get_profile_name_string()
                                    {
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
                                    if let Some(whitelist_permission) =
                                        path_permission.get_whitelist_permission(requester_name)
                                    {
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
                Err(_) => {
                    eprintln!("Failed to acquire read lock for permissions index");
                    // Sleep for 2ms before retrying
                    thread::sleep(Duration::from_millis(2));
                }
            }
        }
    }

    /// Validates the permission for a given requester ShinkaiName + Path in the node's VectorFS.
    /// If it returns Ok(()), then permission has passed.
    pub fn validate_write_access(&self, requester_name: &ShinkaiName, path: &VRPath) -> Result<(), VectorFSError> {
        let mut path = path.clone();

        loop {
            // Attempt to acquire a read lock to access the permissions
            match self.fs_permissions.try_read() {
                Ok(fs_permissions) => {
                    {
                        let path_permission_json = fs_permissions.get(&path).cloned();
                        // Explicitly drop the lock here
                        drop(fs_permissions);

                        if let Some(path_permission_json) = path_permission_json {
                            let path_permission = PathPermission::from_json(&path_permission_json.clone())?;

                            // Global profile owner check
                            if requester_name.get_profile_name_string() == self.profile_name.get_profile_name_string() {
                                return Ok(());
                            }

                            // Otherwise check specific permission
                            match &path_permission.write_permission {
                                // If private, then writing is allowed for the specific profile that owns the VectorFS
                                WritePermission::Private => {
                                    if requester_name.get_profile_name_string()
                                        == self.profile_name.get_profile_name_string()
                                    {
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
                Err(_) => {
                    // Sleep for 2ms before retrying
                    thread::sleep(Duration::from_millis(2));
                }
            }
        }
    }

    /// Finds all paths that have one of the specified type of read permissions, starting from a given path, and returns them as a Vec.
    #[allow(dead_code)]
    async fn find_paths_with_read_permissions_as_vec(
        &self,
        starting_path: VRPath,
        read_permissions_to_find: Vec<ReadPermission>,
    ) -> Result<Vec<(VRPath, ReadPermission)>, VectorFSError> {
        let hashmap_result = self
            .find_paths_with_read_permissions_as_hashmap(starting_path, read_permissions_to_find)
            .await?;
        Ok(hashmap_result.into_iter().collect())
    }

    /// Finds all paths that have one of the specified type of read permissions, starting from a given path, and returns them as a HashMap.
    async fn find_paths_with_read_permissions_as_hashmap(
        &self,
        starting_path: VRPath,
        read_permissions_to_find: Vec<ReadPermission>,
    ) -> Result<HashMap<VRPath, ReadPermission>, VectorFSError> {
        let mut paths_with_permissions = HashMap::new();

        // Acquire a read lock to access the fs_permissions hashmap
        let fs_permissions = self.fs_permissions.read().await;

        // Iterate through the fs_permissions hashmap
        for (path, permission_json) in fs_permissions.iter() {
            // Check if the current path is a descendant of the starting path
            if starting_path.is_descendant_path(path) {
                match PathPermission::from_json(permission_json) {
                    Ok(path_permission) => {
                        if read_permissions_to_find.contains(&path_permission.read_permission) {
                            paths_with_permissions.insert(path.clone(), path_permission.read_permission.clone());
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        Ok(paths_with_permissions)
    }

    /// Finds all paths that have one of the specified type of write permissions, starting from a given path, and returns them as a Vec.
    pub async fn find_paths_with_write_permissions_as_vec(
        &self,
        starting_path: VRPath,
        write_permissions_to_find: Vec<WritePermission>,
    ) -> Result<Vec<(VRPath, WritePermission)>, VectorFSError> {
        let hashmap_result = self
            .find_paths_with_write_permissions_as_hashmap(starting_path, write_permissions_to_find)
            .await?;
        Ok(hashmap_result.into_iter().collect())
    }

    /// Finds all paths that have one of the specified type of write permissions, starting from a given path, and returns them as a HashMap.
    pub async fn find_paths_with_write_permissions_as_hashmap(
        &self,
        starting_path: VRPath,
        write_permissions_to_find: Vec<WritePermission>,
    ) -> Result<HashMap<VRPath, WritePermission>, VectorFSError> {
        let mut paths_with_permissions = HashMap::new();

        // Acquire a read lock to access the fs_permissions hashmap
        let fs_permissions = self.fs_permissions.read().await;

        // Iterate through the fs_permissions hashmap
        for (path, permission_json) in fs_permissions.iter() {
            // Check if the current path is a descendant of the starting path
            if starting_path.is_ancestor_path(path) {
                match PathPermission::from_json(permission_json) {
                    Ok(path_permission) => {
                        if write_permissions_to_find.contains(&path_permission.write_permission) {
                            paths_with_permissions.insert(path.clone(), path_permission.write_permission.clone());
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
    pub async fn validate_read_access_for_paths(
        &self,
        profile_name: ShinkaiName,
        name_to_check: ShinkaiName,
        paths: Vec<VRPath>,
    ) -> Result<(), VectorFSError> {
        for path in paths {
            let fs_internals = self.get_profile_fs_internals_cloned(&profile_name).await?;
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
    pub async fn validate_write_access_for_paths(
        &self,
        profile_name: ShinkaiName,
        name_to_check: ShinkaiName,
        paths: Vec<VRPath>,
    ) -> Result<(), VectorFSError> {
        for path in paths {
            let fs_internals = self.get_profile_fs_internals_cloned(&profile_name).await?;
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
    pub async fn get_path_permission_for_paths(
        &self,
        profile_name: ShinkaiName,
        paths: Vec<VRPath>,
    ) -> Result<Vec<(VRPath, PathPermission)>, VectorFSError> {
        let mut path_permissions = Vec::new();

        for path in paths {
            let fs_internals = self.get_profile_fs_internals_cloned(&profile_name).await?;
            match fs_internals.permissions_index.get_path_permission(&path).await {
                Ok(permission) => path_permissions.push((path, permission)),
                Err(e) => return Err(e),
            }
        }

        Ok(path_permissions)
    }

    /// Finds all paths that have one of the specified types of read permissions, starting from the path in the given VFSReader.
    /// Includes folders, sub-folders and items.
    pub async fn find_paths_with_read_permissions_as_vec(
        &self,
        reader: &VFSReader,
        read_permissions_to_find: Vec<ReadPermission>,
    ) -> Result<Vec<(VRPath, ReadPermission)>, VectorFSError> {
        let hashmap_result = self
            .find_paths_with_read_permissions_as_hashmap(reader, read_permissions_to_find)
            .await?;

        Ok(hashmap_result.into_iter().collect())
    }

    /// Finds all paths that have one of the specified types of read permissions, starting from the path in the given VFSReader.
    /// TODO: Remove the logic to fetch the actual available paths in the FS, and just make perms match the reality (find the bug).
    pub async fn find_paths_with_read_permissions_as_hashmap(
        &self,
        reader: &VFSReader,
        read_permissions_to_find: Vec<ReadPermission>,
    ) -> Result<HashMap<VRPath, ReadPermission>, VectorFSError> {
        let fs_internals = self.get_profile_fs_internals_cloned(&reader.profile).await?;

        // Fetches the actual available paths in the FS. // TODO: Remove this and make sure perms are actually accurate.
        let ret_nodes = fs_internals.fs_core_resource.retrieve_nodes_exhaustive_unordered(None);
        let mut all_internals_paths = HashMap::new();
        ret_nodes.iter().for_each(|p| {
            all_internals_paths.insert(p.retrieval_path.clone(), true);
        });

        let hashmap_result = fs_internals
            .permissions_index
            .find_paths_with_read_permissions_as_hashmap(reader.path.clone(), read_permissions_to_find)
            .await?;

        let final_result = hashmap_result
            .into_iter()
            .filter(|(path, _)| all_internals_paths.contains_key(path))
            .collect();

        Ok(final_result)
    }

    /// Finds all paths that have one of the specified types of write permissions, starting from the path in the given VFSReader.
    /// TODO: Remove the logic to fetch the actual available paths in the FS, and just make perms match the reality (find the bug).
    pub async fn find_paths_with_write_permissions_as_vec(
        &self,
        reader: &VFSReader,
        write_permissions_to_find: Vec<WritePermission>,
    ) -> Result<Vec<(VRPath, WritePermission)>, VectorFSError> {
        let hashmap_result = self
            .find_paths_with_write_permissions_as_hashmap(reader, write_permissions_to_find)
            .await?;

        Ok(hashmap_result.into_iter().collect())
    }

    /// Finds all paths that have one of the specified types of write permissions, starting from the path in the given VFSReader.
    pub async fn find_paths_with_write_permissions_as_hashmap(
        &self,
        reader: &VFSReader,
        write_permissions_to_find: Vec<WritePermission>,
    ) -> Result<HashMap<VRPath, WritePermission>, VectorFSError> {
        let fs_internals = self.get_profile_fs_internals_cloned(&reader.profile).await?;

        // Fetches the actual available paths in the FS. // TODO: Remove this and make sure perms are actually accurate.
        let ret_nodes = fs_internals.fs_core_resource.retrieve_nodes_exhaustive_unordered(None);
        let mut all_internals_paths = HashMap::new();
        ret_nodes.iter().for_each(|p| {
            all_internals_paths.insert(p.retrieval_path.clone(), true);
        });

        let hashmap_result = fs_internals
            .permissions_index
            .find_paths_with_write_permissions_as_hashmap(reader.path.clone(), write_permissions_to_find)
            .await?;

        let final_result = hashmap_result
            .into_iter()
            .filter(|(path, _)| all_internals_paths.contains_key(path))
            .collect();

        Ok(final_result)
    }

    /// Sets the read/write permissions for the FSEntry at the writer's path (overwrites).
    /// This action is only allowed to be performed by the profile owner.
    /// No remove_path_permission is implemented, as all FSEntries must have a path permission.
    pub async fn set_path_permission(
        &self,
        writer: &VFSWriter,
        read_permission: ReadPermission,
        write_permission: WritePermission,
    ) -> Result<(), VectorFSError> {
        // Example of acquiring a write lock if internals_map is wrapped in an RwLock
        let internals_map = self.internals_map.write().await;

        if let Some(fs_internals) = internals_map.get(&writer.profile) {
            if writer.requester_name == writer.profile {
                fs_internals
                    .permissions_index
                    .insert_path_permission(writer.path.clone(), read_permission, write_permission)
                    .await?;
                self.save_profile_fs_internals(fs_internals.clone(), &writer.profile)
                    .await?;
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
    pub async fn set_whitelist_permission(
        &self,
        writer: &VFSWriter,
        name_to_whitelist: ShinkaiName,
        whitelist_perm: WhitelistPermission,
    ) -> Result<(), VectorFSError> {
        // Acquire a write lock on internals_map to ensure thread-safe access
        let internals_map = self.internals_map.write().await;

        if let Some(fs_internals) = internals_map.get(&writer.profile) {
            if writer.requester_name == writer.profile {
                // Ensure the operation on permissions_index is awaited if it's an async operation
                fs_internals
                    .permissions_index
                    .insert_to_whitelist(writer.path.clone(), name_to_whitelist, whitelist_perm)
                    .await?;
                // Assuming save_profile_fs_internals is an async operation, ensure it's awaited
                self.save_profile_fs_internals(fs_internals.clone(), &writer.profile)
                    .await?;
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
    pub async fn remove_whitelist_permission(
        &self,
        writer: &VFSWriter,
        name_to_remove: ShinkaiName,
    ) -> Result<(), VectorFSError> {
        // Acquire a write lock on internals_map to ensure thread-safe access
        let mut internals_map = self.internals_map.write().await;

        if let Some(fs_internals) = internals_map.get_mut(&writer.profile) {
            if writer.requester_name == writer.profile {
                // Perform the removal operation, ensuring it's awaited if it's an async operation
                fs_internals
                    .permissions_index
                    .remove_from_whitelist(writer.path.clone(), name_to_remove)
                    .await?;
                // Assuming save_profile_fs_internals is an async operation, ensure it's awaited
                self.save_profile_fs_internals(fs_internals.clone(), &writer.profile)
                    .await?;
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
