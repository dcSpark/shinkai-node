use super::fs_entry_tree::{FSEntryTree, WebLink};
use super::http_manager::http_upload_manager::FileLink;
use crate::network::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_permissions::ReadPermission;
use crate::vector_fs::vector_fs_types::{FSEntry, FSFolder};
use chrono::{DateTime, Utc};
use chrono::{NaiveDateTime, TimeZone};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::vector_resource::VRPath;
use std::collections::HashMap;
use std::result::Result::Ok;
use std::sync::{Arc, Weak};
pub struct FSEntryTreeGenerator {}

impl FSEntryTreeGenerator {
    /// Builds an FSEntryTree for a profile's VectorFS starting at a specific path
    pub async fn shared_folders_to_tree(
        vector_fs: Weak<VectorFS>,
        full_streamer_profile_subidentity: ShinkaiName,
        full_subscriber_profile_subidentity: ShinkaiName,
        path: String,
        http_subscription_results: Vec<FileLink>,
    ) -> Result<FSEntryTree, SubscriberManagerError> {
        // Acquire VectorFS
        let vector_fs = vector_fs.upgrade().ok_or(SubscriberManagerError::VectorFSNotAvailable(
            "VectorFS instance is not available".to_string(),
        ))?;

        // Create Reader and find paths with read permissions
        let vr_path = VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

        // Use the full origin profile subidentity for both Reader inputs to only fetch all paths with public (or whitelist later) read perms without issues.
        let perms_reader = vector_fs
            .new_reader(
                full_streamer_profile_subidentity.clone(),
                vr_path,
                full_streamer_profile_subidentity.clone(),
            )
            .await
            .map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;
        let shared_folders = vector_fs
            .find_paths_with_read_permissions_as_vec(&perms_reader, vec![ReadPermission::Public])
            .await?;

        let filtered_results = Self::filter_to_top_level_folders(shared_folders); // Note: do we need this?
        // Convert HTTP subscription results to a HashMap
        let http_results_map: HashMap<String, FileLink> = http_subscription_results
            .into_iter()
            .map(|file_link| {
                let path = if file_link.path.ends_with(".checksum") {
                    // Find the position to cut the last 8 characters before ".checksum"
                    let cut_position = file_link.path.rfind(".checksum").unwrap() - 9; // 9 to account for the dot before the 8 characters
                    let mut new_path = file_link.path[..cut_position].to_string();
                    new_path.push_str(".checksum");
                    new_path
                } else {
                    file_link.path.clone()
                };
                (path, file_link)
            })
            .collect();

        // Create the FSEntryTree by iterating through results, fetching the FSEntry, and then parsing/adding it into the tree
        let mut root_children: HashMap<String, Arc<FSEntryTree>> = HashMap::new();
        for (path, _permission) in filtered_results {
            // Now use the requester subidentity for actual perm checking. Required for whitelist perms in the future.
            if let Ok(reader) = vector_fs
                .new_reader(
                    full_subscriber_profile_subidentity.clone(),
                    path.clone(),
                    full_streamer_profile_subidentity.clone(),
                )
                .await
            {
                let fs_entry = vector_fs.retrieve_fs_entry(&reader).await?;

                match fs_entry {
                    FSEntry::Folder(fs_folder) => {
                        let folder_tree = Self::process_folder(&fs_folder, &path.to_string(), &http_results_map)?;
                        root_children.insert(fs_folder.name.clone(), Arc::new(folder_tree));
                    }
                    FSEntry::Item(fs_item) => {
                        let mut item_tree = FSEntryTree {
                            name: fs_item.name.clone(),
                            path: path.clone().to_string(),
                            last_modified: fs_item.last_written_datetime,
                            web_link: None,
                            children: HashMap::new(), // Items do not have children
                        };

                        // Convert VRPath to String for map lookup
                        let path_str = path.to_string();

                        // Check if there is a corresponding HTTP link
                        if let Some(file_link) = http_results_map.get(&path_str) {
                            let checksum_path = format!("{}.{}.checksum", path_str, file_link.last_8_hash); // Correctly format the checksum path
                            item_tree.web_link = Some(WebLink {
                                file: file_link.clone(),
                                checksum: http_results_map
                                    .get(&checksum_path)
                                    .cloned()
                                    .unwrap_or_else(|| FileLink {
                                        link: String::new(), // Provide a default or handle this case as needed
                                        path: checksum_path,
                                        last_8_hash: String::new(), // Default or appropriate value
                                        expiration: file_link.expiration, // Use the same expiration or handle appropriately
                                    }),
                            });
                            item_tree.last_modified = DateTime::<Utc>::from(file_link.expiration);
                            // Convert SystemTime to DateTime<Utc>
                        }

                        root_children.insert(fs_item.name.clone(), Arc::new(item_tree));
                    }
                    _ => {} // Handle FSEntry::Root if necessary
                }
            }
        }

        // Construct the root of the tree
        let tree = FSEntryTree {
            name: "/".to_string(),
            path,
            last_modified: Utc::now(),
            web_link: None,
            children: root_children,
        };

        Ok(tree)
    }

