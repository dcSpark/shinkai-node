use super::{db_errors::ShinkaiDBError, db_main::Topic, ShinkaiDB};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_sheet::sheet::Sheet;

impl ShinkaiDB {
    /// Saves a Sheet to the database under the Sheets topic.
    pub fn save_sheet(&self, sheet: Sheet, profile: ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Generate the key for the sheet using the profile and sheet's uuid
        let key = format!(
            "useragentsheets_{}_{}",
            Self::user_profile_to_half_hash(profile),
            sheet.uuid
        );

        // Serialize the sheet to bytes using serde_json
        let sheet_bytes = serde_json::to_vec(&sheet).expect("Failed to serialize sheet");

        // Use shared CFs
        let cf_sheets = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Create a write batch and add the sheet to the batch
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_sheets, key.as_bytes(), &sheet_bytes);

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }

    /// Removes a Sheet from the database for the given profile and sheet uuid.
    pub fn remove_sheet(&self, sheet_uuid: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Generate the key for the sheet using the profile and sheet uuid
        let key = format!(
            "useragentsheets_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            sheet_uuid
        );

        // Use shared CFs
        let cf_sheets = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Create a write batch and delete the sheet from the batch
        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(cf_sheets, key.as_bytes());

        // Write the batch to the database
        self.db.write(batch)?;

        Ok(())
    }

    /// Lists all Sheets for a specific user profile.
    pub fn list_all_sheets_for_user(&self, profile: &ShinkaiName) -> Result<Vec<Sheet>, ShinkaiDBError> {
        let profile_hash = Self::user_profile_to_half_hash(profile.clone());
        let prefix_search_key = format!("useragentsheets_{}_", profile_hash);
        let cf_sheets = self.get_cf_handle(Topic::Toolkits).unwrap();

        let mut sheets = Vec::new();

        let iterator = self.db.prefix_iterator_cf(cf_sheets, prefix_search_key.as_bytes());

        for item in iterator {
            let (_, value) = item.map_err(ShinkaiDBError::RocksDBError)?;
            let sheet: Sheet = serde_json::from_slice(&value).map_err(ShinkaiDBError::JsonSerializationError)?;

            sheets.push(sheet);
        }

        Ok(sheets)
    }

    /// Gets a specific Sheet for a user profile.
    pub fn get_sheet(&self, sheet_uuid: &str, profile: &ShinkaiName) -> Result<Sheet, ShinkaiDBError> {
        // Generate the key for the sheet using the profile and sheet uuid
        let key = format!(
            "useragentsheets_{}_{}",
            Self::user_profile_to_half_hash(profile.clone()),
            sheet_uuid
        );

        // Use shared CFs
        let cf_sheets = self.get_cf_handle(Topic::Toolkits).unwrap();

        // Fetch the sheet bytes from the database
        let sheet_bytes = self
            .db
            .get_cf(cf_sheets, key.as_bytes())?
            .ok_or_else(|| ShinkaiDBError::SheetNotFound(format!("Sheet not found for uuid: {}", sheet_uuid)))?;

        // Deserialize the sheet from bytes using serde_json
        let sheet: Sheet = serde_json::from_slice(&sheet_bytes)
            .map_err(|_| ShinkaiDBError::DeserializationFailed("Failed to deserialize sheet".to_string()))?;

        Ok(sheet)
    }
}