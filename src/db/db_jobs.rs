use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::managers::job_manager::{Job, JobLike};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use rand::RngCore;
use rocksdb::{Error, IteratorMode, Options, WriteBatch};
use shinkai_message_primitives::schemas::{inbox_name::InboxName, shinkai_time::ShinkaiTime};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobScope;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

enum JobInfo {
    IsFinished,
    DatetimeCreated,
    ParentAgentId,
    ConversationInboxName,
}

impl JobInfo {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "is_finished" => Some(Self::IsFinished),
            "datetime_created" => Some(Self::DatetimeCreated),
            "parent_agent_id" => Some(Self::ParentAgentId),
            "conversation_inbox_name" => Some(Self::ConversationInboxName),
            _ => None,
        }
    }

    fn to_str(&self) -> &'static str {
        match self {
            Self::IsFinished => "is_finished",
            Self::DatetimeCreated => "datetime_created",
            Self::ParentAgentId => "parent_agent_id",
            Self::ConversationInboxName => "conversation_inbox_name",
        }
    }
}

impl ShinkaiDB {
    pub fn create_new_job(&mut self, job_id: String, agent_id: String, scope: JobScope) -> Result<(), ShinkaiDBError> {
        // Create Options for ColumnFamily
        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);
        cf_opts.create_missing_column_families(true);

        // Create ColumnFamilyDescriptors for inbox and permission lists
        let cf_job_id_scope_name = format!("{}_scope", &job_id); // keyed by name and value link to bucket or document
        let cf_job_id_step_history_name = format!("{}_step_history", &job_id); // keyed by time (do I need composite? probably)
        let cf_agent_id_name = format!("agentid_{}", &agent_id);
        let cf_job_id_name = format!("jobtopic_{}", &job_id);
        let cf_conversation_inbox_name = format!("job_inbox::{}::false", &job_id);
        let cf_job_id_perms_name = format!("job_inbox::{}::false_perms", &job_id);
        let cf_job_id_unread_list_name = format!("job_inbox::{}::false_unread_list", &job_id);
        let cf_job_id_unprocessed_messages_name = format!("{}_unprocessed_messages", &job_id);

        // Check that the cf handles exist, and create them
        if self.db.cf_handle(&cf_job_id_scope_name).is_some()
            || self.db.cf_handle(&cf_job_id_step_history_name).is_some()
            || self.db.cf_handle(&cf_job_id_name).is_some()
            || self.db.cf_handle(&cf_conversation_inbox_name).is_some()
            || self.db.cf_handle(&cf_job_id_perms_name).is_some()
            || self.db.cf_handle(&cf_job_id_unread_list_name).is_some()
            || self.db.cf_handle(&cf_job_id_unprocessed_messages_name).is_some()
        {
            return Err(ShinkaiDBError::JobAlreadyExists(cf_job_id_name.to_string()));
        }

        if self.db.cf_handle(&cf_agent_id_name).is_none() {
            self.db.create_cf(&cf_agent_id_name, &cf_opts)?;
        }
        self.db.create_cf(&cf_job_id_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_scope_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_step_history_name, &cf_opts)?;
        self.db.create_cf(&cf_conversation_inbox_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_perms_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_unread_list_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_unprocessed_messages_name, &cf_opts)?;

        // Start a write batch
        let mut batch = WriteBatch::default();

        // Generate time currently, used as a key. It should be safe because it's generated here so it shouldn't be duplicated (presumably)
        let current_time = ShinkaiTime::generate_time_now();
        let scope_bytes = scope.to_bytes()?;

        let cf_job_id = self.db.cf_handle(&cf_job_id_name).unwrap();
        let cf_agent_id = self.db.cf_handle(&cf_agent_id_name).unwrap();
        let cf_job_id_scope = self.db.cf_handle(&cf_job_id_scope_name).unwrap();

        batch.put_cf(cf_agent_id, current_time.as_bytes(), job_id.as_bytes());
        batch.put_cf(cf_job_id_scope, job_id.as_bytes(), &scope_bytes);
        batch.put_cf(cf_job_id, JobInfo::IsFinished.to_str().as_bytes(), b"false");
        batch.put_cf(
            cf_job_id,
            JobInfo::DatetimeCreated.to_str().as_bytes(),
            current_time.as_bytes(),
        );
        batch.put_cf(
            cf_job_id,
            JobInfo::ParentAgentId.to_str().as_bytes(),
            agent_id.as_bytes(),
        );
        batch.put_cf(
            cf_job_id,
            JobInfo::ConversationInboxName.to_str().as_bytes(),
            cf_conversation_inbox_name.as_bytes(),
        );

        let cf_jobs = self
            .db
            .cf_handle(Topic::AllJobsTimeKeyed.as_str())
            .expect("to be able to access Topic::AllJobsTimeKeyed");
        batch.put_cf(cf_jobs, &current_time, &job_id);

