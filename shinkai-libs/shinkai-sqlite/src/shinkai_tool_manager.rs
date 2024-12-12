use crate::{SqliteManager, SqliteManagerError};
use bytemuck::cast_slice;
use keyphrases::KeyPhraseExtractor;
use rusqlite::{params, Result};
use shinkai_tools_primitives::tools::shinkai_tool::{ShinkaiTool, ShinkaiToolHeader};
use std::collections::HashSet;

impl SqliteManager {
    // Adds a ShinkaiTool entry to the shinkai_tools table
    pub async fn add_tool(&mut self, tool: ShinkaiTool) -> Result<ShinkaiTool, SqliteManagerError> {
        // Generate or retrieve the embedding
        let embedding = match tool.get_embedding() {
            Some(embedding) => embedding.vector,
            None => self.generate_embeddings(&tool.format_embedding_string()).await?,
        };

        self.add_tool_with_vector(tool, embedding)
    }

    pub fn add_tool_with_vector(
        &mut self,
        tool: ShinkaiTool,
        embedding: Vec<f32>,
    ) -> Result<ShinkaiTool, SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Check if the tool already exists
        let tool_key = tool.tool_router_key().to_lowercase();
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM shinkai_tools WHERE tool_key = ?1)",
            params![tool_key],
            |row| row.get(0),
        )?;

        if exists {
            println!("Tool already exists with key: {}", tool_key);
            return Err(SqliteManagerError::ToolAlreadyExists(tool_key));
        }

        let tool_seos = tool.format_embedding_string();
        let tool_type = tool.tool_type().to_string();
        let tool_header = serde_json::to_vec(&tool.to_header()).unwrap();

        // Clone the tool to make it mutable
        let mut tool_clone = tool.clone();

        // Determine if the tool can be enabled
        let is_enabled = tool_clone.is_enabled() && tool_clone.can_be_enabled();
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
                is_network
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                tool_clone.name(),
                tool_clone.description(),
                tool_clone.tool_router_key(),
                tool_seos,
                tool_data,
                tool_header,
                tool_type,
                tool_clone.author(),
                tool_clone.version(),
                is_enabled as i32,
                on_demand_price,
                is_network as i32,
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
        let mut stmt = conn.prepare("SELECT tool_header FROM shinkai_tools WHERE tool_key = ?1")?;

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

    /// Retrieves a ShinkaiTool based on its tool_key
    pub fn get_tool_by_key(&self, tool_key: &str) -> Result<ShinkaiTool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT tool_data FROM shinkai_tools WHERE tool_key = ?1")?;

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
        &mut self,
        tool: ShinkaiTool,
        embedding: Vec<f32>,
    ) -> Result<ShinkaiTool, SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Get the tool_key and find the rowid
        let tool_key = tool.tool_router_key().to_lowercase();
        let rowid: i64 = tx
            .query_row(
                "SELECT rowid FROM shinkai_tools WHERE tool_key = ?1",
                params![tool_key],
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
                is_network = ?12
             WHERE rowid = ?13",
            params![
                tool.name(),
                tool.description(),
                tool.tool_router_key(),
                tool.format_embedding_string(),
                tool_data,
                tool_header,
                tool.tool_type().to_string(),
                tool.author(),
                tool.version(),
                is_enabled as i32,
                on_demand_price,
                is_network as i32,
                rowid
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
    pub async fn update_tool(&mut self, tool: ShinkaiTool) -> Result<ShinkaiTool, SqliteManagerError> {
        // Generate or retrieve the embedding
        let embedding = match tool.get_embedding() {
            Some(embedding) => embedding.vector,
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

    /// Removes a ShinkaiTool entry from the shinkai_tools table
    // Note: should we also auto-remove the tool from the tool_playground table?
    pub fn remove_tool(&self, tool_key: &str) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Get the rowid for the tool to be removed
        let rowid: i64 = tx
            .query_row(
                "SELECT rowid FROM shinkai_tools WHERE tool_key = ?1",
                params![tool_key.to_lowercase()],
                |row| row.get(0),
            )
            .map_err(|e| {
                eprintln!("Tool not found with key: {}", tool_key);
                SqliteManagerError::DatabaseError(e)
            })?;

        // Delete the tool from the shinkai_tools table
        tx.execute("DELETE FROM shinkai_tools WHERE rowid = ?1", params![rowid])?;

        // Delete the embedding from the shinkai_tools_vec_items table
        tx.execute("DELETE FROM shinkai_tools_vec_items WHERE rowid = ?1", params![rowid])?;

        tx.commit()?;

        // Get a connection from the in-memory pool for FTS operations
        let fts_conn = self.fts_pool.get().map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(1), // Using a generic error code
                Some(e.to_string()),
            )
        })?;

        fts_conn.execute("DELETE FROM shinkai_tools_fts WHERE rowid = ?1", params![rowid])?;

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
    pub fn tool_exists(&self, tool_key: &str) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM shinkai_tools WHERE tool_key = ?1)",
                params![tool_key.to_lowercase()],
                |row| row.get(0),
            )
            .map_err(|e| {
                eprintln!("Database error: {}", e);
                SqliteManagerError::DatabaseError(e)
            })?;

        Ok(exists)
    }

    /// Checks if there are any JS tools in the shinkai_tools table
    pub fn has_any_js_tools(&self) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM shinkai_tools WHERE tool_type = 'JS')",
                [],
                |row| row.get(0),
            )
            .map_err(|e| {
                eprintln!("Database error: {}", e);
                SqliteManagerError::DatabaseError(e)
            })?;

        Ok(exists)
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
        tx.commit()?;

        Ok(())
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
                    let mut stmt = conn.prepare("SELECT tool_header FROM shinkai_tools WHERE name = ?1")?;
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
            params![
                cast_slice(&embedding),
                is_enabled,
                is_network,
                tool_key
            ],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_message_primitives::schemas::shinkai_tool_offering::AssetPayment;
    use shinkai_message_primitives::schemas::shinkai_tool_offering::ToolPrice;
    use shinkai_message_primitives::schemas::shinkai_tool_offering::UsageType;
    use shinkai_message_primitives::schemas::wallet_mixed::Asset;
    use shinkai_message_primitives::schemas::wallet_mixed::NetworkIdentifier;
    use shinkai_tools_primitives::tools::tool_output_arg::ToolOutputArg;
    use shinkai_tools_primitives::tools::deno_tools::DenoTool;
    use shinkai_tools_primitives::tools::deno_tools::ToolResult;
    use shinkai_tools_primitives::tools::network_tool::NetworkTool;
    use shinkai_tools_primitives::tools::parameters::Parameters;
    use shinkai_vector_resources::embeddings::Embedding;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
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
    async fn test_add_deno_tool() {
        let mut manager = setup_test_db().await;

        // Create a DenoTool instance
        let deno_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Test Tool".to_string(),
            author: "Deno Author".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: None,
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
        let retrieved_tool = manager.get_tool_by_key(&shinkai_tool.tool_router_key()).unwrap();

        // Assert that the retrieved tool matches the added tool
        assert_eq!(retrieved_tool.name(), shinkai_tool.name());
        assert_eq!(retrieved_tool.description(), shinkai_tool.description());
        assert_eq!(retrieved_tool.author(), shinkai_tool.author());

        // Remove the tool from the database
        manager.remove_tool(&shinkai_tool.tool_router_key()).unwrap();

        // Verify that the tool is removed from the shinkai_tools table
        let tool_removal_result = manager.get_tool_by_key(&shinkai_tool.tool_router_key());
        assert!(tool_removal_result.is_err());

        // Verify that the embedding is removed from the shinkai_tools_vec_items table
        let conn = manager.get_connection().unwrap();
        let embedding_removal_result: Result<i64, _> = conn.query_row(
            "SELECT rowid FROM shinkai_tools_vec_items WHERE rowid = ?1",
            params![shinkai_tool.tool_router_key().to_lowercase()],
            |row| row.get(0),
        );

        assert!(embedding_removal_result.is_err());
    }

    #[tokio::test]
    async fn test_tool_vector_search() {
        let mut manager = setup_test_db().await;

        // Create and add three DenoTool instances
        let deno_tool_1 = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Test Tool 1".to_string(),
            author: "Deno Author 1".to_string(),
            js_code: "console.log('Hello, Deno 1!');".to_string(),
            tools: None,
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
        };

        let deno_tool_2 = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Test Tool 2".to_string(),
            author: "Deno Author 2".to_string(),
            js_code: "console.log('Hello, Deno 2!');".to_string(),
            tools: None,
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
        };

        let deno_tool_3 = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Test Tool 3".to_string(),
            author: "Deno Author 3".to_string(),
            js_code: "console.log('Hello, Deno 3!');".to_string(),
            tools: None,
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
        let mut manager = setup_test_db().await;

        // Create three DenoTool instances
        let deno_tool_1 = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Tool 1".to_string(),
            author: "Author 1".to_string(),
            js_code: "console.log('Tool 1');".to_string(),
            tools: None,
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
        };

        let deno_tool_2 = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Tool 2".to_string(),
            author: "Author 2".to_string(),
            js_code: "console.log('Tool 2');".to_string(),
            tools: None,
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
        };

        let deno_tool_3 = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Tool 3".to_string(),
            author: "Author 3".to_string(),
            js_code: "console.log('Tool 3');".to_string(),
            tools: None,
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
            deno_tool.embedding = Some(Embedding::new("test", SqliteManager::generate_vector_for_testing(0.21)));
        }
        eprintln!("Updating tool: {:?}", updated_tool_2);

        manager.update_tool(updated_tool_2.clone()).await.unwrap();

        // Retrieve the updated tool and verify the changes
        let retrieved_tool = manager.get_tool_by_key(&updated_tool_2.tool_router_key()).unwrap();
        assert_eq!(retrieved_tool.name(), "Deno Tool 2");
        assert_eq!(retrieved_tool.description(), "Updated second Deno tool");

        // Manually query the shinkai_tools_vec_items table to verify the vector
        let conn = manager.get_connection().unwrap();
        let rowid: i64 = conn
            .query_row(
                "SELECT rowid FROM shinkai_tools WHERE tool_key = ?1",
                params![updated_tool_2.tool_router_key().to_lowercase()],
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
        let mut manager = setup_test_db().await;

        // Create a DenoTool instance
        let deno_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Duplicate Tool".to_string(),
            author: "Deno Author".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            tools: None,
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
        let mut manager = setup_test_db().await;

        // Create multiple tools with different names
        let tools = vec![
            DenoTool {
                toolkit_name: "Deno Toolkit".to_string(),
                name: "Image Processing Tool".to_string(),
                author: "Author 1".to_string(),
                js_code: "console.log('Tool 1');".to_string(),
                tools: None,
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
            },
            DenoTool {
                toolkit_name: "Deno Toolkit".to_string(),
                name: "Text Analysis Helper".to_string(),
                author: "Author 2".to_string(),
                js_code: "console.log('Tool 2');".to_string(),
                tools: None,
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
            },
            DenoTool {
                toolkit_name: "Deno Toolkit".to_string(),
                name: "Data Visualization Tool".to_string(),
                author: "Author 3".to_string(),
                js_code: "console.log('Tool 3');".to_string(),
                tools: None,
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
        let mut manager = setup_test_db().await;

        // Create two DenoTool instances - one enabled, one disabled
        let enabled_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Enabled Test Tool".to_string(),
            author: "Author 1".to_string(),
            js_code: "console.log('Enabled');".to_string(),
            tools: None,
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
            oauth: None,
        };

        let disabled_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Disabled Test Tool".to_string(),
            author: "Author 2".to_string(),
            js_code: "console.log('Disabled');".to_string(),
            tools: None,
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
            manager.update_tool_with_vector(updated_tool, SqliteManager::generate_vector_for_testing(0.1)).unwrap();
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
        let mut manager = setup_test_db().await;

        // Create three tools: one enabled non-network, one disabled non-network, one enabled network
        let enabled_non_network_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Enabled Non-Network Tool".to_string(),
            author: "Author 1".to_string(),
            js_code: "console.log('Enabled Non-Network');".to_string(),
            tools: None,
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
        };

        let disabled_non_network_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Disabled Non-Network Tool".to_string(),
            author: "Author 2".to_string(),
            js_code: "console.log('Disabled Non-Network');".to_string(),
            tools: None,
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
            toolkit_name: "Network Toolkit".to_string(),
            description: "An enabled network tool".to_string(),
            version: "v0.1".to_string(),
            provider: ShinkaiName::new("@@agent_provider.arb-sep-shinkai".to_string()).unwrap(),
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
}
