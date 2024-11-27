use chrono::{DateTime, Utc};
use rusqlite::params;
use shinkai_message_primitives::{schemas::retry::RetryMessage, shinkai_message::shinkai_message::ShinkaiMessage};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn add_message_to_retry(
        &self,
        retry_message: &RetryMessage,
        retry_time: DateTime<Utc>,
    ) -> Result<(), SqliteManagerError> {
        let hash_key = retry_message.message.calculate_message_hash_for_pagination();
        let time_key = retry_time.to_rfc3339();
        let retry_message_bytes =
            bincode::serialize(retry_message).map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO retry_messages (hash_key, time_key, message) VALUES (?1, ?2, ?3)",
            params![hash_key, time_key, retry_message_bytes],
        )?;

        Ok(())
    }

    pub fn remove_message_from_retry(&self, message: &ShinkaiMessage) -> Result<(), SqliteManagerError> {
        let hash_key = message.calculate_message_hash_for_pagination();
        let conn = self.get_connection()?;
        conn.execute("DELETE FROM retry_messages WHERE hash_key = ?1", params![hash_key])?;

        Ok(())
    }

    pub fn get_messages_to_retry_before(
        &self,
        up_to_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<RetryMessage>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT message FROM retry_messages WHERE time_key <= ?1")?;
        let rows = stmt.query_map(params![up_to_time.map(|t| t.to_rfc3339())], |row| {
            let message_bytes: Vec<u8> = row.get(0)?;
            let message: RetryMessage = bincode::deserialize(&message_bytes).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            Ok(message)
        })?;

        let mut messages = Vec::new();
        for message in rows {
            messages.push(message?);
        }

        Ok(messages)
    }
}
