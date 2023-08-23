use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::managers::job_manager::{Job, JobLike};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use rand::RngCore;
use rocksdb::{Error, IteratorMode, Options, WriteBatch};
use shinkai_message_wasm::schemas::inbox_name::InboxName;
use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::JobScope;
use shinkai_message_wasm::shinkai_utils::shinkai_message_handler::ShinkaiMessageHandler;
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
    pub fn create_new_job(
        &mut self,
        job_id: String,
        agent_id: String,
        scope: JobScope,
    ) -> Result<(), ShinkaiDBError> {
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

        // Check that the profile name exists in ProfilesIdentityKey, ProfilesEncryptionKey and ProfilesIdentityType
        if self.db.cf_handle(&cf_job_id_scope_name).is_some()
            || self.db.cf_handle(&cf_job_id_step_history_name).is_some()
            || self.db.cf_handle(&cf_job_id_name).is_some()
            || self.db.cf_handle(&cf_conversation_inbox_name).is_some()
        {
            return Err(ShinkaiDBError::ProfileNameAlreadyExists);
        }

        if self.db.cf_handle(&cf_agent_id_name).is_none() {
            self.db.create_cf(&cf_agent_id_name, &cf_opts)?;
        }

        self.db.create_cf(&cf_job_id_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_scope_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_step_history_name, &cf_opts)?;
        self.db.create_cf(&cf_conversation_inbox_name, &cf_opts)?;

        // Start a write batch
        let mut batch = WriteBatch::default();

        // Generate time now used as a key. it should be safe because it's generated here so it shouldn't be duplicated (presumably)
        let current_time = ShinkaiMessageHandler::generate_time_now();
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

        self.db.write(batch)?;

        Ok(())
    }

    pub fn get_job(&self, job_id: &str) -> Result<Job, ShinkaiDBError> {
        let (scope, is_finished, datetime_created, parent_agent_id, conversation_inbox, step_history) =
            self.get_job_data(job_id, true)?;

        // Construct the job
        let job = Job {
            job_id: job_id.to_string(),
            datetime_created,
            is_finished,
            parent_agent_id,
            scope,
            conversation_inbox_name: conversation_inbox,
            step_history: step_history.unwrap_or_else(Vec::new),
        };

        Ok(job)
    }

    pub fn get_job_like(&self, job_id: &str) -> Result<Box<dyn JobLike>, ShinkaiDBError> {
        let (scope, is_finished, datetime_created, parent_agent_id, conversation_inbox, _) =
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
        };

        Ok(Box::new(job))
    }

    fn get_job_data(
        &self,
        job_id: &str,
        fetch_step_history: bool,
    ) -> Result<(JobScope, bool, String, String, InboxName, Option<Vec<String>>), ShinkaiDBError> {
        let cf_job_id_name = format!("jobtopic_{}", job_id);
        let cf_job_id_scope_name = format!("{}_scope", job_id);
        let cf_job_id_step_history_name = format!("{}_step_history", job_id);
    
        let cf_job_id_scope = self
            .db
            .cf_handle(&cf_job_id_scope_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_job_id_scope_name))?;
        let cf_job_id = self
            .db
            .cf_handle(&cf_job_id_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_job_id_name))?;
    
        let scope_value = self.db.get_cf(cf_job_id_scope, job_id)?.ok_or(ShinkaiDBError::DataNotFound)?;
        let scope = JobScope::from_bytes(&scope_value)?;
    
        let is_finished_value = self.db.get_cf(cf_job_id, JobInfo::IsFinished.to_str().as_bytes())?.ok_or(ShinkaiDBError::DataNotFound)?;
        let is_finished = std::str::from_utf8(&is_finished_value)?.to_string() == "true";
    
        let cf_job_id_step_history = self
            .db
            .cf_handle(&cf_job_id_step_history_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_job_id_step_history_name))?;
    
        let datetime_created_value = self.db.get_cf(cf_job_id, JobInfo::DatetimeCreated.to_str().as_bytes())?.ok_or(ShinkaiDBError::DataNotFound)?;
        let datetime_created = std::str::from_utf8(&datetime_created_value)?.to_string();
    
        let parent_agent_id_value = self.db.get_cf(cf_job_id, JobInfo::ParentAgentId.to_str().as_bytes())?.ok_or(ShinkaiDBError::DataNotFound)?;
        let parent_agent_id = std::str::from_utf8(&parent_agent_id_value)?.to_string();
    
        let mut conversation_inbox: Option<InboxName> = None;
        let mut step_history: Option<Vec<String>> = if fetch_step_history { Some(Vec::new()) } else { None };
    
        let conversation_inbox_value = self.db.get_cf(
            cf_job_id,
            JobInfo::ConversationInboxName.to_str().as_bytes(),
        )?.ok_or(ShinkaiDBError::DataNotFound)?;
        let inbox_name = std::str::from_utf8(&conversation_inbox_value)?.to_string();
        conversation_inbox = Some(InboxName::new(inbox_name)?);
    
        if let Some(ref mut step_history) = step_history {
            let iter = self.db.iterator_cf(cf_job_id_step_history, IteratorMode::Start);
            for item in iter {
                let (_key, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
                let step = std::str::from_utf8(&value)?.to_string();
                step_history.push(step);
            }
        }
    
        Ok((
            scope,
            is_finished,
            datetime_created,
            parent_agent_id,
            conversation_inbox.unwrap(),
            step_history,
        ))
    }
    
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

    pub fn add_step_history(&self, job_id: String, step: String) -> Result<(), ShinkaiDBError> {
        let cf_name = format!("{}_step_history", &job_id);
        let cf_handle = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(cf_name))?;
        let current_time = ShinkaiMessageHandler::generate_time_now();
        self.db.put_cf(cf_handle, current_time.as_bytes(), step.as_bytes())?;
        Ok(())
    }
}
