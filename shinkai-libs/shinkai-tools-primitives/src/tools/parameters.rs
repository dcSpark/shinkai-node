use super::deprecated_argument::DeprecatedArgument;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Parameters {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: std::collections::HashMap<String, Property>,
    pub required: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Property {
    #[serde(rename = "type")]
    pub property_type: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<std::collections::HashMap<String, Property>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<Property>>,
}

impl Property {
    pub fn new(property_type: String, description: String) -> Self {
        Self {
            property_type,
            description,
            properties: None,
            items: None,
        }
    }

    pub fn with_nested_properties(
        property_type: String,
        description: String,
        properties: std::collections::HashMap<String, Property>,
    ) -> Self {
        Self {
            property_type,
            description,
            properties: Some(properties),
            items: None,
        }
    }

    pub fn with_array_items(description: String, items: Property) -> Self {
        Self {
            property_type: "array".to_string(),
            description,
            properties: None,
            items: Some(Box::new(items)),
        }
    }
}

impl Parameters {
    pub fn new() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: std::collections::HashMap::new(),
            required: Vec::new(),
        }
    }

    pub fn add_property(&mut self, name: String, property_type: String, description: String, is_required: bool) {
        self.properties
            .insert(name.clone(), Property::new(property_type, description));
        if is_required {
            self.required.push(name);
        }
    }

    pub fn add_nested_property(
        &mut self,
        name: String,
        property_type: String,
        description: String,
        nested_properties: std::collections::HashMap<String, Property>,
        is_required: bool,
    ) {
        self.properties.insert(
            name.clone(),
            Property::with_nested_properties(property_type, description, nested_properties),
        );
        if is_required {
            self.required.push(name);
        }
    }

    /// Creates a new Parameters instance with a single property.
    pub fn with_single_property(name: &str, property_type: &str, description: &str, is_required: bool) -> Self {
        let mut params = Self {
            schema_type: "object".to_string(),
            properties: std::collections::HashMap::new(),
            required: Vec::new(),
        };
        params.add_property(
            name.to_string(),
            property_type.to_string(),
            description.to_string(),
            is_required,
        );
        params
    }

    /// Converts Parameters to a Vec<DeprecatedArgument>
    pub fn to_deprecated_arguments(&self) -> Vec<DeprecatedArgument> {
        self.properties
            .iter()
            .map(|(name, property)| {
                DeprecatedArgument::new(
                    name.clone(),
                    property.property_type.clone(),
                    property.description.clone(),
                    self.required.contains(name),
                )
            })
            .collect::<Vec<_>>()
    }
}

