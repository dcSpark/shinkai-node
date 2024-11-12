use crate::SqliteManager;
use bytemuck::cast_slice;
use rusqlite::{params, Result};
use shinkai_tools_primitives::tools::shinkai_tool::ShinkaiTool;
use thiserror::Error;

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
    // Add other error variants as needed
}

impl SqliteManager {
    // Adds a ShinkaiTool entry to the shinkai_tools table
    pub async fn add_tool(&self, tool: ShinkaiTool) -> Result<ShinkaiTool, SqliteManagerError> {
        // Generate or retrieve the embedding
        let embedding = match tool.get_embedding() {
            Some(embedding) => {
                println!("Using existing embedding");
                embedding.vector
            }
            None => {
                println!("Generating new embedding");
                self.generate_embeddings(&tool.format_embedding_string()).await?
            }
        };

        self.add_tool_with_vector(tool, embedding)
    }

    pub fn add_tool_with_vector(
        &self,
        tool: ShinkaiTool,
        embedding: Vec<f32>,
    ) -> Result<ShinkaiTool, SqliteManagerError> {
        println!("Starting add_tool with tool: {:?}", tool);

        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        // Check if the tool already exists
        let tool_key = tool.tool_router_key().to_lowercase();
        println!("Checking if tool exists with key: {}", tool_key);
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
        println!("Tool type: {}, SEO: {}", tool_type, tool_seos);

        // Clone the tool to make it mutable
        let mut tool_clone = tool.clone();

        // Determine if the tool can be enabled
        let is_enabled = tool_clone.is_enabled() && tool_clone.can_be_enabled();
        if tool_clone.is_enabled() && !tool_clone.can_be_enabled() {
            println!("Tool cannot be enabled, disabling");
            tool_clone.disable();
        }

        let tool_data = serde_json::to_vec(&tool_clone).map_err(|e| {
            println!("Serialization error: {}", e);
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

        println!("Inserting tool into database");
        // Insert the tool into the database
        tx.execute(
            "INSERT INTO shinkai_tools (
                name,
                description,
                tool_key,
                embedding_seo,
                tool_data,
                tool_type,
                author,
                version,
                is_enabled,
                on_demand_price,
                is_network
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                tool_clone.name(),
                tool_clone.description(),
                tool_clone.tool_router_key(),
                tool_seos,
                tool_data,
                tool_type,
                tool_clone.author(),
                tool_clone.version(),
                is_enabled as i32,
                on_demand_price,
                is_network as i32,
            ],
        )?;

        // Insert the embedding into the tools_vec_items table
        println!("Inserting embedding into tools_vec_items");
        tx.execute(
            "INSERT INTO tools_vec_items (embedding) VALUES (?1)",
            params![cast_slice(&embedding)],
        )?;

        tx.commit()?;
        println!("Tool and embedding added successfully");
        Ok(tool_clone)
    }

    // Performs a vector search for tools using a precomputed vector
    pub fn tool_vector_search_with_vector(
        &self,
        vector: Vec<f32>,
        num_results: u64,
    ) -> Result<Vec<ShinkaiTool>, SqliteManagerError> {
        // Convert Vec<f32> to &[u8] using bytemuck
        let embedding_bytes: &[u8] = cast_slice(&vector);

        // Step 1: Perform the vector search to get rowids
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT rowid FROM tools_vec_items 
             WHERE embedding MATCH ? 
             ORDER BY distance 
             LIMIT ?",
        )?;

        let rowids: Vec<i64> = stmt
            .query_map(params![embedding_bytes, num_results], |row| row.get(0))?
            .collect::<Result<Vec<i64>, _>>()?;

        // Step 2: Retrieve the corresponding ShinkaiTool entries
        let mut tools = Vec::new();
        for rowid in rowids {
            let mut stmt = conn.prepare("SELECT tool_data FROM shinkai_tools WHERE rowid = ?")?;
            let tool_data: Vec<u8> = stmt.query_row(params![rowid], |row| row.get(0))?;

            // Deserialize the tool_data to get the ShinkaiTool
            let tool: ShinkaiTool = serde_json::from_slice(&tool_data).map_err(|e| {
                println!("Deserialization error: {}", e);
                SqliteManagerError::SerializationError(e.to_string())
            })?;

            tools.push(tool);
        }

        Ok(tools)
    }

    // Performs a vector search for tools based on a query string
    pub async fn tool_vector_search(
        &self,
        query: &str,
        num_results: u64,
    ) -> Result<Vec<ShinkaiTool>, SqliteManagerError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Generate the embedding from the query string
        let embedding = self.generate_embeddings(query).await.map_err(|e| {
            println!("Embedding generation error: {}", e);
            SqliteManagerError::EmbeddingGenerationError(e.to_string())
        })?;

