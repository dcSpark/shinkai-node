use embedding_function::EmbeddingFunction;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{ffi::sqlite3_auto_extension, Result, Row, ToSql};
use shinkai_vector_resources::model_type::EmbeddingModelType;
use sqlite_vec::sqlite3_vec_init;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub mod embedding_function;
pub mod files;
pub mod prompt_manager;
pub mod shinkai_tool_manager;
pub mod tool_playground;

#[derive(Error, Debug)]
pub enum SqliteManagerError {
    #[error("Tool already exists with key: {0}")]
    ToolAlreadyExists(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] rusqlite::Error),
    #[error("Embedding generation error: {0}")]
    EmbeddingGenerationError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Tool not found with key: {0}")]
    ToolNotFound(String),
    #[error("ToolPlayground already exists with job_id: {0}")]
    ToolPlaygroundAlreadyExists(String),
    #[error("ToolPlayground not found with job_id: {0}")]
    ToolPlaygroundNotFound(String),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Lock error")]
    LockError,
    // Add other error variants as needed
}

// Updated struct to manage SQLite connections using a connection pool
pub struct SqliteManager {
    pool: Arc<Pool<SqliteConnectionManager>>,
    fts_pool: Arc<Pool<SqliteConnectionManager>>,
    api_url: String,
    model_type: EmbeddingModelType,
}

impl std::fmt::Debug for SqliteManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteManager")
            .field("api_url", &self.api_url)
            .field("model_type", &self.model_type)
            .finish()
    }
}

impl SqliteManager {
    // Creates a new SqliteManager with a connection pool to the specified database path
    pub fn new<P: AsRef<Path>>(
        db_path: P,
        api_url: String,
        model_type: EmbeddingModelType,
    ) -> Result<Self, SqliteManagerError> {
        // Register the sqlite-vec extension
        unsafe {
            sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
        }

        let mut db_path = db_path.as_ref().to_path_buf();
        if db_path.extension().and_then(|ext| ext.to_str()) != Some("db") {
            db_path.set_extension("db");
        }

        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::builder()
            .max_size(10)
            .connection_timeout(Duration::from_secs(60))
            .build(manager)
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;

        // Enable WAL mode, set some optimizations, and enable foreign keys
        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA temp_store=MEMORY;
             PRAGMA mmap_size=262144000; -- 250 MB in bytes (250 * 1024 * 1024)
             PRAGMA foreign_keys = ON;", // Enable foreign key support
        )?;

        // Initialize tables in the persistent database
        Self::initialize_tables(&conn)?;

        // Create a connection pool for the in-memory database
        let fts_manager = SqliteConnectionManager::memory();
        let fts_pool = Pool::builder()
            .max_size(5) // Adjust the pool size as needed
            .build(fts_manager)
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;

        // Initialize FTS table in the in-memory database
        {
            let fts_conn = fts_pool
                .get()
                .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;
            fts_conn.execute_batch(
                "PRAGMA foreign_keys = ON;", // Enable foreign key support for in-memory connection
            )?;
            Self::initialize_fts_tables(&fts_conn)?;
        }

        // Synchronize the FTS table with the main database
        let manager = SqliteManager {
            pool: Arc::new(pool),
            fts_pool: Arc::new(fts_pool), // Use the in-memory connection pool
            api_url,
            model_type,
        };
        let _ = manager.sync_fts_table();

