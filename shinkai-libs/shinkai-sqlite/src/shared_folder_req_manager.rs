use rusqlite::params;
use shinkai_message_primitives::{
    schemas::shinkai_subscription_req::FolderSubscription,
    shinkai_message::shinkai_message_schemas::FileDestinationCredentials,
};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    // Folder requirements

    pub fn set_folder_requirements(
        &self,
        path: &str,
        subscription_requirement: FolderSubscription,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO folder_subscriptions_requirements (
                path,
                minimum_token_delegation,
                minimum_time_delegated_hours,
                monthly_payment,
                is_free,
                has_web_alternative,
                folder_description
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        stmt.execute(params![
            path,
            subscription_requirement.minimum_token_delegation,
            subscription_requirement.minimum_time_delegated_hours,
            serde_json::to_string(&subscription_requirement.monthly_payment).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            subscription_requirement.is_free,
            subscription_requirement.has_web_alternative,
            subscription_requirement.folder_description,
        ])?;

        Ok(())
    }

    pub fn get_folder_requirements(&self, path: &str) -> Result<FolderSubscription, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                minimum_token_delegation,
                minimum_time_delegated_hours,
                monthly_payment,
                is_free,
                has_web_alternative,
                folder_description
            FROM folder_subscriptions_requirements
            WHERE path = ?1",
        )?;

        let mut rows = stmt.query(params![path])?;
        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;

        Ok(FolderSubscription {
            minimum_token_delegation: row.get(0)?,
            minimum_time_delegated_hours: row.get(1)?,
            monthly_payment: serde_json::from_str(&row.get::<_, String>(2)?)?,
            is_free: row.get(3)?,
            has_web_alternative: row.get(4)?,
            folder_description: row.get(5)?,
        })
    }

    pub fn remove_folder_requirements(&self, path: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM folder_subscriptions_requirements WHERE path = ?1")?;
        stmt.execute(params![path])?;

        Ok(())
    }

    pub fn get_all_folder_requirements(&self) -> Result<Vec<(String, FolderSubscription)>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                path,
                minimum_token_delegation,
                minimum_time_delegated_hours,
                monthly_payment,
                is_free,
                has_web_alternative,
                folder_description
            FROM folder_subscriptions_requirements",
        )?;

        let mut rows = stmt.query([])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push((
                row.get(0)?,
                FolderSubscription {
                    minimum_token_delegation: row.get(1)?,
                    minimum_time_delegated_hours: row.get(2)?,
                    monthly_payment: serde_json::from_str(&row.get::<_, String>(3)?)?,
                    is_free: row.get(4)?,
                    has_web_alternative: row.get(5)?,
                    folder_description: row.get(6)?,
                },
            ));
        }

        Ok(results)
    }

    // Upload credentials
    pub fn set_upload_credentials(
        &self,
        path: &str,
        profile: &str,
        credentials: FileDestinationCredentials,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO folder_subscriptions_upload_credentials (
                path,
                profile,
                source,
                access_key_id,
                secret_access_key,
                endpoint_uri,
                bucket
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        stmt.execute(params![
            path,
            profile,
            serde_json::to_string(&credentials.source).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            credentials.access_key_id,
            credentials.secret_access_key,
            credentials.endpoint_uri,
            credentials.bucket,
        ])?;

        Ok(())
    }

    pub fn get_upload_credentials(
        &self,
        path: &str,
        profile: &str,
    ) -> Result<FileDestinationCredentials, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                source,
                access_key_id,
                secret_access_key,
                endpoint_uri,
                bucket
            FROM folder_subscriptions_upload_credentials
            WHERE path = ?1 AND profile = ?2",
        )?;

        let mut rows = stmt.query(params![path, profile])?;
        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;

        Ok(FileDestinationCredentials {
            source: serde_json::from_str(&row.get::<_, String>(0)?)?,
            access_key_id: row.get(1)?,
            secret_access_key: row.get(2)?,
            endpoint_uri: row.get(3)?,
            bucket: row.get(4)?,
        })
    }

    pub fn remove_upload_credentials(&self, path: &str, profile: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("DELETE FROM folder_subscriptions_upload_credentials WHERE path = ?1 AND profile = ?2")?;
        stmt.execute(params![path, profile])?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use shinkai_message_primitives::{
        schemas::shinkai_subscription_req::PaymentOption,
        shinkai_message::shinkai_message_schemas::FileDestinationSourceType,
    };
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::{self, PathBuf};
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
    fn test_folder_requirements() {
        let db = setup_test_db();

        let path1 = "test_path";
        let subscription_requirement1 = FolderSubscription {
            minimum_token_delegation: Some(10),
            minimum_time_delegated_hours: Some(20),
            monthly_payment: None,
            is_free: false,
            has_web_alternative: Some(true),
            folder_description: "test_description".to_string(),
        };

        let path2 = "test_path2";
        let subscription_requirement2 = FolderSubscription {
            minimum_token_delegation: Some(100),
            minimum_time_delegated_hours: Some(200),
            monthly_payment: Some(PaymentOption::USD(Decimal::new(10, 0))),
            is_free: true,
            has_web_alternative: Some(false),
            folder_description: "test_description2".to_string(),
        };

        db.set_folder_requirements(path1, subscription_requirement1.clone())
            .unwrap();
        db.set_folder_requirements(path2, subscription_requirement2.clone())
            .unwrap();

        let result = db.get_folder_requirements(path1).unwrap();
        assert_eq!(result, subscription_requirement1);

        let result = db.get_all_folder_requirements().unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, path1);
        assert_eq!(result[0].1, subscription_requirement1);
        assert_eq!(result[1].0, path2);
        assert_eq!(result[1].1, subscription_requirement2);

        db.remove_folder_requirements(path1).unwrap();

        let result = db.get_folder_requirements(path1);
        assert!(result.is_err());
    }

    #[test]
    fn test_upload_credentials() {
        let db = setup_test_db();

        let path = "test_path";
        let profile = "test_profile";
        let credentials = FileDestinationCredentials {
            source: FileDestinationSourceType::S3,
            access_key_id: "test_access_key_id".to_string(),
            secret_access_key: "test_secret_access_key".to_string(),
            endpoint_uri: "test_endpoint_uri".to_string(),
            bucket: "test_bucket".to_string(),
        };

        db.set_upload_credentials(path, profile, credentials.clone()).unwrap();

        let result = db.get_upload_credentials(path, profile).unwrap();
        assert_eq!(result, credentials);

        db.remove_upload_credentials(path, profile).unwrap();

        let result = db.get_upload_credentials(path, profile);
        assert!(result.is_err());
    }
}
