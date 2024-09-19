use chrono::{DateTime, Utc};

use serde::de::Error as SerdeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use std::collections::HashMap;

use super::shared_folder_info::SharedFolderInfo;


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExternalNodeState {
    ResponseAvailable,
    ResponseError(String),
    WaitingForExtNodeResponse, // TODO: Actually this may never be used
    CachedOutdatedRequesting,
    CachedAvailableButStillRequesting,
    CachedNotAvailableRequesting,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedFoldersExternalNodeSM {
    pub node_name: ShinkaiName,
    pub last_ext_node_response: Option<DateTime<Utc>>,
    pub last_request_to_ext_node: Option<DateTime<Utc>>,
    pub last_updated: DateTime<Utc>,
    pub state: ExternalNodeState,
    pub response_last_updated: Option<DateTime<Utc>>,
    pub response: HashMap<String, SharedFolderInfo>,
}

impl SharedFoldersExternalNodeSM {
    /// Creates a new placeholder `SharedFoldersExternalNodeSM` with a specified state.
    ///
    /// # Arguments
    ///
    /// * `node_name` - A `ShinkaiName` representing the node name.
    /// * `outdated` - A boolean flag indicating whether the placeholder should be for outdated data.
    ///                If `true`, the state will be `CachedOutdatedRequesting`.
    ///                If `false`, the state will be `CachedNotAvailableRequesting`.
    ///
    /// # Returns
    ///
    /// A new instance of `SharedFoldersExternalNodeSM` with minimal initialization.
    pub fn new_placeholder(node_name: ShinkaiName, outdated: bool) -> Self {
        SharedFoldersExternalNodeSM {
            node_name,
            last_ext_node_response: None,
            last_request_to_ext_node: None,
            last_updated: Utc::now(),
            state: if outdated {
                ExternalNodeState::CachedOutdatedRequesting
            } else {
                ExternalNodeState::CachedNotAvailableRequesting
            },
            response_last_updated: None,
            response: HashMap::new(),
        }
    }

    /// Creates a new `SharedFoldersExternalNodeSM` with a specified node name and multiple `SharedFolderInfo` entries.
    ///
    /// # Arguments
    ///
    /// * `node_name` - A `ShinkaiName` representing the node name.
    /// * `folders_info` - A vector of `SharedFolderInfo` to be added to the response.
    ///
    /// # Returns
    ///
    /// A new instance of `SharedFoldersExternalNodeSM`.
    pub fn new_with_folders_info(node_name: ShinkaiName, folders_info: Vec<SharedFolderInfo>) -> Self {
        let mut response = HashMap::new();
        for folder_info in folders_info {
            response.insert(folder_info.path.clone(), folder_info);
        }

        SharedFoldersExternalNodeSM {
            node_name,
            last_ext_node_response: Some(Utc::now()),
            last_request_to_ext_node: Some(Utc::now()),
            last_updated: Utc::now(),
            state: ExternalNodeState::ResponseAvailable,
            response_last_updated: Some(Utc::now()),
            response,
        }
    }

    /// Updates the state of the `SharedFoldersExternalNodeSM`.
    ///
    /// # Arguments
    ///
    /// * `new_state` - The new `ExternalNodeState` to set.
    ///
    /// # Returns
    ///
    /// A new instance of `SharedFoldersExternalNodeSM` with the updated state.
    pub fn with_updated_state(mut self, new_state: ExternalNodeState) -> Self {
        self.state = new_state;
        self
    }
}

impl Serialize for SharedFoldersExternalNodeSM {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("SharedFoldersExternalNodeSM", 7)?;
        state.serialize_field("node_name", &self.node_name)?;
        state.serialize_field("last_ext_node_response", &self.last_ext_node_response)?;
        state.serialize_field("last_request_to_ext_node", &self.last_request_to_ext_node)?;
        state.serialize_field("last_updated", &self.last_updated)?;
        state.serialize_field("state", &self.state)?;
        state.serialize_field("response_last_updated", &self.response_last_updated)?;
        // Directly serialize the response as it now holds SharedFolderInfo
        state.serialize_field("response", &self.response)?;

        state.end()
    }
}

