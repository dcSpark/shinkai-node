use crate::{LogEntry, LogStatus, LogTree, SqliteManager, Tool, WorkflowOperation, WorkflowStep};
use rusqlite::{Result, params};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use futures::future::try_join_all;

// Updated struct representing the SQLite logger
#[derive(Clone)]
pub struct SqliteLogger {
    manager: Arc<SqliteManager>,
}

impl SqliteLogger {
    // Constructor for SqliteLogger, initializes the logger and creates necessary tables
    pub fn new(manager: Arc<SqliteManager>) -> Result<Self> {
        let logger = SqliteLogger { manager };
        logger.initialize_tables()?;
        Ok(logger)
    }

    // Initializes the required tables in the SQLite database
    fn initialize_tables(&self) -> Result<()> {
        let conn = self.manager.get_connection()?;
        
        // Create the tools table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tools (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                tool_type TEXT NOT NULL,
                tool_router_key TEXT,
                instructions TEXT
            )",
            [],
        )?;

        // Create the logs table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id TEXT NOT NULL,
                tool_id TEXT NOT NULL,
                subprocess TEXT,
                parent_id TEXT,
                execution_order INTEGER NOT NULL,
                input TEXT NOT NULL,
                duration REAL,
                result TEXT NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                timestamp TEXT NOT NULL,
                log_type TEXT NOT NULL,
                additional_info TEXT,
                FOREIGN KEY(tool_id) REFERENCES tools(id),
                FOREIGN KEY(parent_id) REFERENCES logs(id) ON DELETE CASCADE
            )",
            [],
        )?;

        Ok(())
    }

    // Adds a log entry to the logs table
    pub fn add_log(&self, log: &LogEntry) -> Result<i64> {
        let conn = self.manager.get_connection()?;
        conn.execute(
            "INSERT INTO logs (message_id, tool_id, subprocess, parent_id, execution_order, input, duration, result, status, error_message, timestamp, log_type, additional_info)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                log.message_id,
                log.tool_id,
                log.subprocess,
                log.parent_id,
                log.execution_order,
                log.input.to_string(),
                log.duration,
                log.result.to_string(),
                serde_json::to_string(&log.status).unwrap(),
                log.error_message,
                log.timestamp,
                log.log_type,
                log.additional_info.as_ref().map(|v| v.to_string()),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    // Adds a tool entry to the tools table
    pub fn add_tool(&self, tool: &Tool) -> Result<i32> {
        let conn = self.manager.get_connection()?;
        conn.execute(
            "INSERT INTO tools (name, tool_type, tool_router_key, instructions) VALUES (?1, ?2, ?3, ?4)",
            params![tool.name, tool.tool_type, tool.tool_router_key, tool.instructions],
        )?;
        Ok(conn.last_insert_rowid() as i32)
    }

    // Logs the execution of a workflow, including its steps and operations
    pub fn log_workflow_execution(&self, message_id: String, tool_id: String, workflow: &[WorkflowStep]) -> Result<()> {
        let workflow_log_id = self.add_log(&LogEntry {
            id: None,
            message_id: message_id.clone(),
            tool_id: tool_id.clone(),
            subprocess: None,
            parent_id: None,
            execution_order: 0,
            input: Value::Null,
            duration: None,
            result: Value::Null,
            status: LogStatus::Success,
            error_message: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            log_type: "workflow_execution".to_string(),
            additional_info: None,
        })?;

        for (step_index, step) in workflow.iter().enumerate() {
            let step_log_id = self.add_log(&LogEntry {
                id: None,
                message_id: message_id.clone(),
                tool_id: tool_id.clone(),
                subprocess: Some(step.name.clone()),
                parent_id: Some(workflow_log_id.to_string()),
                execution_order: step_index as i32,
                input: Value::Null,
                duration: None,
                result: Value::Null,
                status: LogStatus::Success,
                error_message: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                log_type: "workflow_step".to_string(),
                additional_info: None,
            })?;

            for (op_index, operation) in step.operations.iter().enumerate() {
                // Determine the type and details of the operation
                let (operation_type, operation_details) = match operation {
                    WorkflowOperation::RegisterOperation { register, value } => (
                        "RegisterOperation",
                        format!("Setting register {} to {:?}", register, value),
                    ),
                    WorkflowOperation::FunctionCall { name, args } => (
                        "FunctionCall",
                        format!("Calling function {} with args: {:?}", name, args),
                    ),
                };

                // Create a JSON object for additional information about the operation
                let additional_info = json!({
                    "operation_type": operation_type,
                    "operation_details": operation_details,
                });

                // Log the operation
                self.add_log(&LogEntry {
                    id: None,
                    message_id: message_id.clone(),
                    tool_id: tool_id.clone(),
                    subprocess: Some(format!("{}.{}", step.name, operation_type)),
                    parent_id: Some(step_log_id.to_string()),
                    execution_order: op_index as i32,
                    input: Value::Null,
                    duration: None,
                    result: Value::Null,
                    status: LogStatus::Success,
                    error_message: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    log_type: "workflow_operation".to_string(),
                    additional_info: Some(additional_info),
                })?;
            }
        }

        Ok(())
    }

    // New method to get log IDs for a specific message
    pub fn get_log_ids_for_message(&self, message_id: &str) -> Result<Vec<i64>> {
        let conn = self.manager.get_connection()?;
        let mut stmt = conn.prepare("SELECT id FROM logs WHERE message_id = ?")?;
        let log_ids = stmt.query_map([message_id], |row| row.get(0))?;
        log_ids.collect()
    }

    // Retrieves logs based on optional filters for message_id, tool_id, and subprocess
    pub fn get_logs(&self, message_id: Option<&str>, tool_id: Option<&str>, subprocess: Option<&str>) -> Result<Vec<LogEntry>> {
        let mut query = "SELECT * FROM logs WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(mid) = message_id {
            query.push_str(" AND message_id = ?");
            params.push(Box::new(mid.to_string()));
        }
        if let Some(tid) = tool_id {
            query.push_str(" AND tool_id = ?");
            params.push(Box::new(tid.to_string()));
        }
        if let Some(sp) = subprocess {
            query.push_str(" AND subprocess = ?");
            params.push(Box::new(sp.to_string()));
        }

        let conn = self.manager.get_connection()?;
        let mut stmt = conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let log_iter = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(LogEntry {
                id: Some(row.get(0)?),
                message_id: row.get(1)?,
                tool_id: row.get(2)?,
                subprocess: row.get(3)?,
                parent_id: row.get(4)?,
                execution_order: row.get(5)?,
                input: serde_json::from_str(&row.get::<_, String>(6)?).unwrap(),
                duration: row.get(7)?,
                result: serde_json::from_str(&row.get::<_, String>(8)?).unwrap(),
                status: serde_json::from_str(&row.get::<_, String>(9)?).unwrap(),
                error_message: row.get(10)?,
                timestamp: row.get(11)?,
                log_type: row.get(12)?,
                additional_info: row.get::<_, Option<String>>(13)?.map(|s| serde_json::from_str(&s).unwrap()),
            })
        })?;

        log_iter.collect()
    }

    // Standalone async function to build the log tree
    async fn build_tree(logger: Arc<SqliteLogger>, log_id: i64, cache: Arc<Mutex<HashMap<i64, LogEntry>>>) -> Result<LogTree> {
        let log = {
            let mut cache = cache.lock().await;
            if let Some(log) = cache.get(&log_id) {
                log.clone()
            } else {
                let log = logger.get_log(log_id).await?;
                cache.insert(log_id, log.clone());
                log
            }
        };

        let child_logs = logger.get_child_logs(&log.id.unwrap().to_string()).await?;
        let child_futures = child_logs.into_iter().map(|child_log| {
            let logger = Arc::clone(&logger);
            let cache = Arc::clone(&cache);
            async move {
                Self::build_tree(logger, child_log.id.unwrap() as i64, cache).await
            }
        });

        let children = try_join_all(child_futures).await?;

        Ok(LogTree { log, children })
    }

    pub async fn get_log_tree(&self, log_id: i64) -> Result<LogTree> {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        Self::build_tree(Arc::new(self.clone()), log_id, cache).await
    }

    async fn get_log(&self, log_id: i64) -> Result<LogEntry> {
        let conn = self.manager.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM logs WHERE id = ?")?;
        let log = stmt.query_row(params![log_id], |row| {
            Ok(LogEntry {
                id: Some(row.get(0)?),
                message_id: row.get(1)?,
                tool_id: row.get(2)?,
                subprocess: row.get(3)?,
                parent_id: row.get(4)?,
                execution_order: row.get(5)?,
                input: serde_json::from_str(&row.get::<_, String>(6)?).unwrap(),
                duration: row.get(7)?,
                result: serde_json::from_str(&row.get::<_, String>(8)?).unwrap(),
                status: serde_json::from_str(&row.get::<_, String>(9)?).unwrap(),
                error_message: row.get(10)?,
                timestamp: row.get(11)?,
                log_type: row.get(12)?,
                additional_info: row.get::<_, Option<String>>(13)?.map(|s| serde_json::from_str(&s).unwrap()),
            })
        })?;
        Ok(log)
    }

    async fn get_child_logs(&self, parent_id: &str) -> Result<Vec<LogEntry>> {
        let conn = self.manager.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM logs WHERE parent_id = ?")?;
        let logs = stmt.query_map(params![parent_id], |row| {
            Ok(LogEntry {
                id: Some(row.get(0)?),
                message_id: row.get(1)?,
                tool_id: row.get(2)?,
                subprocess: row.get(3)?,
                parent_id: row.get(4)?,
                execution_order: row.get(5)?,
                input: serde_json::from_str(&row.get::<_, String>(6)?).unwrap(),
                duration: row.get(7)?,
                result: serde_json::from_str(&row.get::<_, String>(8)?).unwrap(),
                status: serde_json::from_str(&row.get::<_, String>(9)?).unwrap(),
                error_message: row.get(10)?,
                timestamp: row.get(11)?,
                log_type: row.get(12)?,
                additional_info: row.get::<_, Option<String>>(13)?.map(|s| serde_json::from_str(&s).unwrap()),
            })
        })?;
        logs.collect()
    }
}