    // Adjusted to directly build FSEntryTree structure
    fn process_folder(
        fs_folder: &FSFolder,
        parent_path: &str,
        http_results_map: &HashMap<String, FileLink>,
    ) -> Result<FSEntryTree, SubscriberManagerError> {
        let mut children: HashMap<String, Arc<FSEntryTree>> = HashMap::new();

        // Process child folders and add them to the children map
        for child_folder in &fs_folder.child_folders {
            let child_tree = Self::process_folder(
                child_folder,
                &format!("{}/{}", parent_path, child_folder.name),
                http_results_map,
            )?;
            children.insert(child_folder.name.clone(), Arc::new(child_tree));
        }

        // Process child items and add them to the children map
        for child_item in &fs_folder.child_items {
            let child_path = format!("{}/{}", parent_path, child_item.name);
            let mut child_tree = FSEntryTree {
                name: child_item.name.clone(),
                path: child_path.clone(),
                last_modified: child_item.last_written_datetime,
                web_link: None,
                children: HashMap::new(), // Items do not have children
            };

            // Check if there is a corresponding HTTP link
            if let Some(file_link) = http_results_map.get(&child_path) {
                let checksum_path = format!("{}.{}.checksum", child_path, file_link.last_8_hash); // Correctly format the checksum path
                child_tree.web_link = Some(WebLink {
                    file: file_link.clone(),
                    checksum: http_results_map
                        .get(&checksum_path)
                        .cloned()
                        .unwrap_or_else(|| FileLink {
                            link: String::new(), // Provide a default or handle this case as needed
                            path: checksum_path,
                            last_8_hash: String::new(),       // Default or appropriate value
                            expiration: file_link.expiration, // Use the same expiration or handle appropriately
                        }),
                });
                child_tree.last_modified = DateTime::<Utc>::from(file_link.expiration);
                // Convert SystemTime to DateTime<Utc>
            }

            children.insert(child_item.name.clone(), Arc::new(child_tree));
        }

        // Construct the current folder's tree
        let folder_tree = FSEntryTree {
            name: fs_folder.name.clone(),
            path: parent_path.to_string(),
            last_modified: fs_folder.last_written_datetime,
            web_link: None,
            children,
        };

        Ok(folder_tree)
    }

    pub fn compare_fs_item_trees(client_tree: &FSEntryTree, server_tree: &FSEntryTree) -> FSEntryTree {
        let mut differences = FSEntryTree {
            name: server_tree.name.clone(),
            path: server_tree.path.clone(),
            last_modified: server_tree.last_modified,
            web_link: None,
            children: HashMap::new(),
        };

        // Compare children of the current node for server to client
        for (child_name, server_child_tree) in &server_tree.children {
            if let Some(client_child_tree) = client_tree.children.get(child_name) {
                // If both trees have the child, compare them recursively
                let child_differences = Self::compare_fs_item_trees(client_child_tree, server_child_tree);
                if !child_differences.children.is_empty()
                    || child_differences.last_modified != server_child_tree.last_modified
                {
                    differences
                        .children
                        .insert(child_name.clone(), Arc::new(child_differences));
                }
                // Check if the last_modified dates are different, even if the children are the same
                if client_child_tree.last_modified != server_child_tree.last_modified {
                    differences
                        .children
                        .insert(child_name.clone(), server_child_tree.clone());
                }
            } else {
                // Server has an item/folder client doesn't have; it's a new item/folder for the client
                differences
                    .children
                    .insert(child_name.clone(), server_child_tree.clone());
            }
        }

        // Compare children of the current node for client to server (looking for deletions)
        for (child_name, client_child_tree) in &client_tree.children {
            if !server_tree.children.contains_key(child_name) {
                // Client has an item/folder server doesn't have; it's deleted in the server
                let deleted_item = Arc::new(Self::mark_as_deleted(client_child_tree));
                differences.children.insert(child_name.clone(), deleted_item);
            }
        }

        differences
    }

    fn mark_as_deleted(entry: &FSEntryTree) -> FSEntryTree {
        let mut deleted_children = HashMap::new();
        for (child_name, child_tree) in &entry.children {
            deleted_children.insert(child_name.clone(), Arc::new(Self::mark_as_deleted(child_tree)));
        }

        FSEntryTree {
            name: entry.name.clone(),
            path: entry.path.clone(),
            last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            web_link: None,
            children: deleted_children,
        }
    }

    pub fn filter_to_top_level_folders(results: Vec<(VRPath, ReadPermission)>) -> Vec<(VRPath, ReadPermission)> {
        let mut filtered_results: Vec<(VRPath, ReadPermission)> = Vec::new();
        for (path, permission) in results {
            let is_subpath = filtered_results.iter().any(|(acc_path, _): &(VRPath, ReadPermission)| {
                // Check if `path` is a subpath of `acc_path`
                path.path_ids.len() > acc_path.path_ids.len() && path.path_ids.starts_with(&acc_path.path_ids)
            });

            if !is_subpath {
                // Before adding, make sure it's not a parent path of an already added path
                filtered_results.retain(|(acc_path, _): &(VRPath, ReadPermission)| {
                    if acc_path.path_ids.len() > path.path_ids.len() && acc_path.path_ids.starts_with(&path.path_ids) {
                        false // Remove if current path is a parent of the acc_path
                    } else {
                        true
                    }
                });
                filtered_results.push((path, permission));
            }
        }
        filtered_results
    }

