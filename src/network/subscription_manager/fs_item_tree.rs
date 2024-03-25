use chrono::{DateTime, Utc};
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::Serializer;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::result::Result::Ok;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FSItemTree {
    pub name: String,
    pub path: String,
    pub last_modified: DateTime<Utc>,
    pub children: HashMap<String, Arc<FSItemTree>>,
}

impl Serialize for FSItemTree {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("FSItemTree", 4)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("path", &self.path)?;
        state.serialize_field("last_modified", &self.last_modified.to_rfc3339())?;
        let children: HashMap<_, _> = self.children.iter().map(|(k, v)| (k, &**v)).collect();
        state.serialize_field("children", &children)?;
        state.end()
    }
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
impl<'de> Deserialize<'de> for FSItemTree {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Name,
            Path,
            LastModified,
            Children,
        };

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("`name`, `path`, `last_modified`, or `children`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "name" => Ok(Field::Name),
                            "path" => Ok(Field::Path),
                            "last_modified" => Ok(Field::LastModified),
                            "children" => Ok(Field::Children),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct FSItemTreeVisitor;

        impl<'de> Visitor<'de> for FSItemTreeVisitor {
            type Value = FSItemTree;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct FSItemTree")
            }

            fn visit_map<V>(self, mut map: V) -> Result<FSItemTree, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut name = None;
                let mut path = None;
                let mut last_modified = None;
                let mut children = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                        Field::Path => {
                            if path.is_some() {
                                return Err(de::Error::duplicate_field("path"));
                            }
                            path = Some(map.next_value()?);
                        }
                        Field::LastModified => {
                            if last_modified.is_some() {
                                return Err(de::Error::duplicate_field("last_modified"));
                            }
                            let last_modified_str: String = map.next_value()?;
                            last_modified = Some(
                                DateTime::parse_from_rfc3339(&last_modified_str)
                                    .map_err(de::Error::custom)?
                                    .with_timezone(&Utc),
                            );
                        }
                        Field::Children => {
                            if children.is_some() {
                                return Err(de::Error::duplicate_field("children"));
                            }
                            let map_children: HashMap<String, FSItemTree> = map.next_value()?;
                            children = Some(map_children.into_iter().map(|(k, v)| (k, Arc::new(v))).collect());
                        }
                    }
                }
                let name = name.ok_or_else(|| de::Error::missing_field("name"))?;
                let path = path.ok_or_else(|| de::Error::missing_field("path"))?;
                let last_modified = last_modified.ok_or_else(|| de::Error::missing_field("last_modified"))?;
                let children = children.unwrap_or_default();

                Ok(FSItemTree {
                    name,
                    path,
                    last_modified,
                    children,
                })
            }
        }

        const FIELDS: &'static [&'static str] = &["name", "path", "last_modified", "children"];
        deserializer.deserialize_struct("FSItemTree", FIELDS, FSItemTreeVisitor)
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
}