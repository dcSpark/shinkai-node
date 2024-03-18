use crate::agent::queue::job_queue_manager::JobQueueManager;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::network::network_job::subscriber_manager_error::SubscriberManagerError;
use crate::network::Node;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_error::VectorFSError;
use crate::vector_fs::vector_fs_permissions::ReadPermission;
use crate::vector_fs::vector_fs_types::{FSEntry, FSFolder, FSItem};
use chrono::NaiveDateTime;
use chrono::{DateTime, Utc};
use serde_json::json;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionAction, ShinkaiSubscriptionRequest,
};
use shinkai_message_primitives::schemas::shinkai_subscription_req::ShinkaiFolderSubscription;
use shinkai_vector_resources::vector_resource::VRPath;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::Write;
use std::result::Result::Ok;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::{Mutex, MutexGuard};

use super::subscriber_manager::SubscriberManager;

#[derive(Debug, Clone)]
pub struct FSItemTree {
    pub name: String,
    pub path: String,
    pub last_modified: DateTime<Utc>,
    pub children: HashMap<String, Arc<FSItemTree>>,
}

impl FSItemTree {
    // Method to transform the tree into a visually pleasant JSON string
    pub fn to_pretty_json(&self) -> serde_json::Value {
        json!({
            "name": self.name,
            "path": self.path,
            "last_modified": self.last_modified.to_rfc3339(),
            "children": self.children.iter().map(|(_, child)| child.to_pretty_json()).collect::<Vec<_>>(),
        })
    }

    // Optionally, if you want to print it directly in a more human-readable format
    pub fn pretty_print(&self, indent: usize) {
        println!(
            "{}- {} ({}) [Last Modified: {}]",
            " ".repeat(indent * 2),
            self.name,
            self.path,
            self.last_modified.format("%Y-%m-%d %H:%M:%S")
        );
        for child in self.children.values() {
            child.pretty_print(indent + 1);
        }
    }
}

impl SubscriberManager {
    pub async fn shared_folders_to_tree(
        &self,
        requester_shinkai_identity: ShinkaiName,
        path: String,
    ) -> Result<FSItemTree, SubscriberManagerError> {
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;
        let mut vector_fs = vector_fs.lock().await;

        let vr_path = VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;
        let reader = vector_fs
            .new_reader(
                requester_shinkai_identity.clone(),
                vr_path,
                requester_shinkai_identity.clone(),
            )
            .map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

        let shared_folders = vector_fs.find_paths_with_read_permissions(&reader, vec![ReadPermission::Public])?;
        let filtered_results = self.filter_to_top_level_folders(shared_folders);

        let mut root_children: HashMap<String, Arc<FSItemTree>> = HashMap::new();
        for (path, _permission) in filtered_results {
            let reader = vector_fs
                .new_reader(
                    requester_shinkai_identity.clone(),
                    path.clone(),
                    requester_shinkai_identity.clone(),
                )
                .map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

            let result = vector_fs.retrieve_fs_entry(&reader);
            let fs_entry = result
                .map_err(|e| SubscriberManagerError::InvalidRequest(format!("Failed to retrieve fs entry: {}", e)))?;

            match fs_entry {
                FSEntry::Folder(fs_folder) => {
                    let folder_tree = Self::process_folder(&fs_folder, &path.to_string())?;
                    root_children.insert(fs_folder.name.clone(), Arc::new(folder_tree));
                }
                FSEntry::Item(fs_item) => {
                    // If you need to handle items at the root level, adjust here
                }
                _ => {} // Handle FSEntry::Root if necessary
            }
        }

        // Construct the root of the tree
        let tree = FSItemTree {
            name: "/".to_string(), // Adjust based on your root naming convention
            path: path,
            last_modified: Utc::now(),
            children: root_children,
        };

        Ok(tree)
    }

    // Adjusted to directly build FSItemTree structure
    fn process_folder(fs_folder: &FSFolder, parent_path: &str) -> Result<FSItemTree, SubscriberManagerError> {
        let mut children: HashMap<String, Arc<FSItemTree>> = HashMap::new();

        // Process child folders and add them to the children map
        for child_folder in &fs_folder.child_folders {
            let child_tree = Self::process_folder(child_folder, &format!("{}/{}", parent_path, child_folder.name))?;
            children.insert(child_folder.name.clone(), Arc::new(child_tree));
        }

        // Process child items and add them to the children map
        for child_item in &fs_folder.child_items {
            let child_path = format!("{}/{}", parent_path, child_item.name);
            let child_tree = FSItemTree {
                name: child_item.name.clone(),
                path: child_path,
                last_modified: child_item.last_written_datetime,
                children: HashMap::new(), // Items do not have children
            };
            children.insert(child_item.name.clone(), Arc::new(child_tree));
        }

        // Construct the current folder's tree
        let folder_tree = FSItemTree {
            name: fs_folder.name.clone(),
            path: parent_path.to_string(),
            last_modified: fs_folder.last_written_datetime,
            children,
        };

        Ok(folder_tree)
    }

    pub fn compare_fs_item_trees(&self, client_tree: &FSItemTree, server_tree: &FSItemTree) -> FSItemTree {
        let mut differences = FSItemTree {
            name: server_tree.name.clone(),
            path: server_tree.path.clone(),
            last_modified: server_tree.last_modified,
            children: HashMap::new(),
        };

        // Compare children of the current node
        for (child_name, server_child_tree) in &server_tree.children {
            if let Some(client_child_tree) = client_tree.children.get(child_name) {
                // If both trees have the child, compare them recursively
                let child_differences = self.compare_fs_item_trees(client_child_tree, server_child_tree);
                if !child_differences.children.is_empty()
                    || child_differences.last_modified != server_child_tree.last_modified
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
                    Arc::new(FSItemTree {
                        name: client_child_tree.name.clone(),
                        path: client_child_tree.path.clone(),
                        last_modified: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
                        children: HashMap::new(),
                    }),
                );
            }
        }

        differences
    }

    pub fn filter_to_top_level_folders(&self, results: Vec<(VRPath, ReadPermission)>) -> Vec<(VRPath, ReadPermission)> {
        let mut filtered_results: Vec<(VRPath, ReadPermission)> = Vec::new();
        for (path, permission) in results {
            let is_subpath = filtered_results.iter().any(|(acc_path, _): &(VRPath, ReadPermission)| {
                // Check if `path` is a subpath of `acc_path`
                if path.path_ids.len() > acc_path.path_ids.len() && path.path_ids.starts_with(&acc_path.path_ids) {
                    true
                } else {
                    false
                }
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
}
