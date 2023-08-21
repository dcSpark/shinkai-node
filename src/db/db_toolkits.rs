use super::db::ProfileBoundWriteBatch;
use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::tools::error::ToolError;
use crate::tools::js_toolkit::{InstalledJSToolkitMap, JSToolkit, JSToolkitInfo};
use rocksdb::IteratorMode;
use rocksdb::{Error, Options};
use serde_json::{from_str, to_string};
use shinkai_message_wasm::schemas::shinkai_name::ShinkaiName;

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

    /// Saves the `InstalledJSToolkitMap` into the database
    fn _save_profile_toolkit_map(
        &self,
        toolkit_map: &InstalledJSToolkitMap,
        profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let (bytes, cf) = self._prepare_profile_toolkit_map(toolkit_map, profile)?;
        self.put_cf_pb(cf, &InstalledJSToolkitMap::db_key(), bytes, profile)?;
        Ok(())
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

    /// Fetches the `InstalledJSToolkitMap` from the DB (for the provided profile)
    pub fn get_installed_toolkit_map(&self, profile: &ShinkaiName) -> Result<InstalledJSToolkitMap, ShinkaiDBError> {
        let bytes = self.get_cf_pb(Topic::Toolkits, &InstalledJSToolkitMap::db_key(), profile)?;
        let json_str = std::str::from_utf8(&bytes)?;

        let toolkit_map: InstalledJSToolkitMap = from_str(json_str)?;
        Ok(toolkit_map)
    }

    /// Uninstalls a JSToolkit based on its name, and removes it from the profile-wide Installed Toolkit List.
    /// Note, any Toolkit headers (ie. API keys) will not be removed, and will stay in the DB.
    fn uninstall_toolkit(&self, toolkit_name: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // ...
        Ok(())
    }

    /// Installs a provided JSToolkit, and saving it to the profile-wide Installed Toolkit List.
    /// The toolkit will be set as inactive and will require activating to be used.
    ///
    /// If an existing toolkit has the same name/version, this function will error.
    /// If an existing toolkit has same name but a different version (higher or lower), the old one will be replaced.
    pub fn install_toolkit(&self, toolkit: JSToolkit, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        self.install_toolkits(vec![toolkit], profile)
    }

    /// Installs the provided JSToolkits, and saving them to the profile-wide Installed Toolkit List.
    /// The toolkits will be set as inactive and will require activating to be used.
    ///
    /// If an existing toolkit has the same name/version, this function will error.
    /// If an existing toolkit has same name but a different version (higher or lower), the old one will be replaced.
    pub fn install_toolkits(&self, toolkits: Vec<JSToolkit>, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Get the toolkit map
        let mut toolkit_map = self.get_installed_toolkit_map(profile)?;

        // For each toolkit, save the toolkit itself, and add the info to the map
        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;
        for toolkit in toolkits {
            // Check if an equivalent version of the toolkit is already installed
            if self.check_equivalent_toolkit_version_installed(&toolkit, profile)? {
                return Err(ToolError::ToolkitVersionAlreadyInstalled(toolkit.name, toolkit.version))?;
            }
            // Check if the toolkit is installed with a different version
            if self.check_if_toolkit_installed(&toolkit, profile)? {
                // If a different version of the toolkit is installed, uninstall it
                self.uninstall_toolkit(&toolkit.name, profile)?;
            }

            // Saving the toolkit itself
            let (bytes, cf) = self._prepare_toolkit(&toolkit, profile)?;
            pb_batch.put_cf_pb(cf, &toolkit.db_key(), &bytes);

            // Add the toolkit info to the map
            let toolkit_info = JSToolkitInfo::from(&toolkit);
            toolkit_map.add_toolkit_info(&toolkit_info);
        }

        // Finally save the toolkit map
        let (bytes, cf) = self._prepare_profile_toolkit_map(&toolkit_map, profile)?;
        pb_batch.put_cf_pb(cf, &InstalledJSToolkitMap::db_key(), &bytes);

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

    /// Initializes a `InstalledJSToolkitMap` if one does not exist in the DB.
    pub fn init_profile_toolkit_map(&self, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        if let Err(_) = self.get_installed_toolkit_map(profile) {
            let toolkit_map = InstalledJSToolkitMap::new();
            self._save_profile_toolkit_map(&toolkit_map, profile)?;
        }
        Ok(())
    }
}
