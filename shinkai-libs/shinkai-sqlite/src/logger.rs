use crate::{SqliteManager, LogEntry, Tool, WorkflowStep, WorkflowOperation, LogStatus};
use rusqlite::{Result, params};
use serde_json::{Value, json};

// Struct representing the SQLite logger
pub struct SqliteLogger<'a> {
    manager: &'a SqliteManager,
}

impl<'a> SqliteLogger<'a> {
    // Constructor for SqliteLogger, initializes the logger and creates necessary tables
    pub fn new(manager: &'a SqliteManager) -> Result<Self> {
        let logger = SqliteLogger { manager };
        logger.initialize_tables()?;
        Ok(logger)
    }

    // Initializes the required tables in the SQLite database
    fn initialize_tables(&self) -> Result<()> {
        // Create the tools table if it doesn't exist
        self.manager.get_connection().execute(
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
        self.manager.get_connection().execute(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL,
                tool_id INTEGER NOT NULL,
                subprocess TEXT,
                parent_id INTEGER,
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
    pub fn add_log(&self, log: &LogEntry) -> Result<i32> {
        self.manager.get_connection().execute(
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
        Ok(self.manager.get_connection().last_insert_rowid() as i32)
    }

    // Adds a tool entry to the tools table
    pub fn add_tool(&self, tool: &Tool) -> Result<i32> {
        self.manager.get_connection().execute(
            "INSERT INTO tools (name, tool_type, tool_router_key, instructions) VALUES (?1, ?2, ?3, ?4)",
            params![tool.name, tool.tool_type, tool.tool_router_key, tool.instructions],
        )?;
        Ok(self.manager.get_connection().last_insert_rowid() as i32)
    }

    // Logs the execution of a workflow, including its steps and operations
    pub fn log_workflow_execution(&self, message_id: i32, tool_id: i32, workflow: &[WorkflowStep]) -> Result<()> {
        let workflow_log_id = self.add_log(&LogEntry {
            id: None,  // Changed from 0 to None
            message_id,
            tool_id,
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
                id: None,  // Changed from 0 to None
                message_id,
                tool_id,
                subprocess: Some(step.name.clone()),
                parent_id: Some(workflow_log_id),
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
                    id: None,  // Changed from 0 to None
                    message_id,
                    tool_id,
                    subprocess: Some(format!("{}.{}", step.name, operation_type)),
                    parent_id: Some(step_log_id),
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

    // Retrieves logs based on optional filters for message_id, tool_id, and subprocess
    pub fn get_logs(&self, message_id: Option<i32>, tool_id: Option<i32>, subprocess: Option<&str>) -> Result<Vec<LogEntry>> {
        // Build the query string with optional filters
        let mut query = "SELECT * FROM logs WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(mid) = message_id {
            query.push_str(" AND message_id = ?");
            params.push(Box::new(mid));
        }
        if let Some(tid) = tool_id {
            query.push_str(" AND tool_id = ?");
            params.push(Box::new(tid));
        }
        if let Some(sp) = subprocess {
            query.push_str(" AND subprocess = ?");
            params.push(Box::new(sp.to_string()));
        }

        // Prepare the query and execute it
        let mut stmt = self.manager.get_connection().prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let log_iter = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(LogEntry {
                id: Some(row.get(0)?),  // Changed to Some(...)
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

        // Collect and return the logs
        log_iter.collect()
    }
}