        // Use the new function to perform the search
        self.tool_vector_search_with_vector(embedding, num_results)
    }

    // Retrieves a ShinkaiTool based on its tool_key
    pub fn get_tool_by_key(&self, tool_key: &str) -> Result<ShinkaiTool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT tool_data FROM shinkai_tools WHERE tool_key = ?1")?;

        let tool_data: Vec<u8> = stmt
            .query_row(params![tool_key.to_lowercase()], |row| row.get(0))
            .map_err(|e| {
                println!("Database error: {}", e);
                SqliteManagerError::DatabaseError(e)
            })?;

        // Deserialize the tool_data to get the ShinkaiTool
        let tool: ShinkaiTool = serde_json::from_slice(&tool_data).map_err(|e| {
            println!("Deserialization error: {}", e);
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
        println!("Starting update_tool with tool: {:?}", tool);

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
                println!("Tool not found with key: {}", tool_key);
                SqliteManagerError::DatabaseError(e)
            })?;

        // Serialize the updated tool data
        let tool_data = serde_json::to_vec(&tool).map_err(|e| {
            println!("Serialization error: {}", e);
            SqliteManagerError::SerializationError(e.to_string())
        })?;

        // Update the tool in the database
        println!("Updating tool in database");
        tx.execute(
            "UPDATE shinkai_tools SET tool_data = ?1 WHERE rowid = ?2",
            params![tool_data, rowid],
        )?;

        // Update the embedding in the tools_vec_items table
        println!("Updating embedding in tools_vec_items");
        tx.execute(
            "UPDATE tools_vec_items SET embedding = ?1 WHERE rowid = ?2",
            params![cast_slice(&embedding), rowid],
        )?;

        tx.commit()?;
        println!("Tool and embedding updated successfully");
        Ok(tool)
    }

    // Updates a ShinkaiTool entry by generating a new embedding
    pub async fn update_tool(&self, tool: ShinkaiTool) -> Result<ShinkaiTool, SqliteManagerError> {
        // Generate or retrieve the embedding
        let embedding = match tool.get_embedding() {
            Some(embedding) => {
                println!("Using existing embedding");
                embedding.vector
            }
            None => {
                println!("Generating new embedding");
                self.generate_embeddings(&tool.format_embedding_string()).await?
            }
        };

        self.update_tool_with_vector(tool, embedding)
    }

    // Retrieves all ShinkaiTool entries from the shinkai_tools table
    pub fn get_all_tools(&self) -> Result<Vec<ShinkaiTool>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT tool_data FROM shinkai_tools")?;

        let tool_iter = stmt.query_map([], |row| {
            let tool_data: Vec<u8> = row.get(0)?;
            serde_json::from_slice(&tool_data).map_err(|e| {
                println!("Deserialization error: {}", e);
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })
        })?;

        let mut tools = Vec::new();
        for tool in tool_iter {
            tools.push(tool.map_err(|e| {
                println!("Database error: {}", e);
                SqliteManagerError::DatabaseError(e)
            })?);
        }

        Ok(tools)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_tools_primitives::tools::deno_tools::DenoTool;
    use shinkai_tools_primitives::tools::deno_tools::JSToolResult;
    use shinkai_vector_resources::embeddings::Embedding;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    // Utility function to generate a vector of length 384 filled with a specified value
    fn generate_vector(value: f32) -> Vec<f32> {
        vec![value; 384]
    }

    #[tokio::test]
    async fn test_add_deno_tool() {
        let manager = setup_test_db();

        // Create a DenoTool instance
        let deno_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Test Tool".to_string(),
            author: "Deno Author".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            config: vec![],
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: vec![],
            activated: true,
            embedding: None,
            result: JSToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output: "".to_string(),
        };

        // Wrap the DenoTool in a ShinkaiTool::Deno variant
        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);

        // Debug: Print the tool before adding
        println!("Testing add_tool with: {:?}", shinkai_tool);

        let vector = generate_vector(0.1);

        // Add the tool to the database
        let result = manager.add_tool_with_vector(shinkai_tool.clone(), vector);
        assert!(result.is_ok());

        // Retrieve the tool using the new method
        let retrieved_tool = manager.get_tool_by_key(&shinkai_tool.tool_router_key()).unwrap();

        // Assert that the retrieved tool matches the added tool
        assert_eq!(retrieved_tool.name(), shinkai_tool.name());
        assert_eq!(retrieved_tool.description(), shinkai_tool.description());
        assert_eq!(retrieved_tool.author(), shinkai_tool.author());
    }

    #[tokio::test]
    async fn test_tool_vector_search() {
        let manager = setup_test_db();

        // Create and add a DenoTool instance
        let deno_tool = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Test Tool".to_string(),
            author: "Deno Author".to_string(),
            js_code: "console.log('Hello, Deno!');".to_string(),
            config: vec![],
            description: "A Deno tool for testing".to_string(),
            keywords: vec!["deno".to_string(), "test".to_string()],
            input_args: vec![],
            activated: true,
            embedding: None,
            result: JSToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output: "".to_string(),
        };

        let shinkai_tool = ShinkaiTool::Deno(deno_tool, true);
        let vector = generate_vector(0.1);
        manager.add_tool_with_vector(shinkai_tool.clone(), vector).unwrap();

        // Generate an embedding vector for the query
        let embedding_query = generate_vector(0.09);

        // Perform a vector search using the generated embedding
        let num_results = 1;
        let search_results = manager
            .tool_vector_search_with_vector(embedding_query, num_results)
            .unwrap();

        // Assert that the search results contain the added tool
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].name(), "Deno Test Tool");
    }

    #[tokio::test]
    async fn test_update_middle_tool() {
        let manager = setup_test_db();

        // Create three DenoTool instances
        let deno_tool_1 = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Tool 1".to_string(),
            author: "Author 1".to_string(),
            js_code: "console.log('Tool 1');".to_string(),
            config: vec![],
            description: "First Deno tool".to_string(),
            keywords: vec!["deno".to_string(), "tool1".to_string()],
            input_args: vec![],
            activated: true,
            embedding: None,
            result: JSToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output: "".to_string(),
        };

        let deno_tool_2 = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Tool 2".to_string(),
            author: "Author 2".to_string(),
            js_code: "console.log('Tool 2');".to_string(),
            config: vec![],
            description: "Second Deno tool".to_string(),
            keywords: vec!["deno".to_string(), "tool2".to_string()],
            input_args: vec![],
            activated: true,
            embedding: None,
            result: JSToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output: "".to_string(),
        };

        let deno_tool_3 = DenoTool {
            toolkit_name: "Deno Toolkit".to_string(),
            name: "Deno Tool 3".to_string(),
            author: "Author 3".to_string(),
            js_code: "console.log('Tool 3');".to_string(),
            config: vec![],
            description: "Third Deno tool".to_string(),
            keywords: vec!["deno".to_string(), "tool3".to_string()],
            input_args: vec![],
            activated: true,
            embedding: None,
            result: JSToolResult::new("object".to_string(), serde_json::Value::Null, vec![]),
            output: "".to_string(),
        };

        // Wrap the DenoTools in ShinkaiTool::Deno variants
        let shinkai_tool_1 = ShinkaiTool::Deno(deno_tool_1, true);
        let shinkai_tool_2 = ShinkaiTool::Deno(deno_tool_2, true);
        let shinkai_tool_3 = ShinkaiTool::Deno(deno_tool_3, true);

        // Add the tools to the database
        manager
            .add_tool_with_vector(shinkai_tool_1.clone(), generate_vector(0.1))
            .unwrap();
        manager
            .add_tool_with_vector(shinkai_tool_2.clone(), generate_vector(0.2))
            .unwrap();
        manager
            .add_tool_with_vector(shinkai_tool_3.clone(), generate_vector(0.3))
            .unwrap();

        // Print out the name and key for each tool in the database
        let all_tools = manager.get_all_tools().unwrap();
        for tool in &all_tools {
            eprintln!("Tool name: {}, Tool key: {}", tool.name(), tool.tool_router_key());
        }

        // Update the second tool without changing the name
        let mut updated_tool_2 = shinkai_tool_2.clone();
        if let ShinkaiTool::Deno(ref mut deno_tool, _) = updated_tool_2 {
            deno_tool.description = "Updated second Deno tool".to_string();
            deno_tool.embedding = Some(Embedding::new("test", generate_vector(0.21)));
        }
        eprintln!("Updating tool: {:?}", updated_tool_2);

        manager.update_tool(updated_tool_2.clone()).await.unwrap();

        // Retrieve the updated tool and verify the changes
        let retrieved_tool = manager.get_tool_by_key(&updated_tool_2.tool_router_key()).unwrap();
        assert_eq!(retrieved_tool.name(), "Deno Tool 2");
        assert_eq!(retrieved_tool.description(), "Updated second Deno tool");
    }
}
