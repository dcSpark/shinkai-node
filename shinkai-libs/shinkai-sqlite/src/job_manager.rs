use std::{collections::HashMap, sync::Arc};

use rusqlite::params;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName,
        job::{ForkedJob, Job, JobLike, JobStepResult},
        job_config::JobConfig,
        prompts::Prompt,
        subprompts::SubPromptType,
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
        let job_inbox_name = format!("job_inbox::{}::false", job_id);

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
                execution_context,
                associated_ui,
                config
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;

        stmt.execute(params![
            job_id,
            is_hidden,
            current_time,
            false,
            llm_provider_id,
            scope_bytes,
            scope_with_files_bytes,
            job_inbox_name.clone(),
            serde_json::to_vec(&HashMap::<String, String>::new()).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            serde_json::to_vec(&associated_ui).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
            serde_json::to_vec(&config).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?,
        ])?;

        self.create_empty_inbox(job_inbox_name)?;

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

        let scope_with_files = match fetch_scope_with_files {
            true => "scope_with_files",
            false => "NULL",
        };

        let mut stmt = conn.prepare(&format!(
            "SELECT
            job_id,
            is_hidden,
            datetime_created,
            is_finished,
            parent_agent_or_llm_provider_id,
            scope,
            {scope_with_files},
            conversation_inbox_name,
            execution_context,
            associated_ui,
            config
            FROM jobs WHERE job_id = ?1"
        ))?;

        let mut rows = stmt.query(params![job_id])?;

        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;

        let scope_bytes: Vec<u8> = row.get(5)?;
        let is_finished: bool = row.get(3)?;
        let is_hidden: bool = row.get(1)?;
        let datetime_created: String = row.get(2)?;
        let parent_agent_id: String = row.get(4)?;
        let inbox_name: String = row.get(7)?;
        let conversation_inbox: InboxName =
            InboxName::new(inbox_name).map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;
        let execution_context_bytes: Option<Vec<u8>> = row.get(8)?;
        let associated_ui_bytes: Option<Vec<u8>> = row.get(9)?;
        let config_bytes: Option<Vec<u8>> = row.get(10)?;

        let scope = serde_json::from_slice(&scope_bytes)?;
        let scope_with_files = if fetch_scope_with_files {
            let scope_with_files_bytes: Option<Vec<u8>> = row.get(6)?;
            serde_json::from_slice(&scope_with_files_bytes.unwrap_or_default())?
        } else {
            None
        };

        let step_history = if fetch_step_history {
            self.get_step_history(job_id, true)?
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
            let scope_with_files_bytes: Option<Vec<u8>> = row.get(6)?;
            let inbox_name: String = row.get(7)?;
            let conversation_inbox: InboxName =
                InboxName::new(inbox_name).map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;
            let execution_context_bytes: Option<Vec<u8>> = row.get(8)?;
            let associated_ui_bytes: Option<Vec<u8>> = row.get(9)?;
            let config_bytes: Option<Vec<u8>> = row.get(10)?;
            let scope = serde_json::from_slice(&scope_bytes)?;
            let scope_with_files = serde_json::from_slice(&scope_with_files_bytes.unwrap_or_default())?;
            let step_history = self.get_step_history(&job_id, false)?;
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

        let scope_bytes = serde_json::to_vec(&scope.to_json_value_minimal()?)?;
        let scope_with_files_bytes = scope.to_bytes()?;

        let mut stmt = conn.prepare("UPDATE jobs SET scope = ?1, scope_with_files = ?2 WHERE job_id = ?3")?;

        stmt.execute(params![scope_bytes, scope_with_files_bytes, job_id])?;

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
            let scope_with_files_bytes: Option<Vec<u8>> = row.get(6)?;
            let inbox_name: String = row.get(7)?;
            let conversation_inbox: InboxName =
                InboxName::new(inbox_name).map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;
            let execution_context_bytes: Option<Vec<u8>> = row.get(8)?;
            let associated_ui_bytes: Option<Vec<u8>> = row.get(9)?;
            let config_bytes: Option<Vec<u8>> = row.get(10)?;
            let scope = serde_json::from_slice(&scope_bytes)?;
            let scope_with_files = serde_json::from_slice(&scope_with_files_bytes.unwrap_or_default())?;
            let step_history = self.get_step_history(&job_id, false)?;
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
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM jobs WHERE job_id = ?1")?;
        let mut rows = stmt.query(params![job_id])?;
        let count: i32 = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?.get(0)?;

        if count == 0 {
            return Err(SqliteManagerError::DataNotFound);
        }

        let mut stmt = conn.prepare("UPDATE jobs SET is_finished = ?1 WHERE job_id = ?2")?;
        stmt.execute(params![true, job_id])?;
        Ok(())
    }

    pub fn add_step_history(
        &self,
        job_id: String,
        user_message: String,
        user_files: Option<HashMap<String, String>>,
        agent_response: String,
        agent_files: Option<HashMap<String, String>>,
        message_key: Option<String>,
    ) -> Result<(), SqliteManagerError> {
        let message_key = match message_key {
            Some(key) => key,
            None => {
                // Fetch the most recent message from the job's inbox
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone())
                    .map_err(|e| SqliteManagerError::SomeError(format!("Error getting inbox name: {}", e)))?;
                let last_messages = self.get_last_messages_from_inbox(inbox_name.to_string(), 1, None)?;
                if let Some(message) = last_messages.first() {
                    if let Some(message) = message.first() {
                        message.calculate_message_hash_for_pagination()
                    } else {
                        return Err(SqliteManagerError::SomeError(
                            "No messages found in the inbox".to_string(),
                        ));
                    }
                } else {
                    return Err(SqliteManagerError::SomeError(
                        "No messages found in the inbox".to_string(),
                    ));
                }
            }
        };

        // Create prompt & JobStepResult
        let mut prompt = Prompt::new();
        let user_files = user_files.unwrap_or_default();
        let agent_files = agent_files.unwrap_or_default();
        prompt.add_omni(user_message, user_files, SubPromptType::User, 100);
        prompt.add_omni(agent_response, agent_files, SubPromptType::Assistant, 100);
        let mut job_step_result = JobStepResult::new();
        job_step_result.add_new_step_revision(prompt);

        let step_result_bytes = serde_json::to_vec(&job_step_result)
            .map_err(|e| SqliteManagerError::SerializationError(format!("Error serializing JobStepResult: {}", e)))?;

        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("INSERT INTO step_history (message_key, job_id, job_step_result) VALUES (?1, ?2, ?3)")?;
        stmt.execute(params![message_key, job_id, step_result_bytes])?;

        Ok(())
    }

    pub fn get_step_history(
        &self,
        job_id: &str,
        fetch_step_history: bool,
    ) -> Result<Option<Vec<JobStepResult>>, SqliteManagerError> {
        if !fetch_step_history {
            return Ok(None);
        }

        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())
            .map_err(|e| SqliteManagerError::SomeError(format!("Error getting inbox name: {}", e)))?;
        let mut step_history: Vec<JobStepResult> = Vec::new();
        let mut until_offset_key: Option<String> = None;

        let conn = self.get_connection()?;

        loop {
            // Note(Nico): changing n to 2 helps a lot to debug potential pagination problems
            let mut messages =
                self.get_last_messages_from_inbox(inbox_name.to_string(), 2, until_offset_key.clone())?;

            if messages.is_empty() {
                break;
            }

            messages.reverse();

            for message_path in &messages {
                if let Some(message) = message_path.first() {
                    let message_key = message.calculate_message_hash_for_pagination();
                    let mut stmt = conn.prepare("SELECT job_step_result FROM step_history WHERE message_key = ?1")?;
                    let mut rows = stmt.query(params![message_key])?;

                    while let Some(row) = rows.next()? {
                        let step_result_bytes: Vec<u8> = row.get(0)?;
                        let step_result: JobStepResult = serde_json::from_slice(&step_result_bytes)?;

                        step_history.push(step_result);
                    }
                }
            }

            if let Some(last_message_path) = messages.last() {
                if let Some(last_message) = last_message_path.first() {
                    until_offset_key = Some(last_message.calculate_message_hash_for_pagination());
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Reverse the step history before returning
        step_history.reverse();
        Ok(Some(step_history))
    }

    pub fn is_job_inbox_empty(&self, job_id: &str) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT conversation_inbox_name FROM jobs WHERE job_id = ?1")?;
        let mut rows = stmt.query(params![job_id])?;

        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;
        let inbox_name: String = row.get(0)?;

        self.is_inbox_empty(&inbox_name)
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

    pub fn remove_job(&self, job_id: &str) -> Result<(), SqliteManagerError> {
        let mut conn = self.get_connection()?;
        let tx = conn.transaction()?;

        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())
            .map_err(|e| SqliteManagerError::SomeError(format!("Error getting inbox name: {}", e)))?;

        tx.execute(
            "DELETE FROM inbox_profile_permissions WHERE inbox_name = ?1",
            params![inbox_name.to_string()],
        )?;
        tx.execute(
            "DELETE FROM inbox_messages WHERE inbox_name = ?1",
            params![inbox_name.to_string()],
        )?;
        tx.execute(
            "DELETE FROM inboxes WHERE inbox_name = ?1",
            params![inbox_name.to_string()],
        )?;

        tx.execute("DELETE FROM step_history WHERE job_id = ?1", params![job_id])?;
        tx.execute("DELETE FROM jobs WHERE job_id = ?1", params![job_id])?;

        tx.commit()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use shinkai_message_primitives::schemas::identity::StandardIdentity;
    use shinkai_message_primitives::schemas::inbox_permission::InboxPermission;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_message_primitives::schemas::subprompts::SubPromptType::{Assistant, User};
    use shinkai_message_primitives::{
        schemas::{identity::StandardIdentityType, subprompts::SubPrompt},
        shinkai_message::shinkai_message_schemas::{IdentityPermissions, JobMessage, MessageSchemaType},
        shinkai_utils::{
            encryption::{unsafe_deterministic_encryption_keypair, EncryptionMethod},
            shinkai_message_builder::ShinkaiMessageBuilder,
            signatures::{clone_signature_secret_key, unsafe_deterministic_signature_keypair},
        },
    };
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::{collections::HashSet, path::PathBuf, time::Duration};
    use tempfile::NamedTempFile;
    use tokio::time::sleep;
    use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    fn create_new_job(db: &SqliteManager, job_id: String, agent_id: String, scope: JobScope) {
        match db.create_new_job(job_id, agent_id, scope, false, None, None) {
            Ok(_) => (),
            Err(e) => panic!("Failed to create a new job: {}", e),
        }
    }

    fn generate_message_with_text(
        content: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        recipient_subidentity_name: String,
        origin_destination_identity_name: String,
        timestamp: String,
    ) -> ShinkaiMessage {
        let inbox_name = InboxName::get_job_inbox_name_from_params("test_job".to_string()).unwrap();

        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(content.to_string())
            .body_encryption(EncryptionMethod::None)
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata_with_inbox(
                "".to_string(),
                recipient_subidentity_name.clone().to_string(),
                inbox_name_value,
                EncryptionMethod::None,
                None,
            )
            .external_metadata_with_schedule(
                origin_destination_identity_name.clone().to_string(),
                origin_destination_identity_name.clone().to_string(),
                timestamp,
            )
            .build()
            .unwrap()
    }

    #[test]
    fn test_create_new_job() {
        let db = setup_test_db();
        let job_id = "job1".to_string();
        let agent_id = "agent1".to_string();
        let scope = JobScope::new_default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        // Retrieve all jobs
        let jobs = db.get_all_jobs().unwrap();

        // Check if the job exists
        let job_ids: Vec<String> = jobs.iter().map(|job| job.job_id().to_string()).collect();
        assert!(job_ids.contains(&job_id));

        // Check that the job has the correct properties
        let job = db.get_job(&job_id).unwrap();
        assert_eq!(job.job_id, job_id);
        assert_eq!(job.parent_agent_or_llm_provider_id, agent_id);
        assert!(!job.is_finished);
    }

    #[test]
    fn test_get_agent_jobs() {
        let db = setup_test_db();
        let agent_id = "agent2".to_string();

        // Create new jobs for the agent
        for i in 1..=5 {
            let job_id = format!("job{}", i);
            eprintln!("job_id: {}", job_id.clone());
            let scope = JobScope::new_default();
            create_new_job(&db, job_id, agent_id.clone(), scope);
        }

        eprintln!("agent_id: {}", agent_id.clone());

        // Get all jobs for the agent
        let jobs = db.get_agent_jobs(agent_id.clone()).unwrap();

        // Assert that all jobs are returned
        assert_eq!(jobs.len(), 5);

        // Additional check that all jobs have correct agent_id
        for job in jobs {
            assert_eq!(job.parent_llm_provider_id(), &agent_id);
        }
    }

    #[test]
    fn test_change_job_agent() {
        let db = setup_test_db();
        let job_id = "job_to_change_agent".to_string();
        let initial_agent_id = "initial_agent".to_string();
        let new_agent_id = "new_agent".to_string();
        let scope = JobScope::new_default();

        // Create a new job with the initial agent
        create_new_job(&db, job_id.clone(), initial_agent_id.clone(), scope);

        // Change the agent of the job
        db.change_job_llm_provider(&job_id, &new_agent_id).unwrap();

        // Retrieve the job and check that the agent has been updated
        let job = db.get_job(&job_id).unwrap();
        assert_eq!(job.parent_agent_or_llm_provider_id, new_agent_id);

        // Check that the job is listed under the new agent
        let new_agent_jobs = db.get_agent_jobs(new_agent_id.clone()).unwrap();
        let job_ids: Vec<String> = new_agent_jobs.iter().map(|job| job.job_id().to_string()).collect();
        assert!(job_ids.contains(&job_id));

        // Check that the job is no longer listed under the initial agent
        let initial_agent_jobs = db.get_agent_jobs(initial_agent_id.clone()).unwrap();
        let initial_job_ids: Vec<String> = initial_agent_jobs.iter().map(|job| job.job_id().to_string()).collect();
        assert!(!initial_job_ids.contains(&job_id));
    }

    #[test]
    fn test_update_job_to_finished() {
        let db = setup_test_db();
        let job_id = "job3".to_string();
        let agent_id = "agent3".to_string();
        // let inbox_name =
        //     InboxName::new("inbox::@@node1.shinkai/subidentity::@@node2.shinkai/subidentity2::true".to_string())
        //         .unwrap();
        let scope = JobScope::new_default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        // Update job to finished
        db.update_job_to_finished(&job_id.clone()).unwrap();

        // Retrieve the job and check that is_finished is set to true
        let job = db.get_job(&job_id.clone()).unwrap();
        assert!(job.is_finished);
    }

    #[tokio::test]
    async fn test_update_step_history() {
        let db = setup_test_db();
        let job_id = "test_job".to_string();

        let node1_identity_name = "@@node1.shinkai";
        let node1_subidentity_name = "main_profile_node1";
        let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        let message = generate_message_with_text(
            "Hello World".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.810Z".to_string(),
        );

        // Insert the ShinkaiMessage into the database
        db.unsafe_insert_inbox_message(&message, None, None).await.unwrap();

        // Update step history
        db.add_step_history(
            job_id.clone(),
            "What is 10 + 25".to_string(),
            None,
            "The answer is 35".to_string(),
            None,
            None,
        )
        .unwrap();
        sleep(Duration::from_millis(10)).await;
        db.add_step_history(
            job_id.clone(),
            "2) What is 10 + 25".to_string(),
            None,
            "2) The answer is 35".to_string(),
            None,
            None,
        )
        .unwrap();

        // Retrieve the job and check that step history is updated
        let job = db.get_job(&job_id.clone()).unwrap();
        assert_eq!(job.step_history.len(), 2);
    }

    #[test]
    fn test_get_non_existent_job() {
        let db = setup_test_db();
        let job_id = "non_existent_job".to_string();

        match db.get_job(&job_id) {
            Ok(_) => panic!("Expected an error when getting a non-existent job"),
            Err(e) => assert!(matches!(e, SqliteManagerError::DataNotFound)),
        }
    }

    #[test]
    fn test_get_agent_jobs_none_exist() {
        let db = setup_test_db();
        let agent_id = "agent_without_jobs".to_string();

        // Attempt to get all jobs for the agent
        let jobs_result = db.get_agent_jobs(agent_id.clone());

        assert!(jobs_result.unwrap().is_empty());
    }

    #[test]
    fn test_update_non_existent_job() {
        let db = setup_test_db();
        let job_id = "non_existent_job".to_string();

        match db.update_job_to_finished(&job_id.clone()) {
            Ok(_) => panic!("Expected an error when updating a non-existent job"),
            Err(e) => assert!(matches!(e, SqliteManagerError::DataNotFound)),
        }
    }

    #[test]
    fn test_get_agent_jobs_multiple_jobs() {
        let db = setup_test_db();
        let agent_id = "agent5".to_string();

        // Create new jobs for the agent
        for i in 1..=5 {
            let job_id = format!("job{}", i);
            let scope = JobScope::new_default();
            create_new_job(&db, job_id, agent_id.clone(), scope);
        }

        // Get all jobs for the agent
        let jobs = db.get_agent_jobs(agent_id.clone()).unwrap();

        // Assert that all jobs are returned
        assert_eq!(jobs.len(), 5);

        // Additional check that all jobs have correct agent_id and they are unique
        let unique_jobs: HashSet<String> = jobs.iter().map(|job| job.job_id().to_string()).collect();
        assert_eq!(unique_jobs.len(), 5);
    }

    #[tokio::test]
    async fn test_job_inbox_empty() {
        let db = setup_test_db();
        let job_id = "job_test".to_string();
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        // Check if the job inbox is empty after creating a new job
        assert!(db.is_job_inbox_empty(&job_id).unwrap());

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            job_id.to_string(),
            "something".to_string(),
            "".to_string(),
            None,
            placeholder_signature_sk,
            "@@node1.shinkai".to_string(),
            "@@node1.shinkai".to_string(),
        )
        .unwrap();

        // Add a message to the job
        let _ = db
            .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, None, None)
            .await;

        // Check if the job inbox is not empty after adding a message
        assert!(!db.is_job_inbox_empty(&job_id).unwrap());
    }

    #[tokio::test]
    async fn test_job_inbox_tree_structure() {
        let db = setup_test_db();
        let job_id = "job_test".to_string();
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);

        let mut parent_message_hash: Option<String> = None;
        let mut parent_message_hash_2: Option<String> = None;

        /*
        The tree that we are creating looks like:
            1
            ├── 2
            │   ├── 4
            └── 3
         */
        for i in 1..=4 {
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.clone(),
                format!("Hello World {}", i),
                "".to_string(),
                None,
                placeholder_signature_sk.clone(),
                "@@node1.shinkai".to_string(),
                "@@node1.shinkai".to_string(),
            )
            .unwrap();

            let parent_hash: Option<String> = match i {
                2 | 3 => parent_message_hash.clone(),
                4 => parent_message_hash_2.clone(),
                _ => None,
            };

            // Add a message to the job
            let _ = db
                .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, parent_hash.clone(), None)
                .await;

            // Update the parent message according to the tree structure
            if i == 1 {
                parent_message_hash = Some(shinkai_message.calculate_message_hash_for_pagination());
            } else if i == 2 {
                parent_message_hash_2 = Some(shinkai_message.calculate_message_hash_for_pagination());
            }
        }

        // Check if the job inbox is not empty after adding a message
        assert!(!db.is_job_inbox_empty(&job_id).unwrap());

        // Get the inbox name
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        // Get the messages from the job inbox
        let last_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 4, None)
            .unwrap();

        // Check the content of the messages
        assert_eq!(last_messages_inbox.len(), 3);

        // Check the content of the first message array
        assert_eq!(last_messages_inbox[0].len(), 1);
        let message_content_1 = last_messages_inbox[0][0].clone().get_message_content().unwrap();
        let job_message_1: JobMessage = serde_json::from_str(&message_content_1).unwrap();
        assert_eq!(job_message_1.content, "Hello World 1".to_string());

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[1].len(), 2);
        let message_content_2 = last_messages_inbox[1][0].clone().get_message_content().unwrap();
        let job_message_2: JobMessage = serde_json::from_str(&message_content_2).unwrap();
        assert_eq!(job_message_2.content, "Hello World 2".to_string());

        let message_content_3 = last_messages_inbox[1][1].clone().get_message_content().unwrap();
        let job_message_3: JobMessage = serde_json::from_str(&message_content_3).unwrap();
        assert_eq!(job_message_3.content, "Hello World 3".to_string());

        // Check the content of the third message array
        assert_eq!(last_messages_inbox[2].len(), 1);
        let message_content_4 = last_messages_inbox[2][0].clone().get_message_content().unwrap();
        let job_message_4: JobMessage = serde_json::from_str(&message_content_4).unwrap();
        assert_eq!(job_message_4.content, "Hello World 4".to_string());
    }

    #[tokio::test]
    async fn test_job_inbox_tree_structure_with_step_history_and_execution_context() {
        let db = setup_test_db();
        let job_id = "job_test".to_string();
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);

        let mut parent_message_hash: Option<String> = None;
        let mut parent_message_hash_2: Option<String> = None;

        /*
        The tree that we are creating looks like:
            1
            ├── 2
            │   ├── 4
            └── 3
         */
        let mut current_level = 0;
        let mut results = Vec::new();
        for i in 1..=4 {
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.clone(),
                format!("Hello World {}", i),
                "".to_string(),
                None,
                placeholder_signature_sk.clone(),
                "@@node1.shinkai".to_string(),
                "@@node1.shinkai".to_string(),
            )
            .unwrap();

            let parent_hash: Option<String> = match i {
                2 | 3 => {
                    current_level += 1;
                    parent_message_hash.clone()
                }
                4 => {
                    results.pop();
                    parent_message_hash_2.clone()
                }
                _ => None,
            };

            // Add a message to the job
            let _ = db
                .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, parent_hash.clone(), None)
                .await;

            // Add a step history
            let result = format!("Result {}", i);
            db.add_step_history(
                job_id.clone(),
                format!("Step {} Level {}", i, current_level),
                None,
                result.clone(),
                None,
                None,
            )
            .unwrap();

            // Add the result to the results vector
            results.push(result);

            // Set job execution context
            let mut execution_context = HashMap::new();
            execution_context.insert("context".to_string(), results.join(", "));
            db.set_job_execution_context(job_id.clone(), execution_context, None)
                .unwrap();

            // Update the parent message according to the tree structure
            if i == 1 {
                parent_message_hash = Some(shinkai_message.calculate_message_hash_for_pagination());
            } else if i == 2 {
                parent_message_hash_2 = Some(shinkai_message.calculate_message_hash_for_pagination());
            }
        }

        // Check if the job inbox is not empty after adding a message
        assert!(!db.is_job_inbox_empty(&job_id).unwrap());

        // Get the inbox name
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        // Get the messages from the job inbox
        let last_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 4, None)
            .unwrap();

        // Check the content of the messages
        assert_eq!(last_messages_inbox.len(), 3);

        // Check the content of the first message array
        assert_eq!(last_messages_inbox[0].len(), 1);
        let message_content_1 = last_messages_inbox[0][0].clone().get_message_content().unwrap();
        let job_message_1: JobMessage = serde_json::from_str(&message_content_1).unwrap();
        assert_eq!(job_message_1.content, "Hello World 1".to_string());

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[1].len(), 2);
        let message_content_2 = last_messages_inbox[1][0].clone().get_message_content().unwrap();
        let job_message_2: JobMessage = serde_json::from_str(&message_content_2).unwrap();
        assert_eq!(job_message_2.content, "Hello World 2".to_string());

        let message_content_3 = last_messages_inbox[1][1].clone().get_message_content().unwrap();
        let job_message_3: JobMessage = serde_json::from_str(&message_content_3).unwrap();
        assert_eq!(job_message_3.content, "Hello World 3".to_string());

        // Check the content of the third message array
        assert_eq!(last_messages_inbox[2].len(), 1);
        let message_content_4 = last_messages_inbox[2][0].clone().get_message_content().unwrap();
        let job_message_4: JobMessage = serde_json::from_str(&message_content_4).unwrap();
        assert_eq!(job_message_4.content, "Hello World 4".to_string());

        // Check the step history and execution context
        let job = db.get_job(&job_id.clone()).unwrap();
        eprintln!("job execution context: {:?}", job.execution_context);

        // Check the execution context
        assert_eq!(
            job.execution_context.get("context").unwrap(),
            "Result 1, Result 2, Result 4"
        );

        // Check the step history
        let step1 = &job.step_history[0];
        let step2 = &job.step_history[1];
        let step4 = &job.step_history[2];

        assert_eq!(
            step1.step_revisions[0].sub_prompts[0],
            SubPrompt::Omni(User, "Step 1 Level 0".to_string(), vec![], 100)
        );
        assert_eq!(
            step1.step_revisions[0].sub_prompts[1],
            SubPrompt::Omni(Assistant, "Result 1".to_string(), vec![], 100)
        );

        assert_eq!(
            step2.step_revisions[0].sub_prompts[0],
            SubPrompt::Omni(User, "Step 2 Level 1".to_string(), vec![], 100)
        );
        assert_eq!(
            step2.step_revisions[0].sub_prompts[1],
            SubPrompt::Omni(Assistant, "Result 2".to_string(), vec![], 100)
        );

        assert_eq!(
            step4.step_revisions[0].sub_prompts[0],
            SubPrompt::Omni(User, "Step 4 Level 2".to_string(), vec![], 100)
        );
        assert_eq!(
            step4.step_revisions[0].sub_prompts[1],
            SubPrompt::Omni(Assistant, "Result 4".to_string(), vec![], 100)
        );
    }

    #[tokio::test]
    async fn test_insert_steps_with_simple_tree_structure() {
        let db = setup_test_db();

        let node1_identity_name = "@@node1.shinkai";
        let node1_subidentity_name = "main_profile_node1";
        let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let job_id = "test_job";
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();

        create_new_job(&db, job_id.to_string(), agent_id.clone(), scope);

        eprintln!("Inserting steps...\n\n");
        let mut parent_message_hash: Option<String> = None;
        let mut parent_message_hash_2: Option<String> = None;

        /*
        The tree that we are creating looks like:
            1
            ├── 2
            │   └── 4
            └── 3
         */
        for i in 1..=4 {
            let user_message = format!("User message {}", i);
            let agent_response = format!("Agent response {}", i);

            // Generate the ShinkaiMessage
            let message = generate_message_with_text(
                format!("Hello World {}", i),
                node1_encryption_sk.clone(),
                clone_signature_secret_key(&node1_identity_sk),
                node1_encryption_pk,
                node1_subidentity_name.to_string(),
                node1_identity_name.to_string(),
                format!("2023-07-02T20:53:34.81{}Z", i),
            );

            eprintln!("Message: {:?}", message);

            let parent_hash: Option<String> = match i {
                2 | 3 => parent_message_hash.clone(),
                4 => parent_message_hash_2.clone(),
                _ => None,
            };

            // Insert the ShinkaiMessage into the database
            db.unsafe_insert_inbox_message(&message, parent_hash.clone(), None)
                .await
                .unwrap();

            db.add_step_history(job_id.to_string(), user_message, None, agent_response, None, None)
                .unwrap();

            // Update the parent message hash according to the tree structure
            if i == 1 {
                parent_message_hash = Some(message.calculate_message_hash_for_pagination());
            } else if i == 2 {
                parent_message_hash_2 = Some(message.calculate_message_hash_for_pagination());
            }
        }

        eprintln!("\n\n Getting messages...");
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string()).unwrap();
        let last_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name.to_string(), 3, None)
            .unwrap();

        let last_messages_content: Vec<Vec<String>> = last_messages_inbox
            .iter()
            .map(|message_array| {
                message_array
                    .iter()
                    .map(|message| message.clone().get_message_content().unwrap())
                    .collect()
            })
            .collect();

        eprintln!("Messages: {:?}", last_messages_content);

        eprintln!("\n\n Getting steps...");

        let step_history = db.get_step_history(job_id, true).unwrap().unwrap();

        let step_history_content: Vec<String> = step_history
            .iter()
            .map(|step| {
                let user_message = match &step.step_revisions[0].sub_prompts[0] {
                    SubPrompt::Omni(_, text, _, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                };
                let agent_response = match &step.step_revisions[0].sub_prompts[1] {
                    SubPrompt::Omni(_, text, _, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                };
                format!("{} {}", user_message, agent_response)
            })
            .collect();

        eprintln!("Step history: {:?}", step_history_content);

        assert_eq!(step_history.len(), 3);

        // Check the content of the steps
        assert_eq!(
            format!(
                "{} {}",
                match &step_history[0].step_revisions[0].sub_prompts[0] {
                    SubPrompt::Omni(_, text, _, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                },
                match &step_history[0].step_revisions[0].sub_prompts[1] {
                    SubPrompt::Omni(_, text, _, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                }
            ),
            "User message 1 Agent response 1".to_string()
        );
        assert_eq!(
            format!(
                "{} {}",
                match &step_history[1].step_revisions[0].sub_prompts[0] {
                    SubPrompt::Omni(_, text, _, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                },
                match &step_history[1].step_revisions[0].sub_prompts[1] {
                    SubPrompt::Omni(_, text, _, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                }
            ),
            "User message 2 Agent response 2".to_string()
        );
        assert_eq!(
            format!(
                "{} {}",
                match &step_history[2].step_revisions[0].sub_prompts[0] {
                    SubPrompt::Omni(_, text, _, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                },
                match &step_history[2].step_revisions[0].sub_prompts[1] {
                    SubPrompt::Omni(_, text, _, _) => text,
                    _ => panic!("Unexpected SubPrompt variant"),
                }
            ),
            "User message 4 Agent response 4".to_string()
        );
    }

    #[tokio::test]
    async fn test_job_inbox_tree_structure_with_invalid_date() {
        let db = setup_test_db();
        let job_id = "job_test".to_string();
        let agent_id = "agent_test".to_string();
        let scope = JobScope::new_default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);

        // Create the messages
        let mut messages = Vec::new();
        for i in [1, 3, 2].iter() {
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.clone(),
                format!("Hello World {}", i),
                "".to_string(),
                None,
                placeholder_signature_sk.clone(),
                "@@node1.shinkai".to_string(),
                "@@node1.shinkai".to_string(),
            )
            .unwrap();
            messages.push(shinkai_message);

            sleep(Duration::from_millis(10)).await;
        }

        /*
        The tree that we are creating looks like:
            1
            ├── 2
                └── 3 (older date than 2. it should'nt fail)
         */

        // Add the messages to the job in a specific order to simulate an invalid date scenario
        for i in [0, 2, 1].iter() {
            let _result = db
                .add_message_to_job_inbox(&job_id.clone(), &messages[*i], None, None)
                .await;
        }

        // Check if the job inbox is not empty after adding a message
        assert!(!db.is_job_inbox_empty(&job_id).unwrap());

        // Get the inbox name
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        // Get the messages from the job inbox
        let last_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 3, None)
            .unwrap();

        // Check the content of the messages
        assert_eq!(last_messages_inbox.len(), 3);

        // Check the content of the first message array
        assert_eq!(last_messages_inbox[0].len(), 1);
        let message_content_1 = last_messages_inbox[0][0].clone().get_message_content().unwrap();
        let job_message_1: JobMessage = serde_json::from_str(&message_content_1).unwrap();
        assert_eq!(job_message_1.content, "Hello World 1".to_string());

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[1].len(), 1);
        let message_content_2 = last_messages_inbox[1][0].clone().get_message_content().unwrap();
        let job_message_2: JobMessage = serde_json::from_str(&message_content_2).unwrap();
        assert_eq!(job_message_2.content, "Hello World 2".to_string());

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[2].len(), 1);
        let message_content_3 = last_messages_inbox[2][0].clone().get_message_content().unwrap();
        let job_message_3: JobMessage = serde_json::from_str(&message_content_3).unwrap();
        assert_eq!(job_message_3.content, "Hello World 3".to_string());
    }

    #[tokio::test]
    async fn test_add_forked_job() {
        let db = setup_test_db();
        let job_id = "job1".to_string();
        let agent_id = "agent1".to_string();
        let scope = JobScope::new_default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope.clone());

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);

        let mut parent_message_hash: Option<String> = None;
        let mut parent_message_hash_2: Option<String> = None;

        /*
        The tree that we are creating looks like:
            1
            ├── 2
            │   ├── 4
            └── 3
         */
        for i in 1..=4 {
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.clone(),
                format!("Hello World {}", i),
                "".to_string(),
                None,
                placeholder_signature_sk.clone(),
                "@@node1.shinkai".to_string(),
                "@@node1.shinkai".to_string(),
            )
            .unwrap();

            let parent_hash: Option<String> = match i {
                2 | 3 => parent_message_hash.clone(),
                4 => parent_message_hash_2.clone(),
                _ => None,
            };

            // Add a message to the job
            let _ = db
                .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, parent_hash.clone(), None)
                .await;

            // Update the parent message according to the tree structure
            if i == 1 {
                parent_message_hash = Some(shinkai_message.calculate_message_hash_for_pagination());
            } else if i == 2 {
                parent_message_hash_2 = Some(shinkai_message.calculate_message_hash_for_pagination());
            }
        }

        // Get the inbox name
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone()).unwrap();
        let inbox_name_value = inbox_name.to_string();

        // Get the messages from the job inbox
        let last_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 4, None)
            .unwrap();

        // Create forked jobs
        let forked_job1_id = "forked_job1".to_string();
        let forked_message1_id = last_messages_inbox
            .last()
            .unwrap()
            .last()
            .unwrap()
            .calculate_message_hash_for_pagination();
        let forked_job2_id = "forked_job2".to_string();
        let forked_message2_id = last_messages_inbox
            .first()
            .unwrap()
            .first()
            .unwrap()
            .calculate_message_hash_for_pagination();
        create_new_job(&db, forked_job1_id.clone(), agent_id.clone(), scope.clone());
        create_new_job(&db, forked_job2_id.clone(), agent_id.clone(), scope);

        let forked_job1 = ForkedJob {
            job_id: forked_job1_id.clone(),
            message_id: forked_message1_id.clone(),
        };
        let forked_job2 = ForkedJob {
            job_id: forked_job2_id.clone(),
            message_id: forked_message2_id.clone(),
        };
        match db.add_forked_job(&job_id, forked_job1) {
            Ok(_) => {}
            Err(e) => panic!("Error adding forked job: {:?}", e),
        }
        match db.add_forked_job(&job_id, forked_job2) {
            Ok(_) => {}
            Err(e) => panic!("Error adding forked job: {:?}", e),
        }

        // Check that the forked jobs are added
        let job = db.get_job(&job_id).unwrap();
        assert_eq!(job.forked_jobs.len(), 2);
        assert_eq!(job.forked_jobs[0].job_id, forked_job1_id);
        assert_eq!(job.forked_jobs[0].message_id, forked_message1_id);
        assert_eq!(job.forked_jobs[1].job_id, forked_job2_id);
        assert_eq!(job.forked_jobs[1].message_id, forked_message2_id);
    }

    #[tokio::test]
    async fn test_remove_job() {
        let db = setup_test_db();
        let job1_id = "job1".to_string();
        let job2_id = "job2".to_string();
        let agent_id = "agent1".to_string();
        let scope = JobScope::new_default();

        // Create new jobs
        create_new_job(&db, job1_id.clone(), agent_id.clone(), scope.clone());
        create_new_job(&db, job2_id.clone(), agent_id.clone(), scope);

        // Check smart_inboxes
        let node1_identity_name = "@@node1.shinkai";
        let node1_subidentity_name = "main_profile_node1";
        let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (_, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
        let (_, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let node1_profile_identity = StandardIdentity::new(
            ShinkaiName::from_node_and_profile_names(
                node1_identity_name.to_string(),
                node1_subidentity_name.to_string(),
            )
            .unwrap(),
            None,
            node1_encryption_pk.clone(),
            node1_identity_pk.clone(),
            Some(node1_subencryption_pk),
            Some(node1_subidentity_pk),
            StandardIdentityType::Profile,
            IdentityPermissions::Standard,
        );

        let _ = db.insert_profile(node1_profile_identity.clone());

        let inbox1_name = InboxName::get_job_inbox_name_from_params(job1_id.clone()).unwrap();
        let inbox2_name = InboxName::get_job_inbox_name_from_params(job2_id.clone()).unwrap();

        db.add_permission(
            &inbox1_name.to_string(),
            &node1_profile_identity,
            InboxPermission::Admin,
        )
        .unwrap();
        db.add_permission(
            &inbox2_name.to_string(),
            &node1_profile_identity,
            InboxPermission::Admin,
        )
        .unwrap();

        let smart_inboxes = db
            .get_all_smart_inboxes_for_profile(node1_profile_identity.clone())
            .unwrap();
        assert_eq!(smart_inboxes.len(), 2);

        // Remove the first job
        db.remove_job(&job1_id).unwrap();

        // Check if the job is removed
        match db.get_job(&job1_id) {
            Ok(_) => panic!("Expected an error when getting a removed job"),
            Err(e) => assert!(matches!(e, SqliteManagerError::DataNotFound)),
        }

        // Check if the smart_inbox is removed
        let smart_inboxes = db
            .get_all_smart_inboxes_for_profile(node1_profile_identity.clone())
            .unwrap();
        assert_eq!(smart_inboxes.len(), 1);
        assert!(smart_inboxes[0].inbox_id != inbox1_name.to_string());
    }
}
