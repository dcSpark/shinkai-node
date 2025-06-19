use embedding_function::EmbeddingFunction;
use errors::SqliteManagerError;
use log::info;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{ffi::sqlite3_auto_extension, Result, Row, ToSql};
use shinkai_embedding::model_type::EmbeddingModelType;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
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
pub mod mcp_server_manager;
pub mod oauth_manager;
pub mod preferences;
pub mod prompt_manager;
pub mod regex_pattern_manager;
pub mod retry_manager;
pub mod settings_manager;
pub mod shinkai_tool_manager;
pub mod source_file_manager;
pub mod tool_payment_req_manager;
pub mod tool_playground;
pub mod tracing;
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
                 PRAGMA busy_timeout = 10000;
                 PRAGMA mmap_size=262144000; -- 250 MB in bytes (250 * 1024 * 1024)
                 PRAGMA foreign_keys = ON;", // Enable foreign key support
        )?;

        // Initialize tables in the persistent database
        Self::initialize_tables(&conn)?;
        Self::migrate_tables(&conn)?;

        // Create a connection pool for the in-memory database
        let fts_manager = SqliteConnectionManager::memory();
        let fts_pool = Pool::builder()
            .max_size(10) // Increased from 5 to match main pool
            .connection_timeout(Duration::from_secs(60))
            .build(fts_manager)
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;

        // Initialize FTS table in the in-memory database
        {
            let fts_conn = fts_pool
                .get()
                .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;
            fts_conn.execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA busy_timeout = 10000;",
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
        Self::migrate_agents_full_identity_name(&manager)?;
        Ok(manager)
    }

    // There might be old agents with partial full_identity_name.
    // This function migrates them to the new format.
    fn migrate_agents_full_identity_name(manager: &SqliteManager) -> Result<(), SqliteManagerError> {
        let agents = manager.get_all_agents()?;
        for mut agent in agents {
            if !agent.full_identity_name.has_profile() {
                println!("Migrating agent: {:?}", agent);
                agent.full_identity_name = ShinkaiName::new(format!(
                    "{}/main/agent/{}",
                    agent.full_identity_name.node_name.clone(),
                    agent.agent_id
                ))
                .map_err(|_e| SqliteManagerError::InvalidData)?;
                manager.update_agent(agent)?;
            }
        }
        Ok(())
    }

    // Initializes the required tables in the SQLite database
    fn initialize_tables(conn: &rusqlite::Connection) -> Result<()> {
        Self::initialize_agents_table(conn)?;
        Self::initialize_cron_tasks_table(conn)?;
        Self::initialize_cron_task_executions_table(conn)?;
        Self::initialize_device_identities_table(conn)?;
        Self::initialize_standard_identities_table(conn)?;
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
        Self::initialize_preferences_table(conn)?;
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
        Self::initialize_tracing_table(conn)?;

        // Vector tables
        Self::initialize_tools_vector_table(conn)?;
        // Initialize the embedding model type table
        Self::initialize_embedding_model_type_table(conn)?;
        // Initialize MCP servers table
        Self::initialize_mcp_servers_table(conn)?;
        Ok(())
    }

    fn migrate_tables(conn: &rusqlite::Connection) -> Result<()> {
        Self::migrate_agents_table(conn)?;
        Self::migrate_llm_providers_table(conn)?;
        Self::migrate_invoices_table(conn)?;
        Self::migrate_tools_table(conn)?;
        Self::migrate_invoice_requests_table(conn)?;
        Self::migrate_mcp_servers_table(conn)?;
        Ok(())
    }

    fn migrate_agents_table(conn: &rusqlite::Connection) -> Result<()> {
        // Check if tool_config_override column exists
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('shinkai_agents') WHERE name = 'tools_config_override'")?;
        let column_exists: i64 = stmt.query_row([], |row| row.get(0))?;

        // Add the column if it doesn't exist
        if column_exists == 0 {
            conn.execute("ALTER TABLE shinkai_agents ADD COLUMN tools_config_override TEXT", [])?;
        }
        // Check if edited column exists
        let mut stmt =
            conn.prepare("SELECT COUNT(*) FROM pragma_table_info('shinkai_agents') WHERE name = 'edited'")?;
        let column_exists: i64 = stmt.query_row([], |row| row.get(0))?;

        // Add the column if it doesn't exist
        if column_exists == 0 {
            conn.execute(
                "ALTER TABLE shinkai_agents ADD COLUMN edited INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        Ok(())
    }

    fn migrate_llm_providers_table(conn: &rusqlite::Connection) -> Result<()> {
        // Check if 'name' column exists
        let mut stmt = conn.prepare("PRAGMA table_info(llm_providers)")?;
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get(1))?
            .collect::<Result<Vec<String>, _>>()?;
        if !columns.contains(&"name".to_string()) {
            conn.execute("ALTER TABLE llm_providers ADD COLUMN name TEXT", [])?;
        }
        if !columns.contains(&"description".to_string()) {
            conn.execute("ALTER TABLE llm_providers ADD COLUMN description TEXT", [])?;
        }
        Ok(())
    }

    fn migrate_invoices_table(conn: &rusqlite::Connection) -> Result<()> {
        // Check if we need to make shinkai_offering_key nullable
        // We do this by checking if the table has the NOT NULL constraint
        let mut stmt = conn.prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='invoices'")?;
        let table_sql: String = match stmt.query_row([], |row| row.get(0)) {
            Ok(sql) => sql,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // Table doesn't exist yet, no migration needed
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        // If the table definition still has "shinkai_offering_key TEXT NOT NULL", we need to migrate
        if table_sql.contains("shinkai_offering_key TEXT NOT NULL") {
            // SQLite doesn't support ALTER COLUMN directly, so we need to:
            // 1. Create a new table with the correct schema
            // 2. Copy data from old table
            // 3. Drop old table
            // 4. Rename new table

            conn.execute(
                "CREATE TABLE invoices_new (
                    invoice_id TEXT NOT NULL UNIQUE,
                    provider_name TEXT NOT NULL,
                    requester_name TEXT NOT NULL,
                    usage_type_inquiry TEXT NOT NULL,
                    shinkai_offering_key TEXT, -- Made nullable
                    request_date_time TEXT NOT NULL,
                    invoice_date_time TEXT NOT NULL,
                    expiration_time TEXT NOT NULL,
                    status TEXT NOT NULL,
                    payment TEXT,
                    address TEXT NOT NULL,
                    tool_data BLOB,
                    response_date_time TEXT,
                    result_str TEXT,
                    parent_message_id TEXT,
                    FOREIGN KEY(shinkai_offering_key) REFERENCES tool_micropayments_requirements(tool_key)
                );",
                [],
            )?;

            // Copy data from old table to new table
            conn.execute(
                "INSERT INTO invoices_new (
                    invoice_id,
                    provider_name,
                    requester_name,
                    usage_type_inquiry,
                    shinkai_offering_key,
                    request_date_time,
                    invoice_date_time,
                    expiration_time,
                    status,
                    payment,
                    address,
                    tool_data,
                    response_date_time,
                    result_str,
                    parent_message_id
                ) SELECT
                    invoice_id,
                    provider_name,
                    requester_name,
                    usage_type_inquiry,
                    shinkai_offering_key,
                    request_date_time,
                    invoice_date_time,
                    expiration_time,
                    status,
                    payment,
                    address,
                    tool_data,
                    response_date_time,
                    result_str,
                    NULL
                FROM invoices",
                [],
            )?;

            // Drop the old table
            conn.execute("DROP TABLE invoices", [])?;

            // Rename the new table
            conn.execute("ALTER TABLE invoices_new RENAME TO invoices", [])?;
        }

        // Add parent_message_id column if it doesn't exist.
        // The column is appended so existing databases remain compatible.
        let mut stmt =
            conn.prepare("SELECT COUNT(*) FROM pragma_table_info('invoices') WHERE name = 'parent_message_id'")?;
        let column_exists: i64 = stmt.query_row([], |row| row.get(0))?;
        if column_exists == 0 {
            conn.execute("ALTER TABLE invoices ADD COLUMN parent_message_id TEXT", [])?;
        }

        Ok(())
    }

    fn migrate_invoice_requests_table(conn: &rusqlite::Connection) -> Result<()> {
        // Check if parent_message_id column exists by trying to select it
        // If it fails, the column doesn't exist and we need to add it
        let column_exists = conn
            .prepare("SELECT parent_message_id FROM invoice_requests LIMIT 1")
            .is_ok();

        if !column_exists {
            conn.execute("ALTER TABLE invoice_requests ADD COLUMN parent_message_id TEXT", [])?;
        }

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
                scope TEXT NOT NULL, -- Change this line to use TEXT instead of BLOB
                tools_config_override TEXT, -- Store as a JSON string
                edited INTEGER NOT NULL DEFAULT 0
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
                read_up_to_message_hash TEXT,
                last_modified TEXT,
                is_hidden BOOLEAN DEFAULT FALSE
            );",
            [],
        )?;

        // Create an index for the inbox_name column
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inboxes_inbox_name ON inboxes (inbox_name);",
            [],
        )?;

        // Create a composite index for filtering hidden inboxes and sorting by last_modified
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inboxes_hidden_modified ON inboxes (is_hidden, last_modified DESC);",
            [],
        )?;

        // Create an index for sorting by last_modified only (for when show_hidden is true)
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_inboxes_last_modified ON inboxes (last_modified DESC);",
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
                model TEXT,
                name TEXT,
                description TEXT
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
                mcp_enabled INTEGER,
                PRIMARY KEY(tool_key, version)
            );",
            [],
        )?;

        Ok(())
    }

    fn migrate_tools_table(conn: &rusqlite::Connection) -> Result<()> {
        // Check if the mcp_enabled column already exists
        let columns = conn
            .prepare("PRAGMA table_info(shinkai_tools)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<String>, _>>()?;

        // Only add the column if it doesn't exist
        if !columns.contains(&"mcp_enabled".to_string()) {
            conn.execute("ALTER TABLE shinkai_tools ADD COLUMN mcp_enabled INTEGER", [])?;
        }

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
                parent_message_id TEXT
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
                shinkai_offering_key TEXT, -- Made nullable to allow tool deletion
                request_date_time TEXT NOT NULL,
                invoice_date_time TEXT NOT NULL,
                expiration_time TEXT NOT NULL,
                status TEXT NOT NULL,
                payment TEXT, -- Store as a JSON string
                address TEXT NOT NULL, -- Store as a JSON string
                tool_data BLOB,
                response_date_time TEXT,
                result_str TEXT,
                parent_message_id TEXT,

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
                request_token_auth_header TEXT,
                request_token_content_type TEXT,
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

    // Initialize MCP servers table
    fn initialize_mcp_servers_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mcp_servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                name TEXT NOT NULL,
                type TEXT NOT NULL CHECK(type IN ('SSE', 'COMMAND', 'HTTP')) DEFAULT 'SSE',
                url TEXT,
                env TEXT,
                command TEXT,
                is_enabled BOOLEAN DEFAULT TRUE
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
    pub fn migrate_mcp_servers_table(conn: &rusqlite::Connection) -> Result<()> {
        // Check if 'HTTP' type exists in the CHECK constraint
        info!("Checking if HTTP type exists in mcp_servers table...");
        let mut stmt = conn.prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name='mcp_servers'")?;
        let table_sql: String = stmt.query_row([], |row| row.get(0))?;
        // Only migrate if HTTP is not in the type constraint
        if table_sql.contains("'HTTP'") {
            info!("HTTP type found in constraint - skipping migration");
            return Ok(());
        }
        info!("HTTP type not found in constraint - proceeding with migration");

        // SQLite doesn't support MODIFY COLUMN, so we need to:
        // 1. Create a new table with the desired schema
        // 2. Copy data from old table
        // 3. Drop old table
        // 4. Rename new table to old name
        conn.execute(
            "CREATE TABLE mcp_servers_new (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                name TEXT NOT NULL,
                type TEXT NOT NULL CHECK(type IN ('SSE', 'COMMAND', 'HTTP')) DEFAULT 'SSE',
                url TEXT,
                env TEXT,
                command TEXT,
                is_enabled BOOLEAN DEFAULT TRUE
            );",
            [],
        )?;

        conn.execute("INSERT INTO mcp_servers_new SELECT * FROM mcp_servers;", [])?;

        conn.execute("DROP TABLE mcp_servers;", [])?;

        conn.execute("ALTER TABLE mcp_servers_new RENAME TO mcp_servers;", [])?;

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

    pub fn get_needs_global_reset(
        current_version: &semver::Version,
        new_version: &semver::Version,
        breaking_versions: &[semver::Version],
    ) -> bool {
        let needs_global_reset = breaking_versions
            .iter()
            .any(|breaking_version| current_version < breaking_version && new_version >= breaking_version);
        needs_global_reset
    }

    // Method to set the version and determine if a global reset is needed
    pub fn set_version(&self, version: &str) -> Result<(), rusqlite::Error> {
        // Note: add breaking versions here as needed
        let new_version = match semver::Version::parse(version) {
            Ok(v) => v,
            Err(e) => {
                return Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(1),
                    Some(format!("failed to parse new version: {}", e)),
                ))
            }
        };

        let breaking_versions = vec!["0.9.0", "0.9.1", "0.9.2", "0.9.3", "0.9.4", "0.9.5", "0.9.7", "0.9.8"]
            .iter()
            .map(|v| semver::Version::parse(v))
            .collect::<Result<Vec<semver::Version>, _>>()
            .map_err(|e| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(1),
                    Some(format!("failed to parse breaking versions: {}", e)),
                )
            })?;

        let mut needs_global_reset = false;

        if let Ok((current_version_str, _)) = self.get_version() {
            let current_version = semver::Version::parse(&current_version_str).map_err(|e| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(1),
                    Some(format!("failed to parse current version: {}", e)),
                )
            })?;
            needs_global_reset = Self::get_needs_global_reset(&current_version, &new_version, &breaking_versions);
        }

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
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM);

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

    #[tokio::test]
    async fn test_needs_global_reset() {
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("0.9.19").unwrap(),
            &semver::Version::parse("0.9.20").unwrap(),
            &vec![
                semver::Version::parse("0.9.0").unwrap(),
                semver::Version::parse("0.9.1").unwrap(),
                semver::Version::parse("0.9.2").unwrap(),
                semver::Version::parse("0.9.3").unwrap(),
                semver::Version::parse("0.9.4").unwrap(),
                semver::Version::parse("0.9.5").unwrap(),
                semver::Version::parse("0.9.7").unwrap(),
                semver::Version::parse("0.9.8").unwrap(),
            ],
        );
        assert!(!result);
    }

    #[tokio::test]
    async fn test_needs_global_reset_edge_cases() {
        // Test case 1: Current version is exactly at a breaking version
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("0.9.5").unwrap(),
            &semver::Version::parse("0.9.6").unwrap(),
            &vec![semver::Version::parse("0.9.5").unwrap()],
        );
        assert!(!result);

        // Test case 2: New version is exactly at a breaking version
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("0.9.4").unwrap(),
            &semver::Version::parse("0.9.5").unwrap(),
            &vec![semver::Version::parse("0.9.5").unwrap()],
        );
        assert!(result);

        // Test case 3: Current version is greater than all breaking versions
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("1.0.0").unwrap(),
            &semver::Version::parse("1.0.1").unwrap(),
            &vec![semver::Version::parse("0.9.5").unwrap()],
        );
        assert!(!result);

        // Test case 4: Current version is less than all breaking versions, new version is greater than all
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("0.8.0").unwrap(),
            &semver::Version::parse("1.0.0").unwrap(),
            &vec![semver::Version::parse("0.9.5").unwrap()],
        );
        assert!(result);

        // Test case 5: Multiple breaking versions, current version between them
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("0.9.3").unwrap(),
            &semver::Version::parse("0.9.7").unwrap(),
            &vec![
                semver::Version::parse("0.9.0").unwrap(),
                semver::Version::parse("0.9.5").unwrap(),
                semver::Version::parse("0.9.8").unwrap(),
            ],
        );
        assert!(result);

        // Test case 6: Multiple breaking versions, current version after all of them
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("0.9.9").unwrap(),
            &semver::Version::parse("1.0.0").unwrap(),
            &vec![
                semver::Version::parse("0.9.0").unwrap(),
                semver::Version::parse("0.9.5").unwrap(),
                semver::Version::parse("0.9.8").unwrap(),
            ],
        );
        assert!(!result);

        // Test case 7: Empty breaking versions list
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("0.9.0").unwrap(),
            &semver::Version::parse("1.0.0").unwrap(),
            &vec![],
        );
        assert!(!result);

        // Test case 8: Major version change (1.0.0 to 2.0.0)
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("1.0.0").unwrap(),
            &semver::Version::parse("2.0.0").unwrap(),
            &vec![semver::Version::parse("1.5.0").unwrap()],
        );
        assert!(result);

        // Test case 9: Minor version change (1.0.0 to 1.1.0)
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("1.0.0").unwrap(),
            &semver::Version::parse("1.1.0").unwrap(),
            &vec![semver::Version::parse("1.0.5").unwrap()],
        );
        assert!(result);

        // Test case 10: Patch version change (1.0.0 to 1.0.1)
        let result = SqliteManager::get_needs_global_reset(
            &semver::Version::parse("1.0.0").unwrap(),
            &semver::Version::parse("1.0.1").unwrap(),
            &vec![semver::Version::parse("1.0.0").unwrap()],
        );
        assert!(!result);
    }
}
