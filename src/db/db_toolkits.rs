use std::collections::HashMap;

use super::db::ProfileBoundWriteBatch;
use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::tools::error::ToolError;
use crate::tools::js_toolkit::{InstalledJSToolkitMap, JSToolkit, JSToolkitInfo};
use crate::tools::js_toolkit_executor::JSToolkitExecutor;
use crate::tools::router::{ShinkaiTool, ToolRouter};
use serde_json::from_str;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_vector_resources::embedding_generator::EmbeddingGenerator;

impl ShinkaiDB {
    /// Prepares the `JSToolkit` for saving into the ShinkaiDB.
    fn _prepare_toolkit(
        &self,
        toolkit: &JSToolkit,
        profile: &ShinkaiName,
    ) -> Result<(Vec<u8>, &rocksdb::ColumnFamily), ShinkaiDBError> {
        // Convert JSON to bytes for storage
        let json = toolkit.to_json()?;
        let bytes = json.as_bytes().to_vec(); // Clone the bytes here
        let cf = self.get_cf_handle(Topic::Toolkits)?;
        Ok((bytes, cf))
    }

    /// Prepares the `InstalledJSToolkitMap` for saving into the ShinkaiDB as the profile toolkits map.
    fn _prepare_profile_toolkit_map(
        &self,
        toolkit_map: &InstalledJSToolkitMap,
        profile: &ShinkaiName,
    ) -> Result<(Vec<u8>, &rocksdb::ColumnFamily), ShinkaiDBError> {
        // Convert JSON to bytes for storage
        let json = toolkit_map.to_json()?;
        let bytes = json.as_bytes().to_vec(); // Clone the bytes here
        let cf = self.get_cf_handle(Topic::Toolkits)?;
        Ok((bytes, cf))
    }

    /// Saves the `InstalledJSToolkitMap` into the database
    fn _save_profile_toolkit_map(
        &self,
        toolkit_map: &InstalledJSToolkitMap,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let (bytes, cf) = self._prepare_profile_toolkit_map(toolkit_map, profile)?;
        self.put_cf_pb(cf, &InstalledJSToolkitMap::shinkai_db_key(), bytes, profile)?;
        Ok(())
    }

    /// Prepares the `ToolRouter` for saving into the ShinkaiDB as the profile tool router.
    fn _prepare_profile_tool_router(
        &self,
        tool_router: &ToolRouter,
        profile: &ShinkaiName,
    ) -> Result<(Vec<u8>, &rocksdb::ColumnFamily), ShinkaiDBError> {
        // Convert JSON to bytes for storage
        let json = tool_router.to_json()?;
        let bytes = json.as_bytes().to_vec(); // Clone the bytes here
        let cf = self.get_cf_handle(Topic::Toolkits)?;
        Ok((bytes, cf))
    }

    /// Saves the `ToolRouter` into the database (overwriting the old saved instance)
    fn _save_profile_tool_router(&self, tool_router: &ToolRouter, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        let (bytes, cf) = self._prepare_profile_tool_router(tool_router, profile)?;
        self.put_cf_pb(cf, &ToolRouter::profile_router_shinkai_db_key(), bytes, profile)?;
        Ok(())
    }

    /// Fetches the `ToolRouter` from the DB (for the provided profile)
    pub fn get_tool_router(&self, profile: &ShinkaiName) -> Result<ToolRouter, ShinkaiDBError> {
        let bytes = self.get_cf_pb(Topic::Toolkits, &ToolRouter::profile_router_shinkai_db_key(), profile)?;
        let json_str = std::str::from_utf8(&bytes)?;

        let tool_router: ToolRouter = from_str(json_str)?;
        Ok(tool_router)
    }

    /// Fetches the `InstalledJSToolkitMap` from the DB (for the provided profile)
    pub fn get_installed_toolkit_map(&self, profile: &ShinkaiName) -> Result<InstalledJSToolkitMap, ShinkaiDBError> {
        let bytes = self.get_cf_pb(Topic::Toolkits, &InstalledJSToolkitMap::shinkai_db_key(), profile)?;
        let json_str = std::str::from_utf8(&bytes)?;

        let toolkit_map: InstalledJSToolkitMap = from_str(json_str)?;
        Ok(toolkit_map)
    }

    /// Fetches the `JSToolkit` from the DB (for the provided profile and toolkit name)
    pub fn get_toolkit(&self, toolkit_name: &str, profile: &ShinkaiName) -> Result<JSToolkit, ShinkaiDBError> {
        let key = JSToolkit::shinkai_db_key_from_name(toolkit_name);
        let bytes = self.get_cf_pb(Topic::Toolkits, &key, profile)?;
        let json_str = std::str::from_utf8(&bytes)?;

        let toolkit: JSToolkit = from_str(json_str)?;
        Ok(toolkit)
    }