        Ok(manager)
    }

    // Initializes the required tables in the SQLite database
    fn initialize_tables(conn: &rusqlite::Connection) -> Result<()> {
        Self::initialize_prompt_table(conn)?;
        Self::initialize_prompt_vector_tables(conn)?;
        Self::initialize_tools_table(conn)?;
        Self::initialize_tools_vector_table(conn)?;
        Self::initialize_tool_playground_table(conn)?;
        Self::initialize_tool_playground_code_history_table(conn)?;
        Self::initialize_version_table(conn)?;
        Ok(())
    }

    // Initializes the shinkai_prompts table and its indexes
    fn initialize_prompt_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shinkai_prompts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                is_system INTEGER NOT NULL,
                is_enabled INTEGER NOT NULL,
                version TEXT NOT NULL,
                prompt TEXT NOT NULL,
                is_favorite INTEGER NOT NULL
            );",
            [],
        )?;

        // Create indexes for the shinkai_prompts table if needed
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_shinkai_prompts_name ON shinkai_prompts (name);",
            [],
        )?;

        Ok(())
    }

    // New method to initialize prompt vector and associated information tables
    fn initialize_prompt_vector_tables(conn: &rusqlite::Connection) -> Result<()> {
        // Create a table for prompt vector embeddings
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS prompt_vec_items USING vec0(
                embedding float[384],
                is_enabled integer,
                +prompt_id integer
            )",
            [],
        )?;

        Ok(())
    }

    // Updated method to initialize the tools table with name and description columns at the top
    fn initialize_tools_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shinkai_tools (
                name TEXT NOT NULL,
                description TEXT,
                tool_key TEXT NOT NULL UNIQUE,
                embedding_seo TEXT NOT NULL,
                tool_data BLOB NOT NULL,
                tool_header BLOB NOT NULL,
                tool_type TEXT NOT NULL,
                author TEXT NOT NULL,
                version TEXT NOT NULL,
                is_enabled INTEGER NOT NULL,
                on_demand_price REAL,
                is_network INTEGER NOT NULL
            );",
            [],
        )?;

        // Create indexes for the shinkai_tools table if needed
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_shinkai_tools_key ON shinkai_tools (tool_key);",
            [],
        )?;

        Ok(())
    }

    // New method to initialize the tools vector table
    fn initialize_tools_vector_table(conn: &rusqlite::Connection) -> Result<()> {
        // Create a table for tool vector embeddings with metadata columns
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS shinkai_tools_vec_items USING vec0(
                embedding float[384],
                is_enabled integer,
                +is_network integer,
                +tool_key text
            )",
            [],
        )?;

        Ok(())
    }

    // New method to initialize FTS tables
    fn initialize_fts_tables(conn: &rusqlite::Connection) -> Result<()> {
        Self::initialize_tools_fts_table(conn)?;
        Self::initialize_prompts_fts_table(conn)?;
        Ok(())
    }

    // Initialize the FTS table for tool names
    fn initialize_tools_fts_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS shinkai_tools_fts USING fts5(name)",
            [],
        )?;
        Ok(())
    }

    // Initialize the FTS table for prompt names
    fn initialize_prompts_fts_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS shinkai_prompts_fts USING fts5(name)",
            [],
        )?;
        Ok(())
    }

    // Updated method to initialize the tool_playground table with non-nullable and unique tool_router_key
    fn initialize_tool_playground_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_playground (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT,
                author TEXT,
                keywords TEXT, -- Store as a comma-separated list
                configurations TEXT, -- Store as a JSON string
                parameters TEXT, -- Store as a JSON string
                result TEXT, -- Store as a JSON string
                tool_router_key TEXT NOT NULL UNIQUE, -- Non-nullable and unique
                job_id TEXT, -- Allow NULL values
                job_id_history TEXT, -- Store as a comma-separated list
                code TEXT NOT NULL,
                FOREIGN KEY(tool_router_key) REFERENCES shinkai_tools(tool_key) -- Foreign key constraint
            );",
            [],
        )?;
        Ok(())
    }

    // New method to initialize the tool_playground_code_history table
    fn initialize_tool_playground_code_history_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_playground_code_history (
                message_id TEXT PRIMARY KEY,
                tool_router_key TEXT NOT NULL,
                code TEXT NOT NULL,
                FOREIGN KEY(tool_router_key) REFERENCES tool_playground(tool_router_key)
            );",
            [],
        )?;
        Ok(())
    }

    // New method to initialize the version table
    fn initialize_version_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS app_version (
                version TEXT NOT NULL UNIQUE,
                needs_global_reset INTEGER NOT NULL CHECK (needs_global_reset IN (0, 1))
            );",
            [],
        )?;
        Ok(())
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

    // New method to generate embeddings
    pub async fn generate_embeddings(&self, prompt: &str) -> Result<Vec<f32>> {
        let embedding_function = EmbeddingFunction::new(&self.api_url, self.model_type.clone());
        embedding_function.request_embeddings(prompt).await
    }

    // Utility function to generate a vector of length 384 filled with a specified value
    pub fn generate_vector_for_testing(value: f32) -> Vec<f32> {
        vec![value; 384]
    }

    // Method to set the version and determine if a global reset is needed
    pub fn set_version(&self, version: &str) -> Result<()> {
        // Note: add breaking versions here as needed
        let breaking_versions = vec!["0.9.0"];

        let needs_global_reset = self.get_version().map_or(false, |(current_version, _)| {
            breaking_versions
                .iter()
                .any(|&breaking_version| current_version.as_str() < breaking_version && version >= breaking_version)
        });

        let conn = self.get_connection()?;
        conn.execute("DELETE FROM app_version;", [])?;
        conn.execute(
            "INSERT INTO app_version (version, needs_global_reset) VALUES (?, ?);",
            &[&version as &dyn ToSql, &(needs_global_reset as i32) as &dyn ToSql],
        )?;

        Ok(())
    }

    // Method to get the version and reset status
    pub fn get_version(&self) -> Result<(String, bool)> {
        let conn = self.get_connection()?;
        conn.query_row(
            "SELECT version, needs_global_reset FROM app_version LIMIT 1;",
            [],
            |row| {
                let version: String = row.get(0)?;
                let needs_global_reset: i32 = row.get(1)?;
                Ok((version, needs_global_reset != 0))
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_vector_resources::model_type::OllamaTextEmbeddingsInference;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    async fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_set_version_no_reset_needed() {
        let manager = setup_test_db().await;
        manager.set_version("1.0.0").unwrap();
        let (version, needs_reset) = manager.get_version().unwrap();
        assert_eq!(version, "1.0.0");
        assert!(!needs_reset);
    }

    #[tokio::test]
    async fn test_set_version_reset_needed() {
        let manager = setup_test_db().await;
        manager.set_version("0.8.0").unwrap();
        let (version, needs_reset) = manager.get_version().unwrap();
        assert_eq!(version, "0.8.0");
        assert!(!needs_reset);
    }

    #[tokio::test]
    async fn test_set_version_update_no_reset() {
        let manager = setup_test_db().await;
        manager.set_version("0.8.0").unwrap();
        manager.set_version("1.0.0").unwrap();
        let (version, needs_reset) = manager.get_version().unwrap();
        eprintln!("version: {}", version);
        assert_eq!(version, "1.0.0");
        assert!(needs_reset);
    }

    #[tokio::test]
    async fn test_update_from_breaking_version_no_reset() {
        let manager = setup_test_db().await;
        manager.set_version("0.9.0").unwrap();
        manager.set_version("0.9.1").unwrap();
        let (version, needs_reset) = manager.get_version().unwrap();
        assert_eq!(version, "0.9.1");
        assert!(!needs_reset);
    }

    #[tokio::test]
    async fn test_set_version_update_to_breaking_version() {
        let manager = setup_test_db().await;
        manager.set_version("0.8.0").unwrap();
        manager.set_version("0.9.0").unwrap();
        let (version, needs_reset) = manager.get_version().unwrap();
        assert_eq!(version, "0.9.0");
        assert!(needs_reset);
    }
}
