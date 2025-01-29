use embedding_function::EmbeddingFunction;
use errors::SqliteManagerError;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{ffi::sqlite3_auto_extension, Result, Row, ToSql};
use shinkai_embedding::model_type::EmbeddingModelType;
use sqlite_vec::sqlite3_vec_init;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub mod agent_manager;
pub mod cron_task_manager;
pub mod embedding_function;
pub mod errors;
pub mod file_inbox_manager;
pub mod file_system;
pub mod files;
pub mod identity_manager;
pub mod identity_registration;
pub mod inbox_manager;
pub mod invoice_manager;
pub mod invoice_request_manager;
pub mod job_manager;
pub mod job_queue_manager;
pub mod keys_manager;
pub mod llm_provider_manager;
pub mod oauth_manager;
pub mod prompt_manager;
pub mod regex_pattern_manager;
pub mod retry_manager;
pub mod settings_manager;
pub mod sheet_manager;
pub mod shinkai_tool_manager;
pub mod source_file_manager;
pub mod tool_payment_req_manager;
pub mod tool_playground;
pub mod wallet_manager;

// Updated struct to manage SQLite connections using a connection pool
pub struct SqliteManager {
    pool: Arc<Pool<SqliteConnectionManager>>,
    fts_pool: Arc<Pool<SqliteConnectionManager>>,
    api_url: String,
}

