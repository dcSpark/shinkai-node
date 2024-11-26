use std::{collections::HashMap, sync::Arc};

use rusqlite::params;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName,
        job::{ForkedJob, Job, JobLike, JobStepResult},
        job_config::JobConfig,
        ws_types::WSUpdateHandler,
    },
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::AssociatedUI},
    shinkai_utils::job_scope::{JobScope, MinimalJobScope},
};
use shinkai_vector_resources::shinkai_time::ShinkaiStringTime;
use tokio::sync::Mutex;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn create_new_job(
        &self,
        job_id: String,
        llm_provider_id: String,
        scope: JobScope,
        is_hidden: bool,
        associated_ui: Option<AssociatedUI>,
        config: Option<JobConfig>,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        let current_time = ShinkaiStringTime::generate_time_now();
        let scope_with_files_bytes = scope.to_bytes()?;
        let scope_bytes = serde_json::to_vec(&scope.to_json_value_minimal()?)?;

        let mut stmt = conn.prepare(
            "INSERT INTO jobs (
                job_id,
                is_hidden,
                datetime_created,
                is_finished,
                parent_agent_or_llm_provider_id,
                scope,
                scope_with_files,
                conversation_inbox_name,
                step_history,
                execution_context,
                associated_ui,
                config
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;

        stmt.execute(params![
            job_id,
            is_hidden,
            current_time,
            false,
            llm_provider_id,
            scope_bytes,
            scope_with_files_bytes,
            None::<String>,
            None::<String>,
            None::<String>,
            serde_json::to_vec(&associated_ui).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            serde_json::to_vec(&config).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
        ])?;

        Ok(())
    }

    pub fn update_job_config(&self, job_id: &str, config: JobConfig) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        let config_bytes = serde_json::to_vec(&config)?;

        let mut stmt = conn.prepare("UPDATE jobs SET config = ?1 WHERE job_id = ?2")?;

        stmt.execute(params![config_bytes, job_id])?;

        Ok(())
    }

    pub fn change_job_llm_provider(&self, job_id: &str, new_agent_id: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare("UPDATE jobs SET parent_agent_or_llm_provider_id = ?1 WHERE job_id = ?2")?;

        stmt.execute(params![new_agent_id, job_id])?;

        Ok(())
    }

    /// Returns the first half of the blake3 hash of the job id value
    pub fn job_id_to_hash(job_id: &str) -> String {
        let input = &format!("job_inbox::{}::false", job_id);
        let full_hash = blake3::hash(input.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    /// Returns the first half of the blake3 hash of the message key value
    pub fn message_key_to_hash(message_key: String) -> String {
        let full_hash = blake3::hash(message_key.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    pub fn get_job_with_options(
        &self,
        job_id: &str,
        fetch_step_history: bool,
        fetch_scope_with_files: bool,
    ) -> Result<Job, SqliteManagerError> {
        let (
            scope,
            scope_with_files,
            is_finished,
            is_hidden,
            datetime_created,
            parent_agent_id,
            conversation_inbox,
            step_history,
            execution_context,
            associated_ui,
            config,
            forked_jobs,
        ) = self.get_job_data(job_id, fetch_step_history, fetch_scope_with_files)?;

        let job = Job {
            job_id: job_id.to_string(),
            is_hidden,
            datetime_created,
            is_finished,
            parent_agent_or_llm_provider_id: parent_agent_id,
            scope,
            scope_with_files,
            conversation_inbox_name: conversation_inbox,
            step_history: step_history.unwrap_or_else(Vec::new),
            execution_context,
            associated_ui,
            config,
            forked_jobs,
        };

        Ok(job)
    }

    pub fn get_job(&self, job_id: &str) -> Result<Job, SqliteManagerError> {
        self.get_job_with_options(job_id, true, true)
    }

    pub fn get_job_like(&self, job_id: &str) -> Result<Box<dyn JobLike>, SqliteManagerError> {
        let (
            scope,
            scope_with_files,
            is_finished,
            is_hidden,
            datetime_created,
            parent_agent_id,
            conversation_inbox,
            _,
            execution_context,
            associated_ui,
            config,
            forked_jobs,
        ) = self.get_job_data(job_id, false, true)?;

        let job = Job {
            job_id: job_id.to_string(),
            is_hidden,
            datetime_created,
            is_finished,
            parent_agent_or_llm_provider_id: parent_agent_id,
            scope,
            scope_with_files,
            conversation_inbox_name: conversation_inbox,
            step_history: Vec::new(), // Empty step history for JobLike
            execution_context,
            associated_ui,
            config,
            forked_jobs,
        };

        Ok(Box::new(job))
    }

    #[allow(clippy::type_complexity)]
    fn get_job_data(
        &self,
        job_id: &str,
        fetch_step_history: bool,
        fetch_scope_with_files: bool,
    ) -> Result<
        (
            MinimalJobScope,
            Option<JobScope>,
            bool,
            bool,
            String,
            String,
            InboxName,
            Option<Vec<JobStepResult>>,
            HashMap<String, String>,
            Option<AssociatedUI>,
            Option<JobConfig>,
            Vec<ForkedJob>,
        ),
        SqliteManagerError,
    > {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare("SELECT * FROM jobs WHERE job_id = ?1")?;

        let mut rows = stmt.query(params![job_id])?;

        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;

        let scope_bytes: Vec<u8> = row.get(5)?;
        let scope_with_files_bytes: Vec<u8> = row.get(6)?;
        let is_finished: bool = row.get(3)?;
        let is_hidden: bool = row.get(1)?;
        let datetime_created: String = row.get(2)?;
        let parent_agent_id: String = row.get(4)?;
        let inbox_name: String = row.get(7)?;
        let conversation_inbox: InboxName =
            InboxName::new(inbox_name).map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;
        let step_history_bytes: Option<Vec<u8>> = row.get(8)?;
        let execution_context_bytes: Option<Vec<u8>> = row.get(9)?;
        let associated_ui_bytes: Option<Vec<u8>> = row.get(10)?;
        let config_bytes: Option<Vec<u8>> = row.get(11)?;

        let scope = serde_json::from_slice(&scope_bytes)?;
        let scope_with_files = if fetch_scope_with_files {
            Some(serde_json::from_slice(&scope_with_files_bytes)?)
        } else {
            None
        };

        let step_history = if fetch_step_history {
            Some(serde_json::from_slice(&step_history_bytes.unwrap_or_default())?)
        } else {
            None
        };

        let execution_context = serde_json::from_slice(&execution_context_bytes.unwrap_or_default())?;
        let associated_ui = serde_json::from_slice(&associated_ui_bytes.unwrap_or_default())?;
        let config = serde_json::from_slice(&config_bytes.unwrap_or_default())?;

        let mut forked_jobs = vec![];

        let mut stmt = conn.prepare("SELECT * FROM forked_jobs WHERE parent_job_id = ?1")?;
        let mut rows = stmt.query(params![job_id])?;

        while let Some(row) = rows.next()? {
            let forked_job_id: String = row.get(1)?;
            let message_id: String = row.get(2)?;

            forked_jobs.push(ForkedJob {
                job_id: forked_job_id,
                message_id,
            });
        }

        Ok((
            scope,
            scope_with_files,
            is_finished,
            is_hidden,
            datetime_created,
            parent_agent_id,
            conversation_inbox,
            step_history,
            execution_context,
            associated_ui,
            config,
            forked_jobs,
        ))
    }

    pub fn get_all_jobs(&self) -> Result<Vec<Box<dyn JobLike>>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM jobs")?;
        let mut rows = stmt.query([])?;

        let mut jobs = vec![];

        while let Some(row) = rows.next()? {
            let job_id: String = row.get(0)?;
            let is_hidden: bool = row.get(1)?;
            let datetime_created: String = row.get(2)?;
            let is_finished: bool = row.get(3)?;
            let parent_agent_id: String = row.get(4)?;
            let scope_bytes: Vec<u8> = row.get(5)?;
            let scope_with_files_bytes: Vec<u8> = row.get(6)?;
            let inbox_name: String = row.get(7)?;
            let conversation_inbox: InboxName =
                InboxName::new(inbox_name).map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;
            let step_history_bytes: Option<Vec<u8>> = row.get(8)?;
            let execution_context_bytes: Option<Vec<u8>> = row.get(9)?;
            let associated_ui_bytes: Option<Vec<u8>> = row.get(10)?;
            let config_bytes: Option<Vec<u8>> = row.get(11)?;
            let scope = serde_json::from_slice(&scope_bytes)?;
            let scope_with_files = Some(serde_json::from_slice(&scope_with_files_bytes)?);
            let step_history: Option<Vec<JobStepResult>> =
                serde_json::from_slice(&step_history_bytes.unwrap_or_default())?;
            let execution_context = serde_json::from_slice(&execution_context_bytes.unwrap_or_default())?;
            let associated_ui = serde_json::from_slice(&associated_ui_bytes.unwrap_or_default())?;
            let config = serde_json::from_slice(&config_bytes.unwrap_or_default())?;

            let mut forked_jobs = vec![];

            let mut stmt = conn.prepare("SELECT * FROM forked_jobs WHERE parent_job_id = ?1")?;

            let mut rows = stmt.query(params![job_id])?;

            while let Some(row) = rows.next()? {
                let forked_job_id: String = row.get(1)?;
                let message_id: String = row.get(2)?;

                forked_jobs.push(ForkedJob {
                    job_id: forked_job_id,
                    message_id,
                });
            }

            let job = Job {
                job_id,
                is_hidden,
                datetime_created,
                is_finished,
                parent_agent_or_llm_provider_id: parent_agent_id,
                scope,
                scope_with_files,
                conversation_inbox_name: conversation_inbox,
                step_history: step_history.unwrap_or_else(Vec::new),
                execution_context,
                associated_ui,
                config,
                forked_jobs,
            };

            jobs.push(job);
        }

        Ok(jobs.into_iter().map(|job| Box::new(job) as Box<dyn JobLike>).collect())
    }

    pub fn update_job_scope(&self, job_id: String, scope: JobScope) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        let scope_bytes = serde_json::to_vec(&scope)?;

        let mut stmt = conn.prepare("UPDATE jobs SET scope = ?1 WHERE job_id = ?2")?;

        stmt.execute(params![scope_bytes, job_id])?;

        Ok(())
    }

    pub fn get_agent_jobs(&self, agent_id: String) -> Result<Vec<Box<dyn JobLike>>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM jobs WHERE parent_agent_or_llm_provider_id = ?1")?;
        let mut rows = stmt.query(params![agent_id])?;

        let mut jobs = vec![];

        while let Some(row) = rows.next()? {
            let job_id: String = row.get(0)?;
            let is_hidden: bool = row.get(1)?;
            let datetime_created: String = row.get(2)?;
            let is_finished: bool = row.get(3)?;
            let parent_agent_id: String = row.get(4)?;
            let scope_bytes: Vec<u8> = row.get(5)?;
            let scope_with_files_bytes: Vec<u8> = row.get(6)?;
            let inbox_name: String = row.get(7)?;
            let conversation_inbox: InboxName =
                InboxName::new(inbox_name).map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;
            let step_history_bytes: Option<Vec<u8>> = row.get(8)?;
            let execution_context_bytes: Option<Vec<u8>> = row.get(9)?;
            let associated_ui_bytes: Option<Vec<u8>> = row.get(10)?;
            let config_bytes: Option<Vec<u8>> = row.get(11)?;
            let scope = serde_json::from_slice(&scope_bytes)?;
            let scope_with_files = Some(serde_json::from_slice(&scope_with_files_bytes)?);
            let step_history: Option<Vec<JobStepResult>> =
                serde_json::from_slice(&step_history_bytes.unwrap_or_default())?;
            let execution_context = serde_json::from_slice(&execution_context_bytes.unwrap_or_default())?;
            let associated_ui = serde_json::from_slice(&associated_ui_bytes.unwrap_or_default())?;
            let config = serde_json::from_slice(&config_bytes.unwrap_or_default())?;

            let mut forked_jobs = vec![];

            let mut stmt = conn.prepare("SELECT * FROM forked_jobs WHERE parent_job_id = ?1")?;
            let mut rows = stmt.query(params![job_id])?;

            while let Some(row) = rows.next()? {
                let forked_job_id: String = row.get(1)?;
                let message_id: String = row.get(2)?;

                forked_jobs.push(ForkedJob {
                    job_id: forked_job_id,
                    message_id,
                });
            }

            let job = Job {
                job_id,
                is_hidden,
                datetime_created,
                is_finished,
                parent_agent_or_llm_provider_id: parent_agent_id,
                scope,
                scope_with_files,
                conversation_inbox_name: conversation_inbox,
                step_history: step_history.unwrap_or_else(Vec::new),
                execution_context,
                associated_ui,
                config,
                forked_jobs,
            };

            jobs.push(job);
        }

        Ok(jobs.into_iter().map(|job| Box::new(job) as Box<dyn JobLike>).collect())
    }

    pub fn set_job_execution_context(
        &self,
        job_id: String,
        context: HashMap<String, String>,
        _message_key: Option<String>,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        let context_bytes = serde_json::to_vec(&context)?;

        let mut stmt = conn.prepare("UPDATE jobs SET execution_context = ?1 WHERE job_id = ?2")?;

        stmt.execute(params![context_bytes, job_id])?;

        Ok(())
    }

    pub fn get_job_execution_context(&self, job_id: &str) -> Result<HashMap<String, String>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT execution_context FROM jobs WHERE job_id = ?1")?;
        let mut rows = stmt.query(params![job_id])?;

        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;

        let execution_context_bytes: Vec<u8> = row.get(0)?;
        let execution_context = serde_json::from_slice(&execution_context_bytes)?;

        Ok(execution_context)
    }

    pub fn update_job_to_finished(&self, job_id: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("UPDATE jobs SET is_finished = ?1 WHERE job_id = ?2")?;
        stmt.execute(params![true, job_id])?;
        Ok(())
    }

    pub fn is_job_inbox_empty(&self, job_id: &str) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT conversation_inbox_name FROM jobs WHERE job_id = ?1")?;
        let mut rows = stmt.query(params![job_id])?;

        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;

        let inbox_name: String = row.get(0)?;
        let messages = self.get_last_messages_from_inbox(inbox_name, 1, None)?;

        Ok(messages.is_empty())
    }

    pub async fn add_message_to_job_inbox(
        &self,
        _: &str,
        message: &ShinkaiMessage,
        parent_message_key: Option<String>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), SqliteManagerError> {
        self.unsafe_insert_inbox_message(message, parent_message_key, ws_manager)
            .await?;
        Ok(())
    }

    pub fn add_forked_job(&self, job_id: &str, forked_job: ForkedJob) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("INSERT INTO forked_jobs (parent_job_id, forked_job_id, message_id) VALUES (?1, ?2, ?3)")?;
        stmt.execute(params![job_id, forked_job.job_id, forked_job.message_id])?;
        Ok(())
    }
}
