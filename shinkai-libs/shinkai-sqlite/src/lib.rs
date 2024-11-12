use embedding_function::EmbeddingFunction;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{ffi::sqlite3_auto_extension, Result, Row, ToSql};
use shinkai_vector_resources::model_type::EmbeddingModelType;
use sqlite_vec::sqlite3_vec_init;
use std::path::Path;
use std::sync::Arc;

pub mod embedding_function;
pub mod prompt_manager;
pub mod shinkai_prompt;
pub mod tool_header_manager;
// Updated struct to manage SQLite connections using a connection pool
pub struct SqliteManager {
    pool: Arc<Pool<SqliteConnectionManager>>,
    api_url: String,
    model_type: EmbeddingModelType,
}

impl SqliteManager {
    // Creates a new SqliteManager with a connection pool to the specified database path
    pub fn new<P: AsRef<Path>>(db_path: P, api_url: String, model_type: EmbeddingModelType) -> Result<Self> {
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

        // Initialize tables
        Self::initialize_tables(&conn)?;

        Ok(SqliteManager {
            pool: Arc::new(pool),
            api_url,
            model_type,
        })
    }

    // Initializes the required tables in the SQLite database
    fn initialize_tables(conn: &rusqlite::Connection) -> Result<()> {
        Self::initialize_prompt_table(conn)?;
        Self::initialize_prompt_vector_tables(conn)?;
        Self::initialize_tools_table(conn)?;
        Self::initialize_tools_vector_table(conn)?;
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
            "CREATE VIRTUAL TABLE IF NOT EXISTS prompt_vec_items USING vec0(embedding float[384])",
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
                tool_key TEXT NOT NULL,
                embedding_seo TEXT NOT NULL,
                tool_data BLOB NOT NULL,
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
        // Create a table for tool vector embeddings
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS tools_vec_items USING vec0(embedding float[384])",
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
}
