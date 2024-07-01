use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::tools::{js_toolkit::JSToolkit, js_tools::JSTool, shinkai_tool::ShinkaiTool};
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

// Only used for storage in the database
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JSToolkitWithToolReferences {
    pub toolkit: JSToolkit,
    pub tool_keys: Vec<String>,
}

impl ShinkaiDB {
    /// Returns the first half of the blake3 hash of the folder name value
    fn user_profile_to_half_hash(profile: ShinkaiName) -> String {
        let full_hash = blake3::hash(profile.full_name.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    /// Adds a ShinkaiTool to the database under the Toolkits topic.
    pub fn add_shinkai_tool(&self, tool: ShinkaiTool, profile: ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Verify that the tool is of type JS
        match tool {
            ShinkaiTool::JS(_) => {}
            _ => {
                return Err(ShinkaiDBError::InvalidToolType(
                    "Only JS tools can be added".to_string(),
                ))
            }
        }

        // Generate the key for the tool using tool_router_key
        let key = format!(
            "user_ts_tools_{}_{}",
            Self::user_profile_to_half_hash(profile),
            tool.tool_router_key()
        );

        // Serialize the tool to bytes
        let tool_bytes = bincode::serialize(&tool).expect("Failed to serialize tool");

        // Use shared CFs
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Create a write batch and add the tool to the batch
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_toolkits, key.as_bytes(), &tool_bytes);

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }

    /// Removes a ShinkaiTool from the database for the given profile and tool key.
    pub fn remove_shinkai_tool(&self, tool_key: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Generate the key for the tool using tool_router_key
        let key = format!(
            "user_ts_tools_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            tool_key
        );

        // Use shared CFs
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Create a write batch and delete the tool from the batch
        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(cf_toolkits, key.as_bytes());

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }

    /// Reads and returns a ShinkaiTool from the database for the given profile and tool key.
    pub fn get_shinkai_tool(&self, tool_key: &str, profile: &ShinkaiName) -> Result<ShinkaiTool, ShinkaiDBError> {
        // Generate the key for the tool using tool_router_key
        let key = format!(
            "user_ts_tools_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            tool_key
        );

        // Use shared CFs
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Fetch the tool bytes from the database
        let tool_bytes = self
            .db
            .get_cf(cf_toolkits, key.as_bytes())?
            .ok_or_else(|| ShinkaiDBError::ToolNotFound(format!("Tool not found for key: {}", tool_key)))?;

        // Deserialize the tool from bytes
        let tool: ShinkaiTool = bincode::deserialize(&tool_bytes)
            .map_err(|_| ShinkaiDBError::DeserializationFailed("Failed to deserialize tool".to_string()))?;

        Ok(tool)
    }

    /// Retrieves all ShinkaiTools for a given user profile.
    pub fn all_tools_for_user(&self, profile: &ShinkaiName) -> Result<Vec<ShinkaiTool>, ShinkaiDBError> {
        let profile_hash = Self::user_profile_to_half_hash(profile.clone());
        let prefix_search_key = format!("user_ts_tools_{}_", profile_hash);
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        let mut tools = Vec::new();

        let iterator = self.db.prefix_iterator_cf(cf_toolkits, prefix_search_key.as_bytes());

        for item in iterator {
            let (_, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let tool: ShinkaiTool = bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;

            tools.push(tool);
        }

        Ok(tools)
    }

    /// Removes all JSToolkits and their tools for a specific user profile.
    pub fn remove_all_toolkits_for_user(&self, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        let profile_hash = Self::user_profile_to_half_hash(profile.clone());
        let prefix_search_key = format!("user_toolkits_{}_", profile_hash);
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        let iterator = self.db.prefix_iterator_cf(cf_toolkits, prefix_search_key.as_bytes());

        let mut batch = rocksdb::WriteBatch::default();

        for item in iterator {
            let (key, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let toolkit: JSToolkit = bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;

            // Remove each tool in the toolkit
            for tool in &toolkit.tools {
                let shinkai_tool = ShinkaiTool::JS(tool.clone());
                let tool_key = format!("user_ts_tools_{}_{}", profile_hash, shinkai_tool.tool_router_key());
                batch.delete_cf(cf_toolkits, tool_key.as_bytes());
            }

            // Remove the toolkit itself
            batch.delete_cf(cf_toolkits, &key);
        }

        self.db.write(batch)?;

        Ok(())
    }

    /// Activates a JSTool for a given profile.
    pub fn activate_jstool(&self, tool_key: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        let mut tool = self.get_shinkai_tool(tool_key, profile)?;
        if let ShinkaiTool::JS(ref mut js_tool) = tool {
            if !js_tool.activated {
                js_tool.activated = true;
                self.add_shinkai_tool(tool, profile.clone())?;
            }
        } else {
            return Err(ShinkaiDBError::ToolNotFound(format!(
                "Tool not found for key: {}",
                tool_key
            )));
        }

        Ok(())
    }

    /// Deactivates a JSTool for a given profile.
    pub fn deactivate_jstool(&self, tool_key: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        let mut tool = self.get_shinkai_tool(tool_key, profile)?;
        if let ShinkaiTool::JS(ref mut js_tool) = tool {
            if js_tool.activated {
                js_tool.activated = false;
                self.add_shinkai_tool(tool, profile.clone())?;
            }
        } else {
            return Err(ShinkaiDBError::ToolNotFound(format!(
                "Tool not found for key: {}",
                tool_key
            )));
        }

        Ok(())
    }

    /// Adds a JSToolkit to the database under the Toolkits topic.
    pub fn add_jstoolkit(&self, toolkit: JSToolkit, profile: ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Add each tool in the toolkit
        for tool in &toolkit.tools {
            let shinkai_tool = ShinkaiTool::JS(tool.clone());
            self.add_shinkai_tool(shinkai_tool, profile.clone())?;
        }

        // Create JSToolkitWithToolReferences
        let toolkit_with_references = JSToolkitWithToolReferences {
            toolkit: JSToolkit {
                name: toolkit.name.clone(),
                tools: Vec::new(), // Empty vec as requested
                author: toolkit.author.clone(),
                version: toolkit.version.clone(),
            },
            tool_keys: toolkit
                .tools
                .iter()
                .map(|t| ShinkaiTool::gen_router_key(t.name.clone(), toolkit.name.clone()))
                .collect(),
        };

        // Serialize the toolkit with references
        let toolkit_bytes = bincode::serialize(&toolkit_with_references).expect("Failed to serialize toolkit");

        // Generate the key for the toolkit
        let key = format!(
            "user_toolkits_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            toolkit_with_references.toolkit.name
        );

        // Use shared CFs
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Create a write batch and add the toolkit to the batch
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_toolkits, key.as_bytes(), &toolkit_bytes);

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }

    /// Lists all JSToolkits for a specific user profile.
    pub fn list_toolkits_for_user(&self, profile: &ShinkaiName) -> Result<Vec<JSToolkit>, ShinkaiDBError> {
        let profile_hash = Self::user_profile_to_half_hash(profile.clone());
        let prefix_search_key = format!("user_toolkits_{}_", profile_hash);
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        let mut toolkits = Vec::new();

        let iterator = self.db.prefix_iterator_cf(cf_toolkits, prefix_search_key.as_bytes());

        for item in iterator {
            let (_, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let toolkit_with_references: JSToolkitWithToolReferences =
                bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;

            // Reconstruct the full toolkit by fetching each tool
            let full_tools = toolkit_with_references
                .tool_keys
                .into_iter()
                .map(|key| match self.get_shinkai_tool(&key, profile)? {
                    ShinkaiTool::JS(full_tool) => Ok(full_tool),
                    _ => Err(ShinkaiDBError::InvalidToolType("Expected JS tool".to_string())),
                })
                .collect::<Result<Vec<JSTool>, ShinkaiDBError>>()?;

            let full_toolkit = JSToolkit {
                name: toolkit_with_references.toolkit.name,
                tools: full_tools,
                author: toolkit_with_references.toolkit.author,
                version: toolkit_with_references.toolkit.version,
            };

            toolkits.push(full_toolkit);
        }

        Ok(toolkits)
    }

    /// Gets a specific JSToolkit for a user profile.
    pub fn get_toolkit(&self, toolkit_name: &str, profile: &ShinkaiName) -> Result<JSToolkit, ShinkaiDBError> {
        let key = format!(
            "user_toolkits_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            toolkit_name
        );
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        let toolkit_bytes = self
            .db
            .get_cf(cf_toolkits, key.as_bytes())?
            .ok_or_else(|| ShinkaiDBError::ToolkitNotFound(format!("Toolkit not found for name: {}", toolkit_name)))?;

        let toolkit_with_references: JSToolkitWithToolReferences = bincode::deserialize(&toolkit_bytes)
            .map_err(|e| ShinkaiDBError::DeserializationFailed(format!("Failed to deserialize toolkit: {}", e)))?;

        // Reconstruct the full toolkit by fetching each tool
        let full_tools = toolkit_with_references
            .tool_keys
            .into_iter()
            .map(|key| match self.get_shinkai_tool(&key, profile)? {
                ShinkaiTool::JS(full_tool) => Ok(full_tool),
                _ => Err(ShinkaiDBError::InvalidToolType("Expected JS tool".to_string())),
            })
            .collect::<Result<Vec<JSTool>, ShinkaiDBError>>()?;

        Ok(JSToolkit {
            name: toolkit_with_references.toolkit.name,
            tools: full_tools,
            author: toolkit_with_references.toolkit.author,
            version: toolkit_with_references.toolkit.version,
        })
    }

    /// Removes a JSToolkit and all of its tools from the database for the given profile and toolkit name.
    pub fn remove_jstoolkit(&self, toolkit_name: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Generate the key for the toolkit
        let toolkit_key = format!(
            "user_toolkits_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            toolkit_name
        );

        // Use shared CFs
        let cf_toolkits = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Fetch the toolkit to get its tool keys
        let toolkit_bytes = self
            .db
            .get_cf(cf_toolkits, toolkit_key.as_bytes())?
            .ok_or_else(|| ShinkaiDBError::ToolkitNotFound(format!("Toolkit not found for name: {}", toolkit_name)))?;

        let toolkit_with_references: JSToolkitWithToolReferences = bincode::deserialize(&toolkit_bytes)
            .map_err(|_| ShinkaiDBError::DeserializationFailed("Failed to deserialize toolkit".to_string()))?;

        // Remove each tool in the toolkit using remove_shinkai_tool
        for tool_key in &toolkit_with_references.tool_keys {
            self.remove_shinkai_tool(tool_key, profile)?;
        }

        // Create a write batch to remove the toolkit itself
        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(cf_toolkits, toolkit_key.as_bytes());

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }
}
