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

pub mod agent_manager;
pub mod cron_task_manager;
pub mod embedding_function;
pub mod files;
pub mod identity_manager;
pub mod identity_registration;
pub mod invoice_manager;
pub mod llm_provider_manager;
pub mod my_subscriptions_manager;
pub mod network_notifications_manager;
pub mod prompt_manager;
pub mod settings_manager;
pub mod shinkai_tool_manager;
pub mod tool_payment_req_manager;
pub mod tool_playground;
pub mod wallet_manager;

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
    #[error("Tool offering not found with key: {0}")]
    ToolOfferingNotFound(String),
    #[error("DateTime parse error: {0}")]
    DateTimeParseError(String),
    #[error("Subscription not found with id: {0}")]
    SubscriptionNotFound(String),
    #[error("Wallet manager not found")]
    WalletManagerNotFound,
    #[error("Data not found")]
    DataNotFound,
    #[error("Data already exists")]
    DataAlreadyExists,
    #[error("Invalid identity name: {0}")]
    InvalidIdentityName(String),
    #[error("Invoice not found with id: {0}")]
    InvoiceNotFound(String),
    #[error("Network error not found with id: {0}")]
    InvoiceNetworkErrorNotFound(String),
    #[error("Profile does not exist: {0}")]
    ProfileDoesNotExist(String),
    #[error("Profile name already exists")]
    ProfileNameAlreadyExists,
    #[error("Invalid profile name: {0}")]
    InvalidProfileName(String),
    #[error("Invalid attribute name: {0}")]
    InvalidAttributeName(String),
    #[error("Registration code does not exist")]
    CodeNonExistent,
    #[error("Registration code already used")]
    CodeAlreadyUsed,
    #[error("Error: {0}")]
    SomeError(String),
    #[error("Missing value: {0}")]
    MissingValue(String),
    // Add other error variants as needed
}

