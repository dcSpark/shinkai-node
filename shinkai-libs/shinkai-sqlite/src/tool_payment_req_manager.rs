use rusqlite::params;
use shinkai_message_primitives::schemas::x402::PaymentRequirements;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn set_tool_payment_requirements(&self, tool_key: &str, req: PaymentRequirements) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "INSERT INTO tool_micropayments_requirements (tool_key, usage_type, meta_description) VALUES (?1, ?2, NULL)"
        )?;
        let req_json = serde_json::to_string(&req).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
        })?;
        stmt.execute(params![tool_key, req_json])?;
        Ok(())
    }

    pub fn get_tool_payment_requirements(&self, tool_key: &str) -> Result<PaymentRequirements, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT usage_type FROM tool_micropayments_requirements WHERE tool_key = ?1")?;
        let req = stmt.query_row(params![tool_key], |row| {
            let json: String = row.get(0)?;
            let req: PaymentRequirements = serde_json::from_str(&json).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            Ok(req)
        }).map_err(|e| {
            if e == rusqlite::Error::QueryReturnedNoRows {
                SqliteManagerError::ToolOfferingNotFound(tool_key.to_string())
            } else {
                SqliteManagerError::DatabaseError(e)
            }
        })?;
        Ok(req)
    }

    pub fn remove_tool_payment_requirements(&self, tool_key: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("DELETE FROM tool_micropayments_requirements WHERE tool_key = ?1")?;
        stmt.execute(params![tool_key])?;
        Ok(())
    }

    pub fn get_all_tool_payment_requirements(&self) -> Result<Vec<(String, PaymentRequirements)>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT tool_key, usage_type FROM tool_micropayments_requirements")?;
        let rows = stmt.query_map([], |row| {
            let tool_key: String = row.get(0)?;
            let usage_type: String = row.get(1)?;
            let req: PaymentRequirements = serde_json::from_str(&usage_type).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            Ok((tool_key, req))
        })?;
        let mut results = Vec::new();
        for r in rows { results.push(r?); }
        Ok(results)
    }
}
