use super::{
    argument::ToolArgument,
    deno_tools::DenoToolResult,
    tool_config::{BasicConfig, OAuth, ToolConfig},
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as JsonValue;

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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolPlaygroundMetadata {
    pub name: String,
    pub description: String,
    pub author: String,
    pub keywords: Vec<String>,
    #[serde(deserialize_with = "deserialize_configurations")]
    pub configurations: Vec<ToolConfig>,
    #[serde(deserialize_with = "deserialize_parameters")]
    pub parameters: Vec<ToolArgument>,
    pub result: DenoToolResult,

    #[serde(default)]
    #[serde(deserialize_with = "deserialize_sql_tables")]
    #[serde(rename = "sqlTables")]
    pub sql_tables: Vec<SqlTable>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_sql_queries")]
    #[serde(rename = "sqlQueries")]
    pub sql_queries: Vec<SqlQuery>,
    // This is optional as:
    // None -> All tools.
    // Empty vector -> No tools.
    pub tools: Option<Vec<String>>,
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

fn deserialize_parameters<'de, D>(deserializer: D) -> Result<Vec<ToolArgument>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: JsonValue = Deserialize::deserialize(deserializer)?;
    match value {
        JsonValue::Array(params) => {
            // If it's already an array, assume it's a list of ToolArgument objects
            let tool_arguments: Vec<ToolArgument> = params
                .into_iter()
                .map(|param| {
                    // Assuming each param is a valid ToolArgument JSON object
                    serde_json::from_value(param).map_err(serde::de::Error::custom)
                })
                .collect::<Result<_, _>>()?;
            Ok(tool_arguments)
        }
        JsonValue::Object(param_obj) => {
            if let Some(JsonValue::Object(properties)) = param_obj.get("properties") {
                let required_keys: Vec<String> = param_obj
                    .get("required")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                let arguments = properties
                    .iter()
                    .map(|(key, val)| {
                        let arg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let description = val
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let is_required = required_keys.contains(key);
                        ToolArgument::new(key.clone(), arg_type, description, is_required)
                    })
                    .collect();

                return Ok(arguments);
            }
            Err(serde::de::Error::custom("Invalid object structure for parameters"))
        }
        _ => Err(serde::de::Error::custom("Invalid type for parameters")),
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
                "author": "Author Name",
                "keywords": ["example", "test"],
                "configurations": [],
                "parameters": [],
                "result": {
                    "type": "string",
                    "properties": "{}",
                    "required": []
                }
            },
            "tool_router_key": "example_key",
            "job_id": "job_123",
            "job_id_history": [],
            "code": "console.log('Hello, world!');"
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
            "code": "import { shinkaiDownloadPages } from '@shinkai/local-tools'; type CONFIG = {}; type INPUTS = { urls: string[] }; type OUTPUT = { markdowns: string[] }; export async function run(config: CONFIG, inputs: INPUTS): Promise<OUTPUT> { const { urls } = inputs; if (!urls || urls.length === 0) { throw new Error('URL list is required'); } return shinkaiDownloadPages(urls); }"
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
                "author": "Author Name",
                "keywords": ["sql", "test"],
                "configurations": [],
                "parameters": [],
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
            "code": "console.log('Hello, world!');"
        }
        "#;

        let deserialized: ToolPlayground = serde_json::from_str(json_data).expect("Failed to deserialize");

        assert_eq!(deserialized.metadata.sql_tables.len(), 1);
        assert_eq!(deserialized.metadata.sql_tables[0].name, "website_data");
        assert_eq!(deserialized.metadata.sql_queries.len(), 1);
        assert_eq!(deserialized.metadata.sql_queries[0].name, "Get markdown by URL");
        // assert_eq!(deserialized.metadata.sql_database_path, Some("test.db".to_string()));
    }
}
