use serde_json::Value;
use shinkai_message_primitives::{
    schemas::shinkai_subscription_req::{FolderSubscription, SubscriptionPayment},
    shinkai_message::shinkai_message_schemas::FileDestinationCredentials,
};

use crate::{
    http_requests::PostRequestError,
    shinkai::{shinkai_manager_for_subs::ShinkaiManagerForSubs, shinkai_response_types::NodeHealthStatus},
};

pub struct SubscriptionManager {
    pub shinkai_manager_for_subs: ShinkaiManagerForSubs,
}

impl SubscriptionManager {
    pub async fn new(shinkai_manager_for_subs: ShinkaiManagerForSubs) -> Self {
        SubscriptionManager {
            shinkai_manager_for_subs,
        }
    }

    pub async fn check_node_health(&self) -> Result<NodeHealthStatus, &'static str> {
        self.shinkai_manager_for_subs.check_node_health().await
    }

    pub async fn get_my_node_folder(&self, path: String) -> Result<String, PostRequestError> {
        let resp = self.shinkai_manager_for_subs.get_node_folder(&path).await;
        match resp {
            Ok(resp) => {
                let formatted_tree = Self::format_tree_simple(&resp.to_string());
                Ok(formatted_tree)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn get_my_node_folder_raw(&self, path: String) -> Result<serde_json::Value, PostRequestError> {
        self.shinkai_manager_for_subs.get_node_folder(&path).await
    }

    pub async fn create_folder(&self, folder_name: String, path: String) -> Result<(), &'static str> {
        self.shinkai_manager_for_subs.create_folder(&folder_name, &path).await
    }

    pub async fn share_folder(
        &self,
        path: String,
        req: FolderSubscription,
        file_credentials: Option<FileDestinationCredentials>,
    ) -> Result<(), &'static str> {
        self.shinkai_manager_for_subs
            .create_share_folder(&path, req, file_credentials)
            .await
    }

    pub async fn subscribe_to_folder(
        &self,
        path: String,
        node_name: String,
        profile_name: String,
        subscription_req: SubscriptionPayment,
        http_preferred: Option<bool>,
        base_folder: Option<String>,
    ) -> Result<(), &'static str> {
        self.shinkai_manager_for_subs
            .subscribe_to_folder(
                &path,
                node_name,
                profile_name,
                subscription_req,
                http_preferred,
                base_folder,
            )
            .await
    }

    pub async fn my_subscriptions(&self) -> Result<Value, &'static str> {
        self.shinkai_manager_for_subs.my_subscriptions().await
    }

    pub async fn my_shared_folders(&self) -> Result<Value, &'static str> {
        let sender = self.shinkai_manager_for_subs.node_receiver.clone();
        let sender_profile = self.shinkai_manager_for_subs.node_receiver_subidentity.clone();
        self.shinkai_manager_for_subs
            .available_shared_items("/", sender, sender_profile)
            .await
    }

    pub async fn available_shared_items(
        &self,
        path: String,
        node_name: String,
        profile_name: String,
    ) -> Result<Value, &'static str> {
        self.shinkai_manager_for_subs
            .available_shared_items(&path, node_name, profile_name)
            .await
    }

    fn format_tree_simple(json_str: &str) -> String {
        let mut result = String::new();

        // Remove the outer double quotes from the JSON string
        let json_str = json_str.trim_matches('"');

        // Unescape the remaining string
        let json_str = json_str.replace("\\\"", "\"").replace("\\\\", "\\");

        // Attempt to parse the JSON string into a serde_json::Value
        match serde_json::from_str::<serde_json::Value>(&json_str) {
            Ok(val) => {
                // Directly call format_subtree to handle the root element,
                // which represents the entire JSON structure.
                // Since the root itself doesn't have a name and directly contains "child_folders",
                // we start with an empty indent and treat the whole structure as the initial subtree.
                result.push_str(&Self::format_subtree(&val, "", true));
            }
            Err(_) => {
                result.push_str("Failed to parse JSON\n");
            }
        }

        result
    }

    fn format_subtree(folder: &serde_json::Value, indent: &str, is_last: bool) -> String {
        let mut result = String::new();

        let new_indent = if is_last {
            format!("{}    ", indent)
        } else {
            format!("{}│   ", indent)
        };

        // Process child folders
        if let Some(child_folders) = folder["child_folders"].as_array() {
            for (index, child_folder) in child_folders.iter().enumerate() {
                let folder_name = child_folder["name"].as_str().unwrap_or("Unknown Folder");
                let prefix = if index < child_folders.len() - 1 {
                    "├── "
                } else {
                    "└── "
                };
                result.push_str(&format!("{}{}{}\n", indent, prefix, folder_name));
                result.push_str(&Self::format_subtree(
                    child_folder,
                    &new_indent,
                    index == child_folders.len() - 1,
                ));
            }
        }

        // Process child items within the folder
        if let Some(child_items) = folder["child_items"].as_array() {
            for (index, child_item) in child_items.iter().enumerate() {
                let item_name = child_item["name"].as_str().unwrap_or("Unknown Item");
                let prefix = if index < child_items.len() - 1 {
                    "├── "
                } else {
                    "└── "
                };
                result.push_str(&format!("{}{}{}\n", new_indent, prefix, item_name));
            }
        }

        result
    }
}

// TODO:
// - create a folder DONE
// - share a folder with free req DONE
// - unshare a folder PENDING - missing stuff in the node
// - subscribe to a folder (free) DONE
// - show my subscriptions DONE
// - available shared folders for node DONE
// - retrieve my stuff (tree . style though) PENDING
