use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::parameters::Parameters;
use shinkai_tools_primitives::tools::{shinkai_tool::ShinkaiToolHeader, tool_output_arg::ToolOutputArg};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;

use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::tools::tool_implementation::tool_traits::ToolExecutor;
use crate::utils::environment::fetch_node_environment;

use tokio::sync::Mutex;

use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params_from_iter, ToSql};

// LLM Tool
pub struct SQLProcessorTool {
    pub tool: ShinkaiToolHeader,
    pub tool_embedding: Option<Vec<f32>>,
}

impl SQLProcessorTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai SQLite Query Executor".to_string(),
                description: r#"Tool for executing a single SQL query on a specified database file. 
If this tool is used, you need to create if not exists the tables used other queries.
Table creation should always use 'CREATE TABLE IF NOT EXISTS'.

-- Example table creation:
CREATE TABLE IF NOT EXISTS table_name (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    field_1 TEXT NOT NULL,
    field_2 DATETIME DEFAULT CURRENT_TIMESTAMP,
    field_3 INTEGER,
    field_4 TEXT
);

-- Example insert:
INSERT INTO table_name (field_1, field_3, field_4) 
    VALUES ('value_1', 3, 'value_4')
    ON CONFLICT(id) DO UPDATE SET field_1 = 'value_1', field_3 = 3, field_4 = 'value_4';
;

-- Example read:
SELECT * FROM table_name WHERE field_2 > datetime('now', '-1 day');
SELECT field_1, field_3 FROM table_name WHERE field_3 > 100 ORDER BY field_2 DESC LIMIT 10;"#
                    .to_string(),
                tool_router_key: "local:::__official_shinkai:::shinkai_sqlite_query_executor".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Execute SQLite queries".to_string(),
                author: "@@official.shinkai".to_string(),
                version: "1.0".to_string(),
                enabled: true,
                mcp_enabled: Some(false),
                input_args: {
                    let mut params = Parameters::new();
                    params.add_property("query".to_string(), "string".to_string(), "The SQL query to execute".to_string(), true);
                    params.add_property("params".to_string(), "any[]".to_string(), "The parameters to pass to the query".to_string(), false);
                    params.add_property("database_name".to_string(), "string".to_string(), "By default, the database name is the app_id. You can specify a different name to share the same database in multiple contexts.".to_string(), false);
                    params
                },
                output_arg: ToolOutputArg {
                    json: r#"{"type": "object", "properties": {"result": {"oneOf": [{"type": "string"},{"type": "array"}]}, "type": {"type": "string"}, "rowCount": {"type": "number"}, "rowsAffected": {"type": "number"}}}"#.to_string(),
                },
                config: None,
                usage_type: None,
                tool_offering: None,
            },
            tool_embedding: None, // TODO: add tool embedding
        }
    }
}

fn get_folder_path(app_id: String) -> Result<PathBuf, ToolError> {
    let node_env = fetch_node_environment();
    let node_storage_path = node_env
        .node_storage_path
        .clone()
        .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
    Ok(Path::new(&node_storage_path)
        .join("tools_storage")
        .join(app_id)
        .join("home")
        .join("db.sqlite"))
}

fn get_database_path_from_db_name_config(database_name: String) -> Result<PathBuf, ToolError> {
    let node_env = fetch_node_environment();
    let node_storage_path = node_env
        .node_storage_path
        .clone()
        .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
    let adapted_database_name = database_name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();
    Ok(Path::new(&node_storage_path)
        .join("tools_storage")
        .join("shared_sql_databases")
        .join(format!("{}.sqlite", adapted_database_name)))
}

