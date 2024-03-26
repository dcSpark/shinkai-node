use chrono::{DateTime, Utc};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use std::collections::HashMap;
use std::result::Result::Ok;
use std::sync::Arc;

// Custom serialization for the children field
fn serialize_children<S>(children: &HashMap<String, Arc<FSItemTree>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let temp_children: HashMap<String, FSItemTree> = children.iter().map(|(k, v)| (k.clone(), (**v).clone())).collect();
    temp_children.serialize(serializer)
}

// Custom deserialization for the children field
fn deserialize_children<'de, D>(deserializer: D) -> Result<HashMap<String, Arc<FSItemTree>>, D::Error>
where
    D: Deserializer<'de>,
{
    let temp_children: HashMap<String, FSItemTree> = HashMap::deserialize(deserializer)?;
    Ok(temp_children.into_iter().map(|(k, v)| (k, Arc::new(v))).collect())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FSItemTree {
    pub name: String,
    pub path: String,
    pub last_modified: DateTime<Utc>,
    #[serde(serialize_with = "serialize_children", deserialize_with = "deserialize_children")]
    pub children: HashMap<String, Arc<FSItemTree>>,
}

impl FSItemTree {
     pub fn new_empty() -> Self {
        FSItemTree {
            name: "/".to_string(),
            path: "/".to_string(),
            last_modified: Utc::now(),
            children: HashMap::new(),
        }
    }
    
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_fs_item_tree_serialization() {
        // Create a sample FSItemTree
        let tree = FSItemTree {
            name: "root".to_string(),
            path: "/".to_string(),
            last_modified: Utc.ymd(2023, 5, 20).and_hms(10, 30, 0),
            children: HashMap::from([
                ("child1".to_string(), Arc::new(FSItemTree {
                    name: "child1".to_string(),
                    path: "/child1".to_string(),
                    last_modified: Utc.ymd(2023, 5, 20).and_hms(11, 0, 0),
                    children: HashMap::new(),
                })),
                ("child2".to_string(), Arc::new(FSItemTree {
                    name: "child2".to_string(),
                    path: "/child2".to_string(),
                    last_modified: Utc.ymd(2023, 5, 20).and_hms(11, 30, 0),
                    children: HashMap::new(),
                })),
            ]),
        };

        // Serialize the FSItemTree to JSON
        let serialized = serde_json::to_string(&tree).expect("Failed to serialize FSItemTree");

        // Deserialize the JSON back to FSItemTree
        let deserialized: FSItemTree = serde_json::from_str(&serialized).expect("Failed to deserialize FSItemTree");

        // Assert that the deserialized FSItemTree is equal to the original
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
        let tree = FSItemTree {
            name: "root".to_string(),
            path: "/".to_string(),
            last_modified: Utc.ymd(2023, 5, 20).and_hms(10, 30, 0),
            children: HashMap::from([
                ("child1".to_string(), Arc::new(FSItemTree {
                    name: "child1".to_string(),
                    path: "/child1".to_string(),
                    last_modified: Utc.ymd(2023, 5, 20).and_hms(11, 0, 0),
                    children: HashMap::new(),
                })),
                ("child2".to_string(), Arc::new(FSItemTree {
                    name: "child2".to_string(),
                    path: "/child2".to_string(),
                    last_modified: Utc.ymd(2023, 5, 20).and_hms(11, 30, 0),
                    children: HashMap::new(),
                })),
            ]),
        };

        // Serialize the FSItemTree using bincode
        let serialized = bincode::serialize(&tree).expect("Failed to serialize FSItemTree with bincode");
        eprintln!("Serialization successful");

        // Deserialize the bincode bytes back to FSItemTree
        let deserialized: FSItemTree = bincode::deserialize(&serialized).expect("Failed to deserialize FSItemTree with bincode");
        eprintln!("Deserialization successful");

        // Assert that the deserialized FSItemTree is equal to the original
        assert_eq!(deserialized.name, tree.name);
        assert_eq!(deserialized.path, tree.path);
        assert_eq!(deserialized.last_modified, tree.last_modified);
        assert_eq!(deserialized.children.len(), tree.children.len());

        for (key, value) in &tree.children {
            let deserialized_child = deserialized.children.get(key).expect("Child not found in deserialized data");
            assert_eq!(deserialized_child.name, value.name);
            assert_eq!(deserialized_child.path, value.path);
            assert_eq!(deserialized_child.last_modified, value.last_modified);
        }
    }
}