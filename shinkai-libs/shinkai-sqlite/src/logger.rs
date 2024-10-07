use crate::{LogEntry, LogStatus, LogTree, SqliteManager, Tool};
use chrono::{DateTime, Utc};
use futures::future::try_join_all;
use rusqlite::{params, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shinkai_dsl::dsl_schemas::Workflow;
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowLogEntry {
    pub subprocess: Option<String>, // Renamed from step_name and made optional
    pub input: Option<String>,      // New optional field for input
    pub additional_info: String,    // Renamed from message
    pub timestamp: DateTime<Utc>,
    pub status: WorkflowLogEntryStatus, // Updated to include a string for success
    pub result: Option<String>,
}

// Define an enum to represent the status of a log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowLogEntryStatus {
    Success(String), // Now includes a string for additional success information
    Error(String),   // Includes an error message
}

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

    // Initializes the required tables and indexes in the SQLite database
    fn initialize_tables(&self) -> Result<()> {
        let conn = self.manager.get_connection()?;

        // Enable foreign key constraints
        conn.execute("PRAGMA foreign_keys = ON;", [])?;

        // Create the tools table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tools (
                tool_router_key TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                tool_type TEXT NOT NULL,
                instructions TEXT
            );",
            [],
        )?;

        // Create the logs table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id TEXT NOT NULL,
                tool_id TEXT NOT NULL,
                subprocess TEXT,
                parent_id INTEGER,
                execution_order INTEGER NOT NULL,
                input TEXT NOT NULL,
                duration_ms INTEGER,
                result TEXT NOT NULL,
                status TEXT NOT NULL CHECK (status IN ('Success', 'Failure', 'Canceled', 'NonDetermined')),
                error_message TEXT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                log_type TEXT NOT NULL,
                additional_info TEXT,
                FOREIGN KEY(tool_id) REFERENCES tools(tool_router_key) ON DELETE CASCADE,
                FOREIGN KEY(parent_id) REFERENCES logs(id) ON DELETE CASCADE
            );",
            [],
        )?;

        // Create individual indexes on frequently queried columns
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_message_id ON logs (message_id);",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_logs_tool_id ON logs (tool_id);", [])?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_subprocess ON logs (subprocess);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_execution_order ON logs (execution_order);",
            [],
        )?;

        // Create a composite index for queries filtering on multiple columns and sorting
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_message_tool_subprocess_order ON logs (message_id, tool_id, subprocess, execution_order);",
            [],
        )?;

        Ok(())
    }

    // Adds a log entry to the logs table
    pub fn add_log(&self, log: &LogEntry) -> Result<i64> {
        let conn = self.manager.get_connection()?;
        conn.execute(
            "INSERT INTO logs (
                message_id,
                tool_id,
                subprocess,
                parent_id,
                execution_order,
                input,
                duration_ms,
                result,
                status,
                error_message,
                timestamp,
                log_type,
                additional_info
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                log.message_id,
                log.tool_id, // Remains as String
                log.subprocess,
                log.parent_id, // Should be Option<i64>
                log.execution_order,
                log.input.to_string(),
                log.duration_ms, // Updated field name
                log.result.to_string(),
                log.status.to_string(),
                log.error_message,
                log.timestamp,
                log.log_type,
                log.additional_info.as_ref().map(|v| v.to_string()),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    // Adds a tool entry to the tools table
    pub fn add_tool(&self, tool: &Tool) -> Result<String> {
        let conn = self.manager.get_connection()?;
        conn.execute(
            "INSERT INTO tools (tool_router_key, name, tool_type, instructions) VALUES (?1, ?2, ?3, ?4)",
            params![tool.tool_router_key, tool.name, tool.tool_type, tool.instructions],
        )?;
        Ok(tool.tool_router_key.clone())
    }

    pub async fn log_workflow_execution(
        &self,
        message_id: String,
        workflow: Workflow,
        logs: Arc<RwLock<VecDeque<WorkflowLogEntry>>>,
    ) -> Result<()> {
        // Generate the tool key from the workflow
        let tool_key = ShinkaiTool::gen_router_key("local".to_string(), workflow.author.clone(), workflow.name.clone());

        // Check if the tool exists by tool key
        let tool_exists = self.tool_exists_by_key(&tool_key)?;

        // Use the tool key directly as the tool ID
        let tool_id = if tool_exists {
            tool_key.clone()
        } else {
            self.add_tool(&Tool {
                name: workflow.name.clone(),
                tool_type: "Workflow".to_string(),
                tool_router_key: tool_key.clone(),
                instructions: Some(workflow.description.clone().unwrap_or_default()),
            })?
        };

        // Create a log entry for the workflow execution
        let workflow_log_id = self.add_log(&LogEntry {
            id: None,
            message_id: message_id.clone(),
            tool_id: tool_id.clone(),
            subprocess: None,
            parent_id: None,
            execution_order: 0,
            input: Value::Null,
            duration_ms: None,
            result: Value::Null,
            status: LogStatus::Success,
            error_message: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            log_type: "workflow_execution".to_string(),
            additional_info: None,
        })?;

        // Collect entries from the logs VecDeque
        let logs_entries = {
            let logs = logs.read().await;
            logs.clone()
        };

        // Iterate over each step in the logs
        for (step_index, workflow_log_entry) in logs_entries.iter().enumerate() {
            eprintln!("step_index: {:?}", step_index);
            eprintln!("workflow_log_entry: {:?}", workflow_log_entry);
            eprintln!("---");

            let duration_ms = if step_index > 0 { // Updated variable name
                let previous_entry = &logs_entries[step_index - 1];
                let duration = workflow_log_entry.timestamp - previous_entry.timestamp;
                Some(duration.num_milliseconds() as u64)
            } else {
                None
            };

            self.add_log(&LogEntry {
                id: None,
                message_id: message_id.clone(),
                tool_id: tool_id.clone(),
                subprocess: workflow_log_entry.subprocess.clone(),
                parent_id: Some(workflow_log_id),
                execution_order: step_index as i32,
                input: workflow_log_entry.input.clone().map_or(Value::Null, |s| Value::String(s)),
                duration_ms, // Updated field name
                result: workflow_log_entry.result.clone().map_or(Value::Null, |s| Value::String(s)),
                status: match workflow_log_entry.status {
                    WorkflowLogEntryStatus::Success(_) => LogStatus::Success,
                    WorkflowLogEntryStatus::Error(_) => LogStatus::Failure,
                },
                error_message: None,
                timestamp: workflow_log_entry.timestamp.to_rfc3339(),
                log_type: "workflow_operation".to_string(),
                additional_info: Some(Value::String(workflow_log_entry.additional_info.clone())),
            })?;
        }

        Ok(())
    }

    // Updated helper method to check if a tool exists by tool key
    fn tool_exists_by_key(&self, tool_key: &str) -> Result<bool> {
        let conn = self.manager.get_connection()?;
        let mut stmt = conn.prepare("SELECT EXISTS(SELECT 1 FROM tools WHERE tool_router_key = ?1);")?;
        let exists: bool = stmt.query_row(params![tool_key], |row| row.get(0))?;
        Ok(exists)
    }

    // New method to get log IDs for a specific message
    pub fn get_log_ids_for_message(&self, message_id: &str) -> Result<Vec<i64>> {
        let conn = self.manager.get_connection()?;
        let mut stmt = conn.prepare("SELECT id FROM logs WHERE message_id = ?1;")?;
        let log_ids = stmt.query_map(params![message_id], |row| row.get(0))?;
        log_ids.collect()
    }

    // Retrieves logs based on optional filters for message_id, tool_id, subprocess, and sorts by execution_order
    pub fn get_logs(
        &self,
        message_id: Option<&str>,
        tool_id: Option<&str>,
        subprocess: Option<&str>,
    ) -> Result<Vec<LogEntry>> {
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

        // Append ORDER BY clause to sort by execution_order
        query.push_str(" ORDER BY execution_order ASC;");

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
                input: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or(Value::Null),
                duration_ms: row.get(7)?, // Updated field name
                result: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or(Value::Null),
                status: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or(LogStatus::NonDetermined),
                error_message: row.get(10)?,
                timestamp: row.get(11)?,
                log_type: row.get(12)?,
                additional_info: row
                    .get::<_, Option<String>>(13)?
                    .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null)),
            })
        })?;

        log_iter.collect()
    }

    // Standalone async function to build the log tree
    async fn build_tree(
        logger: Arc<SqliteLogger>,
        log_id: i64,
        cache: Arc<Mutex<HashMap<i64, LogEntry>>>,
    ) -> Result<LogTree> {
        let log = {
            let mut cache = cache.lock().await;
            if let Some(log) = cache.get(&log_id) {
                log.clone()
            } else {
                let log = logger.get_log(log_id)?;
                cache.insert(log_id, log.clone());
                log
            }
        };

        let child_logs = logger.get_child_logs(log.id.unwrap())?;
        let child_futures = child_logs.into_iter().map(|child_log| {
            let logger = Arc::clone(&logger);
            let cache = Arc::clone(&cache);
            async move { Self::build_tree(logger, child_log.id.unwrap(), cache).await }
        });

        let children = try_join_all(child_futures).await?;

        Ok(LogTree { log, children })
    }

    pub async fn get_log_tree(&self, log_id: i64) -> Result<LogTree> {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        Self::build_tree(Arc::new(self.clone()), log_id, cache).await
    }

    // Synchronous helper to get a log entry by ID
    fn get_log(&self, log_id: i64) -> Result<LogEntry> {
        let conn = self.manager.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM logs WHERE id = ?1;")?;
        let log = stmt.query_row(params![log_id], |row| {
            Ok(LogEntry {
                id: Some(row.get(0)?),
                message_id: row.get(1)?,
                tool_id: row.get(2)?,
                subprocess: row.get(3)?,
                parent_id: row.get(4)?,
                execution_order: row.get(5)?,
                input: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or(Value::Null),
                duration_ms: row.get(7)?, // Updated field name
                result: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or(Value::Null),
                status: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or(LogStatus::NonDetermined),
                error_message: row.get(10)?,
                timestamp: row.get(11)?,
                log_type: row.get(12)?,
                additional_info: row
                    .get::<_, Option<String>>(13)?
                    .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null)),
            })
        })?;
        Ok(log)
    }

    // Retrieves child logs based on parent_id
    fn get_child_logs(&self, parent_id: i64) -> Result<Vec<LogEntry>> {
        let conn = self.manager.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM logs WHERE parent_id = ?1 ORDER BY execution_order ASC;")?;
        let logs = stmt.query_map(params![parent_id], |row| {
            Ok(LogEntry {
                id: Some(row.get(0)?),
                message_id: row.get(1)?,
                tool_id: row.get(2)?,
                subprocess: row.get(3)?,
                parent_id: row.get(4)?,
                execution_order: row.get(5)?,
                input: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or(Value::Null),
                duration_ms: row.get(7)?, // Updated field name
                result: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or(Value::Null),
                status: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or(LogStatus::NonDetermined),
                error_message: row.get(10)?,
                timestamp: row.get(11)?,
                log_type: row.get(12)?,
                additional_info: row
                    .get::<_, Option<String>>(13)?
                    .map(|s| serde_json::from_str(&s).unwrap_or(Value::Null)),
            })
        })?;
        logs.collect()
    }
}