pub async fn get_current_tables(app_id: String) -> Result<Vec<String>, ToolError> {
    let full_path = get_folder_path(app_id)?;

    if !full_path.exists() {
        return Ok(vec![]);
    }

    let manager = SqliteConnectionManager::file(full_path.clone());
    let pool = Pool::new(manager)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to create connection pool: {}", e)))?;

    let conn = pool
        .get()
        .map_err(|e| ToolError::ExecutionError(format!("Failed to get connection: {}", e)))?;

    let query = "SELECT sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'";

    let tables = conn
        .prepare(query)
        .map_err(|e| ToolError::ExecutionError(format!("Failed to prepare query: {}", e)))?
        .query_map(params_from_iter(&[] as &[&dyn ToSql]), |row| {
            let table_sql: String = row.get(0).unwrap_or_default();
            Ok(table_sql.replace("\n", ""))
        })
        .map_err(|e| ToolError::ExecutionError(format!("Failed to execute query: {}", e)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ToolError::ExecutionError(format!("Failed to collect results: {}", e)))?;
    Ok(tables)
}

#[async_trait]
impl ToolExecutor for SQLProcessorTool {
    async fn execute(
        _bearer: String,
        _tool_id: String,
        app_id: String,
        _db_clone: Arc<SqliteManager>,
        _node_name_clone: ShinkaiName,
        _identity_manager_clone: Arc<Mutex<IdentityManager>>,
        _job_manager_clone: Arc<Mutex<JobManager>>,
        _encryption_secret_key_clone: EncryptionStaticKey,
        _encryption_public_key_clone: EncryptionPublicKey,
        _signing_secret_key_clone: SigningKey,
        parameters: &Map<String, Value>,
        _llm_provider: String,
    ) -> Result<Value, ToolError> {
        let query = parameters
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::ExecutionError("Query parameter is required".to_string()))?;

        let query_params = parameters
            .get("params")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or_default())
                    .collect::<Vec<&str>>()
            })
            .unwrap_or(vec![]);

        let database_name = parameters.get("database_name").and_then(|v| v.as_str());

        let full_path = if let Some(database_name) = database_name {
            get_database_path_from_db_name_config(database_name.to_string())?
        } else {
            get_folder_path(app_id)?
        };

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ToolError::ExecutionError(format!("Failed to create directory structure: {}", e)))?;
        }

        let manager = SqliteConnectionManager::file(full_path.clone());
        let pool = Pool::new(manager)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create connection pool: {}", e)))?;

        let conn = pool
            .get()
            .map_err(|e| ToolError::ExecutionError(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn
            .prepare(query)
            .map_err(|e| ToolError::ExecutionError(format!("Failed to prepare query: {}", e)))?;
        println!("[execute_sqlite_query] path: {:?}", full_path.clone());
        let qp: String = format!("{:?}", query_params);

        // Helper function to safely truncate strings
        fn truncate_string(s: &str, max_length: usize) -> String {
            if s.len() <= max_length {
                s.to_string()
            } else {
                let prefix = s.chars().take(max_length / 2).collect::<String>();
                let suffix = s
                    .chars()
                    .rev()
                    .take(max_length / 2)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
                format!("{}...{}", prefix, suffix)
            }
        }

        println!(
            "[execute_sqlite_query] query: {} {:?}",
            truncate_string(query, 200),
            truncate_string(&qp, 200)
        );

        // For SELECT queries, fetch column names and rows
        if query.trim().to_lowercase().starts_with("select") {
            let column_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();

            let rows = stmt
                .query_map(params_from_iter(query_params.iter()), |row| {
                    let mut map = Map::new();
                    for (i, column_name) in column_names.iter().enumerate() {
                        let value: Value = match row.get_ref(i) {
                            Ok(val) => match val {
                                rusqlite::types::ValueRef::Null => Value::Null,
                                rusqlite::types::ValueRef::Integer(i) => Value::Number(i.into()),
                                rusqlite::types::ValueRef::Real(f) => Value::Number(
                                    serde_json::Number::from_f64(f).unwrap_or(serde_json::Number::from(0)),
                                ),
                                rusqlite::types::ValueRef::Text(s) => {
                                    Value::String(String::from_utf8_lossy(s).into_owned())
                                }
                                rusqlite::types::ValueRef::Blob(b) => {
                                    Value::String(format!("<BLOB: {} bytes>", b.len()))
                                }
                            },
                            Err(_) => Value::Null,
                        };
                        map.insert(column_name.clone(), value);
                    }
                    Ok(map)
                })
                .map_err(|e| ToolError::ExecutionError(format!("Failed to execute query: {}", e)))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ToolError::ExecutionError(format!("Failed to collect results: {}", e)))?;

            Ok(json!({
                "result": rows,
                "type": "select",
                "rowCount": rows.len()
            }))
        } else {
            // For non-SELECT queries (INSERT, UPDATE, DELETE, etc)
            let rows_affected = stmt
                .execute(params_from_iter(query_params.iter()))
                .map_err(|e| ToolError::ExecutionError(format!("Failed to execute query: {}", e)))?;

            Ok(json!({
                "result": format!("Query executed successfully"),
                "type": "modify",
                "rowsAffected": rows_affected
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use shinkai_tools_primitives::tools::rust_tools::RustTool;

    use super::*;

    #[test]
    fn test_tool_router_key() {
        let sql_processor_tool = SQLProcessorTool::new();
        assert_eq!(
            sql_processor_tool.tool.tool_router_key,
            "local:::__official_shinkai:::shinkai_sqlite_query_executor"
        );
    }

    #[test]
    fn test_conversion_to_rust_tool() {
        let sql_processor_tool = SQLProcessorTool::new();

        let rust_tool = RustTool {
            name: sql_processor_tool.tool.name.clone(),
            description: sql_processor_tool.tool.description.clone(),
            mcp_enabled: sql_processor_tool.tool.mcp_enabled.clone(),
            input_args: sql_processor_tool.tool.input_args.clone(),
            output_arg: sql_processor_tool.tool.output_arg.clone(),
            tool_embedding: sql_processor_tool.tool_embedding.clone(),
            tool_router_key: sql_processor_tool.tool.tool_router_key.clone(),
        };

        assert_eq!(rust_tool.name, sql_processor_tool.tool.name);
        assert_eq!(rust_tool.description, sql_processor_tool.tool.description);
        assert_eq!(rust_tool.input_args, sql_processor_tool.tool.input_args);
        assert_eq!(rust_tool.output_arg, sql_processor_tool.tool.output_arg);
        assert_eq!(rust_tool.tool_embedding, sql_processor_tool.tool_embedding);
        assert_eq!(rust_tool.tool_router_key, sql_processor_tool.tool.tool_router_key);
    }
}
