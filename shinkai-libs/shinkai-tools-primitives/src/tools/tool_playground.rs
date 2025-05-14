use super::{
    parameters::Parameters,
    tool_config::{BasicConfig, OAuth, ToolConfig},
    tool_types::{OperatingSystem, RunnerType, ToolResult},
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as JsonValue;
use shinkai_message_primitives::schemas::{shinkai_tools::CodeLanguage, tool_router_key::ToolRouterKey};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlTable {
    pub name: String,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SqlQuery {
    pub name: String,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPlayground {
    pub metadata: ToolPlaygroundMetadata,
    pub tool_router_key: Option<String>,
    pub job_id: String,
    #[serde(default)]
    pub job_id_history: Vec<String>,
    pub code: String,
    pub language: CodeLanguage,
    pub assets: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPlaygroundMetadata {
    pub name: String,
    pub version: String,
    pub homepage: Option<String>,
    pub description: String,
    pub author: String,
    pub keywords: Vec<String>,
    #[serde(
        serialize_with = "serialize_configurations",
        deserialize_with = "deserialize_configurations"
    )]
    pub configurations: Vec<ToolConfig>,
    pub parameters: Parameters,
    pub result: ToolResult,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_sql_tables")]
    #[serde(serialize_with = "serialize_sql_tables")]
    #[serde(rename = "sqlTables")]
    pub sql_tables: Vec<SqlTable>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_sql_queries")]
    #[serde(serialize_with = "serialize_sql_queries")]
    #[serde(rename = "sqlQueries")]
    pub sql_queries: Vec<SqlQuery>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_tools", serialize_with = "serialize_tools")]
    pub tools: Option<Vec<ToolRouterKey>>,
    pub oauth: Option<Vec<OAuth>>,
    pub runner: RunnerType,
    pub operating_system: Vec<OperatingSystem>,
    pub tool_set: Option<String>,
}

fn deserialize_configurations<'de, D>(deserializer: D) -> Result<Vec<ToolConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: JsonValue = Deserialize::deserialize(deserializer)?;
    match value {
        JsonValue::Array(configs) => {
            // If it's already an array, assume it's a list of ToolConfig objects
            let tool_configs: Vec<ToolConfig> = configs
                .into_iter()
                .map(|config| {
                    // Assuming each config is a valid ToolConfig JSON object
                    serde_json::from_value(config).map_err(serde::de::Error::custom)
                })
                .collect::<Result<_, _>>()?;
            Ok(tool_configs)
        }
        JsonValue::Object(config_obj) => {
            if let Some(JsonValue::Object(properties)) = config_obj.get("properties") {
                let required_keys: Vec<String> = config_obj
                    .get("required")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                let configs = properties
                    .iter()
                    .map(|(key, val)| {
                        let description = val
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let type_name = val.get("type").and_then(|v| v.as_str()).map(String::from);
                        let required = required_keys.contains(key);
                        let basic_config = BasicConfig {
                            key_name: key.clone(),
                            description,
                            required,
                            type_name,
                            key_value: None, // or extract a default value if needed
                        };
                        ToolConfig::BasicConfig(basic_config)
                    })
                    .collect();

                return Ok(configs);
            }
            Err(serde::de::Error::custom("Invalid object structure for configurations"))
        }
        _ => Err(serde::de::Error::custom("Invalid type for configurations")),
    }
}

fn serialize_configurations<S>(configs: &Vec<ToolConfig>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;
    use std::collections::HashMap;

    let mut map = serializer.serialize_map(Some(3))?;

    // Create properties object
    let mut properties = HashMap::new();
    let mut required = Vec::new();

    for config in configs {
        let ToolConfig::BasicConfig(basic) = config;
        let mut property = HashMap::new();
        property.insert("description", basic.description.clone());
        // If type_name is None, default to "string"
        let type_value = basic
            .type_name
            .as_ref()
            .map_or_else(|| "string".to_string(), |t| t.clone());
        property.insert("type", type_value);
        properties.insert(basic.key_name.clone(), property);

        if basic.required {
            required.push(basic.key_name.clone());
        }
    }

    map.serialize_entry("type", "object")?;
    map.serialize_entry("properties", &properties)?;
    map.serialize_entry("required", &required)?;
    map.end()
}
fn deserialize_sql_tables<'de, D>(deserializer: D) -> Result<Vec<SqlTable>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: JsonValue = Deserialize::deserialize(deserializer)?;
    match value {
        JsonValue::Array(tables) => {
            // If it's already an array, assume it's a list of SqlTable objects
            let sql_tables: Vec<SqlTable> = tables
                .into_iter()
                .map(|table| serde_json::from_value(table).map_err(serde::de::Error::custom))
                .collect::<Result<_, _>>()?;
            Ok(sql_tables)
        }
        JsonValue::Null => Ok(Vec::new()),
        _ => Err(serde::de::Error::custom("Invalid type for sql_tables")),
    }
}

