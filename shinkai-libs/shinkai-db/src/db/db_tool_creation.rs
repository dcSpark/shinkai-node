use shinkai_tools_primitives::tools::playground_tool::PlaygroundTool;

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

impl ShinkaiDB {
    /// Saves a PlaygroundTool to the database.
    pub fn save_playground_tool(&self, tool: PlaygroundTool) -> Result<(), ShinkaiDBError> {
        if tool.tool_router_key.is_none() {
            return Err(ShinkaiDBError::ToolNotFound("Tool router key is required".to_string()));
        }

        let half_hash = Self::hex_blake3_to_half_hash(&tool.tool_router_key.clone().unwrap());

        // Generate the key for the tool using the job_id
        let key = format!("playgroundtool_{}", half_hash);

        // Serialize the tool to bytes using serde_json
        let tool_bytes = serde_json::to_vec(&tool).expect("Failed to serialize PlaygroundTool");

        // Use shared CFs
        let cf_tools = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Create a write batch and add the tool to the batch
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_tools, key.as_bytes(), &tool_bytes);

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }

    /// Removes a PlaygroundTool from the database for the given job_id.
    pub fn remove_playground_tool(&self, tool_key: &str) -> Result<(), ShinkaiDBError> {
        // Generate the key for the tool using the job_id
        let key = format!("playgroundtool_{}", tool_key);

        // Use shared CFs
        let cf_tools = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Create a write batch and delete the tool from the batch
        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(cf_tools, key.as_bytes());

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }

    /// Lists all PlaygroundTools.
    pub fn list_all_playground_tools(&self) -> Result<Vec<PlaygroundTool>, ShinkaiDBError> {
        let prefix_search_key = "playgroundtool_";
        let cf_tools = self.get_cf_handle(Topic::Toolkits).unwrap();

        let mut tools = Vec::new();

        let iterator = self.db.prefix_iterator_cf(cf_tools, prefix_search_key.as_bytes());

        for item in iterator {
            match item {
                Ok((_, value)) => {
                    if let Ok(tool) = serde_json::from_slice::<PlaygroundTool>(&value) {
                        tools.push(tool);
                    } else {
                        eprintln!("Failed to deserialize PlaygroundTool, ignoring entry.");
                    }
                }
                Err(e) => {
                    eprintln!("Error iterating over database entries: {:?}", e);
                }
            }
        }

        Ok(tools)
    }

    /// Gets a specific PlaygroundTool by job_id.
    pub fn get_playground_tool(&self, tool_key: &str) -> Result<PlaygroundTool, ShinkaiDBError> {
        // Generate the key for the tool using the job_id
        let key = format!("playgroundtool_{}", tool_key);

        // Use shared CFs
        let cf_tools = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Fetch the tool bytes from the database
        let tool_bytes = self
            .db
            .get_cf(cf_tools, key.as_bytes())?
            .ok_or_else(|| ShinkaiDBError::ToolNotFound(format!("Tool not found for tool_key: {}", tool_key)))?;

        // Deserialize the tool from bytes using serde_json
        let tool: PlaygroundTool = serde_json::from_slice(&tool_bytes)
            .map_err(|_| ShinkaiDBError::DeserializationFailed("Failed to deserialize PlaygroundTool".to_string()))?;

        Ok(tool)
    }
}
