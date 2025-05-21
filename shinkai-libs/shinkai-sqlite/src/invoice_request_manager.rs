use rusqlite::params;
use chrono::{DateTime, Utc};
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
                date_time
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
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
                date_time
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
                date_time
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

    pub fn update_internal_invoice_request_unique_id(
        &self,
        old_unique_id: &str,
        new_unique_id: &str,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "UPDATE invoice_requests SET unique_id = ?1 WHERE unique_id = ?2",
        )?;

        let rows_updated = stmt.execute(params![new_unique_id, old_unique_id])?;
        if rows_updated == 0 {
            return Err(SqliteManagerError::DataNotFound);
        }
        Ok(())
    }

    pub fn get_internal_invoice_request_by_details(
        &self,
        provider_name: &ShinkaiName,
        requester_name: &ShinkaiName,
        tool_key_name: &str,
        date_time: DateTime<Utc>,
    ) -> Result<InternalInvoiceRequest, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT unique_id, usage_type_inquiry FROM invoice_requests WHERE provider_name = ?1 AND requester_name = ?2 AND tool_key_name = ?3 AND date_time = ?4",
        )?;

        let mut rows = stmt.query(params![
            provider_name.full_name,
            requester_name.full_name,
            tool_key_name,
            date_time.to_rfc3339(),
        ])?;

        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;

        Ok(InternalInvoiceRequest {
            provider_name: provider_name.clone(),
            requester_name: requester_name.clone(),
            tool_key_name: tool_key_name.to_string(),
            usage_type_inquiry: serde_json::from_str(&row.get::<_, String>(1)?).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(
                    SqliteManagerError::SerializationError(e.to_string()),
                ))
            })?,
            date_time,
            unique_id: row.get(0)?,
        })
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
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbedM);

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
        };

        let invoice_request2 = InternalInvoiceRequest {
            unique_id: "test_unique_id2".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            tool_key_name: "test_tool_key_name".to_string(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            date_time: chrono::Utc::now(),
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
        };

        db.set_internal_invoice_request(&invoice_request).unwrap();

        db.remove_internal_invoice_request("test_unique_id").unwrap();

        let result = db.get_internal_invoice_request("test_unique_id");

        assert!(result.is_err());
    }

    #[test]
    fn test_update_internal_invoice_request_unique_id() {
        let db = setup_test_db();

        let invoice_request = InternalInvoiceRequest {
            unique_id: "old_id".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            tool_key_name: "test_tool_key_name".to_string(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            date_time: chrono::Utc::now(),
        };

        db.set_internal_invoice_request(&invoice_request).unwrap();

        db.update_internal_invoice_request_unique_id("old_id", "new_id").unwrap();

        let updated = db.get_internal_invoice_request("new_id").unwrap();
        assert_eq!(updated.unique_id, "new_id");

        let old_result = db.get_internal_invoice_request("old_id");
        assert!(old_result.is_err());
    }

    #[test]
    fn test_get_internal_invoice_request_by_details() {
        let db = setup_test_db();

        let dt = chrono::Utc::now();
        let invoice_request = InternalInvoiceRequest {
            unique_id: "detail_id".to_string(),
            provider_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
            requester_name: ShinkaiName::new("@@node2.shinkai/main_profile_node2".to_string()).unwrap(),
            tool_key_name: "test_tool_key_name".to_string(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            date_time: dt,
        };

        db.set_internal_invoice_request(&invoice_request).unwrap();

        let fetched = db
            .get_internal_invoice_request_by_details(
                &invoice_request.provider_name,
                &invoice_request.requester_name,
                &invoice_request.tool_key_name,
                dt,
            )
            .unwrap();

        assert_eq!(fetched.unique_id, "detail_id");
    }
}
