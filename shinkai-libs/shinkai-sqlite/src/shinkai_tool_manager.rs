use crate::{SqliteManager, SqliteManagerError};
use bytemuck::cast_slice;
use keyphrases::KeyPhraseExtractor;
use rusqlite::{params, Result};
use shinkai_message_primitives::schemas::indexable_version::IndexableVersion;
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader};
use shinkai_tools_primitives::tools::tool_config::{BasicConfig, ToolConfig};
use std::collections::HashSet;

impl SqliteManager {
    // Adds a ShinkaiTool entry to the shinkai_tools table
    pub async fn add_tool(&self, tool: ShinkaiTool) -> Result<ShinkaiTool, SqliteManagerError> {
        // Generate or retrieve the embedding
        let embedding = match tool.get_embedding() {
            Some(embedding) => embedding,
            None => self.generate_embeddings(&tool.format_embedding_string()).await?,
        };

        self.add_tool_with_vector(tool, embedding)
    }

    pub fn add_tool_with_vector(
        &self,
        tool: ShinkaiTool,
        embedding: Vec<f32>,
    ) -> Result<ShinkaiTool, SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Check if the tool already exists with the same key and version
        let tool_key = tool.tool_router_key().to_string_without_version().to_lowercase();
        let version = tool.version_number()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shinkai_tools WHERE tool_key = ?1 AND version = ?2)",
            params![tool_key, version],
            |row| row.get(0),
        )?;

        if exists {
            println!("Tool already exists with key: {} and version: {}", tool_key, version);
            return Err(SqliteManagerError::ToolAlreadyExists(tool_key));
        }

        let tool_seos = tool.format_embedding_string();
        let tool_type = tool.tool_type().to_string();
        let tool_header = serde_json::to_vec(&tool.to_header()).unwrap();

        // Clone the tool to make it mutable
        let mut tool_clone = tool.clone();
        tool_clone.set_embedding(embedding.clone());

        // Determine if the tool can be enabled
        let is_enabled = tool_clone.is_enabled() && tool_clone.can_be_enabled();
        let mcp_enabled = tool_clone.is_mcp_enabled();
        if tool_clone.is_enabled() && !tool_clone.can_be_enabled() {
            tool_clone.disable();
        }

        let tool_data = serde_json::to_vec(&tool_clone).map_err(|e| {
            eprintln!("Serialization error: {}", e);
            SqliteManagerError::SerializationError(e.to_string())
        })?;

        // Extract on_demand_price and is_network
        let (on_demand_price, is_network) = match tool_clone {
            ShinkaiTool::Network(ref network_tool, _) => {
                let price = network_tool.usage_type.per_use_usd_price();
                (Some(price), true)
            }
            _ => (None, false),
        };

        let version_number = tool_clone.version_number()?;

        // Insert the tool into the database
        tx.execute(
            "INSERT INTO shinkai_tools (
                name,
                description,
                tool_key,
                embedding_seo,
                tool_data,
                tool_header,
                tool_type,
                author,
                version,
                is_enabled,
                on_demand_price,
                is_network,
                mcp_enabled
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                tool_clone.name(),
                tool_clone.description(),
                tool_clone.tool_router_key().to_string_without_version(),
                tool_seos,
                tool_data,
                tool_header,
                tool_type,
                tool_clone.author(),
                version_number,
                is_enabled as i32,
                on_demand_price,
                is_network as i32,
                mcp_enabled as i32,
            ],
        )?;

        // Extract is_enabled and is_network
        let is_enabled = tool_clone.is_enabled() && tool_clone.can_be_enabled();
        let (_, is_network) = match tool_clone {
            ShinkaiTool::Network(_, _) => (Some(0.0), true),
            _ => (None, false),
        };

        // Insert the embedding into the shinkai_tools_vec_items table with metadata
        tx.execute(
            "INSERT INTO shinkai_tools_vec_items (
                embedding, 
                is_enabled, 
                is_network, 
                tool_key
            ) VALUES (?1, ?2, ?3, ?4)",
            params![cast_slice(&embedding), is_enabled as i32, is_network as i32, tool_key],
        )?;

        // Update the FTS table using the in-memory connection
        self.update_tools_fts(&tool)?;

        tx.commit()?;

        Ok(tool_clone)
    }

    pub async fn upgrade_tool(&self, new_tool: ShinkaiTool) -> Result<ShinkaiTool, SqliteManagerError> {
        // Generate or retrieve the embedding
        let embedding = self.generate_embeddings(&new_tool.format_embedding_string()).await?;
        self.upgrade_tool_with_vector(new_tool, embedding)
    }

    pub fn upgrade_tool_with_vector(
        &self,
        new_tool: ShinkaiTool,
        embedding: Vec<f32>,
    ) -> Result<ShinkaiTool, SqliteManagerError> {
        // Use the tool_router_key (without version) to locate the old version
        let tool_key = new_tool.tool_router_key().to_string_without_version();
        let old_tool = self.get_tool_by_key(&tool_key)?;

        // Get configurations based on tool type
        let (_old_config, upgraded): (Vec<ToolConfig>, ShinkaiTool) = match (old_tool, new_tool) {
            (ShinkaiTool::Deno(old_deno, _), ShinkaiTool::Deno(mut new_deno, is_enabled)) => {
                let old_config = old_deno.config.clone();

                // Merge configuration
                let merged_config: Vec<ToolConfig> = new_deno
                    .config
                    .into_iter()
                    .map(|new_entry| match new_entry {
                        ToolConfig::BasicConfig(new_basic) => {
                            let preserved_value = old_config.iter().find_map(|old_entry| match old_entry {
                                ToolConfig::BasicConfig(old_basic) => {
                                    if old_basic.key_name == new_basic.key_name {
                                        return old_basic.key_value.clone();
                                    }
                                    None
                                }
                                _ => None,
                            });
                            ToolConfig::BasicConfig(BasicConfig {
                                key_name: new_basic.key_name,
                                description: new_basic.description,
                                required: new_basic.required,
                                type_name: new_basic.type_name,
                                key_value: preserved_value,
                            })
                        }
                    })
                    .collect();

                new_deno.config = merged_config;
                (old_config, ShinkaiTool::Deno(new_deno, is_enabled))
            }
            (ShinkaiTool::Network(old_network, _), ShinkaiTool::Network(mut new_network, is_enabled)) => {
                let old_config = old_network.config.clone();

                // Merge configuration
                let merged_config: Vec<ToolConfig> = new_network
                    .config
                    .into_iter()
                    .map(|new_entry| match new_entry {
                        ToolConfig::BasicConfig(new_basic) => {
                            let preserved_value = old_config.iter().find_map(|old_entry| match old_entry {
                                ToolConfig::BasicConfig(old_basic) => {
                                    if old_basic.key_name == new_basic.key_name {
                                        return old_basic.key_value.clone();
                                    }
                                    None
                                }
                                _ => None,
                            });
                            ToolConfig::BasicConfig(BasicConfig {
                                key_name: new_basic.key_name,
                                description: new_basic.description,
                                required: new_basic.required,
                                type_name: new_basic.type_name,
                                key_value: preserved_value,
                            })
                        }
                    })
                    .collect();

                new_network.config = merged_config;
                (old_config, ShinkaiTool::Network(new_network, is_enabled))
            }
            (ShinkaiTool::Python(old_python, _), ShinkaiTool::Python(mut new_python, is_enabled)) => {
                let old_config = old_python.config.clone();

                // Merge configuration
                let merged_config: Vec<ToolConfig> = new_python
                    .config
                    .into_iter()
                    .map(|new_entry| match new_entry {
                        ToolConfig::BasicConfig(new_basic) => {
                            let preserved_value = old_config.iter().find_map(|old_entry| match old_entry {
                                ToolConfig::BasicConfig(old_basic) => {
                                    if old_basic.key_name == new_basic.key_name {
                                        return old_basic.key_value.clone();
                                    }
                                    None
                                }
                                _ => None,
                            });
                            ToolConfig::BasicConfig(BasicConfig {
                                key_name: new_basic.key_name,
                                description: new_basic.description,
                                required: new_basic.required,
                                type_name: new_basic.type_name,
                                key_value: preserved_value,
                            })
                        }
                    })
                    .collect();

                new_python.config = merged_config;
                (old_config, ShinkaiTool::Python(new_python, is_enabled))
            }
            _ => return Err(SqliteManagerError::ToolTypeMismatch),
        };

        // Add the upgraded tool to the database
        self.add_tool_with_vector(upgraded.clone(), embedding)
    }

    // Performs a vector search for tools using a precomputed vector
    pub fn tool_vector_search_with_vector(
        &self,
        vector: Vec<f32>,
        num_results: u64,
        include_disabled: bool,
        include_network: bool,
    ) -> Result<Vec<(ShinkaiToolHeader, f64)>, SqliteManagerError> {
        // TODO: implement an LRU cache for the vector search
        // so we are not searching the database every time
        // be careful with the memory! and if the tools change we need to invalidate the cache

        // Serialize the vector to a JSON array string
        let vector_json = serde_json::to_string(&vector).map_err(|e| {
            eprintln!("Vector serialization error: {}", e);
            SqliteManagerError::SerializationError(e.to_string())
        })?;

        // Perform the vector search to get tool_keys and distances
        let conn = self.get_connection()?;
        let query = match (include_disabled, include_network) {
            (true, true) => {
                "SELECT v.tool_key, v.distance 
                 FROM shinkai_tools_vec_items v
                 WHERE v.embedding MATCH json(?1)
                 ORDER BY distance 
                 LIMIT ?2"
            }
            (true, false) => {
                "SELECT v.tool_key, v.distance 
                 FROM shinkai_tools_vec_items v
                 WHERE v.embedding MATCH json(?1)
                 AND v.is_network = 0
                 ORDER BY distance 
                 LIMIT ?2"
            }
            (false, true) => {
                "SELECT v.tool_key, v.distance 
                 FROM shinkai_tools_vec_items v
                 WHERE v.embedding MATCH json(?1)
                 AND v.is_enabled = 1
                 ORDER BY distance 
                 LIMIT ?2"
            }
            (false, false) => {
                "SELECT v.tool_key, v.distance 
                 FROM shinkai_tools_vec_items v
                 WHERE v.embedding MATCH json(?1)
                 AND v.is_enabled = 1
                 AND v.is_network = 0
                 ORDER BY distance 
                 LIMIT ?2"
            }
        };

        let mut stmt = conn.prepare(query)?;

        // Retrieve tool_keys and distances
        let tool_keys_and_distances: Vec<(String, f64)> = stmt
            .query_map(params![vector_json, num_results], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        // Retrieve the corresponding ShinkaiToolHeaders and pair with distances
        let mut tools_with_distances = Vec::new();
        for (tool_key, distance) in tool_keys_and_distances {
            if let Ok(tool_header) = self.get_tool_header_by_key(&tool_key) {
                tools_with_distances.push((tool_header, distance));
            }
        }

        Ok(tools_with_distances)
    }

    // Performs a vector search for tools based on a query string
    pub async fn tool_vector_search(
        &self,
        query: &str,
        num_results: u64,
        include_disabled: bool,
        include_network: bool,
    ) -> Result<Vec<(ShinkaiToolHeader, f64)>, SqliteManagerError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Generate the embedding from the query string
        let embedding = self.generate_embeddings(query).await.map_err(|e| {
            eprintln!("Embedding generation error: {}", e);
            SqliteManagerError::EmbeddingGenerationError(e.to_string())
        })?;

        // Use the new function to perform the search
        self.tool_vector_search_with_vector(embedding, num_results, include_disabled, include_network)
    }

    /// Retrieves a ShinkaiToolHeader based on its tool_key
    pub fn get_tool_header_by_key(&self, tool_key: &str) -> Result<ShinkaiToolHeader, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT tool_header FROM shinkai_tools WHERE tool_key = ?1 ORDER BY version DESC LIMIT 1")?;

        let tool_header_data: Vec<u8> = stmt
            .query_row(params![tool_key.to_lowercase()], |row| row.get(0))
            .map_err(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    eprintln!("Tool not found with key: {}", tool_key);
                    SqliteManagerError::ToolNotFound(tool_key.to_string())
                } else {
                    eprintln!("Database error: {}", e);
                    SqliteManagerError::DatabaseError(e)
                }
            })?;

        let tool_header: ShinkaiToolHeader = serde_json::from_slice(&tool_header_data).map_err(|e| {
            eprintln!("Deserialization error: {}", e);
            SqliteManagerError::SerializationError(e.to_string())
        })?;

        Ok(tool_header)
    }

    /// Retrieves a ShinkaiTool based on its tool_key, sorted by descending version
    pub fn get_tool_by_key(&self, tool_key: &str) -> Result<ShinkaiTool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT tool_data FROM shinkai_tools WHERE tool_key = ?1 ORDER BY version DESC LIMIT 1")?;

        let tool_data: Vec<u8> = stmt
            .query_row(params![tool_key.to_lowercase()], |row| row.get(0))
            .map_err(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    eprintln!("Tool not found with key: {}", tool_key);
                    SqliteManagerError::ToolNotFound(tool_key.to_string())
                } else {
                    eprintln!("Database error: {}", e);
                    SqliteManagerError::DatabaseError(e)
                }
            })?;

        // Deserialize the tool_data to get the ShinkaiTool
        let tool: ShinkaiTool = serde_json::from_slice(&tool_data).map_err(|e| {
            eprintln!("Deserialization error: {}", e);
            SqliteManagerError::SerializationError(e.to_string())
        })?;

        Ok(tool)
    }

    // Updates a ShinkaiTool entry in the shinkai_tools table with a new embedding
    pub fn update_tool_with_vector(
        &self,
        tool: ShinkaiTool,
        embedding: Vec<f32>,
    ) -> Result<ShinkaiTool, SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Get the tool_key and find the rowid
        let tool_key = tool.tool_router_key().to_string_without_version().to_lowercase();

        // Convert version string to IndexableVersion
        let indexable_version = IndexableVersion::from_string(&tool.version())
            .map_err(|e| SqliteManagerError::VersionConversionError(e))?;
        let version_number = indexable_version.get_version_number();

        let rowid: i64 = tx
            .query_row(
                "SELECT rowid FROM shinkai_tools WHERE tool_key = ?1 AND version = ?2",
                params![tool_key, version_number],
                |row| row.get(0),
            )
            .map_err(|e| {
                eprintln!("Tool not found with key: {}", tool_key);
                SqliteManagerError::DatabaseError(e)
            })?;

        // Serialize the updated tool data
        let tool_data = serde_json::to_vec(&tool).map_err(|e| {
            eprintln!("Serialization error: {}", e);
            SqliteManagerError::SerializationError(e.to_string())
        })?;

        // Generate the tool header
        let tool_header = serde_json::to_vec(&tool.to_header()).unwrap();

        // Determine if the tool can be enabled
        let is_enabled = tool.is_enabled() && tool.can_be_enabled();
        if tool.is_enabled() && !tool.can_be_enabled() {
            eprintln!("Tool cannot be enabled, disabling");
        }

        // Extract on_demand_price and is_network
        let (on_demand_price, is_network) = match tool {
            ShinkaiTool::Network(ref network_tool, _) => {
                let price = network_tool.usage_type.per_use_usd_price();
                (Some(price), true)
            }
            _ => (None, false),
        };

        // Update the tool in the database
        tx.execute(
            "UPDATE shinkai_tools SET 
                name = ?1,
                description = ?2,
                tool_key = ?3,
                embedding_seo = ?4,
                tool_data = ?5,
                tool_header = ?6,
                tool_type = ?7,
                author = ?8,
                version = ?9,
                is_enabled = ?10,
                on_demand_price = ?11,
                is_network = ?12,
                mcp_enabled = ?13
             WHERE rowid = ?14",
            params![
                tool.name(),
                tool.description(),
                tool.tool_router_key().to_string_without_version(),
                tool.format_embedding_string(),
                tool_data,
                tool_header,
                tool.tool_type().to_string(),
                tool.author(),
                version_number,
                is_enabled as i32,
                on_demand_price,
                is_network as i32,
                tool.is_mcp_enabled() as i32,
                rowid,
            ],
        )?;

        // Update the vector using the same transaction
        self.update_tools_vector(&tx, &tool_key, embedding)?;

        // Update the FTS table using the in-memory connection
        self.update_tools_fts(&tool)?;

        tx.commit()?;

        Ok(tool)
    }

    /// Updates a ShinkaiTool entry by generating a new embedding
    pub async fn update_tool(&self, tool: ShinkaiTool) -> Result<ShinkaiTool, SqliteManagerError> {
        // Generate or retrieve the embedding
        let embedding = match tool.get_embedding() {
            Some(embedding) => embedding,
            None => self.generate_embeddings(&tool.format_embedding_string()).await?,
        };

        self.update_tool_with_vector(tool, embedding)
    }

    /// Retrieves all ShinkaiToolHeader entries from the shinkai_tools table
    pub fn get_all_tool_headers(&self) -> Result<Vec<ShinkaiToolHeader>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT tool_header FROM shinkai_tools")?;

        let header_iter = stmt.query_map([], |row| {
            let tool_header_data: Vec<u8> = row.get(0)?;
            serde_json::from_slice(&tool_header_data).map_err(|e| {
                eprintln!("Deserialization error: {}", e);
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })
        })?;

        let mut headers = Vec::new();
        for header in header_iter {
            headers.push(header.map_err(|e| {
                eprintln!("Database error: {}", e);
                SqliteManagerError::DatabaseError(e)
            })?);
        }

        Ok(headers)
    }

    /// Removes one or all versions of a ShinkaiTool entry from the shinkai_tools table.
    /// If `version` is Some("x.y.z"), only that version is removed.
    /// If `version` is None, all versions of `tool_key` are removed.
    pub fn remove_tool(&self, tool_key: &str, version: Option<String>) -> Result<(), SqliteManagerError> {
        let tool_key_lower = tool_key.to_lowercase();
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Gather all matching rowids.
        // If a version was provided, only get that rowid.
        // Otherwise, get all rowids for that tool_key.
        let rowids: Vec<i64> = if let Some(ver_str) = version {
            // Convert to an IndexableVersion
            let idx_ver =
                IndexableVersion::from_string(&ver_str).map_err(SqliteManagerError::VersionConversionError)?;
            let ver_num = idx_ver.get_version_number();

            // Query for a single rowid
            let rowid: i64 = tx
                .query_row(
                    "SELECT rowid FROM shinkai_tools WHERE tool_key = ?1 AND version = ?2",
                    params![tool_key_lower, ver_num],
                    |row| row.get(0),
                )
                .map_err(|e| {
                    eprintln!("Tool not found with key={} version={}", tool_key_lower, ver_num);
                    SqliteManagerError::DatabaseError(e)
                })?;
            vec![rowid]
        } else {
            // No version: remove all rows for this tool_key
            let mut stmt = tx.prepare("SELECT rowid FROM shinkai_tools WHERE tool_key = ?1")?;
            let rows = stmt.query_map(params![tool_key_lower], |row| row.get::<_, i64>(0))?;
            let mut all_rowids = Vec::new();
            for r in rows {
                all_rowids.push(r.map_err(|e| {
                    eprintln!("Tool not found with key={}", tool_key_lower);
                    SqliteManagerError::DatabaseError(e)
                })?);
            }
            if all_rowids.is_empty() {
                eprintln!("No tools found with key={}", tool_key_lower);
                return Err(SqliteManagerError::ToolNotFound(tool_key_lower));
            }
            all_rowids
        };

        // Delete each row from shinkai_tools and shinkai_tools_vec_items
        for rowid in &rowids {
            tx.execute("DELETE FROM shinkai_tools WHERE rowid = ?1", params![rowid])?;
            tx.execute("DELETE FROM shinkai_tools_vec_items WHERE rowid = ?1", params![rowid])?;
        }

        tx.commit()?;

        // Now remove those rowids from the FTS table in the separate in-memory DB
        let fts_conn = self
            .fts_pool
            .get()
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;

        // You can wrap these in a single transaction if you prefer:
        for rowid in rowids {
            fts_conn.execute("DELETE FROM shinkai_tools_fts WHERE rowid = ?1", params![rowid])?;
        }

        Ok(())
    }

    /// Checks if the shinkai_tools table is empty
    pub fn is_empty(&self) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM shinkai_tools", [], |row| row.get(0))
            .map_err(|e| {
                eprintln!("Database error: {}", e);
                SqliteManagerError::DatabaseError(e)
            })?;

        Ok(count == 0)
    }

    /// Checks if a tool exists in the shinkai_tools table by its tool_key
    pub fn tool_exists(&self, tool_key: &str, version: Option<IndexableVersion>) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let exists = match version {
            Some(version) => conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM shinkai_tools WHERE tool_key = ?1 AND version = ?2)",
                params![tool_key.to_lowercase(), version.get_version_number()],
                |row| row.get(0),
            ),
            None => conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM shinkai_tools WHERE tool_key = ?1)",
                params![tool_key.to_lowercase()],
                |row| row.get(0),
            ),
        };
        match exists {
            Ok(exists) => Ok(exists),
            Err(e) => {
                eprintln!("Database error: {}", e);
                Err(SqliteManagerError::DatabaseError(e))
            }
        }
    }

    /// Checks if there are any JS tools in the shinkai_tools table
    pub fn has_any_js_tools(&self) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM shinkai_tools WHERE tool_type = 'Deno')",
                [],
                |row| row.get(0),
            )
            .map_err(|e| {
                eprintln!("Database error: {}", e);
                SqliteManagerError::DatabaseError(e)
            })?;

        Ok(exists)
    }

    /// Checks if there are any Rust tools in the shinkai_tools table
    pub fn has_rust_tools(&self) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM shinkai_tools WHERE tool_type = 'Rust'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| {
                eprintln!("Database error: {}", e);
                SqliteManagerError::DatabaseError(e)
            })?;

        Ok(count >= 4)
    }

    // Update the FTS table when inserting or updating a tool
    pub fn update_tools_fts(&self, tool: &ShinkaiTool) -> Result<(), SqliteManagerError> {
        // Get a connection from the in-memory pool for FTS operations
        let mut fts_conn = self.fts_pool.get().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1), // Using a generic error code
                Some(e.to_string()),
            )
        })?;

        // Start a single transaction
        let tx = fts_conn.transaction()?;

        // Delete the existing entry
        tx.execute("DELETE FROM shinkai_tools_fts WHERE name = ?1", params![tool.name()])?;

        // Insert the updated tool name
        tx.execute("INSERT INTO shinkai_tools_fts(name) VALUES (?1)", params![tool.name()])?;

        // Commit the transaction
        match tx.commit() {
            Ok(_) => Ok(()),
            Err(e) => {
                // If commit fails due to lock, retry after a short delay
                if let rusqlite::Error::SqliteFailure(err, _) = &e {
                    if err.code == rusqlite::ErrorCode::DatabaseBusy {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        // Retry the operation
                        let tx = fts_conn.transaction()?;
                        tx.execute("DELETE FROM shinkai_tools_fts WHERE name = ?1", params![tool.name()])?;
                        tx.execute("INSERT INTO shinkai_tools_fts(name) VALUES (?1)", params![tool.name()])?;
                        tx.commit()?;
                        return Ok(());
                    }
                }
                Err(SqliteManagerError::DatabaseError(e))
            }
        }
    }

    // Search the FTS table
    pub fn search_tools_fts(&self, query: &str) -> Result<Vec<ShinkaiToolHeader>, SqliteManagerError> {
        // Get a connection from the in-memory pool for FTS operations
        let fts_conn = self
            .fts_pool
            .get()
            .map_err(|e| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some(e.to_string())))?;

        // Extract keyphrases using the `keyphrases` crate (RAKE under the hood).
        // Adjust top_n as needed (e.g. 5, 10) to extract more phrases.
        let extractor = KeyPhraseExtractor::new(query, 5);
        let keywords = extractor.get_keywords();

        // If no key phrases were found, just use the original query
        let phrases_to_search = if keywords.is_empty() {
            vec![query.to_string()]
        } else {
            keywords.iter().map(|(_, kw)| kw.clone()).collect::<Vec<String>>()
        };

        let mut tool_headers = Vec::new();
        let mut seen = HashSet::new(); // avoid duplicates if multiple phrases match the same tool

        let conn = self.get_connection()?;

        for phrase in phrases_to_search {
            let mut stmt = fts_conn.prepare("SELECT name FROM shinkai_tools_fts WHERE shinkai_tools_fts MATCH ?1")?;
            let name_iter = stmt.query_map(rusqlite::params![phrase], |row| row.get::<_, String>(0))?;

            for name_res in name_iter {
                let name = name_res.map_err(|e| {
                    eprintln!("FTS query error: {}", e);
                    SqliteManagerError::DatabaseError(e)
                })?;

                // Only fetch tool header if we haven't seen this one already
                if seen.insert(name.clone()) {
                    let mut stmt =
                        conn.prepare("SELECT tool_header FROM shinkai_tools WHERE name = ?1 ORDER BY version DESC")?;
                    let tool_header_data: Vec<u8> =
                        stmt.query_row(rusqlite::params![name], |row| row.get(0)).map_err(|e| {
                            eprintln!("Persistent DB query error: {}", e);
                            SqliteManagerError::DatabaseError(e)
                        })?;

                    let tool_header: ShinkaiToolHeader = serde_json::from_slice(&tool_header_data).map_err(|e| {
                        eprintln!("Deserialization error: {}", e);
                        SqliteManagerError::SerializationError(e.to_string())
                    })?;

                    tool_headers.push(tool_header);
                }
            }
        }

        Ok(tool_headers)
    }

    // Synchronize the FTS table with the main database
    pub fn sync_tools_fts_table(&self) -> Result<(), SqliteManagerError> {
        // Use the pooled connection to access the shinkai_tools table
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare("SELECT rowid, name FROM shinkai_tools")?;
        let mut rows = stmt.query([])?;

        // Get a connection from the in-memory pool for FTS operations
        let fts_conn = self.fts_pool.get().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1), // Using a generic error code
                Some(e.to_string()),
            )
        })?;

        // Use the in-memory connection for FTS operations
        while let Some(row) = rows.next()? {
            let rowid: i64 = row.get(0)?;
            let name: String = row.get(1)?;

            // Delete the existing entry if it exists
            fts_conn.execute("DELETE FROM shinkai_tools_fts WHERE rowid = ?1", params![rowid])?;

            // Insert the new entry
            fts_conn.execute(
                "INSERT INTO shinkai_tools_fts(rowid, name) VALUES (?1, ?2)",
                params![rowid, name],
            )?;
        }
        Ok(())
    }

    pub fn update_tools_vector(
        &self,
        tx: &rusqlite::Transaction,
        tool_key: &str,
        embedding: Vec<f32>,
    ) -> Result<(), SqliteManagerError> {
        // Get is_enabled and is_network from the main database
        let (is_enabled, is_network): (i32, i32) = tx.query_row(
            "SELECT is_enabled, is_network FROM shinkai_tools WHERE tool_key = ?1",
            params![tool_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        tx.execute(
            "UPDATE shinkai_tools_vec_items SET 
                embedding = ?1,
                is_enabled = ?2,
                is_network = ?3
             WHERE tool_key = ?4",
            params![cast_slice(&embedding), is_enabled, is_network, tool_key],
        )?;

        Ok(())
    }

    // Performs a vector search for tools using a precomputed vector within a limited scope
    pub fn tool_vector_search_with_vector_limited(
        &self,
        vector: Vec<f32>,
        num_results: u64,
        tool_keys: Vec<String>,
    ) -> Result<Vec<(ShinkaiToolHeader, f64)>, SqliteManagerError> {
        // Serialize the vector to a JSON array string for the database query
        let vector_json = serde_json::to_string(&vector).map_err(|e| {
            eprintln!("Vector serialization error: {}", e);
            SqliteManagerError::SerializationError(e.to_string())
        })?;

        // Establish a connection to the database
        let conn = self.get_connection()?;

        // Start with a larger limit to account for filtering
        let mut current_limit = num_results * 2; // Adjust this multiplier as needed

        // SQL query to perform the vector search
        let query = "SELECT v.tool_key, v.distance 
             FROM shinkai_tools_vec_items v
             WHERE v.embedding MATCH json(?1)
             ORDER BY v.distance 
             LIMIT ?2";

        let mut tools_with_distances = Vec::new();

        // Fetch and filter results until we have enough
        loop {
            let mut stmt = conn.prepare(&query)?;
            let tool_keys_and_distances: Vec<(String, f64)> = stmt
                .query_map(&[&vector_json, &current_limit.to_string()], |row| {
                    // Dereference the distance to convert from &f64 to f64
                    Ok((row.get(0)?, row.get::<_, f64>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;

            // Filter results based on the provided tool keys
            for (tool_key, distance) in &tool_keys_and_distances {
                if tool_keys.contains(tool_key) {
                    if let Ok(tool_header) = self.get_tool_header_by_key(tool_key) {
                        tools_with_distances.push((tool_header, *distance));
                    }
                }
                // Break if we have enough results
                if tools_with_distances.len() >= num_results as usize {
                    return Ok(tools_with_distances);
                }
            }

            // Break if the query returned fewer results than the current limit
            if tool_keys_and_distances.len() < current_limit as usize {
                break;
            }

            // Increase the limit for the next query
            current_limit *= 2;
        }

        Ok(tools_with_distances)
    }

    pub fn get_tool_by_key_and_version(
        &self,
        tool_key: &str,
        version: Option<IndexableVersion>,
    ) -> Result<ShinkaiTool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let tool_key_lower = tool_key.to_lowercase();

        let tool: ShinkaiTool = if let Some(version) = version {
            let version_number = version.get_version_number();
            conn.query_row(
                "SELECT tool_data FROM shinkai_tools WHERE tool_key = ?1 AND version = ?2",
                params![tool_key_lower, version_number],
                |row| {
                    let tool_data: Vec<u8> = row.get(0)?;
                    serde_json::from_slice(&tool_data).map_err(|e| {
                        eprintln!("Deserialization error: {}", e);
                        rusqlite::Error::InvalidQuery
                    })
                },
            )?
        } else {
            conn.query_row(
                "SELECT tool_data FROM shinkai_tools WHERE tool_key = ?1 ORDER BY version DESC LIMIT 1",
                params![tool_key_lower],
                |row| {
                    let tool_data: Vec<u8> = row.get(0)?;
                    serde_json::from_slice(&tool_data).map_err(|e| {
                        eprintln!("Deserialization error: {}", e);
                        rusqlite::Error::InvalidQuery
                    })
                },
            )?
        };

        Ok(tool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::EmbeddingModelType;
    use shinkai_embedding::model_type::OllamaTextEmbeddingsInference;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_message_primitives::schemas::shinkai_tool_offering::AssetPayment;
    use shinkai_message_primitives::schemas::shinkai_tool_offering::ToolPrice;
    use shinkai_message_primitives::schemas::shinkai_tool_offering::UsageType;
    use shinkai_message_primitives::schemas::tool_router_key::ToolRouterKey;
    use shinkai_message_primitives::schemas::wallet_mixed::Asset;
    use shinkai_message_primitives::schemas::wallet_mixed::NetworkIdentifier;
    use shinkai_tools_primitives::tools::deno_tools::DenoTool;
    use shinkai_tools_primitives::tools::network_tool::NetworkTool;
    use shinkai_tools_primitives::tools::parameters::Parameters;
    use shinkai_tools_primitives::tools::python_tools::PythonTool;
    use shinkai_tools_primitives::tools::tool_config::BasicConfig;
    use shinkai_tools_primitives::tools::tool_config::ToolConfig;
    use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
    use shinkai_tools_primitives::tools::tool_types::OperatingSystem;
    use shinkai_tools_primitives::tools::tool_types::RunnerType;
    use shinkai_tools_primitives::tools::tool_types::ToolResult;
    use std::path::PathBuf;
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
    async fn test_add_deno_tool() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Deno Author".to_string(),
            "Deno Test Tool".to_string(),
            None,
        );

        // Create a DenoTool instance
        let deno_tool = DenoTool {
            name: "Deno Test Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Deno Author".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        // Wrap the DenoTool in a ShinkaiTool::Deno variant
        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);

        // Debug: Print the tool before adding
        println!("Testing add_tool with: {:?}", shinkai_tool);

        let vector = SqliteManager::generate_vector_for_testing(0.1);

        // Add the tool to the database
        let result = manager.add_tool_with_vector(shinkai_tool.clone(), vector);
        assert!(result.is_ok());

        // Retrieve the tool using the new method
        let retrieved_tool = manager
            .get_tool_by_key(&shinkai_tool.tool_router_key().to_string_without_version())
            .unwrap();

        // Assert that the retrieved tool matches the added tool
        assert_eq!(retrieved_tool.name(), shinkai_tool.name());
        assert_eq!(retrieved_tool.description(), shinkai_tool.description());
        assert_eq!(retrieved_tool.author(), shinkai_tool.author());

        // Remove the tool from the database
        manager
            .remove_tool(&shinkai_tool.tool_router_key().to_string_without_version(), None)
            .unwrap();

        // Verify that the tool is removed from the shinkai_tools table
        let tool_removal_result = manager.get_tool_by_key(&shinkai_tool.tool_router_key().to_string_without_version());
        assert!(tool_removal_result.is_err());

        // Verify that the embedding is removed from the shinkai_tools_vec_items table
        let conn = manager.get_connection().unwrap();
        let embedding_removal_result: Result<i64, _> = conn.query_row(
            "SELECT rowid FROM shinkai_tools_vec_items WHERE rowid = ?1",
            params![shinkai_tool
                .tool_router_key()
                .to_string_without_version()
                .to_lowercase()],
            |row| row.get(0),
        );

        assert!(embedding_removal_result.is_err());
    }

    #[tokio::test]
    async fn test_tool_vector_search() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Deno Author".to_string(),
            "Deno Test Tool".to_string(),
            None,
        );

        // Create and add three DenoTool instances
        let deno_tool_1 = DenoTool {
            name: "Deno Test Tool 1".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Deno Author 1".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno 1!');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing 1".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Deno Author 2".to_string(),
            "Deno Test Tool 2".to_string(),
            None,
        );

        let deno_tool_2 = DenoTool {
            name: "Deno Test Tool 2".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Deno Author 2".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno 2!');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing 2".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Deno Author 3".to_string(),
            "Deno Test Tool 3".to_string(),
            None,
        );

        let deno_tool_3 = DenoTool {
            name: "Deno Test Tool 3".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Deno Author 3".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno 3!');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing 3".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let shinkai_tool_1 = ShinkaiTool::Deno(deno_tool_1, true);
        let shinkai_tool_2 = ShinkaiTool::Deno(deno_tool_2, true);
        let shinkai_tool_3 = ShinkaiTool::Deno(deno_tool_3, true);

        // Add the tools to the database with different vectors
        manager
            .add_tool_with_vector(shinkai_tool_1.clone(), SqliteManager::generate_vector_for_testing(0.1))
            .unwrap();
        manager
            .add_tool_with_vector(shinkai_tool_2.clone(), SqliteManager::generate_vector_for_testing(0.5))
            .unwrap();
        manager
            .add_tool_with_vector(shinkai_tool_3.clone(), SqliteManager::generate_vector_for_testing(0.9))
            .unwrap();

        // Generate an embedding vector for the query that is close to the first tool
        let embedding_query = SqliteManager::generate_vector_for_testing(0.09);

        // Perform a vector search using the generated embedding
        let num_results = 1;
        let search_results: Vec<ShinkaiToolHeader> = manager
            .tool_vector_search_with_vector(embedding_query, num_results, true, true)
            .unwrap()
            .iter()
            .map(|(tool, _distance)| tool.clone())
            .collect();

        // Assert that the search results contain the first tool
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].name, "Deno Test Tool 1");
    }

    #[tokio::test]
    async fn test_update_middle_tool() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 1".to_string(),
            "Deno Tool 1".to_string(),
            None,
        );

        // Create three DenoTool instances
        let deno_tool_1 = DenoTool {
            name: "Deno Tool 1".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Author 1".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Tool 1');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "First Deno tool".to_string(),
            keywords: vec!["deno".to_string(), "tool1".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 2".to_string(),
            "Deno Tool 2".to_string(),
            None,
        );

        let deno_tool_2 = DenoTool {
            name: "Deno Tool 2".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Author 2".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Tool 2');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "Second Deno tool".to_string(),
            keywords: vec!["deno".to_string(), "tool2".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 3".to_string(),
            "Deno Tool 3".to_string(),
            None,
        );

        let deno_tool_3 = DenoTool {
            name: "Deno Tool 3".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Author 3".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Tool 3');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "Third Deno tool".to_string(),
            keywords: vec!["deno".to_string(), "tool3".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        // Wrap the DenoTools in ShinkaiTool::Deno variants
        let shinkai_tool_1 = ShinkaiTool::Deno(deno_tool_1, true);
        let shinkai_tool_2 = ShinkaiTool::Deno(deno_tool_2, true);
        let shinkai_tool_3 = ShinkaiTool::Deno(deno_tool_3, true);

        // Add the tools to the database
        manager
            .add_tool_with_vector(shinkai_tool_1.clone(), SqliteManager::generate_vector_for_testing(0.1))
            .unwrap();
        manager
            .add_tool_with_vector(shinkai_tool_2.clone(), SqliteManager::generate_vector_for_testing(0.2))
            .unwrap();
        manager
            .add_tool_with_vector(shinkai_tool_3.clone(), SqliteManager::generate_vector_for_testing(0.3))
            .unwrap();

        // Print out the name and key for each tool in the database
        let all_tools = manager.get_all_tool_headers().unwrap();
        for tool in &all_tools {
            eprintln!("Tool name: {}, Tool key: {}", tool.name, tool.tool_router_key);
        }

        // Update the second tool without changing the name
        let mut updated_tool_2 = shinkai_tool_2.clone();
        if let ShinkaiTool::Deno(ref mut deno_tool, _) = updated_tool_2 {
            deno_tool.description = "Updated second Deno tool".to_string();
            deno_tool.embedding = Some(SqliteManager::generate_vector_for_testing(0.21));
        }
        eprintln!("Updating tool: {:?}", updated_tool_2);

        manager.update_tool(updated_tool_2.clone()).await.unwrap();

        // Retrieve the updated tool and verify the changes
        let retrieved_tool = manager
            .get_tool_by_key(&updated_tool_2.tool_router_key().to_string_without_version())
            .unwrap();
        assert_eq!(retrieved_tool.name(), "Deno Tool 2");
        assert_eq!(retrieved_tool.description(), "Updated second Deno tool");

        // Manually query the shinkai_tools_vec_items table to verify the vector
        let conn = manager.get_connection().unwrap();
        let rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM shinkai_tools WHERE tool_key = ?1",
                params![updated_tool_2
                    .tool_router_key()
                    .to_string_without_version()
                    .to_lowercase()],
                |row| row.get(0),
            )
            .unwrap();

        let mut stmt = conn
            .prepare("SELECT embedding FROM shinkai_tools_vec_items WHERE rowid = ?1")
            .unwrap();
        let embedding_bytes: Vec<u8> = stmt.query_row(params![rowid], |row| row.get(0)).unwrap();
        let db_vector: &[f32] = cast_slice(&embedding_bytes);

        // Verify the vector in the shinkai_tools_vec_items table
        assert_eq!(db_vector, SqliteManager::generate_vector_for_testing(0.21).as_slice());
    }

    #[tokio::test]
    async fn test_add_duplicate_tool() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Deno Author".to_string(),
            "Deno Duplicate Tool".to_string(),
            None,
        );

        // Create a DenoTool instance
        let deno_tool = DenoTool {
            name: "Deno Duplicate Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: None,
            author: "Deno Author".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "A Deno tool for testing duplicates".to_string(),
            keywords: vec!["deno".to_string(), "duplicate".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        // Wrap the DenoTool in a ShinkaiTool::Deno variant
        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);

        // Add the tool to the database
        let vector = SqliteManager::generate_vector_for_testing(0.1);
        let result = manager.add_tool_with_vector(shinkai_tool.clone(), vector.clone());
        assert!(result.is_ok());

        // Attempt to add the same tool again
        let duplicate_result = manager.add_tool_with_vector(shinkai_tool.clone(), vector);

        // Assert that the error is ToolAlreadyExists
        assert!(matches!(
            duplicate_result,
            Err(SqliteManagerError::ToolAlreadyExists(_))
        ));
    }

    #[tokio::test]
    async fn test_fts_search() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 1".to_string(),
            "Image Processing Tool".to_string(),
            None,
        );

        let tool_router_key_2 = ToolRouterKey::new(
            "local".to_string(),
            "Author 2".to_string(),
            "Text Analysis Helper".to_string(),
            None,
        );

        let tool_router_key_3 = ToolRouterKey::new(
            "local".to_string(),
            "Author 3".to_string(),
            "Data Visualization Tool".to_string(),
            None,
        );

        // Create multiple tools with different names
        let tools = vec![
            DenoTool {
                name: "Image Processing Tool".to_string(),
                tool_router_key: Some(tool_router_key.clone()),
                homepage: Some("http://127.0.0.1/index.html".to_string()),
                author: "Author 1".to_string(),
                version: "1.0.0".to_string(),
                mcp_enabled: Some(false),
                js_code: "console.log('Tool 1');".to_string(),
                tools: vec![],
                config: vec![],
                oauth: None,
                description: "Process and manipulate images".to_string(),
                keywords: vec!["image".to_string(), "processing".to_string()],
                input_args: Parameters::new(),
                activated: true,
                embedding: None,
                result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                output_arg: ToolOutputArg::empty(),
                sql_tables: None,
                sql_queries: None,
                file_inbox: None,
                assets: None,
                runner: RunnerType::OnlyHost,
                operating_system: vec![OperatingSystem::Windows],
                tool_set: None,
            },
            DenoTool {
                name: "Text Analysis Helper".to_string(),
                tool_router_key: Some(tool_router_key_2.clone()),
                homepage: Some("http://127.0.0.1/index.html".to_string()),
                author: "Author 2".to_string(),
                version: "1.0.0".to_string(),
                mcp_enabled: Some(false),
                js_code: "console.log('Tool 2');".to_string(),
                tools: vec![],
                config: vec![],
                oauth: None,
                description: "Analyze text content".to_string(),
                keywords: vec!["text".to_string(), "analysis".to_string()],
                input_args: Parameters::new(),
                activated: true,
                embedding: None,
                result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                output_arg: ToolOutputArg::empty(),
                sql_tables: None,
                sql_queries: None,
                file_inbox: None,
                assets: None,
                runner: RunnerType::OnlyHost,
                operating_system: vec![OperatingSystem::Windows],
                tool_set: None,
            },
            DenoTool {
                name: "Data Visualization Tool".to_string(),
                tool_router_key: Some(tool_router_key_3.clone()),
                homepage: None,
                author: "Author 3".to_string(),
                version: "1.0.0".to_string(),
                mcp_enabled: Some(false),
                js_code: "console.log('Tool 3');".to_string(),
                tools: vec![],
                config: vec![],
                oauth: None,
                description: "Visualize data sets".to_string(),
                keywords: vec!["data".to_string(), "visualization".to_string()],
                input_args: Parameters::new(),
                activated: true,
                embedding: None,
                result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
                output_arg: ToolOutputArg::empty(),
                sql_tables: None,
                sql_queries: None,
                file_inbox: None,
                assets: None,
                runner: RunnerType::OnlyHost,
                operating_system: vec![OperatingSystem::Windows],
                tool_set: None,
            },
        ];

        // Add all tools to the database
        for (i, tool) in tools.into_iter().enumerate() {
            let shinkai_tool = ShinkaiTool::Deno(tool, true);
            let vector = SqliteManager::generate_vector_for_testing(0.1 * (i + 1) as f32);
            if let Err(e) = manager.add_tool_with_vector(shinkai_tool, vector) {
                eprintln!("Failed to add tool: {:?}", e);
            } else {
                eprintln!("Successfully added tool with index: {}", i);
            }
        }

        // Test exact match
        match manager.search_tools_fts("Text Analysis") {
            Ok(results) => {
                eprintln!("Search results: {:?}", results);
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].name, "Text Analysis Helper");
            }
            Err(e) => eprintln!("Search failed: {:?}", e),
        }

        // Test partial match
        let results = manager.search_tools_fts("visualization").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Data Visualization Tool");

        // Test case insensitive match
        let results = manager.search_tools_fts("IMAGE").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Image Processing Tool");

        // Test no match
        let results = manager.search_tools_fts("nonexistent").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_tool_vector_search_with_disabled() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 1".to_string(),
            "Enabled Test Tool".to_string(),
            None,
        );

        // Create two DenoTool instances - one enabled, one disabled
        let enabled_tool = DenoTool {
            name: "Enabled Test Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            author: "Author 1".to_string(),
            js_code: "console.log('Enabled');".to_string(),
            tools: vec![],
            config: vec![],
            description: "An enabled tool for testing".to_string(),
            keywords: vec!["enabled".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            oauth: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 2".to_string(),
            "Disabled Test Tool".to_string(),
            None,
        );

        let disabled_tool = DenoTool {
            name: "Disabled Test Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: None,
            author: "Author 2".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Disabled');".to_string(),
            tools: vec![],
            config: vec![],
            description: "A disabled tool for testing".to_string(),
            keywords: vec!["disabled".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            activated: false,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        // Add both tools to the database
        let shinkai_enabled = ShinkaiTool::Deno(enabled_tool, true);
        let shinkai_disabled = ShinkaiTool::Deno(disabled_tool, false);

        manager
            .add_tool_with_vector(shinkai_enabled.clone(), SqliteManager::generate_vector_for_testing(0.1))
            .unwrap();
        manager
            .add_tool_with_vector(
                shinkai_disabled.clone(),
                SqliteManager::generate_vector_for_testing(0.2),
            )
            .unwrap();

        // Test search excluding disabled tools
        let embedding_query = SqliteManager::generate_vector_for_testing(0.15);
        let search_results: Vec<ShinkaiToolHeader> = manager
            .tool_vector_search_with_vector(embedding_query.clone(), 10, false, true)
            .unwrap()
            .iter()
            .map(|(tool, _distance)| tool.clone())
            .collect();

        // Should only find the enabled tool
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].name, "Enabled Test Tool");

        // Test search including disabled tools
        let search_results: Vec<ShinkaiToolHeader> = manager
            .tool_vector_search_with_vector(embedding_query.clone(), 10, true, true)
            .unwrap()
            .iter()
            .map(|(tool, _distance)| tool.clone())
            .collect();

        // Should find both tools
        assert_eq!(search_results.len(), 2);
        assert!(search_results.iter().any(|t| t.name == "Enabled Test Tool"));
        assert!(search_results.iter().any(|t| t.name == "Disabled Test Tool"));

        // Now disable the previously enabled tool
        if let ShinkaiTool::Deno(mut deno_tool, _is_enabled) = shinkai_enabled {
            deno_tool.activated = false;
            let updated_tool = ShinkaiTool::Deno(deno_tool, false);
            // Just update the tool status - no need to regenerate the vector
            manager
                .update_tool_with_vector(updated_tool, SqliteManager::generate_vector_for_testing(0.1))
                .unwrap();
        }

        // Search again excluding disabled tools - should now return empty results
        let search_results: Vec<ShinkaiToolHeader> = manager
            .tool_vector_search_with_vector(embedding_query, 10, false, true)
            .unwrap()
            .iter()
            .map(|(tool, _distance)| tool.clone())
            .collect();

        // Should find no tools as both are now disabled
        assert_eq!(search_results.len(), 0);
    }

    #[tokio::test]
    async fn test_tool_vector_search_with_network_filter() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 1".to_string(),
            "Enabled Non-Network Tool".to_string(),
            None,
        );

        // Create three tools: one enabled non-network, one disabled non-network, one enabled network
        let enabled_non_network_tool = DenoTool {
            name: "Enabled Non-Network Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Author 1".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Enabled Non-Network');".to_string(),
            tools: vec![],
            config: vec![],
            description: "An enabled non-network tool".to_string(),
            keywords: vec!["enabled".to_string(), "non-network".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 2".to_string(),
            "Disabled Non-Network Tool".to_string(),
            None,
        );

        let disabled_non_network_tool = DenoTool {
            name: "Disabled Non-Network Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Author 2".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Disabled Non-Network');".to_string(),
            tools: vec![],
            config: vec![],
            description: "A disabled non-network tool".to_string(),
            keywords: vec!["disabled".to_string(), "non-network".to_string()],
            input_args: Parameters::new(),
            activated: false, // This tool is disabled
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let usage_type = UsageType::PerUse(ToolPrice::Payment(vec![AssetPayment {
            asset: Asset {
                network_id: NetworkIdentifier::BaseSepolia,
                asset_id: "USDC".to_string(),
                decimals: Some(6),
                contract_address: Some("0x036CbD53842c5426634e7929541eC2318f3dCF7e".to_string()),
            },
            amount: "1000".to_string(), // 0.001 USDC in atomic units (6 decimals)
        }]));

        let input_args = Parameters::with_single_property("message", "string", "The message to send", true);

        let enabled_network_tool = NetworkTool {
            name: "Enabled Network Tool".to_string(),
            author: "Author 3".to_string(),
            description: "An enabled network tool".to_string(),
            version: "0.1".to_string(),
            mcp_enabled: Some(false),
            provider: ShinkaiName::new("@@agent_provider.sep-shinkai".to_string()).unwrap(),
            usage_type: usage_type.clone(),
            activated: true,
            config: vec![],
            input_args: input_args.clone(),
            output_arg: ToolOutputArg { json: "".to_string() },
            embedding: None,
            restrictions: None,
        };

        // Wrap the tools in ShinkaiTool variants
        let shinkai_enabled_non_network = ShinkaiTool::Deno(enabled_non_network_tool, true);
        let shinkai_disabled_non_network = ShinkaiTool::Deno(disabled_non_network_tool, false);
        let shinkai_enabled_network = ShinkaiTool::Network(enabled_network_tool, true);

        // Add the tools to the database
        manager
            .add_tool_with_vector(
                shinkai_enabled_non_network.clone(),
                SqliteManager::generate_vector_for_testing(0.1),
            )
            .unwrap();
        manager
            .add_tool_with_vector(
                shinkai_disabled_non_network.clone(),
                SqliteManager::generate_vector_for_testing(0.2),
            )
            .unwrap();
        manager
            .add_tool_with_vector(
                shinkai_enabled_network.clone(),
                SqliteManager::generate_vector_for_testing(0.3),
            )
            .unwrap();

        // Perform searches and verify results

        // Search including only enabled non-network tools
        let search_results: Vec<ShinkaiToolHeader> = manager
            .tool_vector_search_with_vector(SqliteManager::generate_vector_for_testing(0.15), 10, false, false)
            .unwrap()
            .iter()
            .map(|(tool, _distance)| tool.clone())
            .collect();

        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].name, "Enabled Non-Network Tool");

        // Search including only enabled tools (both network and non-network)
        let search_results: Vec<ShinkaiToolHeader> = manager
            .tool_vector_search_with_vector(SqliteManager::generate_vector_for_testing(0.25), 10, false, true)
            .unwrap()
            .iter()
            .map(|(tool, _distance)| tool.clone())
            .collect();

        assert_eq!(search_results.len(), 2);
        assert!(search_results.iter().any(|t| t.name == "Enabled Non-Network Tool"));
        assert!(search_results.iter().any(|t| t.name == "Enabled Network Tool"));

        // Search including all non-network tools (enabled and disabled)
        let search_results: Vec<ShinkaiToolHeader> = manager
            .tool_vector_search_with_vector(SqliteManager::generate_vector_for_testing(0.15), 10, true, false)
            .unwrap()
            .iter()
            .map(|(tool, _distance)| tool.clone())
            .collect();

        assert_eq!(search_results.len(), 2);
        assert!(search_results.iter().any(|t| t.name == "Enabled Non-Network Tool"));
        assert!(search_results.iter().any(|t| t.name == "Disabled Non-Network Tool"));

        // Search including all tools (enabled, disabled, network, and non-network)
        let search_results: Vec<ShinkaiToolHeader> = manager
            .tool_vector_search_with_vector(SqliteManager::generate_vector_for_testing(0.25), 10, true, true)
            .unwrap()
            .iter()
            .map(|(tool, _distance)| tool.clone())
            .collect();

        assert_eq!(search_results.len(), 3);
        assert!(search_results.iter().any(|t| t.name == "Enabled Non-Network Tool"));
        assert!(search_results.iter().any(|t| t.name == "Disabled Non-Network Tool"));
        assert!(search_results.iter().any(|t| t.name == "Enabled Network Tool"));
    }

    #[tokio::test]
    async fn test_tool_vector_search_with_vector_limited() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 1".to_string(),
            "Tool One".to_string(),
            None,
        );

        // Create three tools with different vectors
        let tool1 = DenoTool {
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            name: "Tool One".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            author: "Author 1".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Tool 1');".to_string(),
            tools: vec![],
            config: vec![],
            description: "First test tool".to_string(),
            keywords: vec!["test".to_string(), "one".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 2".to_string(),
            "Tool Two".to_string(),
            None,
        );

        let tool2 = DenoTool {
            name: "Tool Two".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Author 2".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Tool 2');".to_string(),
            tools: vec![],
            config: vec![],
            description: "Second test tool".to_string(),
            keywords: vec!["test".to_string(), "two".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Author 3".to_string(),
            "Tool Three".to_string(),
            None,
        );

        let tool3 = DenoTool {
            name: "Tool Three".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Author 3".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Tool 3');".to_string(),
            tools: vec![],
            config: vec![],
            description: "Third test tool".to_string(),
            keywords: vec!["test".to_string(), "three".to_string()],
            input_args: Parameters::new(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output_arg: ToolOutputArg::empty(),
            sql_tables: None,
            sql_queries: None,
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        // Add tools to database with specific vectors
        let shinkai_tool1 = ShinkaiTool::Deno(tool1, true);
        let shinkai_tool2 = ShinkaiTool::Deno(tool2, true);
        let shinkai_tool3 = ShinkaiTool::Deno(tool3, true);

        // Tool 2 will have the closest vector to our search query
        manager
            .add_tool_with_vector(shinkai_tool1.clone(), SqliteManager::generate_vector_for_testing(0.1))
            .unwrap();
        manager
            .add_tool_with_vector(shinkai_tool2.clone(), SqliteManager::generate_vector_for_testing(0.5))
            .unwrap();
        manager
            .add_tool_with_vector(shinkai_tool3.clone(), SqliteManager::generate_vector_for_testing(0.9))
            .unwrap();

        // Search vector that's closest to Tool 2's vector
        let search_vector = SqliteManager::generate_vector_for_testing(0.5);

        // Only include Tool 1 and Tool 3 in the search scope
        let limited_tool_keys = vec![
            shinkai_tool1.tool_router_key().to_string_without_version(),
            shinkai_tool3.tool_router_key().to_string_without_version(),
        ];

        // Perform the limited search
        let results = manager
            .tool_vector_search_with_vector_limited(search_vector.clone(), 2, limited_tool_keys.clone())
            .unwrap();

        // Verify results
        assert_eq!(results.len(), 2, "Should only find two tools");

        // Perform the limited search
        let results = manager
            .tool_vector_search_with_vector_limited(search_vector, 10, limited_tool_keys)
            .unwrap();

        // Verify results
        assert_eq!(results.len(), 2, "Should only find two tools");

        // Tool 2 should not be in results despite having the closest vector
        for (tool, _distance) in &results {
            assert_ne!(
                tool.name, "Tool Two",
                "Tool Two should not be in results as it wasn't in the limited scope"
            );
        }

        // Verify that Tool 1 and Tool 3 are in the results
        let result_names: Vec<String> = results.iter().map(|(tool, _)| tool.name.clone()).collect();
        assert!(result_names.contains(&"Tool One".to_string()));
        assert!(result_names.contains(&"Tool Three".to_string()));
    }

    #[tokio::test]
    async fn test_add_tools_with_different_versions() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Version Author".to_string(),
            "Versioned Tool".to_string(),
            None,
        );

        // Create two DenoTool instances with different versions
        let deno_tool_v1 = DenoTool {
            name: "Versioned Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Version Author".to_string(),
            version: "1.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Version 1');".to_string(),
            tools: vec![],
            config: vec![],
            description: "A tool with version 1.0".to_string(),
            keywords: vec!["version".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Version Author".to_string(),
            "Versioned Tool".to_string(),
            None,
        );

        let deno_tool_v2 = DenoTool {
            name: "Versioned Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://127.0.0.1/index.html".to_string()),
            author: "Version Author".to_string(),
            version: "2.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('Version 2');".to_string(),
            tools: vec![],
            config: vec![],
            description: "A tool with version 2.0".to_string(),
            keywords: vec!["version".to_string(), "test".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            oauth: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };

        // Wrap the DenoTools in ShinkaiTool::Deno variants
        let shinkai_tool_v1 = ShinkaiTool::Deno(deno_tool_v1, true);
        let shinkai_tool_v2 = ShinkaiTool::Deno(deno_tool_v2, true);

        // Add both tools to the database
        manager
            .add_tool_with_vector(shinkai_tool_v1.clone(), SqliteManager::generate_vector_for_testing(0.1))
            .unwrap();
        manager
            .add_tool_with_vector(shinkai_tool_v2.clone(), SqliteManager::generate_vector_for_testing(0.2))
            .unwrap();

        // Retrieve and verify both tools are added
        let retrieved_tool_v1 = manager
            .get_tool_by_key(&shinkai_tool_v1.tool_router_key().to_string_without_version())
            .unwrap();
        let retrieved_tool_v2 = manager
            .get_tool_by_key(&shinkai_tool_v2.tool_router_key().to_string_without_version())
            .unwrap();

        assert_eq!(retrieved_tool_v1.version(), "2.0");
        assert_eq!(retrieved_tool_v2.version(), "2.0");

        // Retrieve the tool with version 1.0 using the new function
        let version_1_0 = IndexableVersion::from_string("1.0").unwrap();
        let retrieved_tool_v1_0 = manager
            .get_tool_by_key_and_version(
                &shinkai_tool_v1.tool_router_key().to_string_without_version(),
                Some(version_1_0),
            )
            .unwrap();

        // Verify the retrieved tool is the correct version
        assert_eq!(retrieved_tool_v1_0.version(), "1.0");

        // Retrieve the tool with the highest version using None
        let retrieved_tool_latest = manager
            .get_tool_by_key_and_version(&shinkai_tool_v1.tool_router_key().to_string_without_version(), None)
            .unwrap();

        // Verify the retrieved tool is the latest version
        assert_eq!(retrieved_tool_latest.version(), "2.0");

        // Perform a vector search and ensure it only returns one result
        let search_vector = SqliteManager::generate_vector_for_testing(0.2);
        let search_results = manager
            .tool_vector_search_with_vector(search_vector, 1, true, true)
            .unwrap();

        // Verify that only one result is returned
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].0.name, "Versioned Tool");
        assert_eq!(search_results[0].0.version, "2.0");

        // Perform an FTS search and ensure it only returns one result (version 2.0)
        let fts_results = manager.search_tools_fts("Versioned Tool").unwrap();
        assert_eq!(fts_results.len(), 1);
        assert_eq!(fts_results[0].name, "Versioned Tool");
        assert_eq!(fts_results[0].version, "2.0");
    }

    #[tokio::test]
    async fn test_upgrade_tool_preserves_config() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Test Author".to_string(),
            "Configurable Tool".to_string(),
            None,
        );

        // Create version 1.0.0 with a config entry
        let deno_tool_v1 = DenoTool {
            name: "Configurable Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://example.com".to_string()),
            author: "Test Author".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('v1');".to_string(),
            tools: vec![],
            config: vec![ToolConfig::BasicConfig(BasicConfig {
                key_name: "enable_feature_x".to_string(),
                description: "Enable feature X".to_string(),
                required: false,
                type_name: Some("boolean".to_string()),
                key_value: Some(serde_json::Value::Bool(true)),
            })],
            oauth: None,
            description: "A tool to test config update".to_string(),
            keywords: vec!["config".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };
        let shinkai_tool_v1 = ShinkaiTool::Deno(deno_tool_v1.clone(), true);
        let vector_v1 = SqliteManager::generate_vector_for_testing(0.1);
        manager
            .add_tool_with_vector(shinkai_tool_v1.clone(), vector_v1)
            .unwrap();

        // Create version 2.0.0
        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Test Author".to_string(),
            "Configurable Tool".to_string(),
            None,
        );

        let deno_tool_v2 = DenoTool {
            name: "Configurable Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://example.com".to_string()),
            author: "Test Author".to_string(),
            version: "2.0.0".to_string(),
            mcp_enabled: Some(false),
            js_code: "console.log('v2');".to_string(),
            tools: vec![],
            config: vec![ToolConfig::BasicConfig(BasicConfig {
                key_name: "enable_feature_x".to_string(),
                description: "Enable feature X - updated".to_string(),
                required: false,
                type_name: Some("boolean".to_string()),
                key_value: None,
            })],
            oauth: None,
            description: "A tool to test config upgrade".to_string(),
            keywords: vec!["config".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };
        let shinkai_tool_v2 = ShinkaiTool::Deno(deno_tool_v2.clone(), true);

        // Upgrade to version 2.0.0
        let vector_v2 = SqliteManager::generate_vector_for_testing(0.2);
        let upgraded = manager
            .upgrade_tool_with_vector(shinkai_tool_v2.clone(), vector_v2)
            .unwrap();

        // Verify version 2.0.0
        let version_2 = IndexableVersion::from_string("2.0.0").unwrap();
        let retrieved = manager
            .get_tool_by_key_and_version(&upgraded.tool_router_key().to_string_without_version(), Some(version_2))
            .unwrap();

        if let ShinkaiTool::Deno(new_tool, _) = retrieved {
            assert_eq!(new_tool.version, "2.0.0", "Version mismatch");
            assert_eq!(new_tool.js_code, "console.log('v2');", "JS code mismatch");

            // Check that the config entry was preserved
            let config_value = new_tool.config.iter().find_map(|entry| match entry {
                ToolConfig::BasicConfig(bc) => {
                    if bc.key_name == "enable_feature_x" {
                        return bc.key_value.clone();
                    }
                    None
                }
            });
            assert_eq!(
                config_value,
                Some(serde_json::Value::Bool(true)),
                "Config value not preserved"
            );
        } else {
            panic!("Retrieved tool is not a DenoTool");
        }
    }

    #[tokio::test]
    async fn test_upgrade_tool_preserves_config_python() {
        let manager = setup_test_db().await;
        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Test Author".to_string(),
            "Configurable Python Tool".to_string(),
            None,
        );
        let python_tool_v1 = PythonTool {
            name: "Configurable Python Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://example.com".to_string()),
            author: "Test Author".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            py_code: "print('v1')".to_string(),
            tools: vec![],
            config: vec![ToolConfig::BasicConfig(BasicConfig {
                key_name: "enable_feature_x".to_string(),
                description: "Enable feature X".to_string(),
                required: false,
                type_name: Some("boolean".to_string()),
                key_value: Some(serde_json::Value::Bool(true)),
            })],
            oauth: None,
            description: "A python tool to test config update".to_string(),
            keywords: vec!["config".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };
        let shinkai_tool_v1 = ShinkaiTool::Python(python_tool_v1, true);
        manager
            .add_tool_with_vector(shinkai_tool_v1, SqliteManager::generate_vector_for_testing(0.1))
            .unwrap();

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Test Author".to_string(),
            "Configurable Python Tool".to_string(),
            None,
        );
        let python_tool_v2 = PythonTool {
            name: "Configurable Python Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: Some("http://example.com".to_string()),
            author: "Test Author".to_string(),
            version: "2.0.0".to_string(),
            mcp_enabled: Some(false),
            py_code: "print('v2')".to_string(),
            tools: vec![],
            config: vec![ToolConfig::BasicConfig(BasicConfig {
                key_name: "enable_feature_x".to_string(),
                description: "Enable feature X - updated".to_string(),
                required: false,
                type_name: Some("boolean".to_string()),
                key_value: None,
            })],
            oauth: None,
            description: "A python tool to test config upgrade".to_string(),
            keywords: vec!["config".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Windows],
            tool_set: None,
        };
        let shinkai_tool_v2 = ShinkaiTool::Python(python_tool_v2, true);
        let upgraded = manager
            .upgrade_tool_with_vector(shinkai_tool_v2, SqliteManager::generate_vector_for_testing(0.2))
            .unwrap();

        let version_2 = IndexableVersion::from_string("2.0.0").unwrap();
        let retrieved = manager
            .get_tool_by_key_and_version(&upgraded.tool_router_key().to_string_without_version(), Some(version_2))
            .unwrap();

        if let ShinkaiTool::Python(new_tool, _) = retrieved {
            assert_eq!(new_tool.version, "2.0.0");
            assert_eq!(new_tool.py_code, "print('v2')");
            let config_value = new_tool.config.iter().find_map(|entry| match entry {
                ToolConfig::BasicConfig(bc) => {
                    if bc.key_name == "enable_feature_x" {
                        bc.key_value.clone()
                    } else {
                        None
                    }
                }
                _ => None,
            });
            assert_eq!(config_value, Some(serde_json::Value::Bool(true)));
        } else {
            panic!("Retrieved tool is not a PythonTool");
        }
    }

    #[tokio::test]
    async fn test_add_duplicate_python_tool() {
        let manager = setup_test_db().await;

        let tool_router_key = ToolRouterKey::new(
            "local".to_string(),
            "Python Author".to_string(), 
            "Python Duplicate Tool".to_string(),
            None,
        );

        // Create a PythonTool instance
        let python_tool_data = PythonTool {
            name: "Python Duplicate Tool".to_string(),
            tool_router_key: Some(tool_router_key.clone()),
            homepage: None,
            author: "Python Author".to_string(),
            version: "1.0.0".to_string(),
            mcp_enabled: Some(false),
            py_code: "print('Hello, Python!')".to_string(),
            tools: vec![],
            config: vec![],
            oauth: None,
            description: "A Python tool for testing duplicates".to_string(),
            keywords: vec!["python".to_string(), "duplicate".to_string()],
            input_args: Parameters::new(),
            output_arg: ToolOutputArg::empty(),
            activated: true,
            embedding: None,
            result: ToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            sql_tables: Some(vec![]),
            sql_queries: Some(vec![]),
            file_inbox: None,
            assets: None,
            runner: RunnerType::OnlyHost,
            operating_system: vec![OperatingSystem::Linux], 
            tool_set: None,
        };

        // Wrap the PythonTool in a ShinkaiTool::Python variant
        let shinkai_tool = ShinkaiTool::Python(python_tool_data, true);

        // Add the tool to the database
        let vector = SqliteManager::generate_vector_for_testing(0.1);
        let result = manager.add_tool_with_vector(shinkai_tool.clone(), vector.clone());
        assert!(result.is_ok(), "Initial add failed: {:?}", result.err());

        // Attempt to add the same tool again
        let duplicate_result = manager.add_tool_with_vector(shinkai_tool.clone(), vector);

        // Assert that the error is ToolAlreadyExists
        assert!(
            matches!(
                duplicate_result,
                Err(SqliteManagerError::ToolAlreadyExists(_))
            ),
            "Expected ToolAlreadyExists error, but got: {:?}", duplicate_result
        );
    }
}