impl std::fmt::Debug for SqliteManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteManager").field("api_url", &self.api_url).finish()
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

        // Create all subfolders if they don't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;
        }

        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::builder()
            .max_size(10)
            .connection_timeout(Duration::from_secs(60))
            .build(manager)
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;

        let conn = pool
            .get()
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;

        // Enable WAL mode, set some optimizations, and enable foreign keys
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
                 PRAGMA synchronous=FULL;
                 PRAGMA temp_store=MEMORY;
                 PRAGMA optimize;
                 PRAGMA busy_timeout = 5000;
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
        };
        let fts_sync_result = manager.sync_tools_fts_table();
        if let Err(e) = fts_sync_result {
            eprintln!("Error synchronizing Tools FTS table: {}", e);
        }

        let fts_sync_result = manager.sync_prompts_fts_table();
        if let Err(e) = fts_sync_result {
            eprintln!("Error synchronizing Prompts FTS table: {}", e);
        }

        manager.update_default_embedding_model(model_type)?;

        Ok(manager)
    }

    // Initializes the required tables in the SQLite database
    fn initialize_tables(conn: &rusqlite::Connection) -> Result<()> {
        Self::initialize_agents_table(conn)?;
        Self::initialize_cron_tasks_table(conn)?;
        Self::initialize_cron_task_executions_table(conn)?;
        Self::initialize_device_identities_table(conn)?;
        Self::initialize_standard_identities_table(conn)?;
        // TODO: remove this
        Self::initialize_file_inboxes_table(conn)?;
        Self::initialize_inboxes_table(conn)?;
        Self::initialize_inbox_messages_table(conn)?;
        Self::initialize_inbox_profile_permissions_table(conn)?;
        Self::initialize_invoice_network_errors_table(conn)?;
        Self::initialize_invoice_requests_table(conn)?;
        Self::initialize_invoice_table(conn)?;
        Self::initialize_jobs_table(conn)?;
        Self::initialize_forked_jobs_table(conn)?;
        Self::initialize_job_queue_table(conn)?;
        Self::initialize_llm_providers_table(conn)?;
        Self::initialize_local_node_keys_table(conn)?;
        Self::initialize_message_box_symmetric_keys_table(conn)?;
        Self::initialize_prompt_table(conn)?;
        Self::initialize_prompt_vector_tables(conn)?;
        Self::initialize_registration_code_table(conn)?;
        Self::initialize_retry_messages_table(conn)?;
        Self::initialize_settings_table(conn)?;
        Self::initialize_sheets_table(conn)?;
        Self::initialize_tools_table(conn)?;
        Self::initialize_tool_micropayments_requirements_table(conn)?;
        Self::initialize_tool_playground_table(conn)?;
        Self::initialize_tool_playground_code_history_table(conn)?;
        Self::initialize_version_table(conn)?;
        Self::initialize_wallets_table(conn)?;
        Self::initialize_filesystem_tables(conn)?;
        Self::initialize_oauth_table(conn)?;
        Self::initialize_regex_patterns_table(conn)?;
        // Vector tables
        Self::initialize_tools_vector_table(conn)?;
        // Initialize the embedding model type table
        Self::initialize_embedding_model_type_table(conn)?;
        Ok(())
    }

    fn initialize_fts_tables(conn: &rusqlite::Connection) -> Result<()> {
        Self::initialize_tools_fts_table(conn)?;
        Self::initialize_prompts_fts_table(conn)?;
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
                config TEXT, -- Store as a JSON string
                scope TEXT NOT NULL -- Change this line to use TEXT instead of BLOB
            );",
            [],
        )?;

        // Create an index for the agent_id column
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_shinkai_agents_agent_id ON shinkai_agents (agent_id);",
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

    fn initialize_inboxes_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS inboxes (
                inbox_name TEXT NOT NULL UNIQUE,
                smart_inbox_name TEXT NOT NULL,
                read_up_to_message_hash TEXT
            );",
            [],
        )?;

        // Create an index for the inbox_name column
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inboxes_inbox_name ON inboxes (inbox_name);",
            [],
        )?;

        Ok(())
    }

    fn initialize_inbox_messages_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS inbox_messages (
                message_hash TEXT NOT NULL UNIQUE,
                inbox_name TEXT NOT NULL,
                shinkai_message BLOB NOT NULL,
                parent_message_hash TEXT,
                time_key TEXT NOT NULL
            );",
            [],
        )?;

        // Create an index for the inbox_name column
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inbox_messages_inbox_name ON inbox_messages (inbox_name);",
            [],
        )?;

        // Create an index for the message_hash column
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inbox_messages_message_hash ON inbox_messages (message_hash);",
            [],
        )?;

        // Create an index for the time_key column
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inbox_messages_time_key ON inbox_messages (time_key);",
            [],
        )?;

        Ok(())
    }

    fn initialize_inbox_profile_permissions_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS inbox_profile_permissions (
                inbox_name TEXT NOT NULL,
                profile_name TEXT NOT NULL,
                permission TEXT NOT NULL,

                PRIMARY KEY (inbox_name, profile_name),
                FOREIGN KEY (inbox_name) REFERENCES inboxes(inbox_name),
                FOREIGN KEY (profile_name) REFERENCES standard_identities(profile_name)
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_jobs_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS jobs (
                job_id TEXT NOT NULL UNIQUE,
                is_hidden INTEGER NOT NULL,
                datetime_created TEXT NOT NULL,
                is_finished INTEGER NOT NULL,
                parent_agent_or_llm_provider_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                conversation_inbox_name TEXT NOT NULL,
                associated_ui TEXT,
                config TEXT
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_forked_jobs_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS forked_jobs (
                parent_job_id TEXT NOT NULL,
                forked_job_id TEXT NOT NULL,
                message_id TEXT NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_job_queue_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS job_queues (
                job_id TEXT NOT NULL,
                queue_data TEXT NOT NULL
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

    fn initialize_message_box_symmetric_keys_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS message_box_symmetric_keys (
                hex_blake3_hash TEXT NOT NULL UNIQUE,
                symmetric_key BLOB NOT NULL
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

    fn initialize_retry_messages_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS retry_messages (
                hash_key TEXT NOT NULL,
                time_key TEXT NOT NULL,
                message BLOB NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_settings_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shinkai_settings (
                key TEXT NOT NULL UNIQUE,
                value TEXT
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_sheets_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shinkai_sheets (
                profile_hash TEXT NOT NULL,
                sheet_uuid TEXT NOT NULL,
                sheet_data BLOB NOT NULL,

                PRIMARY KEY (profile_hash, sheet_uuid)
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
            "CREATE VIRTUAL TABLE IF NOT EXISTS prompt_vec_items USING vec0(
                embedding float[384],
                is_enabled integer,
                +prompt_id integer
            )",
            [],
        )?;

        Ok(())
    }

    fn initialize_tools_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shinkai_tools (
                name TEXT NOT NULL,
                description TEXT,
                tool_key TEXT NOT NULL,
                version INTEGER NOT NULL,
                embedding_seo TEXT NOT NULL,
                tool_data BLOB NOT NULL,
                tool_header BLOB NOT NULL,
                tool_type TEXT NOT NULL,
                author TEXT NOT NULL,
                is_enabled INTEGER NOT NULL,
                on_demand_price REAL,
                is_network INTEGER NOT NULL,
                PRIMARY KEY(tool_key, version)
            );",
            [],
        )?;

        // The index is automatically created by the PRIMARY KEY constraint

        Ok(())
    }

    // New method to initialize the tools vector table
    fn initialize_tools_vector_table(conn: &rusqlite::Connection) -> Result<()> {
        // Create a table for tool vector embeddings with metadata columns
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS shinkai_tools_vec_items USING vec0(
                embedding float[384],
                is_enabled integer,
                is_network integer,
                +tool_key text
            )",
            [],
        )?;

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

    fn initialize_tool_playground_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_playground (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT,
                author TEXT,
                keywords TEXT,       -- comma-separated
                configurations TEXT, -- JSON
                parameters TEXT,     -- JSON
                result TEXT,         -- JSON
                tool_router_key TEXT NOT NULL,
                tool_version INTEGER NOT NULL,
                job_id TEXT,
                job_id_history TEXT, -- comma-separated
                code TEXT NOT NULL,
                language TEXT NOT NULL,
                FOREIGN KEY (tool_router_key, tool_version)
                    REFERENCES shinkai_tools (tool_key, version)
                    ON DELETE CASCADE
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_tool_playground_code_history_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_playground_code_history (
                message_id TEXT PRIMARY KEY,
                tool_playground_id INTEGER NOT NULL,
                code TEXT NOT NULL,
                FOREIGN KEY (tool_playground_id)
                    REFERENCES tool_playground (id)
                    ON DELETE CASCADE
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

    fn initialize_invoice_requests_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS invoice_requests (
                unique_id TEXT NOT NULL UNIQUE,
                provider_name TEXT NOT NULL,
                requester_name TEXT NOT NULL,
                tool_key_name TEXT NOT NULL,
                usage_type_inquiry TEXT NOT NULL,
                date_time TEXT NOT NULL,
                secret_prehash TEXT NOT NULL
            );",
            [],
        )?;

        Ok(())
    }

    fn initialize_invoice_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS invoices (
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

    fn initialize_invoice_network_errors_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS invoice_network_errors (
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

    // Updated method to initialize the cron_tasks table
    fn initialize_cron_tasks_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cron_tasks (
                task_id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT,
                cron TEXT NOT NULL,
                created_at TEXT NOT NULL, -- Field to track when the task was created
                last_modified TEXT NOT NULL,
                last_executed TEXT, -- Field to track the last execution time
                action TEXT NOT NULL, -- Store serialized CronTaskAction
                paused INTEGER NOT NULL DEFAULT 0 -- New field to track if the task is paused
            );",
            [],
        )?;
        Ok(())
    }

    // New method to initialize the cron_task_executions table
    fn initialize_cron_task_executions_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cron_task_executions (
                execution_id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                execution_time TEXT NOT NULL,
                success INTEGER NOT NULL CHECK (success IN (0, 1)),
                error_message TEXT,
                job_id TEXT,
                FOREIGN KEY(task_id) REFERENCES cron_tasks(task_id)
            );",
            [],
        )?;
        Ok(())
    }

    fn initialize_file_inboxes_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_inboxes (
                file_inbox_name TEXT NOT NULL,
                file_name TEXT NOT NULL,

                PRIMARY KEY (file_inbox_name, file_name)
            );",
            [],
        )?;

        // Create an index for the file_inbox_name column
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_file_inboxes_file_inbox_name ON file_inboxes (file_inbox_name);",
            [],
        )?;

        Ok(())
    }

    fn initialize_oauth_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS oauth_tokens (
                id INTEGER PRIMARY KEY,       
                connection_name TEXT NOT NULL, -- name used to identify the connection from the app
                state TEXT NOT NULL UNIQUE,    -- verification code
                code TEXT,
                app_id TEXT NOT NULL,          -- app id
                tool_id TEXT NOT NULL,         -- tool id
                tool_key TEXT NOT NULL,        -- tool key
                access_token TEXT,
                access_token_expires_at TIMESTAMP,
                refresh_token TEXT,
                refresh_token_enabled BOOLEAN DEFAULT FALSE,
                refresh_token_expires_at TIMESTAMP,
                token_secret TEXT,             -- For OAuth 1.0 if needed
                response_type TEXT,
                id_token TEXT,                 -- For OIDC tokens
                scope TEXT,
                pkce_type TEXT,             -- Changed from enable_pkce BOOLEAN
                pkce_code_verifier TEXT,
                expires_at TIMESTAMP,
                metadata_json TEXT,
                authorization_url TEXT,
                token_url TEXT,
                client_id TEXT,
                client_secret TEXT,
                redirect_url TEXT,
                version TEXT NOT NULL DEFAULT '1.0.0',  -- Added version field with default
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );",
            [],
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_oauth_tokens_connection_name_tool_key ON oauth_tokens (connection_name, tool_key);",
            [],
        )?;

        Ok(())
    }

    fn initialize_regex_patterns_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS regex_patterns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_name TEXT NOT NULL,
                pattern TEXT NOT NULL,
                response TEXT NOT NULL,
                description TEXT,
                is_enabled BOOLEAN DEFAULT TRUE,
                priority INTEGER DEFAULT 0,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(provider_name, pattern)
            );",
            [],
        )?;

        // Create indexes for pattern lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_regex_patterns_pattern ON regex_patterns (pattern);",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_regex_patterns_provider ON regex_patterns (provider_name);",
            [],
        )?;

        Ok(())
    }

    // fn initialize_source_file_maps_table(conn: &rusqlite::Connection) -> Result<()> {
    //     conn.execute(
    //         "CREATE TABLE IF NOT EXISTS source_file_maps (
    //             profile_name TEXT NOT NULL,
    //             vector_resource_id TEXT NOT NULL,
    //             vr_path TEXT NOT NULL,
    //             source_file_type TEXT NOT NULL,
    //             file_name TEXT NOT NULL,
    //             file_type TEXT NOT NULL,
    //             distribution_info BLOB
    //         );",
    //         [],
    //     )?;

    //     // Create an index for the profile_name column
    //     conn.execute(
    //         "CREATE INDEX IF NOT EXISTS idx_source_file_maps_profile_name ON source_file_maps (profile_name);",
    //         [],
    //     )?;

    //     // Create an index for the vector_resource_id column
    //     conn.execute(
    //         "CREATE INDEX IF NOT EXISTS idx_source_file_maps_vector_resource_id ON source_file_maps (vector_resource_id);",
    //         [],
    //     )?;

    //     Ok(())
    // }

    // New method to initialize the embedding model type table
    fn initialize_embedding_model_type_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS embedding_model_type (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                model_type TEXT NOT NULL UNIQUE
            );",
            [],
        )?;
        Ok(())
    }

    // New method to update the embedding model type
    pub fn update_default_embedding_model(&self, model_type: EmbeddingModelType) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM embedding_model_type;", [])?;
        conn.execute(
            "INSERT INTO embedding_model_type (model_type) VALUES (?);",
            [&model_type.to_string() as &dyn ToSql],
        )?;
        Ok(())
    }

    // New method to get the embedding model type
    pub fn get_default_embedding_model(&self) -> Result<EmbeddingModelType, SqliteManagerError> {
        let conn = self.get_connection()?;
        Ok(
            conn.query_row("SELECT model_type FROM embedding_model_type LIMIT 1;", [], |row| {
                let model_type_str: String = row.get(0)?;
                EmbeddingModelType::from_string(&model_type_str).map_err(|_| rusqlite::Error::InvalidQuery)
            })?,
        )
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
    pub async fn generate_embeddings(&self, prompt: &str) -> Result<Vec<f32>, SqliteManagerError> {
        let model_type = self.get_default_embedding_model()?;
        let embedding_function = EmbeddingFunction::new(&self.api_url, model_type);
        Ok(embedding_function.request_embeddings(prompt).await?)
    }

    // Utility function to generate a vector of length 384 filled with a specified value
    pub fn generate_vector_for_testing(value: f32) -> Vec<f32> {
        vec![value; 384]
    }

    // pub fn get_default_embedding_model(&self) -> Result<EmbeddingModelType, SqliteManagerError> {
    //     Ok(self.model_type.clone())
    // }
    // pub fn update_default_embedding_model(&mut self, model: EmbeddingModelType) -> Result<(), SqliteManagerError> {
    //     self.model_type = model;
    //     Ok(())
    // }

    // Method to set the version and determine if a global reset is needed
    pub fn set_version(&self, version: &str) -> Result<()> {
        // Note: add breaking versions here as needed
        let breaking_versions = ["0.9.0", "0.9.1", "0.9.2", "0.9.3", "0.9.4", "0.9.5", "0.9.7", "0.9.8"];

        let needs_global_reset = self.get_version().map_or(false, |(current_version, _)| {
            breaking_versions
                .iter()
                .any(|&breaking_version| current_version.as_str() < breaking_version && version >= breaking_version)
        });

        let conn = self.get_connection()?;
        conn.execute("DELETE FROM app_version;", [])?;
        conn.execute(
            "INSERT INTO app_version (version, needs_global_reset) VALUES (?, ?);",
            [&version as &dyn ToSql, &(needs_global_reset as i32) as &dyn ToSql],
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
    use shinkai_embedding::model_type::OllamaTextEmbeddingsInference;
    use std::path::PathBuf;
    use std::sync::{Arc, RwLock};
    use std::thread;
    use std::time::{Duration, Instant};
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

    // #[tokio::test]
    async fn test_update_from_breaking_version_no_reset() {
        let manager = setup_test_db().await;
        manager.set_version("0.9.1").unwrap();
        manager.set_version("0.9.5").unwrap();
        let (version, needs_reset) = manager.get_version().unwrap();
        assert_eq!(version, "0.9.5");
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

    #[tokio::test]
    async fn test_concurrent_get_version_reads() {
        let manager = setup_test_db().await;
        manager.set_version("1.0.0").unwrap();

        // Wrap the manager in an Arc<RwLock>
        let manager = Arc::new(RwLock::new(manager));

        // Create a vector to hold the thread handles
        let mut handles = vec![];

        // Spawn multiple threads to read the version concurrently
        for _ in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                let manager_read = manager_clone.read().unwrap();
                let (version, needs_reset) = manager_read.get_version().unwrap();
                assert_eq!(version, "1.0.0");
                assert!(!needs_reset);
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[tokio::test]
    async fn test_concurrent_db_read_lock() {
        let manager = setup_test_db().await;
        manager.set_version("1.0.0").unwrap();

        // Wrap the manager in an Arc to allow shared ownership across threads
        let manager = Arc::new(manager);

        // Start the timer
        let start_time = Instant::now();

        // Simulate a read operation in a separate thread
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            println!("Thread 1: Simulating read operation...");
            let _conn = manager_clone.get_connection().unwrap();
            let (version, needs_reset) = manager_clone.get_version().unwrap();
            assert_eq!(version, "1.0.0");
            assert!(!needs_reset);
            println!("Thread 1: Read complete.");
            thread::sleep(Duration::from_secs(1)); // Simulate a delay
        });

        // Attempt to read from the database in another thread
        let manager_clone = Arc::clone(&manager);
        let read_handle = thread::spawn(move || {
            println!("Thread 2: Reading version...");
            let _conn = manager_clone.get_connection().unwrap();
            let (version, needs_reset) = manager_clone.get_version().unwrap();
            assert_eq!(version, "1.0.0");
            assert!(!needs_reset);
            println!("Thread 2: Read complete.");

            // Measure the elapsed time after the read operation
            let elapsed_time = start_time.elapsed();
            println!("Read operation completed in {:?}", elapsed_time);

            // Fail the test if it takes 1 second or more
            assert!(elapsed_time < Duration::from_secs(1), "Test took too long to complete");
        });

        // Wait for both threads to complete
        handle.join().unwrap();
        read_handle.join().unwrap();
    }

    #[tokio::test]
    async fn test_concurrent_db_write_lock() {
        let manager = setup_test_db().await;
        manager.set_version("1.0.0").unwrap();

        // Wrap the manager in an Arc to allow shared ownership across threads
        let manager = Arc::new(manager);

        // Start the timer
        let start_time = Instant::now();

        // Simulate a write operation in a separate thread
        let manager_clone = Arc::clone(&manager);
        let handle = thread::spawn(move || {
            println!("Thread 1: Simulating write operation...");
            let conn = manager_clone.get_connection().unwrap();
            conn.execute("BEGIN IMMEDIATE TRANSACTION;", []).unwrap();
            thread::sleep(Duration::from_secs(1)); // Simulate a write operation
            conn.execute("COMMIT;", []).unwrap();
            println!("Thread 1: Finished write operation.");
        });

        // Attempt to read from the database in another thread
        let manager_clone = Arc::clone(&manager);
        let read_handle = thread::spawn(move || {
            println!("Thread 2: Reading version...");
            let _conn = manager_clone.get_connection().unwrap();
            let (version, needs_reset) = manager_clone.get_version().unwrap();
            assert_eq!(version, "1.0.0");
            assert!(!needs_reset);
            println!("Thread 2: Read complete.");

            // Measure the elapsed time after the read operation
            let elapsed_time = start_time.elapsed();
            println!("Read operation completed in {:?}", elapsed_time);

            // Fail the test if it takes 1 second or more
            assert!(elapsed_time < Duration::from_secs(1), "Test took too long to complete");
        });

        // Wait for both threads to complete
        handle.join().unwrap();
        read_handle.join().unwrap();
    }
}
