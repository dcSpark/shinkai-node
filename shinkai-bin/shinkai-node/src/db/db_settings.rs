use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

impl ShinkaiDB {
    /// Gets the local processing preference setting.
    /// If the setting does not exist, it returns true by default.
    pub fn get_local_processing_preference(&self) -> Result<bool, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"settings_local_processing_preference";
        
        match self.db.get_cf(cf, key)? {
            Some(value) => {
                let preference: bool = serde_json::from_slice(&value)?;
                Ok(preference)
            }
            None => Ok(false), // Note(change): Default to true if the setting does not exist
        }
    }

    /// Updates the local processing preference setting.
    pub fn update_local_processing_preference(&self, preference: bool) -> Result<(), ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"settings_local_processing_preference";
        let value = serde_json::to_vec(&preference)?;

        self.db.put_cf(cf, key, value)?;
        Ok(())
    } 
}
