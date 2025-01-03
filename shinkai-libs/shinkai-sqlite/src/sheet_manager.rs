use rusqlite::params;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_sheet::sheet::Sheet;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    /// Returns the first half of the blake3 hash of the folder name value
    pub fn user_profile_to_half_hash(profile: ShinkaiName) -> String {
        let full_hash = blake3::hash(profile.full_name.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    pub fn save_sheet(&self, sheet: Sheet, profile: ShinkaiName) -> Result<(), SqliteManagerError> {
        let profile_hash = Self::user_profile_to_half_hash(profile);

        let sheet_bytes = serde_json::to_vec(&sheet).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;

        let conn = self.get_connection()?;
        let mut stmt = conn
            .prepare("INSERT OR REPLACE INTO shinkai_sheets (profile_hash, sheet_uuid, sheet_data) VALUES (?, ?, ?)")?;
        stmt.execute(params![profile_hash, sheet.uuid, sheet_bytes])?;

        Ok(())
    }

    pub fn remove_sheet(&self, sheet_uuid: &str, profile: &ShinkaiName) -> Result<(), SqliteManagerError> {
        let profile_hash = Self::user_profile_to_half_hash(profile.clone());

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM shinkai_sheets WHERE profile_hash = ? AND sheet_uuid = ?")?;
        stmt.execute(params![profile_hash, sheet_uuid])?;

        Ok(())
    }

    pub fn list_all_sheets_for_user(&self, profile: &ShinkaiName) -> Result<Vec<Sheet>, SqliteManagerError> {
        let profile_hash = Self::user_profile_to_half_hash(profile.clone());

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT sheet_data FROM shinkai_sheets WHERE profile_hash = ?")?;

        let sheets = stmt.query_map(params![profile_hash], |row| {
            let sheet_data: Vec<u8> = row.get(0)?;
            let sheet: Sheet = serde_json::from_slice(&sheet_data).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            Ok(sheet)
        })?;

        let mut result = Vec::new();
        for sheet in sheets {
            result.push(sheet?);
        }

        Ok(result)
    }

    pub fn get_sheet(&self, sheet_uuid: &str, profile: &ShinkaiName) -> Result<Sheet, SqliteManagerError> {
        let profile_hash = Self::user_profile_to_half_hash(profile.clone());

        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT sheet_data FROM shinkai_sheets WHERE profile_hash = ? AND sheet_uuid = ?")?;

        let sheet_data: Vec<u8> = stmt.query_row(params![profile_hash, sheet_uuid], |row| row.get(0))?;
        let sheet: Sheet = serde_json::from_slice(&sheet_data).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;

        Ok(sheet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
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

    #[test]
    fn test_save_sheet() {
        let db = setup_test_db();
        let profile = ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap();
        let sheet1 = Sheet::new();
        let sheet2 = Sheet::new();

        db.save_sheet(sheet1.clone(), profile.clone()).unwrap();
        db.save_sheet(sheet2.clone(), profile.clone()).unwrap();

        let sheet = db.get_sheet(&sheet1.uuid, &profile).unwrap();
        assert_eq!(sheet1.uuid, sheet.uuid);

        let sheets = db.list_all_sheets_for_user(&profile).unwrap();
        assert_eq!(2, sheets.len());

        db.remove_sheet(&sheet1.uuid, &profile).unwrap();
        let sheets = db.list_all_sheets_for_user(&profile).unwrap();
        assert_eq!(1, sheets.len());
    }
}