    /// Uninstalls (and deactivates) a JSToolkit based on its name, and removes it from the profile-wide Installed Toolkit List.
    /// Note, any Toolkit headers (ie. API keys) will not be removed, and will stay in the DB.
    /// TODO: Make this atomic with a batch, not extremely important here due to ordering
    pub fn uninstall_toolkit(&self, toolkit_name: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        let mut toolkit_map = self.get_installed_toolkit_map(profile)?;
        // 1. Deactivate the toolkit if it is active (to remove tools from ToolRouter)
        if toolkit_map.get_toolkit_info(&toolkit_name)?.activated {
            self.deactivate_toolkit(toolkit_name, profile)?;
        }
        // 2. Delete toolkit from toolkit map
        toolkit_map.remove_toolkit_info(toolkit_name)?;
        self._save_profile_toolkit_map(&toolkit_map, profile)?;
        // 3. Delete toolkit itself from db
        let cf = self.get_cf_handle(Topic::Toolkits)?;
        self.delete_cf_pb(cf, &JSToolkit::shinkai_db_key_from_name(toolkit_name), profile)?;

        Ok(())
    }

    /// Activates a JSToolkit and then propagating the internal tools to the ToolRouter. Of note,
    /// this function validates the toolkit headers values are available in DB (or errors), thus after installing
    /// a toolkit they must be set before calling activate.
    pub fn activate_toolkit(
        &self,
        toolkit_name: &str,
        profile: &ShinkaiName,
        toolkit_executor: &JSToolkitExecutor,
        embedding_generator: Box<dyn EmbeddingGenerator>,
    ) -> Result<(), ShinkaiDBError> {
        // 1. Check if toolkit is active then error
        let mut toolkit_map = self.get_installed_toolkit_map(profile)?;
        if toolkit_map.get_toolkit_info(&toolkit_name)?.activated {
            return Err(ToolError::ToolkitAlreadyActivated(toolkit_name.to_string()))?;
        }

        // 2. Check that the toolkit headers are set and validate
        let toolkit = self.get_toolkit(toolkit_name, profile)?;
        let header_values = self.get_toolkit_header_values(toolkit_name, profile)?;
        toolkit_executor.submit_headers_validation_request(&toolkit.js_code, &header_values)?;

        // 3. Propagate the internal tools to the ToolRouter
        // TODO: Use a write batch for 3/4
        let mut tool_router = self.get_tool_router(profile)?;

        for tool in toolkit.tools {
            let js_tool = ShinkaiTool::JS(tool);
            let embedding = embedding_generator.generate_embedding_default(&js_tool.format_embedding_string())?;
            tool_router.add_shinkai_tool(&js_tool, embedding)?;
        }

        self._save_profile_tool_router(&tool_router, profile)?;

        // 4. Set toolkit info in map to active == true
        toolkit_map.activate_toolkit(toolkit_name)?;
        self._save_profile_toolkit_map(&toolkit_map, profile)?;

        Ok(())
    }

    /// Deactivates a JSToolkit, removes its tools from the ToolRouter
    pub fn deactivate_toolkit(&self, toolkit_name: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // 1. Check if toolkit is deactivated then error
        let mut toolkit_map = self.get_installed_toolkit_map(profile)?;
        if !toolkit_map.get_toolkit_info(&toolkit_name)?.activated {
            return Err(ToolError::ToolkitAlreadyDeactivated(toolkit_name.to_string()))?;
        }

        // 2. Remove all of the toolkit's tools from the ToolRouter
        // TODO: Use a write batch for 2/3
        let toolkit = self.get_toolkit(toolkit_name, profile)?;
        let mut tool_router = self.get_tool_router(profile)?;
        for tool in toolkit.tools {
            tool_router.delete_shinkai_tool(&tool.name, toolkit_name)?;
        }
        self._save_profile_tool_router(&tool_router, profile)?;

        // 3. Set toolkit/info to active == false
        toolkit_map.deactivate_toolkit(toolkit_name)?;
        self._save_profile_toolkit_map(&toolkit_map, profile)?;

        Ok(())
    }

    /// Sets the toolkit's header values in the db (to be used when a tool in the toolkit is executed).
    /// Of note, this replaces any previous header values that were in the DB.
    pub fn set_toolkit_header_values(
        &self,
        toolkit_name: &str,
        profile: &ShinkaiName,
        header_values: &HashMap<String, String>,
        toolkit_executor: &JSToolkitExecutor,
    ) -> Result<(), ShinkaiDBError> {
        let toolkit = self.get_toolkit(toolkit_name, profile)?;
        // 1. Test the header values by using them with the validation function in the JS toolkit executor
        toolkit_executor.submit_headers_validation_request(&toolkit.js_code, &header_values)?;

        // 2. Validate that the header_values keys cover the header definitions in the toolkit.
        // If so, save the header values in the db
        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;
        for header in toolkit.header_definitions {
            let value_opt = header_values.get(&header.header());
            if let Some(value) = value_opt {
                let bytes = value.to_string().as_bytes().to_vec(); // Clone the bytes here
                let cf = self.get_cf_handle(Topic::Toolkits)?;
                pb_batch.put_cf_pb(cf, &header.shinkai_db_key(&toolkit_name), &bytes);
            } else {
                return Err(ToolError::JSToolkitHeaderValidationFailed(format!(
                    "Not all required header values have been provided while setting for toolkit: {}",
                    toolkit_name
                )))?;
            }
        }

        // 3. Write the batch to the DB
        self.write_pb(pb_batch)?;

        Ok(())
    }

