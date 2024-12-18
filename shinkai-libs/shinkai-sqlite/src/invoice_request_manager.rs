use rusqlite::params;
use shinkai_message_primitives::schemas::{invoices::InternalInvoiceRequest, shinkai_name::ShinkaiName};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn set_internal_invoice_request(
        &self,
        internal_invoice_request: &InternalInvoiceRequest,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO invoice_requests (
                unique_id,
                provider_name,
                requester_name,
                tool_key_name,
                usage_type_inquiry,
                date_time,
                secret_prehash
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        stmt.execute(params![
            internal_invoice_request.unique_id,
            internal_invoice_request.provider_name.full_name,
            internal_invoice_request.requester_name.full_name,
            internal_invoice_request.tool_key_name,
            serde_json::to_string(&internal_invoice_request.usage_type_inquiry).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            internal_invoice_request.date_time.to_rfc3339(),
            internal_invoice_request.secret_prehash,
        ])?;

        Ok(())
    }

    pub fn get_internal_invoice_request(&self, unique_id: &str) -> Result<InternalInvoiceRequest, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                provider_name,
                requester_name,
                tool_key_name,
                usage_type_inquiry,
                date_time,
                secret_prehash
            FROM invoice_requests
            WHERE unique_id = ?1",
        )?;

        let mut rows = stmt.query(params![unique_id])?;
        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;

        Ok(InternalInvoiceRequest {
            unique_id: unique_id.to_string(),
            provider_name: ShinkaiName::new(row.get(0)?).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            requester_name: ShinkaiName::new(row.get(1)?).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            tool_key_name: row.get(2)?,
            usage_type_inquiry: serde_json::from_str(&row.get::<_, String>(3)?).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            date_time: row
                .get::<_, String>(4)?
                .parse::<chrono::DateTime<chrono::Utc>>()
                .map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                        e.to_string(),
                    )))
                })?,
            secret_prehash: row.get(5)?,
        })
    }

    pub fn get_all_internal_invoice_requests(&self) -> Result<Vec<InternalInvoiceRequest>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                unique_id,
                provider_name,
                requester_name,
                tool_key_name,
                usage_type_inquiry,
                date_time,
                secret_prehash
            FROM invoice_requests",
        )?;

        let mut rows = stmt.query([])?;
        let mut results = Vec::new();

        while let Some(row) = rows.next()? {
            results.push(InternalInvoiceRequest {
                unique_id: row.get(0)?,
                provider_name: ShinkaiName::new(row.get(1)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                requester_name: ShinkaiName::new(row.get(2)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                tool_key_name: row.get(3)?,
                usage_type_inquiry: serde_json::from_str(&row.get::<_, String>(4)?).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?,
                date_time: row
                    .get::<_, String>(5)?
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .map_err(|e| {
                        rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::DateTimeParseError(
                            e.to_string(),
                        )))
                    })?,
                secret_prehash: row.get(6)?,
            });
        }

        Ok(results)
    }

    pub fn remove_internal_invoice_request(&self, unique_id: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM invoice_requests WHERE unique_id = ?1")?;

        stmt.execute(params![unique_id])?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::schemas::{shinkai_name::ShinkaiName, shinkai_tool_offering::UsageTypeInquiry};
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
    fn test_set_and_get_internal_invoice_request() {
        let db = setup_test_db();

        let invoice_request = InternalInvoiceRequest {
            unique_id: "test_unique_id".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            tool_key_name: "test_tool_key_name".to_string(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            date_time: chrono::Utc::now(),
            secret_prehash: "secret_prehash".to_string(),
        };

        db.set_internal_invoice_request(&invoice_request).unwrap();

        let result = db.get_internal_invoice_request("test_unique_id").unwrap();

        assert_eq!(result, invoice_request);
    }

    #[test]
    fn test_get_all_internal_invoice_requests() {
        let db = setup_test_db();

        let invoice_request1 = InternalInvoiceRequest {
            unique_id: "test_unique_id1".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            tool_key_name: "test_tool_key_name".to_string(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            date_time: chrono::Utc::now(),
            secret_prehash: "secret_prehash".to_string(),
        };

        let invoice_request2 = InternalInvoiceRequest {
            unique_id: "test_unique_id2".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            tool_key_name: "test_tool_key_name".to_string(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            date_time: chrono::Utc::now(),
            secret_prehash: "secret_prehash".to_string(),
        };

        db.set_internal_invoice_request(&invoice_request1).unwrap();
        db.set_internal_invoice_request(&invoice_request2).unwrap();

        let result = db.get_all_internal_invoice_requests().unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&invoice_request1));
        assert!(result.contains(&invoice_request2));
    }

    #[test]
    fn test_remove_internal_invoice_request() {
        let db = setup_test_db();

        let invoice_request = InternalInvoiceRequest {
            unique_id: "test_unique_id".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            tool_key_name: "test_tool_key_name".to_string(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            date_time: chrono::Utc::now(),
            secret_prehash: "secret_prehash".to_string(),
        };

        db.set_internal_invoice_request(&invoice_request).unwrap();

        db.remove_internal_invoice_request("test_unique_id").unwrap();

        let result = db.get_internal_invoice_request("test_unique_id");

        assert!(result.is_err());
    }
}