fn deserialize_sql_queries<'de, D>(deserializer: D) -> Result<Vec<SqlQuery>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: JsonValue = Deserialize::deserialize(deserializer)?;
    match value {
        JsonValue::Array(queries) => {
            // If it's already an array, assume it's a list of SqlQuery objects
            let sql_queries: Vec<SqlQuery> = queries
                .into_iter()
                .map(|query| serde_json::from_value(query).map_err(serde::de::Error::custom))
                .collect::<Result<_, _>>()?;
            Ok(sql_queries)
        }
        JsonValue::Null => Ok(Vec::new()),
        _ => Err(serde::de::Error::custom("Invalid type for sql_queries")),
    }
}

fn serialize_sql_tables<S>(tables: &Vec<SqlTable>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(tables.len()))?;
    for table in tables {
        seq.serialize_element(table)?;
    }
    seq.end()
}

fn serialize_sql_queries<S>(queries: &Vec<SqlQuery>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = serializer.serialize_seq(Some(queries.len()))?;
    for query in queries {
        seq.serialize_element(query)?;
    }
    seq.end()
}

fn deserialize_tools<'de, D>(deserializer: D) -> Result<Option<Vec<ToolRouterKey>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: JsonValue = Deserialize::deserialize(deserializer)?;
    match value {
        JsonValue::Array(tools) => {
            let tool_keys = tools
                .into_iter()
                .filter_map(|tool| tool.as_str().and_then(|key| ToolRouterKey::from_string(key).ok()))
                .collect::<Vec<_>>();
            Ok(Some(tool_keys))
        }
        JsonValue::Null => Ok(None),
        _ => Err(serde::de::Error::custom("Invalid type for tools")),
    }
}

