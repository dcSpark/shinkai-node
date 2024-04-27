use super::fs_entry_tree::FSEntryTree;
use crate::network::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_permissions::ReadPermission;
use crate::vector_fs::vector_fs_types::{FSEntry, FSFolder};
use chrono::NaiveDateTime;
use chrono::{DateTime, Utc};
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
                        let folder_tree = Self::process_folder(&fs_folder, &path.to_string())?;
                        root_children.insert(fs_folder.name.clone(), Arc::new(folder_tree));
                    }
                    FSEntry::Item(fs_item) => {
                        let item_tree = FSEntryTree {
                            name: fs_item.name.clone(),
                            path: path.clone().to_string(),
                            last_modified: fs_item.last_written_datetime,
                            children: HashMap::new(), // Items do not have children
                        };
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
            children: root_children,
        };

        Ok(tree)
    }

    // Adjusted to directly build FSEntryTree structure
    fn process_folder(fs_folder: &FSFolder, parent_path: &str) -> Result<FSEntryTree, SubscriberManagerError> {
        let mut children: HashMap<String, Arc<FSEntryTree>> = HashMap::new();

        // Process child folders and add them to the children map
        for child_folder in &fs_folder.child_folders {
            let child_tree = Self::process_folder(child_folder, &format!("{}/{}", parent_path, child_folder.name))?;
            children.insert(child_folder.name.clone(), Arc::new(child_tree));
        }

        // Process child items and add them to the children map
        for child_item in &fs_folder.child_items {
            let child_path = format!("{}/{}", parent_path, child_item.name);
            let child_tree = FSEntryTree {
                name: child_item.name.clone(),
                path: child_path,
                last_modified: child_item.last_written_datetime,
                children: HashMap::new(), // Items do not have children
            };
            children.insert(child_item.name.clone(), Arc::new(child_tree));
        }

        // Construct the current folder's tree
        let folder_tree = FSEntryTree {
            name: fs_folder.name.clone(),
            path: parent_path.to_string(),
            last_modified: fs_folder.last_written_datetime,
            children,
        };

        Ok(folder_tree)
    }

    pub fn compare_fs_item_trees(client_tree: &FSEntryTree, server_tree: &FSEntryTree) -> FSEntryTree {
        let mut differences = FSEntryTree {
            name: server_tree.name.clone(),
            path: server_tree.path.clone(),
            last_modified: server_tree.last_modified,
            children: HashMap::new(),
        };

        // Compare children of the current node
        for (child_name, server_child_tree) in &server_tree.children {
            if let Some(client_child_tree) = client_tree.children.get(child_name) {
                // If both trees have the child, compare them recursively
                let child_differences = Self::compare_fs_item_trees(client_child_tree, server_child_tree);
                if !child_differences.children.is_empty()
                    || child_differences.last_modified != server_child_tree.last_modified
                    || child_differences.last_modified != client_child_tree.last_modified
                // Check if the last_modified dates are different
                {
                    differences
                        .children
                        .insert(child_name.clone(), Arc::new(child_differences));
                }
            } else {
                // If the child is missing in the client tree, add it to the differences
                differences
                    .children
                    .insert(child_name.clone(), server_child_tree.clone());
            }
        }

        // Check for items that are present in the client tree but missing in the server tree
        for (child_name, client_child_tree) in &client_tree.children {
            if !server_tree.children.contains_key(child_name) {
                // Mark the item as deleted in the differences tree by setting its last_modified to a specific value, e.g., the epoch start
                differences.children.insert(
                    child_name.clone(),
                    Arc::new(FSEntryTree {
                        name: client_child_tree.name.clone(),
                        path: client_child_tree.path.clone(),
                        last_modified: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
                        children: HashMap::new(),
                    }),
                );
            }
        }

        // If there are no differences in children and the last_modified dates are the same, consider the trees identical
        if differences.children.is_empty() && differences.last_modified == client_tree.last_modified {
            differences.last_modified = client_tree.last_modified; // Ensure the last_modified date reflects any potential differences
        }

        differences
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
                let folder_tree = Self::process_folder(&fs_folder, &fs_folder.path.clone().format_to_string())?;
                Ok(folder_tree)
            }
            FSEntry::Item(fs_item) => {
                // Process items as before, since they do not have children
                let item_tree = FSEntryTree {
                    name: fs_item.name.clone(),
                    path: fs_item.path.clone().format_to_string(), // Use the item's path directly
                    last_modified: fs_item.last_written_datetime,
                    children: HashMap::new(), // Items do not have children
                };
                Ok(item_tree)
            }
            _ => Err(SubscriberManagerError::InvalidRequest(
                "Unsupported FSEntry type".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use shinkai_vector_resources::vector_resource::VRPath;

    fn create_test_tree() -> FSEntryTree {
        let shinkai_intro_crypto = FSEntryTree {
            name: "shinkai_intro".to_string(),
            path: "/shared_test_folder/crypto/shinkai_intro".to_string(),
            last_modified: Utc.ymd(2024, 2, 26).and_hms(23, 6, 0),
            children: HashMap::new(),
        };

        let zeko_intro_crypto = FSEntryTree {
            name: "zeko_intro".to_string(),
            path: "/shared_test_folder/crypto/zeko_intro".to_string(),
            last_modified: Utc.ymd(2024, 2, 26).and_hms(23, 6, 0),
            children: HashMap::new(),
        };

        let crypto_folder = FSEntryTree {
            name: "crypto".to_string(),
            path: "/shared_test_folder/crypto".to_string(),
            last_modified: Utc.ymd(2024, 3, 18).and_hms(3, 54, 25),
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
            last_modified: Utc.ymd(2024, 2, 26).and_hms(23, 6, 0),
            children: HashMap::new(),
        };

        let shared_test_folder = FSEntryTree {
            name: "shared_test_folder".to_string(),
            path: "/shared_test_folder".to_string(),
            last_modified: Utc.ymd(2024, 3, 18).and_hms(3, 54, 25),
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
            last_modified: Utc.ymd(2024, 3, 18).and_hms(3, 54, 27),
            children: {
                let mut children = HashMap::new();
                children.insert(shared_test_folder.name.clone(), Arc::new(shared_test_folder));
                children
            },
        }
    }

    #[test]
    fn test_compare_fs_item_trees_with_empty_client_tree() {
        let server_tree = create_test_tree();
        let client_tree = FSEntryTree {
            name: "/".to_string(),
            path: "/".to_string(),
            last_modified: Utc.ymd(2024, 3, 18).and_hms(3, 54, 27),
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
        let new_date = Utc.ymd(2024, 2, 25).and_hms(23, 6, 0); // Set to an older date
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
            Utc.ymd(2024, 2, 26).and_hms(23, 6, 0),
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
}
