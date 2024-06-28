use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::tools::{js_toolkit::JSToolkit, shinkai_tool::ShinkaiTool};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;

impl ShinkaiDB {
    /// Returns the first half of the blake3 hash of the folder name value
    fn user_profile_to_half_hash(profile: ShinkaiName) -> String {
        let full_hash = blake3::hash(profile.full_name.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    /// Adds a ShinkaiTool to the database under the Toolkits topic.
    pub fn add_shinkai_tool(&self, tool: ShinkaiTool, profile: ShinkaiName) -> Result<(), ShinkaiDBError> {
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

        // Serialize the toolkit to bytes
        let toolkit_bytes = bincode::serialize(&toolkit).expect("Failed to serialize toolkit");

        // Generate the key for the toolkit
        let key = format!(
            "user_toolkits_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            toolkit.name
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
            let toolkit: JSToolkit = bincode::deserialize(&value).map_err(ShinkaiDBError::BincodeError)?;

            toolkits.push(toolkit);
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

        let toolkit: JSToolkit = bincode::deserialize(&toolkit_bytes)
            .map_err(|_| ShinkaiDBError::DeserializationFailed("Failed to deserialize toolkit".to_string()))?;

        Ok(toolkit)
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

        // Fetch the toolkit to get its tools
        let toolkit_bytes = self
            .db
            .get_cf(cf_toolkits, toolkit_key.as_bytes())?
            .ok_or_else(|| ShinkaiDBError::ToolkitNotFound(format!("Toolkit not found for name: {}", toolkit_name)))?;

        let toolkit: JSToolkit = bincode::deserialize(&toolkit_bytes)
            .map_err(|_| ShinkaiDBError::DeserializationFailed("Failed to deserialize toolkit".to_string()))?;

        // Remove each tool in the toolkit using remove_shinkai_tool
        for tool in &toolkit.tools {
            let shinkai_tool = ShinkaiTool::JS(tool.clone());
            self.remove_shinkai_tool(&shinkai_tool.tool_router_key(), profile)?;
        }

        // Create a write batch to remove the toolkit itself
        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(cf_toolkits, toolkit_key.as_bytes());

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }
}

//     /// Saves the `ToolRouter` into the database (overwriting the old saved instance)
//     fn _save_profile_tool_router(&self, tool_router: &ToolRouter, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
//         // Convert JSON to bytes for storage
//         let json = tool_router.to_json()?;
//         let bytes = json.as_bytes().to_vec(); // Clone the bytes here
//         let cf = self.get_cf_handle(Topic::Toolkits)?;

//         self.pb_put_cf(cf, &ToolRouter::profile_router_shinkai_db_key(), bytes, profile)?;
//         Ok(())
//     }

//     /// Fetches the `ToolRouter` from the DB (for the provided profile)
//     pub fn get_tool_router(&self, profile: &ShinkaiName) -> Result<ToolRouter, ShinkaiDBError> {
//         let bytes = self.pb_topic_get(Topic::Toolkits, &ToolRouter::profile_router_shinkai_db_key(), profile)?;
//         let json_str = std::str::from_utf8(&bytes)?;

//         let tool_router: ToolRouter = from_str(json_str)?;
//         Ok(tool_router)
//     }

//     /// Updates the `ToolRouter` for the provided profile.
//     pub fn update_tool_router<F>(&self, profile: &ShinkaiName, update_fn: F) -> Result<(), ShinkaiDBError>
//     where
//         F: FnOnce(&mut ToolRouter) -> Result<(), ShinkaiDBError>,
//     {
//         // 1. Fetch the existing ToolRouter
//         let mut tool_router = self.get_tool_router(profile)?;

//         // 2. Apply the update function
//         update_fn(&mut tool_router)?;

//         // 3. Save the updated ToolRouter back to the database
//         self._save_profile_tool_router(&tool_router, profile)?;

//         Ok(())
//     }

//     /// Fetches the `JSToolkit` from the DB (for the provided profile and toolkit name)
//     pub fn get_toolkit(&self, toolkit_name: &str, profile: &ShinkaiName) -> Result<JSToolkit, ShinkaiDBError> {
//         let key = JSToolkit::shinkai_db_key_from_name(toolkit_name);
//         let bytes = self.pb_topic_get(Topic::Toolkits, &key, profile)?;
//         let json_str = std::str::from_utf8(&bytes)?;

//         let toolkit: JSToolkit = from_str(json_str)?;
//         Ok(toolkit)
//     }

//     /// Uninstalls (and deactivates) a JSToolkit based on its name, and removes it from the profile-wide Installed Toolkit List.
//     /// Note, any Toolkit headers (ie. API keys) will not be removed, and will stay in the DB.
//     /// TODO: Make this atomic with a batch, not extremely important here due to ordering
//     pub fn uninstall_toolkit(&self, toolkit_name: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
//         let mut toolkit_map = self.get_installed_toolkit_map(profile)?;
//         // 1. Deactivate the toolkit if it is active (to remove tools from ToolRouter)
//         if toolkit_map.get_toolkit_info(toolkit_name)?.activated {
//             self.deactivate_toolkit(toolkit_name, profile)?;
//         }
//         // 2. Delete toolkit from toolkit map
//         toolkit_map.remove_toolkit_info(toolkit_name)?;
//         self._save_profile_toolkit_map(&toolkit_map, profile)?;
//         // 3. Delete toolkit itself from db
//         let cf = self.get_cf_handle(Topic::Toolkits)?;
//         self.pb_delete_cf(cf, &JSToolkit::shinkai_db_key_from_name(toolkit_name), profile)?;

//         Ok(())
//     }

//     TODO: needs to move from JSToolkit to JSTool
//     / Activates a JSToolkit and then propagating the internal tools to the ToolRouter.
//     pub async fn activate_toolkit(
//         &self,
//         toolkit_name: &str,
//         profile: &ShinkaiName,
//         embedding_generator: Box<dyn EmbeddingGenerator>,
//     ) -> Result<(), ShinkaiDBError> {
//         // 1. Check if toolkit is active then error
//         let mut toolkit_map = self.get_installed_toolkit_map(profile)?;
//         if toolkit_map.get_toolkit_info(toolkit_name)?.activated {
//             return Err(ToolError::ToolkitAlreadyActivated(toolkit_name.to_string()))?;
//         }

//         // 2. Check that the toolkit headers are set and validate
//         let toolkit = self.get_toolkit(toolkit_name, profile)?;
//         let header_values = self.get_toolkit_header_values(toolkit_name, profile)?;
//         // TODO: connect header_values

//         // 3. Propagate the internal tools to the ToolRouter
//         // TODO: Use a write batch for 3/4
//         let mut tool_router = self.get_tool_router(profile)?;

//         for tool in toolkit.tools {
//             let js_tool = ShinkaiTool::JS(tool);
//             let embedding = embedding_generator
//                 .generate_embedding_default(&js_tool.format_embedding_string())
//                 .await?;
//             tool_router.add_shinkai_tool(&js_tool, embedding)?;
//         }

//         self._save_profile_tool_router(&tool_router, profile)?;

//         // 4. Set toolkit info in map to active == true
//         toolkit_map.activate_toolkit(toolkit_name)?;
//         self._save_profile_toolkit_map(&toolkit_map, profile)?;

//         Ok(())
//     }

//     TODO: needs to move from JSToolkit to JSTool
//     /// Deactivates a JSToolkit, removes its tools from the ToolRouter
//     pub fn deactivate_toolkit(&self, toolkit_name: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
//         // 1. Check if toolkit is deactivated then error
//         let mut toolkit_map = self.get_installed_toolkit_map(profile)?;
//         if !toolkit_map.get_toolkit_info(toolkit_name)?.activated {
//             return Err(ToolError::ToolkitAlreadyDeactivated(toolkit_name.to_string()))?;
//         }

//         // 2. Remove all of the toolkit's tools from the ToolRouter
//         // TODO: Use a write batch for 2/3
//         let toolkit = self.get_toolkit(toolkit_name, profile)?;
//         let mut tool_router = self.get_tool_router(profile)?;
//         for tool in toolkit.tools {
//             tool_router.delete_shinkai_tool(&tool.name, toolkit_name)?;
//         }
//         self._save_profile_tool_router(&tool_router, profile)?;

//         // 3. Set toolkit/info to active == false
//         toolkit_map.deactivate_toolkit(toolkit_name)?;
//         self._save_profile_toolkit_map(&toolkit_map, profile)?;

//         Ok(())
//     }

//     /// Sets the toolkit's header values in the db (to be used when a tool in the toolkit is executed).
//     /// Of note, this replaces any previous header values that were in the DB.
//     pub async fn set_toolkit_header_values(
//         &self,
//         toolkit_name: &str,
//         profile: &ShinkaiName,
//         header_values: &JsonValue,
//     ) -> Result<(), ShinkaiDBError> {
//         let toolkit = self.get_toolkit(toolkit_name, profile)?;

//         // 1. Validate that the header_values keys cover the header definitions in the toolkit.
//         // If so, save the header values in the db
//         let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;
//         for header in toolkit.header_definitions {
//             let value_opt = header_values.get(&header.header());
//             if let Some(value) = value_opt {
//                 let bytes = value.to_string().as_bytes().to_vec(); // Clone the bytes here
//                 pb_batch.pb_put_cf(Topic::Toolkits.as_str(), &header.shinkai_db_key(toolkit_name), &bytes);
//             } else {
//                 return Err(ToolError::JSToolkitHeaderValidationFailed(format!(
//                     "Not all required header values have been provided while setting for toolkit: {}",
//                     toolkit_name
//                 )))?;
//             }
//         }

//         // 2. Updates the headers_set of the toolkit in the map
//         let mut toolkit_map = self.get_installed_toolkit_map(profile)?;
//         toolkit_map.update_headers_set(toolkit_name, true)?;
//         let (bytes, cf) = self._prepare_profile_toolkit_map(&toolkit_map, profile)?;
//         pb_batch.pb_put_cf(cf, &InstalledJSToolkitMap::shinkai_db_key(), bytes);

//         // 3. Write the batch using rocksdb::WriteBatch
//         let mut batch = rocksdb::WriteBatch::default();
//         for op in pb_batch.operations {
//             match op {
//                 TransactionOperation::Write(cf_name, key, value) => {
//                     let cf_handle = self.db.cf_handle(&cf_name).ok_or(ShinkaiDBError::FailedFetchingCF)?;
//                     batch.put_cf(cf_handle, key.as_bytes(), &value);
//                 }
//                 TransactionOperation::Delete(cf_name, key) => {
//                     let cf_handle = self.db.cf_handle(&cf_name).ok_or(ShinkaiDBError::FailedFetchingCF)?;
//                     batch.delete_cf(cf_handle, key.as_bytes());
//                 }
//             }
//         }
//         self.db.write(batch)?;

//         eprintln!(
//             "set_toolkit_header_values> profile set header values: {:?}",
//             header_values
//         );

//         Ok(())
//     }

//     /// Fetches the toolkit's header values from the DB
//     pub fn get_toolkit_header_values(
//         &self,
//         toolkit_name: &str,
//         profile: &ShinkaiName,
//     ) -> Result<JsonValue, ShinkaiDBError> {
//         let toolkit = self.get_toolkit(toolkit_name, profile)?;
//         let mut header_values = serde_json::Map::new();

//         for header in toolkit.header_definitions {
//             let bytes = self.pb_topic_get(Topic::Toolkits, &header.shinkai_db_key(toolkit_name), profile)?;
//             let value_str = std::str::from_utf8(&bytes)?;
//             let value: JsonValue = serde_json::from_str(value_str)?;
//             header_values.insert(header.header().clone(), value);
//         }

//         Ok(JsonValue::Object(header_values))
//     }

//     / Installs a provided JSToolkit, and saving it to the profile-wide Installed Toolkit List.
//     / The toolkit will be set as inactive and will require activating to be used.
//     /
//     / If an existing toolkit has the same name/version, this function will error.
//     / If an existing toolkit has same name but a different version (higher or lower), the old one will be replaced.
//     pub fn install_toolkit(&self, toolkit: &JSToolkit, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
//         self.install_toolkits(&vec![toolkit.clone()], profile)
//     }

//     / Installs the provided JSToolkits, and saving them to the profile-wide Installed Toolkit List.
//     / The toolkits will be set as inactive and will require activating to be used.
//     /
//     / If an existing toolkit has the same name/version, this function will error.
//     / If an existing toolkit has same name but a different version (higher or lower), the old one will be replaced.
//     pub fn install_toolkits(&self, toolkits: &Vec<JSToolkit>, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
//         // Get the toolkit map
//         let mut toolkit_map = self.get_installed_toolkit_map(profile)?;

//         // For each toolkit, save the toolkit itself, and add the info to the map
//         let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;
//         for toolkit in toolkits {
//             // Check if an equivalent version of the toolkit is already installed
//             if self.check_equivalent_toolkit_version_installed(toolkit, profile)? {
//                 return Err(ToolError::ToolkitVersionAlreadyInstalled(
//                     toolkit.name.clone(),
//                     toolkit.version.clone(),
//                 ))?;
//             }
//             // Check if the toolkit is installed with a different version
//             if self.check_if_toolkit_installed(toolkit, profile)? {
//                 // If a different version of the toolkit is installed, uninstall it
//                 self.uninstall_toolkit(&toolkit.name, profile)?;
//             }

//             // Saving the toolkit itself
//             let (bytes, cf) = self._prepare_toolkit(toolkit, profile)?;
//             pb_batch.pb_put_cf(cf, &toolkit.shinkai_db_key(), &bytes);

//             // Add the toolkit info to the map
//             let toolkit_info = JSToolkitInfo::from(&toolkit.clone());
//             toolkit_map.add_toolkit_info(&toolkit_info);
//         }

//         // Finally save the toolkit map
//         let (bytes, cf) = self._prepare_profile_toolkit_map(&toolkit_map, profile)?;
//         pb_batch.pb_put_cf(cf, &InstalledJSToolkitMap::shinkai_db_key(), &bytes);

//         // Write the batch using rocksdb::WriteBatch
//         let mut batch = rocksdb::WriteBatch::default();
//         for op in pb_batch.operations {
//             match op {
//                 TransactionOperation::Write(cf_name, key, value) => {
//                     let cf_handle = self.db.cf_handle(&cf_name).ok_or(ShinkaiDBError::FailedFetchingCF)?;
//                     batch.put_cf(cf_handle, key.as_bytes(), &value);
//                 }
//                 TransactionOperation::Delete(cf_name, key) => {
//                     let cf_handle = self.db.cf_handle(&cf_name).ok_or(ShinkaiDBError::FailedFetchingCF)?;
//                     batch.delete_cf(cf_handle, key.as_bytes());
//                 }
//             }
//         }
//         self.db.write(batch)?;

//         Ok(())
//     }
//     pub fn check_if_toolkit_installed(
//         &self,
//         toolkit: &JSToolkit,
//         profile: &ShinkaiName,
//     ) -> Result<bool, ShinkaiDBError> {
//         // Fetch the installed toolkit map for the given profile
//         let toolkit_map = self.get_installed_toolkit_map(profile)?;

//         // Check if a toolkit with the same name exists
//         if toolkit_map.get_toolkit_info(&toolkit.name).is_ok() {
//             // If a toolkit with the same name exists, return true
//             return Ok(true);
//         }

//         // If no matching toolkit was found, return false
//         Ok(false)
//     }

//     /// Checks if the provided toolkit is installed and has the same version
//     pub fn check_equivalent_toolkit_version_installed(
//         &self,
//         toolkit: &JSToolkit,
//         profile: &ShinkaiName,
//     ) -> Result<bool, ShinkaiDBError> {
//         // Fetch the installed toolkit map for the given profile
//         let toolkit_map = self.get_installed_toolkit_map(profile)?;

//         // Check if a toolkit with the same name exists
//         if let Ok(existing_toolkit) = toolkit_map.get_toolkit_info(&toolkit.name) {
//             // If the existing toolkit has the same version, return true
//             if existing_toolkit.version == toolkit.version {
//                 return Ok(true);
//             }
//         }

//         // If no matching toolkit was found, return false
//         Ok(false)
//     }

//     /// Initializes a `InstalledJSToolkitMap` and a `ToolRouter` if they do not exist in the DB.
//     pub async fn init_profile_tool_structs(
//         &self,
//         profile: &ShinkaiName,
//         embedding_generator: Box<dyn EmbeddingGenerator>,
//         db: Arc<Mutex<ShinkaiDB>>,
//     ) -> Result<(), ShinkaiDBError> {
//         if let Err(_) = self.get_installed_toolkit_map(profile) {
//             let toolkit_map = InstalledJSToolkitMap::new();
//             self._save_profile_toolkit_map(&toolkit_map, profile)?;
//         }
//         if let Err(_) = self.get_tool_router(profile) {
//             let router = ToolRouter::new(embedding_generator, profile, db.clone()).await;
//             self._save_profile_tool_router(&router, profile)?;
//         }
//         Ok(())
//     }
// }
