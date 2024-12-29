use super::{
    deno_tools::ToolResult,
    parameters::Parameters,
    tool_config::{BasicConfig, OAuth, ToolConfig},
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
    pub description: String,
    pub author: String,
    pub keywords: Vec<String>,
    #[serde(deserialize_with = "deserialize_configurations")]
    pub configurations: Vec<ToolConfig>,
    pub parameters: Parameters,
    pub result: ToolResult,

    #[serde(default)]
    #[serde(deserialize_with = "deserialize_sql_tables")]
    #[serde(rename = "sqlTables")]
    pub sql_tables: Vec<SqlTable>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_sql_queries")]
    #[serde(rename = "sqlQueries")]
    pub sql_queries: Vec<SqlQuery>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_tools")]
    pub tools: Option<Vec<ToolRouterKey>>,
    pub oauth: Option<Vec<OAuth>>,
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
                        let description = val.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let required = required_keys.contains(key);
                        let basic_config = BasicConfig {
                            key_name: key.clone(),
                            description,
                            required,
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
                }
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
                }
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
                "sql_database_path": "test.db"
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
                ]
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
        let tools = deserialized.metadata.tools.unwrap();
        assert_eq!(tools.len(), 2);

        let tool1 = &tools[0];
        assert_eq!(tool1.source, "local");
        assert_eq!(tool1.toolkit_name, "toolkit1");
        assert_eq!(tool1.name, "tool1");
        assert_eq!(tool1.version, None);

        let tool2 = &tools[1];
        assert_eq!(tool2.source, "local");
        assert_eq!(tool2.toolkit_name, "toolkit2");
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
                ]
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
        assert_eq!(valid_tool.toolkit_name, "toolkit2");
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
                "tools": []
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
                "tools": [123, true, null, {"key": "value"}]
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
}