    pub fn fs_entry_to_tree(entry: FSEntry) -> Result<FSEntryTree, SubscriberManagerError> {
        match entry {
            FSEntry::Folder(fs_folder) => {
                // Use the existing process_folder function to correctly handle folders and their children
                let empty_http_results_map = HashMap::new();
                let folder_tree = Self::process_folder(
                    &fs_folder,
                    &fs_folder.path.clone().format_to_string(),
                    &empty_http_results_map,
                )?;
                Ok(folder_tree)
            }
            FSEntry::Item(fs_item) => {
                // Process items as before, since they do not have children
                let item_tree = FSEntryTree {
                    name: fs_item.name.clone(),
                    path: fs_item.path.clone().format_to_string(), // Use the item's path directly
                    last_modified: fs_item.last_written_datetime,
                    web_link: None,
                    children: HashMap::new(), // Items do not have children
                };
                Ok(item_tree)
            }
            _ => Err(SubscriberManagerError::InvalidRequest(
                "Unsupported FSEntry type".to_string(),
            )),
        }
    }

    /// Returns a new FSEntryTree with the specified prefix removed from all paths.
    pub fn remove_prefix_from_paths(tree: &FSEntryTree, prefix: &str) -> FSEntryTree {
        let new_path = if tree.path.starts_with(prefix) {
            tree.path.replacen(prefix, "", 1)
        } else {
            tree.path.clone()
        };

        let new_children = tree
            .children
            .iter()
            .map(|(name, child)| {
                let new_child = Self::remove_prefix_from_paths(child, prefix);
                (name.clone(), Arc::new(new_child))
            })
            .collect();

        FSEntryTree {
            name: tree.name.clone(),
            path: new_path,
            last_modified: tree.last_modified,
            web_link: tree.web_link.clone(),
            children: new_children,
        }
    }

    /// Identifies all deletions within a given FSEntryTree.
    /// A deletion is indicated by an item's last_modified date being set to the epoch start.
    pub fn find_deletions(tree: &FSEntryTree) -> Vec<String> {
        let mut deletions = Vec::new();
        Self::find_deletions_recursive(tree, &mut deletions);
        deletions
    }

    /// Recursive helper function to traverse the FSEntryTree and collect paths of deleted items.
    fn find_deletions_recursive(tree: &FSEntryTree, deletions: &mut Vec<String>) {
        // Check if the current node is marked as deleted
        let is_deleted = tree.last_modified == Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap());

