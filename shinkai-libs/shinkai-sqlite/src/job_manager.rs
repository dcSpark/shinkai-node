use std::sync::Arc;

use rusqlite::params;
use shinkai_message_primitives::{
    schemas::{
        inbox_name::InboxName,
        job::{ForkedJob, Job, JobLike},
        job_config::JobConfig,
        ws_types::WSUpdateHandler,
    },
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::AssociatedUI},
    shinkai_utils::{job_scope::MinimalJobScope, shinkai_time::ShinkaiStringTime},
};
use tokio::sync::Mutex;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn create_new_job(
        &self,
        job_id: String,
        llm_provider_id: String,
        scope: MinimalJobScope,
        is_hidden: bool,
        associated_ui: Option<AssociatedUI>,
        config: Option<JobConfig>,
    ) -> Result<(), SqliteManagerError> {
        let job_inbox_name = format!("job_inbox::{}::false", job_id);

        {
            let conn = self.get_connection()?;

            let current_time = ShinkaiStringTime::generate_time_now();
            let scope_text = serde_json::to_string(&scope)?;
            let associated_ui_text = associated_ui.map_or(Ok("".to_string()), |ui| serde_json::to_string(&ui))?;
            let config_text = match &config {
                Some(cfg) => serde_json::to_string(cfg)?,
                None => "{}".to_string(),
            };

            let mut stmt = conn.prepare(
                "INSERT INTO jobs (
                    job_id,
                    is_hidden,
                    datetime_created,
                    is_finished,
                    parent_agent_or_llm_provider_id,
                    scope,
                    conversation_inbox_name,
                    associated_ui,
                    config
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;

            stmt.execute(params![
                job_id,
                is_hidden,
                current_time,
                false,
                llm_provider_id,
                scope_text,
                job_inbox_name.clone(),
                associated_ui_text,
                config_text,
            ])?;
        }

        self.create_empty_inbox(job_inbox_name)?;

        Ok(())
    }

    pub fn update_job_config(&self, job_id: &str, config: JobConfig) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        // Serialize the config to a JSON string
        let config_text = serde_json::to_string(&config)?;

        let mut stmt = conn.prepare("UPDATE jobs SET config = ?1 WHERE job_id = ?2")?;

        // Store the JSON as a string
        stmt.execute(params![config_text, job_id])?;

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

    pub fn get_job_with_options(&self, job_id: &str, fetch_step_history: bool) -> Result<Job, SqliteManagerError> {
        let (
            scope,
            is_finished,
            is_hidden,
            datetime_created,
            parent_agent_id,
            conversation_inbox,
            step_history,
            associated_ui,
            config,
            forked_jobs,
        ) = self.get_job_data(job_id, fetch_step_history)?;

        let job = Job {
            job_id: job_id.to_string(),
            is_hidden,
            datetime_created,
            is_finished,
            parent_agent_or_llm_provider_id: parent_agent_id,
            scope,
            conversation_inbox_name: conversation_inbox,
            step_history: step_history.unwrap_or_else(Vec::new),
            associated_ui,
            config,
            forked_jobs,
        };

        Ok(job)
    }

    pub fn get_job(&self, job_id: &str) -> Result<Job, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM jobs WHERE job_id = ?1")?;
        let mut rows = stmt.query(params![job_id])?;

        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;
        self.parse_job_from_row(&row, true)
    }

    #[allow(clippy::type_complexity)]
    fn get_job_data(
        &self,
        job_id: &str,
        fetch_step_history: bool,
    ) -> Result<
        (
            MinimalJobScope,
            bool,
            bool,
            String,
            String,
            InboxName,
            Option<Vec<ShinkaiMessage>>,
            Option<AssociatedUI>,
            Option<JobConfig>,
            Vec<ForkedJob>,
        ),
        SqliteManagerError,
    > {
        let conn = self.get_connection()?;

        let mut stmt = conn.prepare(
            "SELECT
            job_id,
            is_hidden,
            datetime_created,
            is_finished,
            parent_agent_or_llm_provider_id,
            scope,
            conversation_inbox_name,
            associated_ui,
            config
            FROM jobs WHERE job_id = ?1",
        )?;

        let mut rows = stmt.query(params![job_id])?;

        let row = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?;

        let scope_text: String = row.get(5)?;
        let is_finished: bool = row.get(3)?;
        let is_hidden: bool = row.get(1)?;
        let datetime_created: String = row.get(2)?;
        let parent_agent_id: String = row.get(4)?;
        let inbox_name: String = row.get(6)?;
        let conversation_inbox: InboxName =
            InboxName::new(inbox_name).map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;
        let associated_ui_text: Option<String> = row.get(7)?;
        let config_text: Option<String> = row.get(8)?;

        if let Some(ref text) = config_text {
            match serde_json::from_str::<JobConfig>(text) {
                Ok(config) => eprintln!("Deserialized config: {:?}", config),
                Err(e) => eprintln!("Failed to deserialize config: {:?}", e),
            }
        }

        let scope: MinimalJobScope = serde_json::from_str(&scope_text)?;
        let associated_ui = associated_ui_text
            .as_deref()
            .filter(|s| !s.is_empty())
            .map_or(Ok(None), |s| serde_json::from_str(s).map(Some))?;
        let config = config_text
            .as_deref()
            .filter(|s| !s.is_empty())
            .map_or(Ok(None), |s| serde_json::from_str(s).map(Some))?;

        let step_history = if fetch_step_history {
            self.get_step_history(job_id, true)?
        } else {
            None
        };

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
            is_finished,
            is_hidden,
            datetime_created,
            parent_agent_id,
            conversation_inbox,
            step_history,
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
            let job = self.parse_job_from_row(&row, false)?;
            jobs.push(Box::new(job) as Box<dyn JobLike>);
        }

        Ok(jobs)
    }

    pub fn update_job_scope(&self, job_id: String, scope: MinimalJobScope) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;

        let scope_text = serde_json::to_string(&scope.to_json_value()?)?;

        let mut stmt = conn.prepare("UPDATE jobs SET scope = ?1 WHERE job_id = ?2")?;

        stmt.execute(params![scope_text, job_id])?;

        Ok(())
    }

    pub fn get_agent_jobs(&self, agent_id: String) -> Result<Vec<Box<dyn JobLike>>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT * FROM jobs WHERE parent_agent_or_llm_provider_id = ?1")?;
        let mut rows = stmt.query(params![agent_id])?;

        let mut jobs = vec![];

        while let Some(row) = rows.next()? {
            let job = self.parse_job_from_row(&row, false)?;
            jobs.push(Box::new(job) as Box<dyn JobLike>);
        }

        Ok(jobs)
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

    pub fn get_step_history(
        &self,
        job_id: &str,
        fetch_step_history: bool,
    ) -> Result<Option<Vec<ShinkaiMessage>>, SqliteManagerError> {
        if !fetch_step_history {
            return Ok(None);
        }

        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())
            .map_err(|e| SqliteManagerError::SomeError(format!("Error getting inbox name: {}", e)))?;

        let messages = self.get_last_messages_from_inbox(inbox_name.to_string(), 1000, None)?;
        eprintln!("(get_step_history) Messages: {:?}", messages);

        // Map and collect the first element of each inner vector
        let first_messages: Vec<ShinkaiMessage> = messages
            .into_iter()
            .filter_map(|mut msg_vec| {
                if !msg_vec.is_empty() {
                    Some(msg_vec.remove(0))
                } else {
                    None
                }
            })
            .collect();

        eprintln!("(get_step_history) First messages: {:?}", first_messages);

        Ok(Some(first_messages))
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

        tx.execute("DELETE FROM jobs WHERE job_id = ?1", params![job_id])?;

        tx.commit()?;

        Ok(())
    }

    fn parse_job_from_row(&self, row: &rusqlite::Row, fetch_step_history: bool) -> Result<Job, SqliteManagerError> {
        let job_id: String = row.get(0)?;
        let is_hidden: bool = row.get(1)?;
        let datetime_created: String = row.get(2)?;
        let is_finished: bool = row.get(3)?;
        let parent_agent_id: String = row.get(4)?;
        let scope_text: String = row.get(5)?;
        let inbox_name: String = row.get(6)?;
        let conversation_inbox: InboxName =
            InboxName::new(inbox_name).map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;
        let associated_ui_text: Option<String> = row.get(7)?;
        let config_text: Option<String> = row.get(8)?;

        eprintln!("Retrieved config_text: {:?}", config_text);

        if let Some(ref text) = config_text {
            match serde_json::from_str::<JobConfig>(text) {
                Ok(config) => eprintln!("Deserialized config: {:?}", config),
                Err(e) => eprintln!("Failed to deserialize config: {:?}", e),
            }
        }

        let scope: MinimalJobScope = serde_json::from_str(&scope_text)?;
        let associated_ui = associated_ui_text
            .as_deref()
            .filter(|s| !s.is_empty())
            .map_or(Ok(None), |s| serde_json::from_str(s).map(Some))?;
        let config = config_text
            .as_deref()
            .filter(|s| !s.is_empty())
            .map_or(Ok(None), |s| serde_json::from_str(s).map(Some))?;

        let step_history = if fetch_step_history {
            self.get_step_history(&job_id, true)?
        } else {
            None
        };

        let mut forked_jobs = vec![];

        let conn = self.get_connection()?;
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

        Ok(Job {
            job_id,
            is_hidden,
            datetime_created,
            is_finished,
            parent_agent_or_llm_provider_id: parent_agent_id,
            scope,
            conversation_inbox_name: conversation_inbox,
            step_history: step_history.unwrap_or_else(Vec::new),
            associated_ui,
            config,
            forked_jobs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use shinkai_message_primitives::schemas::identity::StandardIdentity;
    use shinkai_message_primitives::schemas::inbox_permission::InboxPermission;
    use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
    use shinkai_message_primitives::{
        schemas::identity::StandardIdentityType,
        shinkai_message::shinkai_message_schemas::{IdentityPermissions, JobMessage, MessageSchemaType},
        shinkai_utils::{
            encryption::{unsafe_deterministic_encryption_keypair, EncryptionMethod},
            shinkai_message_builder::ShinkaiMessageBuilder,
            signatures::unsafe_deterministic_signature_keypair,
        },
    };
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

    fn create_new_job(db: &SqliteManager, job_id: String, agent_id: String, scope: MinimalJobScope) {
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
        let scope = MinimalJobScope::default();

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
            let scope = MinimalJobScope::default();
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
        let scope = MinimalJobScope::default();

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
        let scope = MinimalJobScope::default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        // Update job to finished
        db.update_job_to_finished(&job_id.clone()).unwrap();

        // Retrieve the job and check that is_finished is set to true
        let job = db.get_job(&job_id.clone()).unwrap();
        assert!(job.is_finished);
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
            let scope = MinimalJobScope::default();
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
        let scope = MinimalJobScope::default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        // Check if the job inbox is empty after creating a new job
        assert!(db.is_job_inbox_empty(&job_id).unwrap());

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            job_id.to_string(),
            "something".to_string(),
            vec![],
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
        let scope = MinimalJobScope::default();

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
                vec![],
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
            let result = db
                .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, parent_hash.clone(), None)
                .await;
            eprintln!("result {:?}", result);

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
        eprintln!("last_messages_inbox: {:?}", last_messages_inbox);

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
    async fn test_job_inbox_tree_structure_with_invalid_date() {
        let db = setup_test_db();
        let job_id = "job_test".to_string();
        let agent_id = "agent_test".to_string();
        let scope = MinimalJobScope::default();

        // Create a new job
        create_new_job(&db, job_id.clone(), agent_id.clone(), scope);

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);

        // Create the messages
        let mut messages = Vec::new();
        for i in [1, 3, 2].iter() {
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.clone(),
                format!("Hello World {}", i),
                vec![],
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
        let scope = MinimalJobScope::default();

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
                vec![],
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
        create_new_job(&db, forked_job2_id.clone(), agent_id.clone(), scope.clone());

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
        let scope = MinimalJobScope::default();

        // Create new jobs
        create_new_job(&db, job1_id.clone(), agent_id.clone(), scope.clone());
        create_new_job(&db, job2_id.clone(), agent_id.clone(), scope.clone());

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

    #[tokio::test]
    async fn test_get_job_with_messages() {
        let db = setup_test_db();
        let job_id = "job_with_messages".to_string();
        let agent_id = "agent_test".to_string();
        let scope = MinimalJobScope::default();

        // Create a new job
        db.create_new_job(job_id.clone(), agent_id.clone(), scope.clone(), false, None, None)
            .unwrap();

        let (placeholder_signature_sk, _) = unsafe_deterministic_signature_keypair(0);

        // Create and add messages to the job's inbox
        let mut messages = Vec::new();
        for i in 1..=3 {
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.clone(),
                format!("Test Message {}", i),
                vec![],
                None,
                placeholder_signature_sk.clone(),
                "@@node1.shinkai".to_string(),
                "@@node1.shinkai".to_string(),
            )
            .unwrap();
            messages.push(shinkai_message.clone());

            db.unsafe_insert_inbox_message(&shinkai_message, None, None)
                .await
                .unwrap();
            
            // Add 50 ms delay
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Fetch the job with messages
        let job = db.get_job_with_options(&job_id, true).unwrap();

        // Verify that the messages are retrieved
        assert_eq!(job.forked_jobs.len(), 0); // No forked jobs expected

        // Check the messages
        let job_messages = job.step_history;
        eprintln!("Job Mesages: {:?}", job_messages);

        assert_eq!(job_messages.len(), 3);
        for (i, message) in job_messages.iter().enumerate() {
            let message_content: JobMessage = serde_json::from_str(&message.get_message_content().unwrap()).unwrap();
            assert_eq!(message_content.content, format!("Test Message {}", i + 1));
        }
    }
}
