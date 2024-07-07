use chrono::{DateTime, Utc};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use std::collections::HashMap;
use std::result::Result::Ok;
use std::sync::Arc;

use super::http_manager::http_upload_manager::FileLink;

// Custom serialization for the children field
fn serialize_children<S>(children: &HashMap<String, Arc<FSEntryTree>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let temp_children: HashMap<String, FSEntryTree> =
        children.iter().map(|(k, v)| (k.clone(), (**v).clone())).collect();
    temp_children.serialize(serializer)
}

// Custom deserialization for the children field
fn deserialize_children<'de, D>(deserializer: D) -> Result<HashMap<String, Arc<FSEntryTree>>, D::Error>
where
    D: Deserializer<'de>,
{
    let temp_children: HashMap<String, FSEntryTree> = HashMap::deserialize(deserializer)?;
    Ok(temp_children.into_iter().map(|(k, v)| (k, Arc::new(v))).collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebLink {
    pub file: FileLink,
    pub checksum: FileLink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FSEntryTree {
    pub name: String,
    pub path: String,
    pub last_modified: DateTime<Utc>,
    pub web_link: Option<WebLink>,
    #[serde(serialize_with = "serialize_children", deserialize_with = "deserialize_children")]
    pub children: HashMap<String, Arc<FSEntryTree>>,
}

impl FSEntryTree {
    pub fn new_empty() -> Self {
        FSEntryTree {
            name: "/".to_string(),
            path: "/".to_string(),
            last_modified: Utc::now(),
            children: HashMap::new(),
            web_link: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.name == "/" && self.path == "/" && self.children.is_empty()
    }

    /// A folder may be empty if it has not children, so this method could be wrong on that case
    pub fn is_folder(&self) -> bool {
        !self.children.is_empty()
    }

    // Method to transform the tree into a visually pleasant JSON string
    #[allow(dead_code)]
    pub fn to_pretty_json(&self) -> serde_json::Value {
        json!({
            "name": self.name,
            "path": self.path,
            "last_modified": self.last_modified.to_rfc3339(),
            "web_link": self.web_link.as_ref().map(|link| json!({
                "file": link.file,
                "checksum": link.checksum
            })),
            "children": self.children.values().map(|child| child.to_pretty_json()).collect::<Vec<_>>(),
        })
    }

    // Optionally, if you want to print it directly in a more human-readable format
    #[allow(dead_code)]
    pub fn pretty_print(&self, indent: usize) {
        let web_link_str = if let Some(link) = &self.web_link {
            format!(" [Web Link: file={:?}, checksum={:?}]", link.file, link.checksum)
        } else {
            String::from(" [No Web Link]")
        };

        println!(
            "{}- {} ({}) [Last Modified: {}] {}",
            " ".repeat(indent * 2),
            self.name,
            self.path,
            self.last_modified.format("%Y-%m-%d %H:%M:%S"),
            web_link_str
        );
        for child in self.children.values() {
            child.pretty_print(indent + 1);
        }
    }

    // Method to collect all paths from this tree and its children
    pub fn collect_all_paths(&self) -> Vec<String> {
        let mut paths = vec![self.path.clone()];
        for child in self.children.values() {
            paths.extend(child.collect_all_paths());
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_fs_item_tree_serialization() {
        // Create a sample FSEntryTree
        let tree = FSEntryTree {
            name: "root".to_string(),
            path: "/".to_string(),
            last_modified: Utc.with_ymd_and_hms(2023, 5, 20, 10, 30, 0).unwrap(),
            web_link: None,
            children: HashMap::from([
                (
                    "child1".to_string(),
                    Arc::new(FSEntryTree {
                        name: "child1".to_string(),
                        path: "/child1".to_string(),
                        last_modified: Utc.with_ymd_and_hms(2023, 5, 20, 11, 0, 0).unwrap(),
                        web_link: None,
                        children: HashMap::new(),
                    }),
                ),
                (
                    "child2".to_string(),
                    Arc::new(FSEntryTree {
                        name: "child2".to_string(),
                        path: "/child2".to_string(),
                        last_modified: Utc.with_ymd_and_hms(2023, 5, 20, 11, 30, 0).unwrap(),
                        web_link: None,
                        children: HashMap::new(),
                    }),
                ),
            ]),
        };

        // Serialize the FSEntryTree to JSON
        let serialized = serde_json::to_string(&tree).expect("Failed to serialize FSEntryTree");

        // Deserialize the JSON back to FSEntryTree
        let deserialized: FSEntryTree = serde_json::from_str(&serialized).expect("Failed to deserialize FSEntryTree");

        // Assert that the deserialized FSEntryTree is equal to the original
        assert_eq!(deserialized.name, tree.name);
        assert_eq!(deserialized.path, tree.path);
        assert_eq!(deserialized.last_modified, tree.last_modified);
        assert_eq!(deserialized.children.len(), tree.children.len());

        for (key, value) in &tree.children {
            let deserialized_child = deserialized.children.get(key).expect("Child not found");
            assert_eq!(deserialized_child.name, value.name);
            assert_eq!(deserialized_child.path, value.path);
            assert_eq!(deserialized_child.last_modified, value.last_modified);
        }
    }

    #[test]
    fn test_fs_item_tree_bincode_serialization() {
        let tree = FSEntryTree {
            name: "root".to_string(),
            path: "/".to_string(),
            last_modified: Utc.with_ymd_and_hms(2023, 5, 20, 10, 30, 0).unwrap(),
            web_link: None,
            children: HashMap::from([
                (
                    "child1".to_string(),
                    Arc::new(FSEntryTree {
                        name: "child1".to_string(),
                        path: "/child1".to_string(),
                        last_modified: Utc.with_ymd_and_hms(2023, 5, 20, 11, 0, 0).unwrap(),
                        web_link: None,
                        children: HashMap::new(),
                    }),
                ),
                (
                    "child2".to_string(),
                    Arc::new(FSEntryTree {
                        name: "child2".to_string(),
                        path: "/child2".to_string(),
                        last_modified: Utc.with_ymd_and_hms(2023, 5, 20, 11, 30, 0).unwrap(),
                        web_link: None,
                        children: HashMap::new(),
                    }),
                ),
            ]),
        };

        // Serialize the FSEntryTree using bincode
        let serialized = bincode::serialize(&tree).expect("Failed to serialize FSEntryTree with bincode");
        eprintln!("Serialization successful");

        // Deserialize the bincode bytes back to FSEntryTree
        let deserialized: FSEntryTree =
            bincode::deserialize(&serialized).expect("Failed to deserialize FSEntryTree with bincode");
        eprintln!("Deserialization successful");

        // Assert that the deserialized FSEntryTree is equal to the original
        assert_eq!(deserialized.name, tree.name);
        assert_eq!(deserialized.path, tree.path);
        assert_eq!(deserialized.last_modified, tree.last_modified);
        assert_eq!(deserialized.children.len(), tree.children.len());

        for (key, value) in &tree.children {
            let deserialized_child = deserialized
                .children
                .get(key)
                .expect("Child not found in deserialized data");
            assert_eq!(deserialized_child.name, value.name);
            assert_eq!(deserialized_child.path, value.path);
            assert_eq!(deserialized_child.last_modified, value.last_modified);
        }
    }

    #[test]
    fn test_collect_all_paths() {
        let tree = FSEntryTree {
            name: "/".to_string(),
            path: "/shared_test_folder".to_string(),
            last_modified: Utc.with_ymd_and_hms(2024, 4, 20, 4, 52, 44).unwrap(),
            web_link: None,
            children: HashMap::from([
                (
                    "crypto".to_string(),
                    Arc::new(FSEntryTree {
                        name: "crypto".to_string(),
                        path: "/shared_test_folder/crypto".to_string(),
                        last_modified: Utc.with_ymd_and_hms(2024, 4, 20, 4, 52, 42).unwrap(),
                        web_link: None,
                        children: HashMap::from([(
                            "shinkai_intro".to_string(),
                            Arc::new(FSEntryTree {
                                name: "shinkai_intro".to_string(),
                                path: "/shared_test_folder/crypto/shinkai_intro".to_string(),
                                last_modified: Utc.with_ymd_and_hms(2024, 4, 3, 2, 41, 16).unwrap(),
                                web_link: None,
                                children: HashMap::new(),
                            }),
                        )]),
                    }),
                ),
                (
                    "shinkai_intro".to_string(),
                    Arc::new(FSEntryTree {
                        name: "shinkai_intro".to_string(),
                        path: "/shared_test_folder/shinkai_intro".to_string(),
                        last_modified: Utc.with_ymd_and_hms(2024, 4, 3, 2, 41, 16).unwrap(),
                        web_link: None,
                        children: HashMap::new(),
                    }),
                ),
            ]),
        };

        let expected_paths = vec![
            "/shared_test_folder".to_string(),
            "/shared_test_folder/crypto".to_string(),
            "/shared_test_folder/crypto/shinkai_intro".to_string(),
            "/shared_test_folder/shinkai_intro".to_string(),
        ];

        let mut paths = tree.collect_all_paths();
        paths.sort(); // Ensure the order is consistent for comparison
        assert_eq!(paths, expected_paths);
    }
}
