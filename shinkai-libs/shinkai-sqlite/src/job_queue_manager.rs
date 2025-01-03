use std::collections::HashMap;

use rusqlite::params;
use serde::{de::DeserializeOwned, Serialize};

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn persist_queue<T: Serialize>(
        &self,
        job_id: &str,
        queue: &Vec<T>,
        prefix: Option<String>,
    ) -> Result<(), SqliteManagerError> {
        let full_job_id = match &prefix {
            Some(p) => format!("{}{}", p, job_id),
            None => job_id.to_string(),
        };

        let serialized_queue =
            serde_json::to_string(queue).map_err(|e| SqliteManagerError::SerializationError(e.to_string()))?;

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO job_queues (job_id, queue_data) VALUES (?1, ?2)",
            params![full_job_id, serialized_queue],
        )?;

        Ok(())
    }

    pub fn get_all_queues<T: DeserializeOwned>(
        &self,
        prefix: Option<String>,
    ) -> Result<HashMap<String, Vec<T>>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut queues = HashMap::new();
        let mut query = "SELECT job_id, queue_data FROM job_queues WHERE 1=1".to_string();

        if let Some(p) = &prefix {
            query.push_str(&format!(" AND job_id LIKE '{}%'", p));
        }

        let mut stmt = conn.prepare(&query)?;

        let rows = stmt.query_map(params![], |row| {
            let mut job_id: String = row.get(0)?;
            let serialized_queue: String = row.get(1)?;
            let queue: Vec<T> = serde_json::from_str(&serialized_queue).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            // If a prefix is provided, remove it from the job_id
            if let Some(p) = &prefix {
                if job_id.starts_with(p) {
                    job_id = job_id[p.len()..].to_string();
                }
            }

            Ok((job_id, queue))
        })?;

        for row in rows {
            let (job_id, queue) = row?;
            queues.insert(job_id, queue);
        }

        Ok(queues)
    }
}
