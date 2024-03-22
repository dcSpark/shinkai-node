use chrono::{DateTime, Utc};
use serde::de::Error as SerdeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use std::collections::HashMap;
use std::sync::Arc;

use super::fs_item_tree::FSItemTree;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExternalNodeState {
    ResponseAvailable,
    ResponseError(String),
    WaitingForExtNodeResponse,
    CachedOutdatedRequesting,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedFoldersExternalNodeSM {
    // node name
    // last ext node response Option<> time based
    // last request to ext node Option<>
    // last updated
    // states: ResponseAvailable, ResponseError(String), WaitingForExtNodeResponse, CachedOutdatedRequesting
    // response last updated Option<>
    // response Option<HashMap<String, Arc<FSItemTree>>,
    pub node_name: ShinkaiName,
    pub last_ext_node_response: Option<DateTime<Utc>>,
    pub last_request_to_ext_node: Option<DateTime<Utc>>,
    pub last_updated: DateTime<Utc>,
    pub state: ExternalNodeState,
    pub response_last_updated: Option<DateTime<Utc>>,
    pub response: Option<HashMap<String, Arc<FSItemTree>>>,
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

        if let Some(ref response) = self.response {
            let serialized_response: HashMap<_, _> =
                response.iter().map(|(k, v)| (k.clone(), v.as_ref().clone())).collect();
            state.serialize_field("response", &serialized_response)?;
        } else {
            state.serialize_field("response", &None::<HashMap<String, FSItemTree>>)?;
        }

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
            response: Option<HashMap<String, FSItemTree>>,
        }

        let helper = Helper::deserialize(deserializer)?;
        let response = helper
            .response
            .map(|r| r.into_iter().map(|(k, v)| (k, Arc::new(v))).collect());

        // Handle the potential error from ShinkaiName::new manually
        let node_name = match ShinkaiName::new(helper.node_name) {
            Ok(name) => name,
            Err(e) => {
                // Convert the error into the deserializer's error type
                return Err(D::Error::custom(e.to_string()));
            }
        };

        Ok(SharedFoldersExternalNodeSM {
            node_name,
            last_ext_node_response: helper.last_ext_node_response,
            last_request_to_ext_node: helper.last_request_to_ext_node,
            last_updated: helper.last_updated,
            state: helper.state,
            response_last_updated: helper.response_last_updated,
            response,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use serde_json;

    #[test]
    fn test_serialization_deserialization() {
        let child_item_tree_1 = FSItemTree {
            name: "child1.txt".to_string(),
            path: "/path/to/file1/child1.txt".to_string(),
            last_modified: Utc.ymd(2023, 6, 9).and_hms(10, 30, 0),
            children: HashMap::new(),
        };

        let child_item_tree_2 = FSItemTree {
            name: "child2.txt".to_string(),
            path: "/path/to/file1/child2.txt".to_string(),
            last_modified: Utc.ymd(2023, 6, 9).and_hms(10, 45, 0),
            children: HashMap::new(),
        };

        let mut children = HashMap::new();
        children.insert("child1".to_string(), Arc::new(child_item_tree_1));
        children.insert("child2".to_string(), Arc::new(child_item_tree_2));

        let item_tree_1 = FSItemTree {
            name: "file1".to_string(),
            path: "/path/to/file1".to_string(),
            last_modified: Utc.ymd(2023, 6, 9).and_hms(10, 0, 0),
            children,
        };

        let item_tree_2 = FSItemTree {
            name: "file2".to_string(),
            path: "/path/to/file2".to_string(),
            last_modified: Utc.ymd(2023, 6, 9).and_hms(11, 0, 0),
            children: HashMap::new(),
        };

        let mut response = HashMap::new();
        response.insert("file1".to_string(), Arc::new(item_tree_1));
        response.insert("file2".to_string(), Arc::new(item_tree_2));

        let external_node_sm = SharedFoldersExternalNodeSM {
            node_name: ShinkaiName::new("@@node1.shinkai".to_string()).unwrap(),
            last_ext_node_response: Some(Utc.ymd(2023, 6, 9).and_hms(12, 0, 0)),
            last_request_to_ext_node: Some(Utc.ymd(2023, 6, 9).and_hms(11, 30, 0)),
            last_updated: Utc.ymd(2023, 6, 9).and_hms(12, 0, 0),
            state: ExternalNodeState::ResponseAvailable,
            response_last_updated: Some(Utc.ymd(2023, 6, 9).and_hms(12, 0, 0)),
            response: Some(response),
        };

        // Serialize the external_node_sm to JSON
        let serialized = serde_json::to_string(&external_node_sm).unwrap();

        // Deserialize the JSON back into SharedFoldersExternalNodeSM
        let deserialized: SharedFoldersExternalNodeSM = serde_json::from_str(&serialized).unwrap();

        // Assert that the deserialized struct matches the original
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

        // Assert that the deserialized response matches the original
        assert_eq!(deserialized.response.is_some(), external_node_sm.response.is_some());
        if let (Some(deserialized_response), Some(original_response)) =
            (deserialized.response, external_node_sm.response)
        {
            assert_eq!(deserialized_response.len(), original_response.len());
            for (key, value) in deserialized_response {
                assert!(original_response.contains_key(&key));
                assert_eq!(value.name, original_response[&key].name);
                assert_eq!(value.path, original_response[&key].path);
                assert_eq!(value.last_modified, original_response[&key].last_modified);

                // Assert that the children are deserialized correctly
                assert_eq!(value.children.len(), original_response[&key].children.len());
                for (child_key, child_value) in &value.children {
                    assert!(original_response[&key].children.contains_key(child_key));
                    assert_eq!(child_value.name, original_response[&key].children[child_key].name);
                    assert_eq!(child_value.path, original_response[&key].children[child_key].path);
                    assert_eq!(
                        child_value.last_modified,
                        original_response[&key].children[child_key].last_modified
                    );
                }
            }
        }
    }
}
