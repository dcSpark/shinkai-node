use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::argument::ToolArgument;
use shinkai_tools_primitives::tools::{argument::ToolOutputArg, shinkai_tool::ShinkaiToolHeader};
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_resources::embeddings::Embedding;
use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Map, Value};
use shinkai_tools_primitives::tools::error::ToolError;

use ed25519_dalek::SigningKey;
use shinkai_db::db::ShinkaiDB;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

use x25519_dalek::PublicKey as EncryptionPublicKey;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::tools::tool_implementation::tool_traits::ToolExecutor;
use crate::utils::environment::fetch_node_environment;

use tokio::sync::{Mutex, RwLock};

use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params_from_iter;

// LLM Tool
pub struct SQLProcessorTool {
    pub tool: ShinkaiToolHeader,
    pub tool_embedding: Option<Embedding>,
}

impl SQLProcessorTool {
    pub fn new() -> Self {
        Self {
            tool: ShinkaiToolHeader {
                name: "Shinkai SQLite Query Executor".to_string(),
                toolkit_name: "shinkai_custom".to_string(),
                description: r#"Tool for executing a single SQL query on a specified database file. 
                If this tool is used, you need to create if not exists the tables used other queries.
                Table creation should always use 'CREATE TABLE IF NOT EXISTS'.
                
                Example table creation:
                CREATE TABLE IF NOT EXISTS table_name (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    field_1 TEXT NOT NULL,
                    field_2 DATETIME DEFAULT CURRENT_TIMESTAMP,
                    field_3 INTEGER,
                    field_4 TEXT
                );
                
                Example insert:
                INSERT INTO table_name (field_1, field_3, field_4) VALUES ('value_1', 1, 'value_4');
                
                Example read:
                SELECT * FROM table_name WHERE field_2 > datetime('now', '-1 day');
                SELECT field_1, field_3 FROM table_name WHERE field_3 > 100 ORDER BY field_2 DESC LIMIT 10;"#
                    .to_string(),
                tool_router_key: "local:::rust_toolkit:::shinkai_sqlite_query_executor".to_string(),
                tool_type: "Rust".to_string(),
                formatted_tool_summary_for_ui: "Execute SQLite queries".to_string(),
                author: "Shinkai".to_string(),
                version: "1.0".to_string(),
                enabled: true,
                input_args: vec![
                    ToolArgument::new(
                        "database_name".to_string(),
                        "string".to_string(),
                        "Database name. Use 'default' to use default database".to_string(),
                        true,
                    ),
                    ToolArgument::new(
                        "query".to_string(),
                        "string".to_string(),
                        "The SQL query to execute".to_string(),
                        true,
                    ),
                    ToolArgument::new(
                        "query_params".to_string(),
                        "any[]".to_string(),
                        "The parameters to bind to the query".to_string(),
                        false,
                    ),
                ],
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

#[async_trait]
impl ToolExecutor for SQLProcessorTool {
    async fn execute(
        _bearer: String,
        tool_id: String,
        app_id: String,
        _db_clone: Arc<ShinkaiDB>,
        _vector_fs_clone: Arc<VectorFS>,
        _sqlite_manager: Arc<RwLock<SqliteManager>>,
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

        let node_env = fetch_node_environment();
        let node_storage_path = node_env
            .node_storage_path
            .clone()
            .ok_or_else(|| ToolError::ExecutionError("Node storage path is not set".to_string()))?;
        let full_path = Path::new(&node_storage_path)
            .join("tools_storage")
            .join(app_id)
            .join("home")
            .join(tool_id)
            .join("db.sqlite");

        let query_params = parameters
            .get("query_params")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or_default())
                    .collect::<Vec<&str>>()
            })
            .unwrap_or(vec![]);

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
        println!(
            "[execute_sqlite_query] query: {} {:?}",
            if query.len() > 200 {
                format!("{}...{}", &query[..100], &query[query.len() - 100..])
            } else {
                query.to_string()
            },
            if qp.len() > 200 {
                format!("{}...{}", &qp[..100], &qp[qp.len() - 100..])
            } else {
                qp
            }
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
            "local:::rust_toolkit:::shinkai_sqlite_query_executor"
        );
    }

    #[test]
    fn test_conversion_to_rust_tool() {
        let sql_processor_tool = SQLProcessorTool::new();

        let rust_tool = RustTool {
            name: sql_processor_tool.tool.name.clone(),
            description: sql_processor_tool.tool.description.clone(),
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