        // Recurse into the child tree to find more deletions first before deciding on the current node
        let mut child_deletions = 0;
        for child_tree in tree.children.values() {
            Self::find_deletions_recursive(child_tree, deletions);
            if child_tree.last_modified == Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()) {
                child_deletions += 1;
            }
        }

        // If the current node is marked as deleted and none of its children are marked as deleted, add it to deletions
        if is_deleted && child_deletions == 0 {
            deletions.push(tree.path.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone};
    use shinkai_vector_resources::vector_resource::VRPath;

    fn create_test_tree() -> FSEntryTree {
        let shinkai_intro_crypto = FSEntryTree {
            name: "shinkai_intro".to_string(),
            path: "/shared_test_folder/crypto/shinkai_intro".to_string(),
            last_modified: Utc.with_ymd_and_hms(2024, 2, 26, 23, 6, 0).unwrap(),
            web_link: None,
            children: HashMap::new(),
        };

        let zeko_intro_crypto = FSEntryTree {
            name: "zeko_intro".to_string(),
            path: "/shared_test_folder/crypto/zeko_intro".to_string(),
            last_modified: Utc.with_ymd_and_hms(2024, 2, 26, 23, 6, 0).unwrap(),
            web_link: None,
            children: HashMap::new(),
        };

        let crypto_folder = FSEntryTree {
            name: "crypto".to_string(),
            path: "/shared_test_folder/crypto".to_string(),
            last_modified: Utc.with_ymd_and_hms(2024, 3, 18, 3, 54, 25).unwrap(),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(shinkai_intro_crypto.name.clone(), Arc::new(shinkai_intro_crypto));
                children.insert(zeko_intro_crypto.name.clone(), Arc::new(zeko_intro_crypto));
                children
            },
        };

        let shinkai_intro_folder = FSEntryTree {
            name: "shinkai_intro".to_string(),
            path: "/shared_test_folder/shinkai_intro".to_string(),
            web_link: None,
            last_modified: Utc.with_ymd_and_hms(2024, 2, 26, 23, 6, 0).unwrap(),
            children: HashMap::new(),
        };

        let shared_test_folder = FSEntryTree {
            name: "shared_test_folder".to_string(),
            path: "/shared_test_folder".to_string(),
            web_link: None,
            last_modified: Utc.with_ymd_and_hms(2024, 3, 18, 3, 54, 25).unwrap(),
            children: {
                let mut children = HashMap::new();
                children.insert(crypto_folder.name.clone(), Arc::new(crypto_folder));
                children.insert(shinkai_intro_folder.name.clone(), Arc::new(shinkai_intro_folder));
                children
            },
        };

        FSEntryTree {
            name: "/".to_string(),
            path: "/".to_string(),
            web_link: None,
            last_modified: Utc.with_ymd_and_hms(2024, 3, 18, 3, 54, 27).unwrap(),
            children: {
                let mut children = HashMap::new();
                children.insert(shared_test_folder.name.clone(), Arc::new(shared_test_folder));
                children
            },
        }
    }

    // Helper function to create a client tree with an extra item
    fn create_test_tree_with_extra_item() -> FSEntryTree {
        let mut tree = create_test_tree(); // Use the existing function to create a base tree

        // Add an extra item to simulate a deletion scenario
        if let Some(shared_test_folder_arc) = tree.children.get("shared_test_folder").cloned() {
            let mut shared_test_folder = (*shared_test_folder_arc).clone();

            let extra_item = FSEntryTree {
                name: "extra_item".to_string(),
                path: "/shared_test_folder/extra_item".to_string(),
                web_link: None,
                last_modified: Utc::now(),
                children: HashMap::new(), // Assuming it's an item without children
            };

            shared_test_folder
                .children
                .insert("extra_item".to_string(), Arc::new(extra_item));

            tree.children
                .insert("shared_test_folder".to_string(), Arc::new(shared_test_folder));
        }

        tree
    }

    #[test]
    fn test_compare_fs_item_trees_with_empty_client_tree() {
        let server_tree = create_test_tree();
        let client_tree = FSEntryTree {
            name: "/".to_string(),
            path: "/".to_string(),
            web_link: None,
            last_modified: Utc.with_ymd_and_hms(2024, 3, 18, 3, 54, 27).unwrap(),
            children: HashMap::new(),
        };

        let differences = FSEntryTreeGenerator::compare_fs_item_trees(&client_tree, &server_tree);
        eprintln!("Differences: {:#?}", differences);
        assert_eq!(
            differences.children.len(),
            1,
            "Expected differences in the root children"
        );
    }

    fn remove_crypto_from_shared_test_folder(mut tree: FSEntryTree) -> FSEntryTree {
        if let Some(shared_test_folder_arc) = tree.children.get("shared_test_folder") {
            let mut shared_test_folder =
                Arc::try_unwrap(shared_test_folder_arc.clone()).unwrap_or_else(|arc| (*arc).clone());

            // Perform the modification
            shared_test_folder.children.remove("crypto");

            // Replace the modified folder back into the tree
            tree.children
                .insert("shared_test_folder".to_string(), Arc::new(shared_test_folder));
        }
        tree
    }

    #[test]
    fn test_compare_fs_item_trees_with_partial_client_tree() {
        let server_tree = create_test_tree();
        let client_tree = create_test_tree(); // Assuming this returns FSEntryTree

        // Modify the client_tree to simulate the removal of the "crypto" folder
        let client_tree_modified = remove_crypto_from_shared_test_folder(client_tree);

        let differences = FSEntryTreeGenerator::compare_fs_item_trees(&client_tree_modified, &server_tree);
        eprintln!(
            "test_compare_fs_item_trees_with_partial_client_tree Differences: {:#?}",
            differences
        );
        assert!(
            differences
                .children
                .get("shared_test_folder")
                .unwrap()
                .children
                .contains_key("crypto"),
            "Expected 'crypto' folder to be in the differences"
        );
    }

    fn modify_zeko_intro_date(mut tree: FSEntryTree, new_date: DateTime<Utc>) -> FSEntryTree {
        // Attempt to directly access and modify the shared_test_folder if it exists
        if let Some(shared_test_folder_arc) = tree.children.get("shared_test_folder").cloned() {
            let mut shared_test_folder = (*shared_test_folder_arc).clone();

            // Attempt to directly access and modify the crypto folder if it exists
            if let Some(crypto_folder_arc) = shared_test_folder.children.get("crypto").cloned() {
                let mut crypto_folder = (*crypto_folder_arc).clone();

                // Check if zeko_intro exists and modify its date
                if crypto_folder.children.contains_key("zeko_intro") {
                    if let Some(zeko_intro_arc) = crypto_folder.children.get("zeko_intro").cloned() {
                        let mut zeko_intro = (*zeko_intro_arc).clone();
                        zeko_intro.last_modified = new_date;
                        crypto_folder
                            .children
                            .insert("zeko_intro".to_string(), Arc::new(zeko_intro));
                    }
                }

                shared_test_folder
                    .children
                    .insert("crypto".to_string(), Arc::new(crypto_folder));
            }

            tree.children
                .insert("shared_test_folder".to_string(), Arc::new(shared_test_folder));
        }
        tree
    }

    #[test]
    fn test_compare_fs_item_trees_with_date_difference() {
        let server_tree = create_test_tree();
        let client_tree = create_test_tree(); // Clone the server tree for the client

        // Modify the date of "zeko_intro" in the client tree to simulate an older version
        let new_date = Utc.with_ymd_and_hms(2024, 2, 25, 23, 6, 0).unwrap(); // Set to an older date
        let client_tree_modified = modify_zeko_intro_date(client_tree, new_date);

        let differences = FSEntryTreeGenerator::compare_fs_item_trees(&client_tree_modified, &server_tree);
        eprintln!(
            "test_compare_fs_item_trees_with_date_difference Differences: {:#?}",
            differences
        );

        // Check if the differences include the "zeko_intro" with the updated date
        assert!(
            differences
                .children
                .get("shared_test_folder")
                .unwrap()
                .children
                .get("crypto")
                .unwrap()
                .children
                .contains_key("zeko_intro"),
            "Expected 'zeko_intro' folder with date difference to be in the differences"
        );

        // Additionally, check if the last_modified date of "zeko_intro" in the differences matches the server's date
        let zeko_intro_diff = differences
            .children
            .get("shared_test_folder")
            .unwrap()
            .children
            .get("crypto")
            .unwrap()
            .children
            .get("zeko_intro")
            .unwrap();
        assert_eq!(
            zeko_intro_diff.last_modified,
            Utc.with_ymd_and_hms(2024, 2, 26, 23, 6, 0).unwrap(),
            "Expected 'zeko_intro' last_modified date in differences to match the server's date"
        );
    }

    #[test]
    fn test_empty_input() {
        let results = vec![];
        let filtered = FSEntryTreeGenerator::filter_to_top_level_folders(results);
        assert!(filtered.is_empty(), "Expected no results for empty input");
    }

    #[test]
    fn test_no_subpaths() {
        let results = vec![
            (VRPath::from_string("/folder1").unwrap(), ReadPermission::Public),
            (VRPath::from_string("/folder2").unwrap(), ReadPermission::Public),
        ];
        let filtered = FSEntryTreeGenerator::filter_to_top_level_folders(results);
        assert_eq!(filtered.len(), 2, "Expected all unique paths to be returned");
    }

    #[test]
    fn test_with_subpaths() {
        let results = vec![
            (VRPath::from_string("/folder").unwrap(), ReadPermission::Public),
            (
                VRPath::from_string("/folder/subfolder1").unwrap(),
                ReadPermission::Public,
            ),
            (
                VRPath::from_string("/folder/subfolder1/subfolder2").unwrap(),
                ReadPermission::Public,
            ),
            (VRPath::from_string("/another_folder").unwrap(), ReadPermission::Public),
        ];
        let filtered = FSEntryTreeGenerator::filter_to_top_level_folders(results);
        assert_eq!(filtered.len(), 2, "Expected only top-level paths to be returned");
    }

    #[test]
    fn test_with_complex_hierarchy() {
        let results = vec![
            (
                VRPath::from_string("/folder/subfolder1").unwrap(),
                ReadPermission::Public,
            ),
            (
                VRPath::from_string("/folder/subfolder1/subfolder2").unwrap(),
                ReadPermission::Public,
            ),
            (VRPath::from_string("/folder").unwrap(), ReadPermission::Public),
            (VRPath::from_string("/another_folder").unwrap(), ReadPermission::Public),
            (
                VRPath::from_string("/independent_folder").unwrap(),
                ReadPermission::Public,
            ),
            (
                VRPath::from_string("/folder/subfolder3").unwrap(),
                ReadPermission::Public,
            ),
        ];
        let filtered = FSEntryTreeGenerator::filter_to_top_level_folders(results);
        assert_eq!(
            filtered.len(),
            3,
            "Expected only distinct top-level paths to be returned"
        );
    }

    #[test]
    fn test_compare_fs_item_trees_with_deletion() {
        let server_tree = create_test_tree(); // Assuming this returns FSEntryTree with all items
        let client_tree = create_test_tree_with_extra_item(); // Create a modified tree that simulates an extra item in the client

        // Perform the comparison
        let differences = FSEntryTreeGenerator::compare_fs_item_trees(&client_tree, &server_tree);
        eprintln!(
            "test_compare_fs_item_trees_with_deletion Differences: {:#?}",
            differences
        );

        // Check if the differences include the "deleted" item
        assert!(
            differences
                .children
                .get("shared_test_folder")
                .unwrap()
                .children
                .contains_key("extra_item"),
            "Expected 'extra_item' to be marked as deleted in the differences"
        );

        // Additionally, check if the last_modified date of "extra_item" in the differences matches the epoch start
        let extra_item_diff = differences
            .children
            .get("shared_test_folder")
            .unwrap()
            .children
            .get("extra_item")
            .unwrap();
        assert_eq!(
            extra_item_diff.last_modified,
            Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            "Expected 'extra_item' last_modified date in differences to indicate deletion"
        );

        // Now use find_deletions to verify it identifies the "deleted" item correctly
        let deletions = FSEntryTreeGenerator::find_deletions(&differences);
        assert_eq!(
            deletions.len(),
            1,
            "Expected to find one deletion in the differences tree"
        );
        assert_eq!(
            deletions[0], extra_item_diff.path,
            "Expected the path of the deleted item to match"
        );
    }

    #[test]
    fn test_compare_fs_item_trees_with_new_and_deleted_items() {
        // Local shared folder state
        let local_shared_folder_state = FSEntryTree {
            name: "/".to_string(),
            path: "/shared_test_folder".to_string(),
            web_link: None,
            last_modified: Utc.with_ymd_and_hms(2024, 4, 21, 3, 17, 13).unwrap(),
            children: {
                let mut children = HashMap::new();
                children.insert(
                    "crypto".to_string(),
                    Arc::new(FSEntryTree {
                        name: "crypto".to_string(),
                        path: "/shared_test_folder/crypto".to_string(),
                        web_link: None,
                        last_modified: Utc.with_ymd_and_hms(2024, 4, 21, 3, 17, 0).unwrap(),
                        children: {
                            let mut crypto_children = HashMap::new();
                            crypto_children.insert(
                                "shinkai_intro".to_string(),
                                Arc::new(FSEntryTree {
                                    name: "shinkai_intro".to_string(),
                                    path: "/shared_test_folder/crypto/shinkai_intro".to_string(),
                                    web_link: None,
                                    last_modified: Utc.with_ymd_and_hms(2024, 4, 3, 2, 41, 16).unwrap(),
                                    children: HashMap::new(),
                                }),
                            );
                            crypto_children
                        },
                    }),
                );
                children.insert(
                    "shinkai_intro".to_string(),
                    Arc::new(FSEntryTree {
                        name: "shinkai_intro".to_string(),
                        path: "/shared_test_folder/shinkai_intro".to_string(),
                        web_link: None,
                        last_modified: Utc.with_ymd_and_hms(2024, 4, 3, 2, 41, 16).unwrap(),
                        children: HashMap::new(),
                    }),
                );
                children
            },
        };

        // Subscriber folder state
        let subscriber_folder_state = FSEntryTree {
            name: "shared_test_folder".to_string(),
            path: "/shared_test_folder".to_string(),
            last_modified: Utc.with_ymd_and_hms(2024, 4, 21, 3, 17, 13).unwrap(),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(
                    "crypto".to_string(),
                    Arc::new(FSEntryTree {
                        name: "crypto".to_string(),
                        path: "/shared_test_folder/crypto".to_string(),
                        last_modified: Utc.with_ymd_and_hms(2024, 4, 21, 3, 17, 0).unwrap(),
                        web_link: None,
                        children: {
                            let mut crypto_children = HashMap::new();
                            crypto_children.insert(
                                "shinkai_intro".to_string(),
                                Arc::new(FSEntryTree {
                                    name: "shinkai_intro".to_string(),
                                    path: "/shared_test_folder/crypto/shinkai_intro".to_string(),
                                    last_modified: Utc.with_ymd_and_hms(2024, 4, 3, 2, 41, 16).unwrap(),
                                    web_link: None,
                                    children: HashMap::new(),
                                }),
                            );
                            crypto_children
                        },
                    }),
                );
                children.insert(
                    "shinkai_intro".to_string(),
                    Arc::new(FSEntryTree {
                        name: "shinkai_intro".to_string(),
                        path: "/shared_test_folder/shinkai_intro".to_string(),
                        last_modified: Utc.with_ymd_and_hms(2024, 4, 3, 2, 41, 16).unwrap(),
                        web_link: None,
                        children: HashMap::new(),
                    }),
                );
                children.insert(
                    "zeko".to_string(),
                    Arc::new(FSEntryTree {
                        name: "zeko".to_string(),
                        path: "/shared_test_folder/zeko".to_string(),
                        last_modified: Utc.with_ymd_and_hms(2024, 4, 21, 3, 17, 13).unwrap(),
                        web_link: None,
                        children: {
                            let mut zeko_children = HashMap::new();
                            zeko_children.insert(
                                "paper".to_string(),
                                Arc::new(FSEntryTree {
                                    name: "paper".to_string(),
                                    path: "/shared_test_folder/zeko/paper".to_string(),
                                    last_modified: Utc.with_ymd_and_hms(2024, 4, 21, 3, 17, 12).unwrap(),
                                    web_link: None,
                                    children: {
                                        let mut paper_children = HashMap::new();
                                        paper_children.insert(
                                            "shinkai_intro".to_string(),
                                            Arc::new(FSEntryTree {
                                                name: "shinkai_intro".to_string(),
                                                path: "/shared_test_folder/zeko/paper/shinkai_intro".to_string(),
                                                last_modified: Utc.with_ymd_and_hms(2024, 4, 20, 6, 38, 43).unwrap(),
                                                web_link: None,
                                                children: HashMap::new(),
                                            }),
                                        );
                                        paper_children
                                    },
                                }),
                            );
                            zeko_children
                        },
                    }),
                );
                children
            },
        };

        // Expected Diff
        let expected_diff = FSEntryTree {
            name: "/".to_string(),
            path: "/shared_test_folder".to_string(),
            last_modified: Utc.with_ymd_and_hms(2024, 4, 21, 3, 17, 13).unwrap(),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(
                    "zeko".to_string(),
                    Arc::new(FSEntryTree {
                        name: "zeko".to_string(),
                        path: "/shared_test_folder/zeko".to_string(),
                        last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()), // This indicates deletion
                        web_link: None,
                        children: {
                            let mut zeko_children = HashMap::new();
                            zeko_children.insert(
                                "paper".to_string(),
                                Arc::new(FSEntryTree {
                                    name: "paper".to_string(),
                                    path: "/shared_test_folder/zeko/paper".to_string(),
                                    last_modified: Utc
                                        .from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()), // This indicates deletion
                                    web_link: None,
                                    children: {
                                        let mut paper_children = HashMap::new();
                                        paper_children.insert(
                                            "shinkai_intro".to_string(),
                                            Arc::new(FSEntryTree {
                                                name: "shinkai_intro".to_string(),
                                                path: "/shared_test_folder/zeko/paper/shinkai_intro".to_string(),
                                                last_modified: Utc.from_utc_datetime(
                                                    &NaiveDateTime::from_timestamp_opt(0, 0).unwrap(),
                                                ), // This indicates deletion
                                                web_link: None,
                                                children: HashMap::new(),
                                            }),
                                        );
                                        paper_children
                                    },
                                }),
                            );
                            zeko_children
                        },
                    }),
                );
                children
            },
        };

        // Perform the comparison
        let differences =
            FSEntryTreeGenerator::compare_fs_item_trees(&subscriber_folder_state, &local_shared_folder_state);

        eprintln!(
            "test_compare_fs_item_trees_with_new_and_deleted_items Differences: {:#?}",
            differences
        );

        assert_eq!(
            differences, expected_diff,
            "The actual differences do not match the expected differences."
        );

        // Check if the differences match the expected diff
        assert_eq!(
            differences.children.len(),
            1,
            "Differences in children count do not match expected"
        );

        // Check for specific differences
        assert!(
            !differences.children.contains_key("crypto"),
            "Expected 'crypto' not to be present in the differences as there are no changes"
        );
        assert!(
            differences.children.contains_key("zeko"),
            "Expected 'zeko' to be marked as new in the differences"
        );

        // Check if 'zeko' is correctly marked as deleted
        let zeko_diff = differences.children.get("zeko").unwrap();
        assert_eq!(
            zeko_diff.last_modified,
            Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            "Expected 'zeko' last_modified date to indicate deletion"
        );

        // Check for the presence of 'paper' within 'zeko', despite 'zeko' being marked as deleted
        assert!(
            zeko_diff.children.contains_key("paper"),
            "Expected 'paper' to be present in the 'zeko' differences"
        );

        // Verify the 'paper' details
        let paper_diff = zeko_diff.children.get("paper").unwrap();
        assert_eq!(
            paper_diff.last_modified,
            Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            "Expected 'paper' last_modified date to match the expected state"
        );

        // Check for the presence of 'shinkai_intro' within 'paper'
        assert!(
            paper_diff.children.contains_key("shinkai_intro"),
            "Expected 'shinkai_intro' to be present in the 'paper' differences"
        );

        // Verify the 'shinkai_intro' details within 'paper'
        let shinkai_intro_diff = paper_diff.children.get("shinkai_intro").unwrap();
        assert_eq!(
            shinkai_intro_diff.last_modified,
            Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            "Expected 'shinkai_intro' within 'paper' last_modified date to match the expected state"
        );
    }

    #[test]
    fn test_find_deletions_with_mixed_deletion_states() {
        // Construct a tree where a folder contains both deleted and non-deleted items
        let root = FSEntryTree {
            name: "root".to_string(),
            path: "/".to_string(),
            last_modified: Utc::now(),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(
                    "folder1".to_string(),
                    Arc::new(FSEntryTree {
                        name: "folder1".to_string(),
                        path: "/folder1".to_string(),
                        last_modified: Utc::now(), // Not marked as deleted
                        web_link: None,
                        children: {
                            let mut folder1_children = HashMap::new();
                            // Non-deleted item
                            folder1_children.insert(
                                "item1".to_string(),
                                Arc::new(FSEntryTree {
                                    name: "item1".to_string(),
                                    path: "/folder1/item1".to_string(),
                                    last_modified: Utc::now(), // Not marked as deleted
                                    web_link: None,
                                    children: HashMap::new(),
                                }),
                            );
                            // Deleted item
                            folder1_children.insert(
                                "item2".to_string(),
                                Arc::new(FSEntryTree {
                                    name: "item2".to_string(),
                                    path: "/folder1/item2".to_string(),
                                    last_modified: Utc
                                        .from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()), // Marked as deleted
                                    web_link: None,
                                    children: HashMap::new(),
                                }),
                            );
                            folder1_children
                        },
                    }),
                );
                children.insert(
                    "folder2".to_string(),
                    Arc::new(FSEntryTree {
                        name: "folder2".to_string(),
                        path: "/folder2".to_string(),
                        last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()), // Marked as deleted
                        web_link: None,
                        children: HashMap::new(), // No children
                    }),
                );
                children
            },
        };

        let deletions = FSEntryTreeGenerator::find_deletions(&root);

        // Verify that "/folder1/item2" and "/folder2" are identified as deleted
        assert_eq!(deletions.len(), 2, "Expected to find two deletions");
        assert!(
            deletions.contains(&"/folder1/item2".to_string()),
            "Expected '/folder1/item2' to be identified as deleted"
        );
        assert!(
            deletions.contains(&"/folder2".to_string()),
            "Expected '/folder2' to be identified as deleted"
        );
    }

    #[test]
    fn test_find_single_deletion_with_specific_structure() {
        let shinkai_intro = FSEntryTree {
            name: "shinkai_intro".to_string(),
            path: "/shared_test_folder/zeko/paper/shinkai_intro".to_string(),
            last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            web_link: None,
            children: HashMap::new(),
        };

        let paper = FSEntryTree {
            name: "paper".to_string(),
            path: "/shared_test_folder/zeko/paper".to_string(),
            last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(shinkai_intro.name.clone(), Arc::new(shinkai_intro));
                children
            },
        };

        let zeko = FSEntryTree {
            name: "zeko".to_string(),
            path: "/shared_test_folder/zeko".to_string(),
            last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(paper.name.clone(), Arc::new(paper));
                children
            },
        };

        let root = FSEntryTree {
            name: "/".to_string(),
            path: "/shared_test_folder".to_string(),
            last_modified: Utc.with_ymd_and_hms(2024, 4, 21, 4, 10, 2).unwrap(),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(zeko.name.clone(), Arc::new(zeko));
                children
            },
        };

        let deletions = FSEntryTreeGenerator::find_deletions(&root);

        assert_eq!(deletions.len(), 1, "Expected to find one deletion");
        assert_eq!(
            deletions[0], "/shared_test_folder/zeko/paper/shinkai_intro",
            "Expected deletion path to be '/shared_test_folder/zeko/paper/shinkai_intro'"
        );
    }

    #[test]
    fn test_find_multiple_deletions_with_specific_structure() {
        let shinkai_intro = FSEntryTree {
            name: "shinkai_intro".to_string(),
            path: "/shared_test_folder/zeko/paper/shinkai_intro".to_string(),
            last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            web_link: None,
            children: HashMap::new(),
        };

        let zeko_intro = FSEntryTree {
            name: "zeko_intro".to_string(),
            path: "/shared_test_folder/zeko/paper/zeko_intro".to_string(),
            last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            web_link: None,
            children: HashMap::new(),
        };

        let paper = FSEntryTree {
            name: "paper".to_string(),
            path: "/shared_test_folder/zeko/paper".to_string(),
            last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(shinkai_intro.name.clone(), Arc::new(shinkai_intro));
                children.insert(zeko_intro.name.clone(), Arc::new(zeko_intro));
                children
            },
        };

        let zeko = FSEntryTree {
            name: "zeko".to_string(),
            path: "/shared_test_folder/zeko".to_string(),
            last_modified: Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(0, 0).unwrap()),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(paper.name.clone(), Arc::new(paper));
                children
            },
        };

        let root = FSEntryTree {
            name: "/".to_string(),
            path: "/shared_test_folder".to_string(),
            last_modified: Utc.with_ymd_and_hms(2024, 4, 21, 4, 10, 2).unwrap(),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(zeko.name.clone(), Arc::new(zeko));
                children
            },
        };

        let deletions = FSEntryTreeGenerator::find_deletions(&root);
        eprintln!("Deletions: {:#?}", deletions);

        assert_eq!(deletions.len(), 2, "Expected to find two deletions");
        assert!(
            deletions.contains(&"/shared_test_folder/zeko/paper/shinkai_intro".to_string()),
            "Expected deletion path to include '/shared_test_folder/zeko/paper/shinkai_intro'"
        );
        assert!(
            deletions.contains(&"/shared_test_folder/zeko/paper/zeko_intro".to_string()),
            "Expected deletion path to include '/shared_test_folder/zeko/paper/zeko_intro'"
        );
    }

    #[test]
    fn test_remove_prefix_from_paths() {
        let tree = FSEntryTree {
            name: "shared test folder".to_string(),
            path: "/My Subscriptions/shared test folder".to_string(),
            last_modified: Utc::now(),
            web_link: None,
            children: {
                let mut children = HashMap::new();
                children.insert(
                    "shinkai_intro".to_string(),
                    Arc::new(FSEntryTree {
                        name: "shinkai_intro".to_string(),
                        path: "/My Subscriptions/shared test folder/shinkai_intro".to_string(),
                        last_modified: Utc::now(),
                        web_link: None,
                        children: HashMap::new(),
                    }),
                );
                children.insert(
                    "crypto".to_string(),
                    Arc::new(FSEntryTree {
                        name: "crypto".to_string(),
                        path: "/My Subscriptions/shared test folder/crypto".to_string(),
                        last_modified: Utc::now(),
                        web_link: None,
                        children: {
                            let mut crypto_children = HashMap::new();
                            crypto_children.insert(
                                "shinkai_intro".to_string(),
                                Arc::new(FSEntryTree {
                                    name: "shinkai_intro".to_string(),
                                    path: "/My Subscriptions/shared test folder/crypto/shinkai_intro".to_string(),
                                    last_modified: Utc::now(),
                                    web_link: None,
                                    children: HashMap::new(),
                                }),
                            );
                            crypto_children
                        },
                    }),
                );
                children
            },
        };

        let new_tree = FSEntryTreeGenerator::remove_prefix_from_paths(&tree, "/My Subscriptions");

        assert_eq!(new_tree.path, "/shared test folder");
        assert_eq!(
            new_tree.children["shinkai_intro"].path,
            "/shared test folder/shinkai_intro"
        );
        assert_eq!(new_tree.children["crypto"].path, "/shared test folder/crypto");
        assert_eq!(
            new_tree.children["crypto"].children["shinkai_intro"].path,
            "/shared test folder/crypto/shinkai_intro"
        );
    }
}
