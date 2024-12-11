use super::deprecated_argument::DeprecatedArgument;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
        self.properties.insert(name.clone(), Property { property_type, description });
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
        params.add_property(name.to_string(), property_type.to_string(), description.to_string(), is_required);
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
                    String::new(),
                    self.required.contains(name),
                )
            })
            .collect()
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
        params.add_property("url".to_string(), "string".to_string(), "The URL to fetch".to_string(), true);

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
        params.add_property("url".to_string(), "string".to_string(), "The URL to fetch".to_string(), true);

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
}