impl<'de> Deserialize<'de> for SharedFoldersExternalNodeSM {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            node_name: String,
            last_ext_node_response: Option<DateTime<Utc>>,
            last_request_to_ext_node: Option<DateTime<Utc>>,
            last_updated: DateTime<Utc>,
            state: ExternalNodeState,
            response_last_updated: Option<DateTime<Utc>>,
            response: Option<HashMap<String, SharedFolderInfo>>, // Updated to HashMap
        }

        let helper = Helper::deserialize(deserializer)?;

        let node_name = match ShinkaiName::new(helper.node_name) {
            Ok(name) => name,
            Err(e) => return Err(D::Error::custom(e.to_string())),
        };

        Ok(SharedFoldersExternalNodeSM {
            node_name,
            last_ext_node_response: helper.last_ext_node_response,
            last_request_to_ext_node: helper.last_request_to_ext_node,
            last_updated: helper.last_updated,
            state: helper.state,
            response_last_updated: helper.response_last_updated,
            response: helper.response.unwrap_or_default(), // Use the HashMap directly
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::subscription_manager::fs_entry_tree::FSEntryTree;

    use super::*;
    use chrono::{TimeZone, Utc};
    use serde_json;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[test]
    fn test_serialization_deserialization() {
        // Setup for FSEntryTree instances remains the same
        let child_item_tree_1 = FSEntryTree {
            name: "child1.txt".to_string(),
            path: "/path/to/file1/child1.txt".to_string(),
            last_modified: Utc.with_ymd_and_hms(2023, 6, 9, 10, 30, 0).unwrap(),
            web_link: None,
            children: HashMap::new(),
        };

        let child_item_tree_2 = FSEntryTree {
            name: "child2.txt".to_string(),
            path: "/path/to/file1/child2.txt".to_string(),
            last_modified: Utc.with_ymd_and_hms(2023, 6, 9, 10, 45, 0).unwrap(),
            web_link: None,
            children: HashMap::new(),
        };

        let mut children = HashMap::new();
        children.insert("child1".to_string(), Arc::new(child_item_tree_1));
        children.insert("child2".to_string(), Arc::new(child_item_tree_2));

        let item_tree_1 = FSEntryTree {
            name: "file1".to_string(),
            path: "/path/to/file1".to_string(),
            last_modified: Utc.with_ymd_and_hms(2023, 6, 9, 10, 0, 0).unwrap(),
            web_link: None,
            children,
        };
        
        let item_tree_2 = FSEntryTree {
            name: "file2".to_string(),
            path: "/path/to/file2".to_string(),
            last_modified: Utc.with_ymd_and_hms(2023, 6, 9, 11, 0, 0).unwrap(),
            web_link: None,
            children: HashMap::new(),
        };

        // Adjusted to create SharedFolderInfo instances
        let shared_folder_info_1 = SharedFolderInfo {
            path: "/path/to/file1".to_string(),
            profile: "profile1".to_string(),
            permission: "read_write".to_string(),
            tree: item_tree_1,
            subscription_requirement: None, // Assuming None for simplicity; adjust as needed
        };

        let shared_folder_info_2 = SharedFolderInfo {
            path: "/path/to/file2".to_string(),
            profile: "profile2".to_string(),
            permission: "read_only".to_string(),
            tree: item_tree_2,
            subscription_requirement: None, // Assuming None for simplicity; adjust as needed
        };

        // Adjusted to insert SharedFolderInfo instances into the response HashMap
        let mut response = HashMap::new();
        response.insert("file1".to_string(), shared_folder_info_1);
        response.insert("file2".to_string(), shared_folder_info_2);

        let external_node_sm = SharedFoldersExternalNodeSM {
            node_name: ShinkaiName::new("@@node1.shinkai".to_string()).unwrap(),
            last_ext_node_response: Some(Utc.with_ymd_and_hms(2023, 6, 9, 12, 0, 0).unwrap()),
            last_request_to_ext_node: Some(Utc.with_ymd_and_hms(2023, 6, 9, 11, 30, 0).unwrap()),
            last_updated: Utc.with_ymd_and_hms(2023, 6, 9, 12, 0, 0).unwrap(),
            state: ExternalNodeState::ResponseAvailable,
            response_last_updated: Some(Utc.with_ymd_and_hms(2023, 6, 9, 12, 0, 0).unwrap()),
            response, // Correctly using the HashMap with SharedFolderInfo
        };

        // Serialization and deserialization steps remain the same
        let serialized = serde_json::to_string(&external_node_sm).unwrap();
        let deserialized: SharedFoldersExternalNodeSM = serde_json::from_str(&serialized).unwrap();

        // Assertions need to be adjusted to not check for Some/None
        assert_eq!(deserialized.node_name, external_node_sm.node_name);
        assert_eq!(
            deserialized.last_ext_node_response,
            external_node_sm.last_ext_node_response
        );
        assert_eq!(
            deserialized.last_request_to_ext_node,
            external_node_sm.last_request_to_ext_node
        );
        assert_eq!(deserialized.last_updated, external_node_sm.last_updated);
        assert_eq!(deserialized.state, external_node_sm.state);
        assert_eq!(
            deserialized.response_last_updated,
            external_node_sm.response_last_updated
        );

        // Adjusted to directly compare HashMaps
        assert_eq!(deserialized.response.len(), external_node_sm.response.len());
        for (key, value) in deserialized.response {
            assert!(external_node_sm.response.contains_key(&key));
            let original_value = &external_node_sm.response[&key];
            // Adjusted to access the `name` field through the `tree` field of `SharedFolderInfo`
            assert_eq!(value.tree.name, original_value.tree.name);
            assert_eq!(value.path, original_value.path);
            assert_eq!(value.permission, original_value.permission);
            // Assuming `last_modified` is a field you want to compare, but it should be accessed correctly
            // For example, if you're comparing the last modified date of the root item in the tree:
            assert_eq!(value.tree.last_modified, original_value.tree.last_modified);

            // Assert that the children are compared correctly
            // This assumes you want to compare the children of the `tree` field in `SharedFolderInfo`
            assert_eq!(value.tree.children.len(), original_value.tree.children.len());
            for (child_key, child_value_arc) in &value.tree.children {
                assert!(original_value.tree.children.contains_key(child_key));
                let original_child_value_arc = &original_value.tree.children[child_key];
                // Since the values are wrapped in `Arc`, dereference them for comparison
                let child_value = child_value_arc.as_ref();
                let original_child_value = original_child_value_arc.as_ref();
                assert_eq!(child_value.name, original_child_value.name);
                assert_eq!(child_value.path, original_child_value.path);
                assert_eq!(child_value.last_modified, original_child_value.last_modified);
            }
        }
    }
}