// Updated struct to manage SQLite connections using a connection pool
pub struct SqliteManager {
    pool: Arc<Pool<SqliteConnectionManager>>,
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
        Self::initialize_agents_table(conn)?;
        Self::initialize_cron_task_table(conn)?;
        Self::initialize_device_identities_table(conn)?;
        Self::initialize_standard_identities_table(conn)?;
        Self::initialize_llm_providers_table(conn)?;
        Self::initialize_local_node_keys_table(conn)?;
        Self::initialize_my_subscriptions_table(conn)?;
        Self::initialize_network_notifications_table(conn)?;
        Self::initialize_prompt_table(conn)?;
        Self::initialize_prompt_vector_tables(conn)?;
        Self::initialize_registration_code_table(conn)?;
        Self::initialize_settings_table(conn)?;
        Self::initialize_tools_table(conn)?;
        Self::initialize_tools_vector_table(conn)?;
        Self::initialize_tool_micropayments_requirements_table(conn)?;
        Self::initialize_tool_micropayments_tool_invoice_table(conn)?;
        Self::initialize_tool_micropayments_tool_invoice_network_errors_table(conn)?;
        Self::initialize_tool_playground_table(conn)?;
        Self::initialize_tool_playground_code_history_table(conn)?;
        Self::initialize_wallets_table(conn)?;
        Ok(())
    }

    fn initialize_agents_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shinkai_agents (
                agent_id TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                full_identity_name TEXT NOT NULL,
                llm_provider_id TEXT NOT NULL,
                ui_description TEXT NOT NULL,
                knowledge TEXT NOT NULL,
                storage_path TEXT NOT NULL,
                tools TEXT NOT NULL,
                debug_mode INTEGER NOT NULL,
                config TEXT -- Store as a JSON string
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_cron_task_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cron_tasks (
                full_identity_name TEXT NOT NULL,
                task_id TEXT NOT NULL,
                cron TEXT NOT NULL,
                prompt TEXT NOT NULL,
                subprompt TEXT NOT NULL,
                url TEXT NOT NULL,
                crawl_links INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                llm_provider_id TEXT NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_device_identities_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS device_identities (
                device_name TEXT NOT NULL UNIQUE,
                profile_encryption_public_key BLOB NOT NULL,
                profile_signature_public_key BLOB NOT NULL,
                device_encryption_public_key BLOB NOT NULL,
                device_signature_public_key BLOB NOT NULL,
                permission_type TEXT NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_standard_identities_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS standard_identities (
                profile_name TEXT NOT NULL UNIQUE,
                addr BLOB,
                profile_encryption_public_key BLOB,
                profile_signature_public_key BLOB,
                identity_type TEXT NOT NULL,
                permission_type TEXT NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_llm_providers_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS llm_providers (
                db_llm_provider_id TEXT NOT NULL UNIQUE,
                id TEXT NOT NULL,
                full_identity_name TEXT NOT NULL,
                external_url TEXT,
                api_key TEXT,
                model TEXT
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_local_node_keys_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS local_node_keys (
                node_name TEXT NOT NULL UNIQUE,
                node_encryption_public_key BLOB NOT NULL,
                node_signature_public_key BLOB NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_my_subscriptions_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS my_subscriptions (
                subscription_id TEXT NOT NULL UNIQUE,
                subscription_id_data BLOB NOT NULL,
                shared_folder TEXT NOT NULL,
                streaming_node TEXT NOT NULL,
                streaming_profile TEXT NOT NULL,
                subscription_description TEXT,
                subscriber_destination_path TEXT,
                subscriber_node TEXT NOT NULL,
                subscriber_profile TEXT NOT NULL,
                payment TEXT,
                state TEXT NOT NULL,
                date_created TEXT NOT NULL,
                last_modified TEXT NOT NULL,
                last_sync TEXT,
                http_preferred INTEGER
            );",
            [],
        )?;

        // Create indexes for the my_subscriptions table if needed
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_my_subscriptions_subscription_id ON my_subscriptions (subscription_id);",
            [],
        )?;

        Ok(())
    }

    fn initialize_network_notifications_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS network_notifications (
                full_name TEXT NOT NULL,
                message TEXT NOT NULL,
                timestamp TEXT NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_registration_code_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS registration_code (
                code TEXT NOT NULL UNIQUE,
                code_data BLOB NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_settings_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shinkai_settings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                supported_embedding_models TEXT NOT NULL
            );",
            [],
        )?;

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
        // Create a table for tool vector embeddings
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS shinkai_tools_vec_items USING vec0(embedding float[384])",
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

    fn initialize_wallets_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shinkai_wallet (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                wallet_data BLOB NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_tool_micropayments_requirements_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_micropayments_requirements (
                tool_key TEXT NOT NULL UNIQUE,
                usage_type TEXT NOT NULL,
                meta_description TEXT
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_tool_micropayments_tool_invoice_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_micropayments_tool_invoices (
                invoice_id TEXT NOT NULL UNIQUE,
                provider_name TEXT NOT NULL,
                requester_name TEXT NOT NULL,
                usage_type_inquiry TEXT NOT NULL,
                shinkai_offering_key TEXT NOT NULL,
                request_date_time TEXT NOT NULL,
                invoice_date_time TEXT NOT NULL,
                expiration_time TEXT NOT NULL,
                status TEXT NOT NULL,
                payment TEXT, -- Store as a JSON string
                address TEXT NOT NULL, -- Store as a JSON string
                tool_data BLOB,
                response_date_time TEXT,
                result_str TEXT,

                FOREIGN KEY(shinkai_offering_key) REFERENCES tool_micropayments_requirements(tool_key)
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_tool_micropayments_tool_invoice_network_errors_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_micropayments_tool_invoice_network_errors (
                invoice_id TEXT NOT NULL UNIQUE,
                provider_name TEXT NOT NULL,
                requester_name TEXT NOT NULL,
                request_date_time TEXT NOT NULL,
                response_date_time TEXT NOT NULL,
                user_error_message TEXT,
                error_message TEXT NOT NULL
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
}