    /// Fetches the toolkit's header values from the DB
    pub fn get_toolkit_header_values(
        &self,
        toolkit_name: &str,
        profile: &ShinkaiName,
    ) -> Result<HashMap<String, String>, ShinkaiDBError> {
        let toolkit = self.get_toolkit(toolkit_name, profile)?;
        let mut header_values = HashMap::new();

        for header in toolkit.header_definitions {
            let bytes = self.get_cf_pb(Topic::Toolkits, &header.shinkai_db_key(&toolkit_name), profile)?;
            let value = std::str::from_utf8(&bytes)?;
            header_values.insert(header.header().clone(), value.to_string());
        }

        Ok(header_values)
    }

    /// Installs a provided JSToolkit, and saving it to the profile-wide Installed Toolkit List.
    /// The toolkit will be set as inactive and will require activating to be used.
    ///
    /// If an existing toolkit has the same name/version, this function will error.
    /// If an existing toolkit has same name but a different version (higher or lower), the old one will be replaced.
    pub fn install_toolkit(&self, toolkit: &JSToolkit, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        self.install_toolkits(&vec![toolkit.clone()], profile)
    }

    /// Installs the provided JSToolkits, and saving them to the profile-wide Installed Toolkit List.
    /// The toolkits will be set as inactive and will require activating to be used.
    ///
    /// If an existing toolkit has the same name/version, this function will error.
    /// If an existing toolkit has same name but a different version (higher or lower), the old one will be replaced.
    pub fn install_toolkits(&self, toolkits: &Vec<JSToolkit>, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Get the toolkit map
        let mut toolkit_map = self.get_installed_toolkit_map(profile)?;

        // For each toolkit, save the toolkit itself, and add the info to the map
        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;
        for toolkit in toolkits {
            // Check if an equivalent version of the toolkit is already installed
            if self.check_equivalent_toolkit_version_installed(&toolkit, profile)? {
                return Err(ToolError::ToolkitVersionAlreadyInstalled(
                    toolkit.name.clone(),
                    toolkit.version.clone(),
                ))?;
            }
            // Check if the toolkit is installed with a different version
            if self.check_if_toolkit_installed(&toolkit, profile)? {
                // If a different version of the toolkit is installed, uninstall it
                self.uninstall_toolkit(&toolkit.name, profile)?;
            }

            // Saving the toolkit itself
            let (bytes, cf) = self._prepare_toolkit(&toolkit, profile)?;
            pb_batch.put_cf_pb(cf, &toolkit.shinkai_db_key(), &bytes);

            // Add the toolkit info to the map
            let toolkit_info = JSToolkitInfo::from(&toolkit.clone());
            toolkit_map.add_toolkit_info(&toolkit_info);
        }

        // Finally save the toolkit map
        let (bytes, cf) = self._prepare_profile_toolkit_map(&toolkit_map, profile)?;
        pb_batch.put_cf_pb(cf, &InstalledJSToolkitMap::shinkai_db_key(), &bytes);

        // Write the batch
        self.write_pb(pb_batch)?;

        Ok(())
    }

    /// Checks if the provided toolkit is installed
    pub fn check_if_toolkit_installed(
        &self,
        toolkit: &JSToolkit,
        profile: &ShinkaiName,
    ) -> Result<bool, ShinkaiDBError> {
        // Fetch the installed toolkit map for the given profile
        let toolkit_map = self.get_installed_toolkit_map(profile)?;

        // Check if a toolkit with the same name exists
        if toolkit_map.get_toolkit_info(&toolkit.name).is_ok() {
            // If a toolkit with the same name exists, return true
            return Ok(true);
        }

        // If no matching toolkit was found, return false
        Ok(false)
    }

    /// Checks if the provided toolkit is installed and has the same version
    pub fn check_equivalent_toolkit_version_installed(
        &self,
        toolkit: &JSToolkit,
        profile: &ShinkaiName,
    ) -> Result<bool, ShinkaiDBError> {
        // Fetch the installed toolkit map for the given profile
        let toolkit_map = self.get_installed_toolkit_map(profile)?;

        // Check if a toolkit with the same name exists
        if let Ok(existing_toolkit) = toolkit_map.get_toolkit_info(&toolkit.name) {
            // If the existing toolkit has the same version, return true
            if existing_toolkit.version == toolkit.version {
                return Ok(true);
            }
        }

        // If no matching toolkit was found, return false
        Ok(false)
    }

    /// Initializes a `InstalledJSToolkitMap` and a `ToolRouter` if they do not exist in the DB.
    pub fn init_profile_tool_structs(&self, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        if let Err(_) = self.get_installed_toolkit_map(profile) {
            let toolkit_map = InstalledJSToolkitMap::new();
            self._save_profile_toolkit_map(&toolkit_map, profile)?;
        }
        if let Err(_) = self.get_tool_router(profile) {
            let router = ToolRouter::new();
            self._save_profile_tool_router(&router, profile)?;
        }
        Ok(())
    }
}
