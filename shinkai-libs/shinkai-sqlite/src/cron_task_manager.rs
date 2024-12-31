use crate::SqliteManager;
use crate::SqliteManagerError;
use rusqlite::params;
use serde_json;
use shinkai_message_primitives::schemas::crontab::{CronTask, CronTaskAction};

impl SqliteManager {
    pub fn add_cron_task(
        &self,
        name: &str,
        description: Option<&str>,
        cron: &str,
        action: &CronTaskAction,
    ) -> Result<i64, SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let created_at = chrono::Utc::now().to_rfc3339();
        let last_modified = created_at.clone();
        let action_json = serde_json::to_string(action)?;

        tx.execute(
            "INSERT INTO cron_tasks (name, description, cron, created_at, last_modified, action, paused) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![name, description, cron, created_at, last_modified, action_json, false],
        )?;

        let task_id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(task_id)
    }

    pub fn remove_cron_task(&self, task_id: i64) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        
        // First, delete all related execution records
        conn.execute("DELETE FROM cron_task_executions WHERE task_id = ?1", params![task_id])?;
        
        // Then, delete the cron task
        conn.execute("DELETE FROM cron_tasks WHERE task_id = ?1", params![task_id])?;
        
        Ok(())
    }

    pub fn get_cron_task(&self, task_id: i64) -> Result<Option<CronTask>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT task_id, name, description, cron, created_at, last_modified, action, paused 
             FROM cron_tasks WHERE task_id = ?1"
        )?;
        let mut rows = stmt.query(params![task_id])?;

        if let Some(row) = rows.next()? {
            let action_json: String = row.get(6)?;
            let action: CronTaskAction = serde_json::from_str(&action_json)
                .map_err(SqliteManagerError::JsonError)?;

            Ok(Some(CronTask {
                task_id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                cron: row.get(3)?,
                created_at: row.get(4)?,
                last_modified: row.get(5)?,
                action,
                paused: row.get(7)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn update_cron_task(
        &self,
        task_id: i64,
        name: &str,
        description: Option<&str>,
        cron: &str,
        action: &CronTaskAction,
        paused: bool,
    ) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let last_modified = chrono::Utc::now().to_rfc3339();
        let action_json = serde_json::to_string(action)?;

        tx.execute(
            "UPDATE cron_tasks 
             SET name = ?1, description = ?2, cron = ?3, last_modified = ?4, action = ?5, paused = ?6 
             WHERE task_id = ?7",
            params![name, description, cron, last_modified, action_json, paused, task_id],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_all_cron_tasks(&self) -> Result<Vec<CronTask>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT task_id, name, description, cron, created_at, last_modified, action, paused 
             FROM cron_tasks"
        )?;
        let cron_task_iter = stmt.query_map([], |row| {
            let action_json: String = row.get(6)?;
            let action: CronTaskAction = serde_json::from_str(&action_json)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            Ok(CronTask {
                task_id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                cron: row.get(3)?,
                created_at: row.get(4)?,
                last_modified: row.get(5)?,
                action,
                paused: row.get(7)?,
            })
        })?;

        cron_task_iter
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteManagerError::DatabaseError)
    }

    // Add a new execution record for a cron task
    pub fn add_cron_task_execution(
        &self,
        task_id: i64,
        execution_time: &str,
        success: bool,
        error_message: Option<&str>,
        job_id: Option<String>,
    ) -> Result<i64, SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO cron_task_executions (task_id, execution_time, success, error_message, job_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![task_id, execution_time, success as i32, error_message, job_id.as_deref()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    // Get all execution records
    pub fn get_all_cron_task_executions(&self) -> Result<Vec<(i64, String, bool, Option<String>, Option<String>)>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT task_id, execution_time, success, error_message, job_id FROM cron_task_executions")?;
        let execution_iter = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get::<_, i32>(2)? != 0, row.get(3)?, row.get(4)?))
        })?;

        execution_iter
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteManagerError::DatabaseError)
    }

    // Get all executions for a specific cron task
    pub fn get_cron_task_executions(
        &self,
        task_id: i64,
    ) -> Result<Vec<(String, bool, Option<String>, Option<String>)>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT execution_time, success, error_message, job_id 
             FROM cron_task_executions 
             WHERE task_id = ?1 
             ORDER BY execution_time DESC"
        )?;
        let execution_iter = stmt.query_map(params![task_id], |row| {
            Ok((row.get(0)?, row.get::<_, i32>(1)? != 0, row.get(2)?, row.get(3)?))
        })?;

        execution_iter
            .collect::<Result<Vec<_>, _>>()
            .map_err(SqliteManagerError::DatabaseError)
    }

    // Get a specific execution record
    pub fn get_cron_task_execution(
        &self,
        execution_id: i64,
    ) -> Result<Option<(i64, String, bool, Option<String>, Option<String>)>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT task_id, execution_time, success, error_message, job_id FROM cron_task_executions WHERE execution_id = ?1",
        )?;
        let mut rows = stmt.query(params![execution_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some((
                row.get(0)?,
                row.get(1)?,
                row.get::<_, i32>(2)? != 0,
                row.get(3)?,
                row.get(4)?,
            )))
        } else {
            Ok(None)
        }
    }

    pub fn update_cron_task_last_executed(&self, task_id: i64, last_executed: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE cron_tasks SET last_executed = ?1 WHERE task_id = ?2",
            params![last_executed, task_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_message_primitives::{shinkai_message::shinkai_message_schemas::JobMessage, shinkai_utils::shinkai_path::ShinkaiPath};
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
    fn test_add_and_get_cron_task() {
        let manager = setup_test_db();
        let action = CronTaskAction::SendMessageToJob {
            job_id: "test_job_id".to_string(),
            message: JobMessage {
                job_id: "test_job_id".to_string(),
                content: "test_message".to_string(),
                fs_files_paths: vec![],
                job_filenames: vec![],
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
            },
        };
        let name = "Test Task";
        let description = Some("Test Description");
        let cron = "* * * * *";

        let task_id = manager.add_cron_task(name, description, cron, &action).unwrap();
        let retrieved_task = manager.get_cron_task(task_id).unwrap().unwrap();

        assert_eq!(retrieved_task.name, name);
        assert_eq!(retrieved_task.description, description.map(String::from));
        assert_eq!(retrieved_task.cron, cron);
        assert_eq!(retrieved_task.action, action);
    }

    #[test]
    fn test_remove_cron_task() {
        let manager = setup_test_db();
        let action = CronTaskAction::SendMessageToJob {
            job_id: "test_job_id".to_string(),
            message: JobMessage {
                job_id: "test_job_id".to_string(),
                content: "test_message".to_string(),
                fs_files_paths: vec![],
                job_filenames: vec![],
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
            },
        };
        let name = "Test Task";
        let description = Some("Test Description");
        let cron = "* * * * *";

        let task_id = manager.add_cron_task(name, description, cron, &action).unwrap();
        manager.remove_cron_task(task_id).unwrap();
        let retrieved_task = manager.get_cron_task(task_id).unwrap();

        assert!(retrieved_task.is_none());
    }

    #[test]
    fn test_add_multiple_and_get_all_cron_tasks() {
        let manager = setup_test_db();
        let action1 = CronTaskAction::SendMessageToJob {
            job_id: "job_id_1".to_string(),
            message: JobMessage {
                job_id: "job_id_1".to_string(),
                content: "message_1".to_string(),
                fs_files_paths: vec![],
                job_filenames: vec![],
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
            },
        };
        let action2 = CronTaskAction::SendMessageToJob {
            job_id: "job_id_2".to_string(),
            message: JobMessage {
                job_id: "job_id_2".to_string(),
                content: "message_2".to_string(),
                fs_files_paths: vec![],
                job_filenames: vec![],
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
            },
        };
        let name1 = "Task 1";
        let name2 = "Task 2";
        let description = Some("Test Description");
        let cron1 = "0 0 * * *";
        let cron2 = "0 12 * * *";

        manager.add_cron_task(name1, description, cron1, &action1).unwrap();
        manager.add_cron_task(name2, description, cron2, &action2).unwrap();

        let all_tasks = manager.get_all_cron_tasks().unwrap();
        assert_eq!(all_tasks.len(), 2);
        assert_eq!(all_tasks[0].name, name1);
        assert_eq!(all_tasks[0].cron, cron1);
        assert_eq!(all_tasks[1].name, name2);
        assert_eq!(all_tasks[1].cron, cron2);
    }

    #[test]
    fn test_update_cron_task() {
        let manager = setup_test_db();
        let action = CronTaskAction::SendMessageToJob {
            job_id: "test_job_id".to_string(),
            message: JobMessage {
                job_id: "test_job_id".to_string(),
                content: "test_message".to_string(),
                fs_files_paths: vec![],
                job_filenames: vec![],
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
            },
        };
        let name = "Initial Task";
        let description = Some("Initial Description");
        let cron = "* * * * *";

        let task_id = manager.add_cron_task(name, description, cron, &action).unwrap();

        let updated_name = "Updated Task";
        let updated_description = Some("Updated Description");
        let updated_cron = "0 0 * * *";
        let updated_action = CronTaskAction::SendMessageToJob {
            job_id: "updated_job_id".to_string(),
            message: JobMessage {
                job_id: "updated_job_id".to_string(),
                content: "updated_message".to_string(),
                fs_files_paths: vec![],
                job_filenames: vec![],
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
            },
        };
        let updated_paused = true;

        manager
            .update_cron_task(task_id, updated_name, updated_description, updated_cron, &updated_action, updated_paused)
            .unwrap();
        let updated_task = manager.get_cron_task(task_id).unwrap().unwrap();

        assert_eq!(updated_task.name, updated_name);
        assert_eq!(updated_task.description, updated_description.map(String::from));
        assert_eq!(updated_task.cron, updated_cron);
        assert_eq!(updated_task.action, updated_action);
        assert_eq!(updated_task.paused, updated_paused);
    }

    #[test]
    fn test_add_and_get_cron_task_execution() {
        let manager = setup_test_db();
        let action = CronTaskAction::SendMessageToJob {
            job_id: "test_job_id".to_string(),
            message: JobMessage {
                job_id: "test_job_id".to_string(),
                content: "test_message".to_string(),
                fs_files_paths: vec![],
                job_filenames: vec![],
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
            },
        };
        let name = "Test Task";
        let description = Some("Test Description");
        let cron = "* * * * *";

        let task_id = manager.add_cron_task(name, description, cron, &action).unwrap();
        let execution_time = chrono::Utc::now().to_rfc3339();
        let success = true;
        let error_message: Option<&str> = None;
        let job_id = Some("test_job_id".to_string());

        let execution_id = manager
            .add_cron_task_execution(task_id, &execution_time, success, error_message, job_id.clone())
            .unwrap();
        let execution_record = manager.get_cron_task_execution(execution_id).unwrap().unwrap();

        assert_eq!(execution_record.0, task_id);
        assert_eq!(execution_record.1, execution_time);
        assert_eq!(execution_record.2, success);
        assert_eq!(execution_record.3, error_message.map(|s| s.to_string()));
        assert_eq!(execution_record.4, job_id.map(|s| s.to_string()));
    }

    #[test]
    fn test_get_all_cron_task_executions() {
        let manager = setup_test_db();
        let action = CronTaskAction::SendMessageToJob {
            job_id: "test_job_id".to_string(),
            message: JobMessage {
                job_id: "test_job_id".to_string(),
                content: "test_message".to_string(),
                fs_files_paths: vec![],
                job_filenames: vec![],
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
            },
        };
        let name = "Test Task";
        let description = Some("Test Description");
        let cron = "* * * * *";

        let task_id = manager.add_cron_task(name, description, cron, &action).unwrap();
        let execution_time1 = chrono::Utc::now().to_rfc3339();
        let execution_time2 = chrono::Utc::now().to_rfc3339();
        let success = true;
        let error_message = None;
        let job_id = Some("test_job_id".to_string());

        manager
            .add_cron_task_execution(task_id, &execution_time1, success, error_message, job_id.clone())
            .unwrap();
        manager
            .add_cron_task_execution(task_id, &execution_time2, success, error_message, job_id.clone())
            .unwrap();

        let all_executions = manager.get_all_cron_task_executions().unwrap();
        assert_eq!(all_executions.len(), 2);
        assert_eq!(all_executions[0].4, job_id.clone().map(|s| s.to_string()));
        assert_eq!(all_executions[1].4, job_id.map(|s| s.to_string()));
    }

    #[test]
    fn test_get_cron_task_executions_for_specific_task() {
        let manager = setup_test_db();
        let action = CronTaskAction::SendMessageToJob {
            job_id: "test_job_id".to_string(),
            message: JobMessage {
                job_id: "test_job_id".to_string(),
                content: "test_message".to_string(),
                fs_files_paths: vec![],
                job_filenames: vec![],
                parent: None,
                sheet_job_data: None,
                callback: None,
                metadata: None,
                tool_key: None,
            },
        };
        let name = "Test Task";
        let description = Some("Test Description");
        let cron = "* * * * *";

        let task_id = manager.add_cron_task(name, description, cron, &action).unwrap();
        let execution_time1 = chrono::Utc::now().to_rfc3339();
        let execution_time2 = chrono::Utc::now().to_rfc3339();
        let success = true;
        let error_message = None;
        let job_id = Some("test_job_id".to_string());

        manager
            .add_cron_task_execution(task_id, &execution_time1, success, error_message, job_id.clone())
            .unwrap();
        manager
            .add_cron_task_execution(task_id, &execution_time2, success, error_message, job_id.clone())
            .unwrap();

        let task_executions = manager.get_cron_task_executions(task_id).unwrap();
        assert_eq!(task_executions.len(), 2);
        assert_eq!(task_executions[0].3, job_id.clone().map(|s| s.to_string()));
        assert_eq!(task_executions[1].3, job_id.clone().map(|s| s.to_string()));
    }
}