        // Add job inbox name to the list in the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::Inbox.as_str())
            .expect("to be able to access Topic::Inbox");
        batch.put_cf(cf_inbox, &cf_conversation_inbox_name, &cf_conversation_inbox_name);

        self.db.write(batch)?;

        Ok(())
    }

    /// Fetches a job from the DB
    pub fn get_job(&self, job_id: &str) -> Result<Job, ShinkaiDBError> {
        let (
            scope,
            is_finished,
            datetime_created,
            parent_agent_id,
            conversation_inbox,
            step_history,
            unprocessed_messages,
        ) = self.get_job_data(job_id, true)?;

        // Construct the job
        let job = Job {
            job_id: job_id.to_string(),
            datetime_created,
            is_finished,
            parent_agent_id,
            scope,
            conversation_inbox_name: conversation_inbox,
            step_history: step_history.unwrap_or_else(Vec::new),
            unprocessed_messages,
        };

        Ok(job)
    }

    /// Fetches a job from the DB as a Box<dyn JobLik>
    pub fn get_job_like(&self, job_id: &str) -> Result<Box<dyn JobLike>, ShinkaiDBError> {
        let (scope, is_finished, datetime_created, parent_agent_id, conversation_inbox, _, unprocessed_messages) =
            self.get_job_data(job_id, false)?;

        // Construct the job
        let job = Job {
            job_id: job_id.to_string(),
            datetime_created,
            is_finished,
            parent_agent_id,
            scope,
            conversation_inbox_name: conversation_inbox,
            step_history: Vec::new(), // Empty step history for JobLike
            unprocessed_messages,
        };

        Ok(Box::new(job))
    }

    /// Fetches data for a specific Job from the DB
    fn get_job_data(
        &self,
        job_id: &str,
        fetch_step_history: bool,
    ) -> Result<
        (
            JobScope,
            bool,
            String,
            String,
            InboxName,
            Option<Vec<String>>,
            Vec<String>,
        ),
        ShinkaiDBError,
    > {
        // Define cf names for all data we need to fetch
        let cf_job_id_name = format!("jobtopic_{}", job_id);
        let cf_job_id_scope_name = format!("{}_scope", job_id);
        let cf_job_id_step_history_name = format!("{}_step_history", job_id);
        let cf_job_id_unprocessed_messages_name = format!("{}_unprocessed_messages", &job_id);

        // Get the needed cf handles
        let cf_job_id_scope = self
            .db
            .cf_handle(&cf_job_id_scope_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_job_id_scope_name))?;
        let cf_job_id = self
            .db
            .cf_handle(&cf_job_id_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_job_id_name))?;
        let cf_job_id_step_history = self
            .db
            .cf_handle(&cf_job_id_step_history_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_job_id_step_history_name))?;
        let cf_job_id_unprocessed_messages =
            self.db
                .cf_handle(&cf_job_id_unprocessed_messages_name)
                .ok_or(ShinkaiDBError::ColumnFamilyNotFound(
                    cf_job_id_unprocessed_messages_name,
                ))?;

        // Begin fetching the data from the DB
        let scope_value = self
            .db
            .get_cf(cf_job_id_scope, job_id)?
            .ok_or(ShinkaiDBError::DataNotFound)?;
        let scope = JobScope::from_bytes(&scope_value)?;

        let is_finished_value = self
            .db
            .get_cf(cf_job_id, JobInfo::IsFinished.to_str().as_bytes())?
            .ok_or(ShinkaiDBError::DataNotFound)?;
        let is_finished = std::str::from_utf8(&is_finished_value)?.to_string() == "true";

        let datetime_created_value = self
            .db
            .get_cf(cf_job_id, JobInfo::DatetimeCreated.to_str().as_bytes())?
            .ok_or(ShinkaiDBError::DataNotFound)?;
        let datetime_created = std::str::from_utf8(&datetime_created_value)?.to_string();

        let parent_agent_id_value = self
            .db
            .get_cf(cf_job_id, JobInfo::ParentAgentId.to_str().as_bytes())?
            .ok_or(ShinkaiDBError::DataNotFound)?;
        let parent_agent_id = std::str::from_utf8(&parent_agent_id_value)?.to_string();

        let mut conversation_inbox: Option<InboxName> = None;
        let mut step_history: Option<Vec<String>> = if fetch_step_history { Some(Vec::new()) } else { None };
        let conversation_inbox_value = self
            .db
            .get_cf(cf_job_id, JobInfo::ConversationInboxName.to_str().as_bytes())?
            .ok_or(ShinkaiDBError::DataNotFound)?;
        let inbox_name = std::str::from_utf8(&conversation_inbox_value)?.to_string();
        conversation_inbox = Some(InboxName::new(inbox_name)?);

        // Reads all of the step history by iterating
        let mut step_history: Option<Vec<String>> = if fetch_step_history { Some(Vec::new()) } else { None };
        if let Some(ref mut step_history) = step_history {
            let iter = self.db.iterator_cf(cf_job_id_step_history, IteratorMode::Start);
            for item in iter {
                let (_key, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
                let step = std::str::from_utf8(&value)?.to_string();
                step_history.push(step);
            }
        }

        // Reads all of the unprocessed messages by iterating
        let mut unprocessed_messages: Vec<String> = Vec::new();
        let iter = self.db.iterator_cf(cf_job_id_unprocessed_messages, IteratorMode::Start);
        for item in iter {
            let (_key, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
            let message = std::str::from_utf8(&value)?.to_string();
            unprocessed_messages.push(message);
        }

        Ok((
            scope,
            is_finished,
            datetime_created,
            parent_agent_id,
            conversation_inbox.unwrap(),
            step_history,
            unprocessed_messages,
        ))
    }

    /// Fetches all jobs
    pub fn get_all_jobs(&self) -> Result<Vec<Box<dyn JobLike>>, ShinkaiDBError> {
        let cf_handle = self
            .db
            .cf_handle(Topic::AllJobsTimeKeyed.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound("AllJobsTimeKeyed".to_string()))?;

        let mut jobs = Vec::new();
        let iter = self.db.iterator_cf(cf_handle, IteratorMode::Start);
        for item in iter {
            let (_key, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
            let job_id = std::str::from_utf8(&value)?.to_string();
            let job = self.get_job_like(&job_id)?;
            jobs.push(job);
        }
        Ok(jobs)
    }

    pub fn get_agent_jobs(&self, agent_id: String) -> Result<Vec<Box<dyn JobLike>>, ShinkaiDBError> {
        let cf_name = format!("agentid_{}", &agent_id);
        let cf_handle = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_name))?;
        let mut jobs = Vec::new();
        let iter = self.db.iterator_cf(cf_handle, IteratorMode::Start);
        for item in iter {
            let (_, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
            let job_id = std::str::from_utf8(&value)?.to_string();
            let job = self.get_job_like(&job_id)?;
            jobs.push(job);
        }
        Ok(jobs)
    }

    pub fn update_job_to_finished(&self, job_id: String) -> Result<(), ShinkaiDBError> {
        let cf_name = format!("jobtopic_{}", &job_id);
        let cf_handle = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(cf_name))?;
        let mut batch = WriteBatch::default();
        batch.put_cf(cf_handle, JobInfo::IsFinished.to_str().as_bytes(), b"true");
        self.db.write(batch)?;
        Ok(())
    }

    /// Adds a message to a job's unprocessed messages list
    pub fn add_to_unprocessed_messages_list(&self, job_id: String, message: String) -> Result<(), ShinkaiDBError> {
        let cf_name = format!("{}_unprocessed_messages", &job_id);
        let cf_handle = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(cf_name))?;
        let current_time = ShinkaiTime::generate_time_now();
        self.db.put_cf(cf_handle, current_time.as_bytes(), message.as_bytes())?;
        Ok(())
    }

    /// Adds a String to a job's step history
    pub fn add_step_history(&self, job_id: String, content: String) -> Result<(), ShinkaiDBError> {
        let cf_name = format!("{}_step_history", &job_id);
        let cf_handle = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(cf_name))?;
        let current_time = ShinkaiTime::generate_time_now();
        self.db.put_cf(cf_handle, current_time.as_bytes(), content.as_bytes())?;
        Ok(())
    }

    pub fn add_message_to_job_inbox(&self, job_id: &str, message: &ShinkaiMessage) -> Result<(), ShinkaiDBError> {
        let cf_conversation_inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())?.to_string();
        let cf_handle = self
            .db
            .cf_handle(&cf_conversation_inbox_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_conversation_inbox_name.clone()))?;

        // Insert the message to AllMessages column family
        self.insert_message_to_all(message)?;

        // Calculate the hash of the message for the key
        let hash_key = message.calculate_message_hash();

        // Get the scheduled time or calculate current time
        let time_key = match message.external_metadata.scheduled_time.is_empty() {
            true => ShinkaiTime::generate_time_now(),
            false => message.external_metadata.scheduled_time.clone(),
        };

        // Create the composite key by concatenating the time_key and the hash_key, with a separator
        let composite_key = format!("{}:::{}", time_key, hash_key);

        // Start a write batch
        let mut batch = WriteBatch::default();

        // Use the composite_key as the key and hash_key as the value in the inbox
        batch.put_cf(cf_handle, composite_key.as_bytes(), hash_key.as_bytes());

        // Add the message to the unread_list inbox
        let cf_unread_list = self
            .db
            .cf_handle(&format!("{}_unread_list", cf_conversation_inbox_name))
            .expect("Failed to get cf handle for unread_list");
        batch.put_cf(cf_unread_list, composite_key.as_bytes(), hash_key.as_bytes());

        // Write the batch
        self.db.write(batch)?;

        Ok(())
    }
}
