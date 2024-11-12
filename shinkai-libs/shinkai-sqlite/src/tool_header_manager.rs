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

    // // Retrieves ShinkaiTool entries based on optional filters
    // pub fn get_tools(&self, name: Option<&str>, enabled: Option<bool>) -> Result<Vec<ShinkaiTool>> {
    //     let conn = self.get_connection()?;
    //     let mut query = "SELECT * FROM shinkai_tools WHERE 1=1".to_string();
    //     let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

    //     if let Some(n) = name {
    //         query.push_str(" AND name = ?");
    //         params.push(Box::new(n.to_string()));
    //     }
    //     if let Some(en) = enabled {
    //         query.push_str(" AND is_enabled = ?");
    //         params.push(Box::new(en as i32));
    //     }

    //     // Debug: Print the query
    //     println!("Executing query: {}", query);

    //     let mut stmt = conn.prepare(&query)?;
    //     let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    //     let tool_iter = stmt.query_map(param_refs.as_slice(), |row| {
    //         // Debug: Print the row data
    //         println!("Retrieved row: {:?}", row);

    //         Ok(ShinkaiTool {
    //             name: row.get(0)?,
    //             description: row.get(1)?,
    //             tool_router_key: row.get(2)?,
    //             toolkit_name: row.get(3)?,
    //             tool_data: row.get(4)?,
    //             tool_type: row.get(5)?,
    //             formatted_tool_summary_for_ui: row.get(6)?,
    //             author: row.get(7)?,
    //             version: row.get(8)?,
    //             enabled: row.get::<_, i32>(9)? != 0,
    //             input_args: vec![], // Assuming input_args are serialized/deserialized elsewhere
    //             config: None,       // Assuming config is serialized/deserialized elsewhere
    //             usage_type: None,   // Assuming usage_type is serialized/deserialized elsewhere
    //             tool_offering: None, // Add this line to initialize the tool_offering field
    //         })
    //     })?;

    //     tool_iter.collect()
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_tools_primitives::tools::deno_tools::DenoTool;
    use shinkai_tools_primitives::tools::deno_tools::JSToolResult;
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

        let result = manager.add_tool_with_vector(shinkai_tool, vector);
        assert!(result.is_ok());
    }

    // #[test]
    // fn test_get_tools() {
    //     let manager = setup_test_db();
    //     let tool = ShinkaiTool {
    //         name: "Test Tool".to_string(),
    //         toolkit_name: "Test Toolkit".to_string(),
    //         description: "A tool for testing".to_string(),
    //         tool_router_key: "test_tool".to_string(),
    //         tool_type: "Network".to_string(),
    //         formatted_tool_summary_for_ui: "Test Tool Summary".to_string(),
    //         author: "Test Author".to_string(),
    //         version: "1.0".to_string(),
    //         enabled: true,
    //         input_args: vec![],
    //         config: None,
    //         usage_type: Some(UsageType::PerUse(ToolPrice::Free)),
    //         tool_offering: None,
    //     };

    //     manager.add_tool(&tool).unwrap();

    //     // Debug: Print the parameters for get_tools
    //     println!("Testing get_tools with name: {:?}, enabled: {:?}", Some("test_tool"), Some(true));

    //     let retrieved_tools = manager.get_tools(Some("test_tool"), Some(true)).unwrap();
    //     assert_eq!(retrieved_tools.len(), 1);
    //     assert_eq!(retrieved_tools[0].name, "Test Tool");
    // }
}
