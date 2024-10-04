use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Result, Row, ToSql};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

pub mod logger;

// Updated struct to manage SQLite connections using a connection pool
pub struct SqliteManager {
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl SqliteManager {
    // Creates a new SqliteManager with a connection pool to the specified database path
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let mut db_path = db_path.as_ref().to_path_buf();
        if db_path.extension().and_then(|ext| ext.to_str()) != Some("db") {
            db_path.set_extension("db");
        }

        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::builder()
            .max_size(10) // Adjust based on your needs
            .build(manager)
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;

        // Enable WAL mode and set some optimizations
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA temp_store=MEMORY;
             PRAGMA mmap_size=262144000;", // 250 MB in bytes (250 * 1024 * 1024)
        )?;

        Ok(SqliteManager { pool: Arc::new(pool) })
    }

    // Returns a connection from the pool
    pub fn get_connection(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1), // Using a generic error code
                Some(e.to_string()),
            )
        })
    }

    // Execute a SQL query with parameters
    pub fn execute(&self, sql: &str, params: &[&dyn ToSql]) -> Result<usize> {
        let conn = self.get_connection()?;
        conn.execute(sql, params)
    }

    // Query a row from the database
    pub fn query_row<T, F>(&self, sql: &str, params: &[&dyn ToSql], f: F) -> Result<T>
    where
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        let conn = self.get_connection()?;
        conn.query_row(sql, params, f)
    }
}

/// Represents the status of an operation or step in a log entry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum LogStatus {
    Success,
    Failure,
    Cancelled,
    // Add more status types as needed
}

/// Represents a log entry in the database, capturing details of operations, tool executions, and workflow steps.
#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique identifier for the log entry.
    /// This field will be set by the database upon insertion.
    #[serde(skip_deserializing)]
    pub id: Option<i64>,  // Changed from Option<i32> to Option<i64>

    /// Identifier for the related message or task that initiated this log entry.
    /// This allows grouping of logs related to a single user request or system task.
    pub message_id: String,

    /// Identifier for the tool that generated this log entry.
    /// Tools can be workflows, individual operations, or any other executable components in the system.
    pub tool_id: String,

    /// Optional identifier for a subprocess within a tool execution.
    /// For example, in a workflow, this could represent a specific step or operation.
    /// It allows for more granular tracking of complex tool executions.
    pub subprocess: Option<String>,

    /// Optional identifier referencing another log entry that is the "parent" of this entry.
    /// This creates a hierarchical structure in logging, useful for:
    /// 1. Workflow steps: The main workflow execution log can be the parent of its step logs.
    /// 2. Nested operations: A high-level operation log can be the parent of its sub-operation logs.
    /// 3. Error contexts: An error log can have the operation log that caused it as its parent.
    /// This field allows for tracing the execution path and understanding the context of each log entry.
    pub parent_id: Option<String>,

    /// The order in which this log entry was executed relative to other entries in the same context.
    /// This is particularly useful for maintaining the sequence of operations in a workflow or complex process.
    pub execution_order: i32,

    /// The input data or parameters for the operation or step that this log entry represents.
    /// Stored as a JSON Value for flexibility in data structure.
    pub input: Value,

    /// Optional duration of the operation, typically in seconds.
    /// Useful for performance monitoring and optimization.
    pub duration: Option<f64>,

    /// The result or output of the operation or step.
    /// Stored as a JSON Value to accommodate various result structures.
    pub result: Value,

    /// The status of the operation or step.
    /// This enum provides a clear set of possible statuses for the logged action.
    pub status: LogStatus,

    /// Optional error message if the operation or step encountered an error.
    /// This field is particularly useful for debugging and error tracking.
    pub error_message: Option<String>,

    /// Timestamp of when this log entry was created.
    /// Typically stored in a standardized format like ISO 8601.
    pub timestamp: String,

    /// The type of log entry, e.g., "workflow_execution", "tool_operation", "system_event".
    /// This field helps in categorizing and filtering logs for analysis.
    pub log_type: String,

    /// Optional field for any additional information that doesn't fit into the standard fields.
    /// Stored as a JSON Value for flexibility.
    pub additional_info: Option<Value>,
}

impl Clone for LogEntry {
    fn clone(&self) -> Self {
        LogEntry {
            id: self.id,
            message_id: self.message_id.clone(),
            tool_id: self.tool_id.clone(),
            subprocess: self.subprocess.clone(),
            parent_id: self.parent_id.clone(),
            execution_order: self.execution_order,
            input: self.input.clone(),
            duration: self.duration,
            result: self.result.clone(),
            status: self.status.clone(),
            error_message: self.error_message.clone(),
            timestamp: self.timestamp.clone(),
            log_type: self.log_type.clone(),
            additional_info: self.additional_info.clone(),
        }
    }
}

// Struct representing a tool in the database
#[derive(Debug)]
pub struct Tool {
    pub id: i32,                         // Unique identifier for the tool
    pub name: String,                    // Name of the tool
    pub tool_type: String,               // Type of the tool
    pub tool_router_key: Option<String>, // Optional router key for the tool
    pub instructions: Option<String>,    // Optional instructions for the tool
}

// Struct representing a step in a workflow
#[derive(Debug)]
pub struct WorkflowStep {
    pub name: String,                       // Name of the workflow step
    pub operations: Vec<WorkflowOperation>, // List of operations in the workflow step
}

// Enum representing different types of workflow operations
#[derive(Debug)]
pub enum WorkflowOperation {
    RegisterOperation { register: String, value: String }, // Operation to register a value
    FunctionCall { name: String, args: Vec<String> },      // Operation to call a function with arguments
}

// Re-export the logger for convenience
pub use logger::SqliteLogger;

#[derive(Debug, Serialize, Deserialize)]
pub struct LogTree {
    pub log: LogEntry,
    pub children: Vec<LogTree>,
}