impl<'de> serde::Deserialize<'de> for Parameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct ParametersVisitor;

        impl<'de> Visitor<'de> for ParametersVisitor {
            type Value = Parameters;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a Parameters object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Parameters, M::Error>
            where
                M: MapAccess<'de>,
            {
                // If the map is empty, return default Parameters
                if map.size_hint() == Some(0) {
                    return Ok(Parameters::new());
                }

                let mut schema_type = None;
                let mut properties = None;
                let mut required = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "type" => schema_type = Some(map.next_value()?),
                        "properties" => properties = Some(map.next_value()?),
                        "required" => required = Some(map.next_value()?),
                        _ => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                Ok(Parameters {
                    schema_type: schema_type.unwrap_or_else(|| "object".to_string()),
                    properties: properties.unwrap_or_default(),
                    required: required.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_map(ParametersVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn test_serialization_deserialization() {
        // Create a Parameters instance
        let mut params = Parameters::new();
        params.add_property(
            "url".to_string(),
            "string".to_string(),
            "The URL to fetch".to_string(),
            true,
        );

        // Serialize the Parameters instance to JSON
        let serialized = serde_json::to_string(&params).unwrap();

        // Deserialize the serialized JSON string to a serde_json::Value
        let serialized_value: Value = serde_json::from_str(&serialized).unwrap();

        // Correct expected JSON value
        let expected_value = json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                }
            },
            "required": [
                "url"
            ]
        });

        // Check if the serialized JSON value matches the expected JSON value
        assert_eq!(serialized_value, expected_value);

        // Deserialize the JSON back to a Parameters instance
        let deserialized: Parameters = serde_json::from_str(&serialized).unwrap();

        // Check if the deserialized instance matches the original instance
        assert_eq!(deserialized, params);
    }

    #[test]
    fn test_to_deprecated_arguments() {
        // Create a Parameters instance
        let mut params = Parameters::new();
        params.add_property(
            "url".to_string(),
            "string".to_string(),
            "The URL to fetch".to_string(),
            true,
        );

        // Convert Parameters to Vec<DeprecatedArgument>
        let deprecated_args = params.to_deprecated_arguments();

        // Expected Vec<DeprecatedArgument> in JSON format
        let expected_args = json!([
            {
                "name": "url",
                "arg_type": "string",
                "description": "The URL to fetch",
                "is_required": true
            }
        ]);

        // Serialize the Vec<DeprecatedArgument> to JSON
        let serialized_args = serde_json::to_value(&deprecated_args).unwrap();

        // Check if the serialized Vec<DeprecatedArgument> matches the expected JSON
        assert_eq!(serialized_args, expected_args);
    }

    #[test]
    fn test_deserialize_empty_json() {
        let empty_json = "{}";
        let result: Parameters = serde_json::from_str(empty_json).unwrap();

        // Should be equivalent to Parameters::new()
        let expected = Parameters::new();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_nested_properties() {
        let nested_json = r#"{
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "description": "User information",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The user's name"
                        },
                        "age": {
                            "type": "integer",
                            "description": "The user's age"
                        }
                    }
                }
            },
            "required": ["user"]
        }"#;

        // This will currently fail because Property doesn't support nested properties
        let result: Result<Parameters, _> = serde_json::from_str(nested_json);
        assert!(result.is_ok(), "Should be able to parse nested properties");

        let params = result.unwrap();
        assert_eq!(params.schema_type, "object");
        assert!(params.properties.contains_key("user"));
        assert!(params.required.contains(&"user".to_string()));

        let user_prop = &params.properties["user"];
        assert_eq!(user_prop.property_type, "object");
        assert_eq!(user_prop.description, "User information");

        // Verify nested properties
        let nested_props = user_prop
            .properties
            .as_ref()
            .expect("User should have nested properties");

        // Check name property
        let name_prop = nested_props.get("name").expect("Should have name property");
        assert_eq!(name_prop.property_type, "string");
        assert_eq!(name_prop.description, "The user's name");

        // Check age property
        let age_prop = nested_props.get("age").expect("Should have age property");
        assert_eq!(age_prop.property_type, "integer");
        assert_eq!(age_prop.description, "The user's age");
    }

    #[test]
    fn test_add_nested_property() {
        let mut params = Parameters::new();

        // Create nested properties for user
        let mut user_props = std::collections::HashMap::new();
        user_props.insert(
            "name".to_string(),
            Property::new("string".to_string(), "The user's name".to_string()),
        );
        user_props.insert(
            "age".to_string(),
            Property::new("integer".to_string(), "The user's age".to_string()),
        );

        // Add the nested user property
        params.add_nested_property(
            "user".to_string(),
            "object".to_string(),
            "User information".to_string(),
            user_props,
            true,
        );

        // Serialize to JSON and verify the structure
        let serialized = serde_json::to_value(&params).unwrap();
        let expected = json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "description": "User information",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The user's name"
                        },
                        "age": {
                            "type": "integer",
                            "description": "The user's age"
                        }
                    }
                }
            },
            "required": ["user"]
        });

        assert_eq!(serialized, expected);

        // Test deserialization
        let deserialized: Parameters = serde_json::from_value(expected).unwrap();
        assert_eq!(deserialized, params);
    }

    #[test]
    fn test_array_property() {
        // Create a Parameters instance with an array property
        let mut params = Parameters::new();

        // Create an array of strings
        let string_prop = Property::new("string".to_string(), "A string item".to_string());
        let array_prop = Property::with_array_items("An array of strings".to_string(), string_prop);

        params.properties.insert("tags".to_string(), array_prop);
        params.required.push("tags".to_string());

        // Serialize to JSON and verify the structure
        let serialized = serde_json::to_value(&params).unwrap();
        let expected = json!({
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array",
                    "description": "An array of strings",
                    "items": {
                        "type": "string",
                        "description": "A string item"
                    }
                }
            },
            "required": ["tags"]
        });

        assert_eq!(serialized, expected);

        // Test deserialization
        let deserialized: Parameters = serde_json::from_value(expected).unwrap();
        assert_eq!(deserialized, params);
    }

    #[test]
    fn test_complex_array_property() {
        let mut params = Parameters::new();

        // Create an array of objects
        let mut user_props = std::collections::HashMap::new();
        user_props.insert(
            "name".to_string(),
            Property::new("string".to_string(), "The user's name".to_string()),
        );
        user_props.insert(
            "age".to_string(),
            Property::new("integer".to_string(), "The user's age".to_string()),
        );

        let object_prop =
            Property::with_nested_properties("object".to_string(), "A user object".to_string(), user_props);

        let array_prop = Property::with_array_items("List of users".to_string(), object_prop);

        params.properties.insert("users".to_string(), array_prop);
        params.required.push("users".to_string());

        // Serialize to JSON and verify the structure
        let serialized = serde_json::to_value(&params).unwrap();
        let expected = json!({
            "type": "object",
            "properties": {
                "users": {
                    "type": "array",
                    "description": "List of users",
                    "items": {
                        "type": "object",
                        "description": "A user object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "The user's name"
                            },
                            "age": {
                                "type": "integer",
                                "description": "The user's age"
                            }
                        }
                    }
                }
            },
            "required": ["users"]
        });

        assert_eq!(serialized, expected);

        // Test deserialization
        let deserialized: Parameters = serde_json::from_value(expected).unwrap();
        assert_eq!(deserialized, params);
    }
}
