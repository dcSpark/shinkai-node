use crate::SqliteManager;
use rusqlite::Result;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct TracingEntry {
    pub id: Option<i64>,
    pub parent_message_id: Option<String>,
    pub inbox_name: Option<String>,
    pub datetime: Option<String>,
    pub trace_name: String,
    pub trace_info: Value,
}

use crate::errors::SqliteManagerError;

impl SqliteManager {
    pub fn initialize_tracing_table(conn: &rusqlite::Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tracing (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                parent_message_id TEXT,
                inbox_name TEXT,
                datetime TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                trace_name TEXT NOT NULL,
                trace_info TEXT NOT NULL
            );",
            [],
        )?;
        // Index for parent_message_id
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tracing_parent_message_id ON tracing (parent_message_id);",
            [],
        )?;
        Ok(())
    }

    /// Adds a tracing entry to the tracing table, storing trace_info as JSON
    pub fn add_tracing(
        &self,
        parent_message_id: &str,
        inbox_name: Option<&str>,
        trace_name: &str,
        trace_info: &Value,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let trace_info_json =
            serde_json::to_string(trace_info).map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;
        conn.execute(
            "INSERT INTO tracing (parent_message_id, inbox_name, trace_name, trace_info) VALUES (?1, ?2, ?3, ?4)",
            [
                &parent_message_id as &dyn rusqlite::ToSql,
                &inbox_name.map(|s| s.to_string()) as &dyn rusqlite::ToSql,
                &trace_name,
                &trace_info_json,
            ],
        )?;
        Ok(())
    }

    /// Gets all traces for a given parent_message_id, sorted by datetime (oldest to newest)
    pub fn get_traces_by_parent_message_id(
        &self,
        parent_message_id: &str,
    ) -> Result<Vec<TracingEntry>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, parent_message_id, inbox_name, datetime, trace_name, trace_info \
             FROM tracing WHERE parent_message_id = ?1 ORDER BY datetime ASC",
        )?;
        let traces = stmt
            .query_map([parent_message_id], |row| {
                let trace_info_str: String = row.get(5)?;
                let trace_info: Value = serde_json::from_str(&trace_info_str).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(
                        e.to_string(),
                    )))
                })?;
                Ok(TracingEntry {
                    id: row.get(0)?,
                    parent_message_id: row.get(1)?,
                    inbox_name: row.get(2)?,
                    datetime: row.get(3)?,
                    trace_name: row.get(4)?,
                    trace_info,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(traces)
    }

    /// Deletes all traces for a given inbox_name. Returns the number of rows deleted.
    pub fn delete_traces_by_inbox_name(&self, inbox_name: &str) -> Result<usize, SqliteManagerError> {
        let conn = self.get_connection()?;
        let rows_deleted = conn.execute("DELETE FROM tracing WHERE inbox_name = ?1", [inbox_name])?;
        Ok(rows_deleted)
    }
}

#[cfg(test)]
mod tests {
    use crate::SqliteManager;
    use serde_json::json;
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
    fn test_add_and_get_tracing() {
        let manager = setup_test_db();
        // Add traces
        manager
            .add_tracing("msg1", Some("inbox1"), "traceA", &json!({"foo": 1}))
            .unwrap();
        manager
            .add_tracing("msg1", Some("inbox1"), "traceB", &json!({"bar": 2}))
            .unwrap();
        manager
            .add_tracing("msg2", Some("inbox2"), "traceC", &json!({"baz": 3}))
            .unwrap();

        // Get traces for msg1
        let traces = manager.get_traces_by_parent_message_id("msg1").unwrap();
        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].trace_name, "traceA");
        assert_eq!(traces[0].trace_info, json!({"foo": 1}));
        assert_eq!(traces[1].trace_name, "traceB");
        assert_eq!(traces[1].trace_info, json!({"bar": 2}));
    }

    #[test]
    fn test_delete_traces_by_inbox_name() {
        let manager = setup_test_db();
        // Add traces
        manager
            .add_tracing("msg1", Some("inbox1"), "traceA", &json!({"foo": 1}))
            .unwrap();
        manager
            .add_tracing("msg1", Some("inbox1"), "traceB", &json!({"bar": 2}))
            .unwrap();
        manager
            .add_tracing("msg2", Some("inbox2"), "traceC", &json!({"baz": 3}))
            .unwrap();

        // Delete traces for inbox1
        let deleted = manager.delete_traces_by_inbox_name("inbox1").unwrap();
        assert_eq!(deleted, 2);

        // Only msg2/inbox2 trace should remain
        let traces_msg1 = manager.get_traces_by_parent_message_id("msg1").unwrap();
        assert_eq!(traces_msg1.len(), 0);
        let traces_msg2 = manager.get_traces_by_parent_message_id("msg2").unwrap();
        assert_eq!(traces_msg2.len(), 1);
        assert_eq!(traces_msg2[0].trace_name, "traceC");
        assert_eq!(traces_msg2[0].trace_info, json!({"baz": 3}));
    }
}