fn serialize_tools<S>(tools: &Option<Vec<ToolRouterKey>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match tools {
        Some(tools) => {
            use serde::ser::SerializeSeq;
            let mut seq = serializer.serialize_seq(Some(tools.len()))?;
            for tool in tools {
                seq.serialize_element(&tool.to_string_with_version())?;
            }
            seq.end()
        }
        None => serializer.serialize_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_deserialize_playground_tool() {
        let json_data = r#"
        {
            "metadata": {
                "name": "Example Tool",
                "description": "An example tool for testing",
                "version": "1.0.0",
                "author": "Author Name",
                "keywords": ["example", "test"],
                "configurations": [],
                "parameters": {},
                "result": {
                    "type": "string",
                    "properties": "{}",
                    "required": []
                },
                "runner": "any",
                "operating_system": ["windows"],
                "tool_set": null
            },
            "tool_router_key": "example_key",
            "job_id": "job_123",
            "job_id_history": [],
            "code": "console.log('Hello, world!');",
            "language": "Typescript"
        }
        "#;

        let deserialized: ToolPlayground = serde_json::from_str(json_data).expect("Failed to deserialize");

        assert_eq!(deserialized.metadata.name, "Example Tool");
        assert_eq!(deserialized.tool_router_key, Some("example_key".to_string()));
        assert_eq!(deserialized.job_id, "job_123");
        assert_eq!(deserialized.job_id_history, Vec::<String>::new());
        assert_eq!(deserialized.code, "console.log('Hello, world!');");
    }

    #[test]
    fn test_deserialize_playground_tool_with_coinbase_data() {
        let json_data = r#"
        {
            "tool_router_key": null,
            "metadata": {
                "id": "shinkai-tool-coinbase-create-wallet",
                "version": "1.0.0",
                "name": "Shinkai: Coinbase Wallet Creator",
                "description": "Tool for creating a Coinbase wallet",
                "author": "Shinkai",
                "keywords": [
                    "coinbase",
                    "wallet",
                    "creator",
                    "shinkai"
                ],
                "configurations": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string"
                        },
                        "privateKey": {
                            "type": "string"
                        },
                        "useServerSigner": {
                            "type": "string",
                            "default": "false",
                            "nullable": true
                        }
                    },
                    "required": [
                        "name",
                        "privateKey"
                    ]
                },
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                },
                "result": {
                    "type": "object",
                    "properties": {
                        "walletId": {
                            "type": "string",
                            "nullable": true
                        },
                        "seed": {
                            "type": "string",
                            "nullable": true
                        },
                        "address": {
                            "type": "string",
                            "nullable": true
                        }
                    },
                    "required": []
                },
                "runner": "any",
                "operating_system": ["windows"],
                "tool_set": null
            },
            "job_id": "123",
            "job_id_history": [],
            "code": "import { shinkaiDownloadPages } from '@shinkai/local-tools'; type CONFIG = {}; type INPUTS = { urls: string[] }; type OUTPUT = { markdowns: string[] }; export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> { const { urls } = inputs; if (!urls || urls.length === 0) { throw new Error('URL list is required'); } return shinkaiDownloadPages(urls); }",
            "language": "Typescript"
        }
        "#;

        let deserialized: ToolPlayground = serde_json::from_str(json_data).expect("Failed to deserialize");

        assert_eq!(deserialized.metadata.name, "Shinkai: Coinbase Wallet Creator");
        assert_eq!(deserialized.metadata.description, "Tool for creating a Coinbase wallet");
        assert_eq!(deserialized.metadata.author, "Shinkai");
        assert_eq!(
            deserialized.metadata.keywords,
            vec!["coinbase", "wallet", "creator", "shinkai"]
        );
        assert_eq!(deserialized.tool_router_key, None);
        assert_eq!(deserialized.job_id, "123");
        assert_eq!(deserialized.job_id_history, Vec::<String>::new());
        assert_eq!(deserialized.code, "import { shinkaiDownloadPages } from '@shinkai/local-tools'; type CONFIG = {}; type INPUTS = { urls: string[] }; type OUTPUT = { markdowns: string[] }; export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> { const { urls } = inputs; if (!urls || urls.length === 0) { throw new Error('URL list is required'); } return shinkaiDownloadPages(urls); }");
    }

    #[test]
    fn test_deserialize_playground_tool_with_sql() {
        let json_data = r#"
        {
            "metadata": {
                "name": "SQL Example Tool",
                "description": "A tool with SQL configuration",
                "version": "1.0.0",
                "author": "Author Name",
                "keywords": ["sql", "test"],
                "configurations": [],
                "parameters": {},
                "result": {
                    "type": "string",
                    "properties": "{}",
                    "required": []
                },
                "sqlTables": [
                    {
                        "name": "website_data",
                        "definition": "CREATE TABLE IF NOT EXISTS website_data (id INTEGER PRIMARY KEY AUTOINCREMENT, url TEXT NOT NULL, markdown TEXT NOT NULL)"
                    }
                ],
                "sqlQueries": [
                    {
                        "name": "Get markdown by URL",
                        "query": "SELECT markdown FROM website_data WHERE url = :url"
                    }
                ],
                "sql_database_path": "test.db",
                "runner": "any",
                "operating_system": ["windows"],
                "tool_set": null
            },
            "tool_router_key": "example_key",
            "job_id": "job_123",
            "job_id_history": [],
            "code": "console.log('Hello, world!');",
            "language": "Typescript"
        }
        "#;

        let deserialized: ToolPlayground = serde_json::from_str(json_data).expect("Failed to deserialize");

        assert_eq!(deserialized.metadata.sql_tables.len(), 1);
        assert_eq!(deserialized.metadata.sql_tables[0].name, "website_data");
        assert_eq!(deserialized.metadata.sql_queries.len(), 1);
        assert_eq!(deserialized.metadata.sql_queries[0].name, "Get markdown by URL");
        // assert_eq!(deserialized.metadata.sql_database_path, Some("test.db".to_string()));
    }

    #[test]
    fn test_deserialize_playground_tool_with_tools() {
        let json_data = r#"
        {
            "metadata": {
                "name": "Tool With Dependencies",
                "description": "A tool that depends on other tools",
                "author": "Test Author",
                "version": "1.0.0",
                "keywords": ["test", "dependencies"],
                "configurations": [],
                "parameters": {},
                "result": {
                    "type": "string",
                    "properties": "{}",
                    "required": []
                },
                "tools": [
                    "local:::toolkit1:::tool1",
                    "local:::toolkit2:::tool2:::1.0"
                ],
                "runner": "any",
                "operating_system": ["windows"],
                "tool_set": null
            },
            "tool_router_key": "example_key",
            "job_id": "job_123",
            "job_id_history": [],
            "code": "console.log('Hello, world!');",
            "language": "Typescript"
        }
        "#;

        let deserialized: ToolPlayground = serde_json::from_str(json_data).expect("Failed to deserialize");

        // Verify tools were correctly deserialized
        let tools = deserialized.metadata.tools.clone().unwrap();
        assert_eq!(tools.len(), 2);

        let tool1 = &tools[0];
        assert_eq!(tool1.source, "local");
        assert_eq!(tool1.name, "tool1");
        assert_eq!(tool1.version, None);

        let tool2 = &tools[1];
        assert_eq!(tool2.source, "local");
        assert_eq!(tool2.name, "tool2");
        assert_eq!(tool2.version, Some("1.0".to_string()));
    }

    #[test]
    fn test_deserialize_playground_tool_with_invalid_tools() {
        // Test with malformed tool strings
        let json_data = r#"
        {
            "metadata": {
                "name": "Tool With Invalid Dependencies",
                "description": "A tool with invalid tool references",
                "author": "Test Author",
                "version": "1.0.0",
                "keywords": ["test", "invalid"],
                "configurations": [],
                "parameters": {},
                "result": {
                    "type": "string",
                    "properties": "{}",
                    "required": []
                },
                "tools": [
                    "invalid_format",
                    "too:::many:::colons:::here:::version",
                    "not::enough::colons",
                    "local:::toolkit1:::tool1:::version:::extra",
                    "local:::toolkit2:::tool2"
                ],
                "runner": "any",
                "operating_system": ["macos"],
                "tool_set": null
            },
            "tool_router_key": "example_key",
            "job_id": "job_123",
            "job_id_history": [],
            "code": "console.log('Hello, world!');",
            "language": "Typescript"
        }
        "#;

        let deserialized: ToolPlayground = serde_json::from_str(json_data).expect("Failed to deserialize");

        // Only the valid tool should be included, others should be filtered out
        let tools = deserialized.metadata.tools.unwrap_or_default();
        assert_eq!(tools.len(), 1, "Only one valid tool should remain");

        let valid_tool = &tools[0];
        assert_eq!(valid_tool.source, "local");
        assert_eq!(valid_tool.name, "tool2");
        assert_eq!(valid_tool.version, None);
    }

    #[test]
    fn test_deserialize_playground_tool_with_empty_tools() {
        // Test with empty array and non-string values
        let json_data = r#"
        {
            "metadata": {
                "name": "Tool With Empty Dependencies",
                "description": "A tool with empty tool references",
                "author": "Test Author",
                "keywords": ["test", "empty"],
                "version": "1.0.0",
                "configurations": [],
                "parameters": {},
                "result": {
                    "type": "string",
                    "properties": "{}",
                    "required": []
                },
                "tools": [],
                "runner": "any",
                "operating_system": ["linux"],
                "tool_set": null
            },
            "tool_router_key": "example_key",
            "job_id": "job_123",
            "job_id_history": [],
            "code": "console.log('Hello, world!');",
            "language": "Typescript"
        }
        "#;

        let deserialized: ToolPlayground = serde_json::from_str(json_data).expect("Failed to deserialize");
        assert_eq!(
            deserialized.metadata.tools,
            Some(vec![]),
            "Empty tools array should deserialize to empty vec"
        );

        // Test with non-string values in array
        let json_data = r#"
        {
            "metadata": {
                "name": "Tool With Invalid Dependencies",
                "description": "A tool with non-string tool references",
                "author": "Test Author",
                "version": "1.0.0",
                "keywords": ["test", "invalid"],
                "configurations": [],
                "parameters": {},
                "result": {
                    "type": "string",
                    "properties": "{}",
                    "required": []
                },
                "tools": [123, true, null, {"key": "value"}],
                "runner": "any",
                "operating_system": ["linux"],
                "tool_set": null
            },
            "tool_router_key": "example_key",
            "job_id": "job_123",
            "job_id_history": [],
            "code": "console.log('Hello, world!');",
            "language": "Typescript"
        }
        "#;

        let deserialized: ToolPlayground = serde_json::from_str(json_data).expect("Failed to deserialize");
        assert_eq!(
            deserialized.metadata.tools,
            Some(vec![]),
            "Non-string tools should be filtered out"
        );
    }

    #[test]
    fn test_serialize_configurations_empty() {
        let configs: Vec<ToolConfig> = vec![];
        let serialized = serde_json::to_value(&configs).unwrap();
        assert_eq!(serialized, serde_json::json!([]));
    }

    #[test]
    fn test_serialize_configurations_basic() {
        let configs = vec![
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "api_key".to_string(),
                description: "API Key for authentication".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: None,
            }),
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "timeout".to_string(),
                description: "Request timeout in seconds".to_string(),
                required: false,
                type_name: Some("number".to_string()),
                key_value: None,
            }),
        ];

        let metadata = ToolPlaygroundMetadata {
            name: "Test Tool".to_string(),
            homepage: None,
            version: "1.0.0".to_string(),
            description: "Test description".to_string(),
            author: "Test Author".to_string(),
            keywords: vec!["test".to_string()],
            configurations: configs,
            parameters: Parameters {
                schema_type: "object".to_string(),
                properties: std::collections::HashMap::new(),
                required: vec![],
            },
            result: ToolResult {
                r#type: "object".to_string(),
                properties: serde_json::json!({}),
                required: vec![],
            },
            sql_tables: vec![],
            sql_queries: vec![],
            tools: None,
            oauth: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Linux],
            tool_set: None,
        };

        let serialized = serde_json::to_value(&metadata).unwrap();
        let expected = serde_json::json!({
            "name": "Test Tool",
            "homepage": null,
            "version": "1.0.0",
            "description": "Test description",
            "author": "Test Author",
            "keywords": ["test"],
            "configurations": {
                "type": "object",
                "properties": {
                    "api_key": {
                        "description": "API Key for authentication",
                        "type": "string"
                    },
                    "timeout": {
                        "description": "Request timeout in seconds",
                        "type": "number"
                    }
                },
                "required": ["api_key"]
            },
            "parameters": {
                "type": "object",
                "properties": {},
                "required": [],
            },
            "result": {
                "type": "object",
                "properties": {},
                "required": []
            },
            "sqlTables": [],
            "sqlQueries": [],
            "tools": null,
            "oauth": null,
            "runner": "only_host",
            "operating_system": ["linux"],
            "tool_set": null
        });

        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_serialize_configurations_mixed() {
        // Create a mixed configuration with both BasicConfig and other variants
        let configs = vec![
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "name".to_string(),
                description: "User name".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: None,
            }),
            // Assuming there's another variant in ToolConfig enum
            // If there isn't, we can modify this test to use whatever other variants exist
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "custom".to_string(),
                description: "Custom config".to_string(),
                required: false,
                type_name: None,
                key_value: Some(serde_json::Value::String("default".to_string())),
            }),
        ];

        let serialized = serde_json::to_value(&configs).unwrap();

        // Verify it serializes as an array when containing non-basic configs
        assert!(serialized.is_array());
        assert_eq!(serialized.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_serialize_configurations_roundtrip() {
        let original_configs = vec![
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "field1".to_string(),
                description: "First field".to_string(),
                required: true,
                type_name: Some("string".to_string()),
                key_value: None,
            }),
            ToolConfig::BasicConfig(BasicConfig {
                key_name: "field2".to_string(),
                description: "Second field".to_string(),
                required: false,
                type_name: Some("number".to_string()),
                key_value: None,
            }),
        ];

        // Serialize
        let serialized = serde_json::to_value(&original_configs).unwrap();

        // Deserialize
        let deserialized: Vec<ToolConfig> = serde_json::from_value(serialized).unwrap();

        // Compare
        assert_eq!(original_configs, deserialized);
    }

    #[test]
    fn test_serialize_tools_basic() {
        let metadata = ToolPlaygroundMetadata {
            name: "Test Tool".to_string(),
            homepage: None,
            version: "1.0.0".to_string(),
            description: "Test description".to_string(),
            author: "Test Author".to_string(),
            keywords: vec!["test".to_string()],
            configurations: vec![],
            parameters: Parameters {
                schema_type: "object".to_string(),
                properties: std::collections::HashMap::new(),
                required: vec![],
            },
            result: ToolResult {
                r#type: "object".to_string(),
                properties: serde_json::json!({}),
                required: vec![],
            },
            sql_tables: vec![],
            sql_queries: vec![],
            tools: Some(vec![
                ToolRouterKey::new("local".to_string(), "toolkit1".to_string(), "tool1".to_string(), None),
                ToolRouterKey::new(
                    "local".to_string(),
                    "toolkit2".to_string(),
                    "tool2".to_string(),
                    Some("1.0".to_string()),
                ),
            ]),
            oauth: None,
            runner: RunnerType::Any,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let serialized = serde_json::to_value(&metadata).unwrap();
        let expected = serde_json::json!({
            "name": "Test Tool",
            "homepage": null,
            "version": "1.0.0",
            "description": "Test description",
            "author": "Test Author",
            "keywords": ["test"],
            "configurations": {
                "type": "object",
                "properties": {},
                "required": []
            },
            "parameters": {
                "type": "object",
                "properties": {},
                "required": [],
            },
            "result": {
                "type": "object",
                "properties": {},
                "required": []
            },
            "sqlTables": [],
            "sqlQueries": [],
            "tools": [
                "local:::toolkit1:::tool1",
                "local:::toolkit2:::tool2:::1.0"
            ],
            "oauth": null,
            "runner": "any",
            "operating_system": ["windows"],
            "tool_set": null
        });

        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_serialize_sql_components() {
        let metadata = ToolPlaygroundMetadata {
            name: "Test Tool".to_string(),
            homepage: None,
            version: "1.0.0".to_string(),
            description: "Test description".to_string(),
            author: "Test Author".to_string(),
            keywords: vec!["test".to_string()],
            configurations: vec![],
            parameters: Parameters {
                schema_type: "object".to_string(),
                properties: std::collections::HashMap::new(),
                required: vec![],
            },
            result: ToolResult {
                r#type: "object".to_string(),
                properties: serde_json::json!({}),
                required: vec![],
            },
            sql_tables: vec![
                SqlTable {
                    name: "users".to_string(),
                    definition: "CREATE TABLE users (id INTEGER PRIMARY KEY)".to_string(),
                },
                SqlTable {
                    name: "posts".to_string(),
                    definition: "CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER)".to_string(),
                },
            ],
            sql_queries: vec![
                SqlQuery {
                    name: "get_user".to_string(),
                    query: "SELECT * FROM users WHERE id = ?".to_string(),
                },
                SqlQuery {
                    name: "get_posts".to_string(),
                    query: "SELECT * FROM posts WHERE user_id = ?".to_string(),
                },
            ],
            tools: None,
            oauth: None,
            runner: RunnerType::Any,
            operating_system: vec![OperatingSystem::Linux, OperatingSystem::MacOS],
            tool_set: Some("some cool set".to_string()),
        };

        let serialized = serde_json::to_value(&metadata).unwrap();
        let expected = serde_json::json!({
            "name": "Test Tool",
            "homepage": null,
            "version": "1.0.0",
            "description": "Test description",
            "author": "Test Author",
            "keywords": ["test"],
            "configurations": {
                "type": "object",
                "properties": {},
                "required": []
            },
            "parameters": {
                "type": "object",
                "properties": {},
                "required": [],
            },
            "result": {
                "type": "object",
                "properties": {},
                "required": []
            },
            "sqlTables": [
                {
                    "name": "users",
                    "definition": "CREATE TABLE users (id INTEGER PRIMARY KEY)"
                },
                {
                    "name": "posts",
                    "definition": "CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER)"
                }
            ],
            "sqlQueries": [
                {
                    "name": "get_user",
                    "query": "SELECT * FROM users WHERE id = ?"
                },
                {
                    "name": "get_posts",
                    "query": "SELECT * FROM posts WHERE user_id = ?"
                }
            ],
            "tools": null,
            "oauth": null,
            "runner": "any",
            "operating_system": ["linux", "macos"],
            "tool_set": "some cool set"
        });

        assert_eq!(serialized, expected);

        // Test round-trip serialization
        let deserialized: ToolPlaygroundMetadata = serde_json::from_value(serialized).unwrap();
        assert_eq!(deserialized.sql_tables, metadata.sql_tables);
        assert_eq!(deserialized.sql_queries, metadata.sql_queries);
    }

    #[test]
    fn test_deserialize_playground_tool_with_various_properties() {
        let json_data = r#"
        {
            "metadata": {
                "name": "Complex Property Tool",
                "version": "1.0.0",
                "homepage": "https://example.com",
                "description": "A tool with various property types",
                "author": "Test Author",
                "keywords": ["test", "complex", "properties"],
                "configurations": {
                    "type": "object",
                    "properties": {
                        "string_prop": {
                            "type": "string",
                            "description": "A string property",
                            "key_value": "default_value"
                        },
                        "number_prop": {
                            "type": "number",
                            "description": "A number property",
                            "key_value": 42
                        },
                        "boolean_prop": {
                            "type": "boolean",
                            "description": "A boolean property",
                            "key_value": true
                        },
                        "false_boolean_prop": {
                            "type": "boolean",
                            "description": "A boolean property with false default",
                            "key_value": false
                        },
                        "array_prop": {
                            "type": "array",
                            "description": "An array property",
                            "key_value": ["item1", "item2"]
                        },
                        "object_prop": {
                            "type": "object",
                            "description": "An object property",
                            "key_value": {
                                "key1": "value1",
                                "key2": 123
                            }
                        },
                        "required_prop": {
                            "type": "string",
                            "description": "A required property"
                        },
                        "optional_prop": {
                            "type": "string",
                            "description": "An optional property"
                        }
                    },
                    "required": ["required_prop"]
                },
                "parameters": {
                    "type": "object",
                    "properties": {
                        "string_prop": {
                            "type": "string",
                            "description": "A string property",
                            "default": "default_value"
                        },
                        "number_prop": {
                            "type": "number",
                            "description": "A number property",
                            "default": 42
                        },
                        "boolean_prop": {
                            "type": "boolean",
                            "description": "A boolean property",
                            "default": true
                        },
                        "false_boolean_prop": {
                            "type": "boolean",
                            "description": "A boolean property with false default",
                            "default": false
                        },
                        "array_prop": {
                            "type": "array",
                            "description": "An array property",
                            "default": ["item1", "item2"]
                        },
                        "object_prop": {
                            "type": "object",
                            "description": "An object property",
                            "default": {
                                "key1": "value1",
                                "key2": 123
                            }
                        },
                        "required_prop": {
                            "type": "string",
                            "description": "A required property"
                        },
                        "optional_prop": {
                            "type": "string",
                            "description": "An optional property"
                        }
                    },
                    "required": ["required_prop"]
                },
                "result": {
                    "type": "object",
                    "properties": {
                        "output_string": {
                            "type": "string",
                            "description": "Output string result"
                        },
                        "output_number": {
                            "type": "number",
                            "description": "Output number result"
                        }
                    },
                    "required": ["output_string"]
                },
                "sqlTables": [
                    {
                        "name": "test_table",
                        "definition": "CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT)"
                    }
                ],
                "sqlQueries": [
                    {
                        "name": "get_test_data",
                        "query": "SELECT * FROM test_table WHERE id = ?"
                    }
                ],
                "tools": [
                    "local:::toolkit1:::tool1",
                    "local:::toolkit2:::tool2:::1.0"
                ],
                "oauth": [
                    {
                        "name": "test_oauth",
                        "authorizationUrl": "https://example.com/oauth/authorize",
                        "tokenUrl": "https://example.com/oauth/token",
                        "clientId": "test_client_id",
                        "clientSecret": "test_client_secret",
                        "redirectUrl": "https://example.com/callback",
                        "version": "2.0",
                        "responseType": "code",
                        "scopes": ["read", "write"],
                        "pkceType": "plain",
                        "refreshToken": "true",
                        "requestTokenAuthHeader": "Bearer",
                        "requestTokenContentType": "application/json"
                    }
                ],
                "runner": "any",
                "operating_system": ["linux", "macos", "windows"],
                "tool_set": "test-tool-set"
            },
            "tool_router_key": "local:::test-author:::complex-property-tool",
            "job_id": "job_123",
            "job_id_history": [],
            "code": "console.log('Hello, world!');",
            "language": "Typescript"
        }
        "#;

        let deserialized: ToolPlayground = serde_json::from_str(json_data).expect("Failed to deserialize");
        println!("deserialized: {:?}", deserialized);

        // Verify basic metadata
        assert_eq!(deserialized.metadata.name, "Complex Property Tool");
        assert_eq!(deserialized.metadata.version, "1.0.0");
        assert_eq!(deserialized.metadata.homepage, Some("https://example.com".to_string()));
        assert_eq!(deserialized.metadata.description, "A tool with various property types");
        assert_eq!(deserialized.metadata.author, "Test Author");
        assert_eq!(deserialized.metadata.keywords, vec!["test", "complex", "properties"]);

        // Verify configurations
        let configs = &deserialized.metadata.configurations;
        assert_eq!(configs.len(), 8); // 7 properties in configurations

        // Find and verify string property with default
        let string_prop = configs
            .iter()
            .find(|c| {
                if let ToolConfig::BasicConfig(bc) = c {
                    bc.key_name == "string_prop"
                } else {
                    false
                }
            })
            .unwrap();
        if let ToolConfig::BasicConfig(bc) = string_prop {
            assert_eq!(bc.description, "A string property");
            assert_eq!(bc.type_name, Some("string".to_string()));
            assert!(!bc.required);
        }

        // Find and verify number property with default
        let number_prop = configs
            .iter()
            .find(|c| {
                if let ToolConfig::BasicConfig(bc) = c {
                    bc.key_name == "number_prop"
                } else {
                    false
                }
            })
            .unwrap();
        if let ToolConfig::BasicConfig(bc) = number_prop {
            assert_eq!(bc.description, "A number property");
            assert_eq!(bc.type_name, Some("number".to_string()));
            assert!(!bc.required);
            // TODO: key_values are not deserialized into BasicConfig
            // assert_eq!(bc.key_value, Some(serde_json::Value::Number(42.into())));
        }

        // Find and verify required property
        let required_prop = configs
            .iter()
            .find(|c| {
                if let ToolConfig::BasicConfig(bc) = c {
                    bc.key_name == "required_prop"
                } else {
                    false
                }
            })
            .unwrap();
        if let ToolConfig::BasicConfig(bc) = required_prop {
            assert_eq!(bc.description, "A required property");
            assert_eq!(bc.type_name, Some("string".to_string()));
            assert!(bc.required);
            // TODO: key_values are not deserialized into BasicConfig
            // assert_eq!(bc.key_value, None);
        }

        // Verify SQL components
        assert_eq!(deserialized.metadata.sql_tables.len(), 1);
        assert_eq!(deserialized.metadata.sql_tables[0].name, "test_table");
        assert_eq!(deserialized.metadata.sql_queries.len(), 1);
        assert_eq!(deserialized.metadata.sql_queries[0].name, "get_test_data");

        // Verify tools
        let tools = deserialized.metadata.tools.clone().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].source, "local");
        assert_eq!(tools[0].name, "tool1");
        assert_eq!(tools[1].source, "local");
        assert_eq!(tools[1].name, "tool2");
        assert_eq!(tools[1].version, Some("1.0".to_string()));

        // Verify OAuth
        let oauth = deserialized.metadata.oauth.clone().unwrap();
        assert_eq!(oauth.len(), 1);
        assert_eq!(oauth[0].name, "test_oauth");
        assert_eq!(oauth[0].authorization_url, "https://example.com/oauth/authorize");
        assert_eq!(oauth[0].token_url, Some("https://example.com/oauth/token".to_string()));
        assert_eq!(oauth[0].client_id, "test_client_id");
        assert_eq!(oauth[0].client_secret, "test_client_secret");
        assert_eq!(oauth[0].redirect_url, "https://example.com/callback");
        assert_eq!(oauth[0].version, "2.0");
        assert_eq!(oauth[0].response_type, "code");
        assert_eq!(oauth[0].scopes, vec!["read", "write"]);
        assert_eq!(oauth[0].pkce_type, Some("plain".to_string()));
        assert_eq!(oauth[0].refresh_token, Some("true".to_string()));
        assert_eq!(oauth[0].request_token_auth_header, Some("Bearer".to_string()));
        assert_eq!(
            oauth[0].request_token_content_type,
            Some("application/json".to_string())
        );

        // Verify runner and operating system
        assert_eq!(deserialized.metadata.runner, RunnerType::Any);
        assert_eq!(deserialized.metadata.operating_system.len(), 3);
        assert!(deserialized.metadata.operating_system.contains(&OperatingSystem::Linux));
        assert!(deserialized.metadata.operating_system.contains(&OperatingSystem::MacOS));
        assert!(deserialized
            .metadata
            .operating_system
            .contains(&OperatingSystem::Windows));

        // Verify tool set
        assert_eq!(deserialized.metadata.tool_set, Some("test-tool-set".to_string()));

        // Test round-trip serialization
        let serialized = serde_json::to_string(&deserialized).expect("Failed to serialize");
        let deserialized_again: ToolPlayground =
            serde_json::from_str(&serialized).expect("Failed to deserialize again");
        assert_eq!(deserialized, deserialized_again);

        // Verify parameters
        let params = &deserialized.metadata.parameters;
        assert_eq!(params.schema_type, "object");
        assert_eq!(params.properties.len(), 8); // 8 properties in parameters

        // Find and verify string property with default
        let string_prop = params.properties.get("string_prop").unwrap();
        assert_eq!(string_prop.property_type, "string");
        assert_eq!(string_prop.description, "A string property");
        assert_eq!(
            string_prop.default,
            Some(serde_json::Value::String("default_value".to_string()))
        );

        // Find and verify number property with default
        let number_prop = params.properties.get("number_prop").unwrap();
        assert_eq!(number_prop.property_type, "number");
        assert_eq!(number_prop.description, "A number property");
        assert_eq!(number_prop.default, Some(serde_json::Value::Number(42.into())));

        // Find and verify boolean property with default
        let boolean_prop = params.properties.get("boolean_prop").unwrap();
        assert_eq!(boolean_prop.property_type, "boolean");
        assert_eq!(boolean_prop.description, "A boolean property");
        assert_eq!(boolean_prop.default, Some(serde_json::Value::Bool(true)));

        // Find and verify array property with default
        let array_prop = params.properties.get("array_prop").unwrap();
        assert_eq!(array_prop.property_type, "array");
        assert_eq!(array_prop.description, "An array property");
        let default_array = array_prop.default.as_ref().unwrap().as_array().unwrap();
        assert_eq!(default_array.len(), 2);
        assert_eq!(default_array[0].as_str(), Some("item1"));
        assert_eq!(default_array[1].as_str(), Some("item2"));

        // Find and verify object property with default
        let object_prop = params.properties.get("object_prop").unwrap();
        assert_eq!(object_prop.property_type, "object");
        assert_eq!(object_prop.description, "An object property");
        let default_object = object_prop.default.as_ref().unwrap().as_object().unwrap();
        assert_eq!(default_object.get("key1").and_then(|v| v.as_str()), Some("value1"));
        assert_eq!(
            default_object.get("key2").and_then(|v| v.as_number()),
            Some(&serde_json::Number::from(123))
        );

        // Find and verify required property
        let required_prop = params.properties.get("required_prop").unwrap();
        assert_eq!(required_prop.property_type, "string");
        assert_eq!(required_prop.description, "A required property");
        assert_eq!(required_prop.default, None);

        // Find and verify optional property
        let optional_prop = params.properties.get("optional_prop").unwrap();
        assert_eq!(optional_prop.property_type, "string");
        assert_eq!(optional_prop.description, "An optional property");
        assert_eq!(optional_prop.default, None);

        // Verify required fields
        assert_eq!(params.required, vec!["required_prop"]);
    }
}
