use std::collections::HashMap;

use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::agent::execution::job_prompts::{Prompt, SubPromptType};
use crate::agent::job::{Job, JobLike, JobStepResult};
use rocksdb::{IteratorMode, Options, WriteBatch};
use shinkai_message_primitives::schemas::{inbox_name::InboxName, shinkai_time::ShinkaiStringTime};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;

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
        let cf_agent_id_name = format!("agentid_{}", &agent_id);
        let cf_job_id_name = format!("jobtopic_{}", &job_id);
        let cf_conversation_inbox_name = format!("job_inbox::{}::false", &job_id);
        let cf_job_id_perms_name = format!("job_inbox::{}::false_perms", &job_id);
        let cf_job_id_read_list_name = format!("job_inbox::{}::false_read_list", &job_id);
        let cf_job_id_unprocessed_messages_name = format!("{}_unprocessed_messages", &job_id);
        let cf_job_id_smart_inbox_name = format!("job_inbox::{}::false_smart_inbox_name", &job_id);
        let cf_job_id_children_name = format!("{}_children", &cf_conversation_inbox_name);
        let cf_job_id_parents_name = format!("{}_parents", &cf_conversation_inbox_name);

        // Check that the cf handles exist, and create them
        if self.db.cf_handle(&cf_job_id_scope_name).is_some()
            || self.db.cf_handle(&cf_job_id_name).is_some()
            || self.db.cf_handle(&cf_conversation_inbox_name).is_some()
            || self.db.cf_handle(&cf_job_id_perms_name).is_some()
            || self.db.cf_handle(&cf_job_id_read_list_name).is_some()
            || self.db.cf_handle(&cf_job_id_unprocessed_messages_name).is_some()
            || self.db.cf_handle(&cf_job_id_smart_inbox_name).is_some()
            || self.db.cf_handle(&cf_job_id_children_name).is_some()
            || self.db.cf_handle(&cf_job_id_parents_name).is_some()
        {
            return Err(ShinkaiDBError::JobAlreadyExists(cf_job_id_name.to_string()));
        }

        if self.db.cf_handle(&cf_agent_id_name).is_none() {
            self.db.create_cf(&cf_agent_id_name, &cf_opts)?;
        }
        self.db.create_cf(&cf_job_id_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_scope_name, &cf_opts)?;
        self.db.create_cf(&cf_conversation_inbox_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_perms_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_read_list_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_unprocessed_messages_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_smart_inbox_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_children_name, &cf_opts)?;
        self.db.create_cf(&cf_job_id_parents_name, &cf_opts)?;

        // Start a write batch
        let mut batch = WriteBatch::default();

        // Generate time currently, used as a key. It should be safe because it's generated here so it shouldn't be duplicated (presumably)
        let current_time = ShinkaiStringTime::generate_time_now();
        let scope_bytes = scope.to_bytes()?;

        let cf_job_id = self.cf_handle(&cf_job_id_name)?;
        let cf_agent_id = self.cf_handle(&cf_agent_id_name)?;
        let cf_job_id_scope = self.cf_handle(&cf_job_id_scope_name)?;

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

        let cf_smart_inbox_name = self
            .db
            .cf_handle(&cf_job_id_smart_inbox_name)
            .expect("to be able to access smart inbox name column family");
        batch.put_cf(
            cf_smart_inbox_name,
            &cf_conversation_inbox_name,
            &cf_conversation_inbox_name,
        );

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
            execution_context,
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
            execution_context,
        };

        Ok(job)
    }

    /// Fetches a job from the DB as a Box<dyn JobLik>
    pub fn get_job_like(&self, job_id: &str) -> Result<Box<dyn JobLike>, ShinkaiDBError> {
        let (
            scope,
            is_finished,
            datetime_created,
            parent_agent_id,
            conversation_inbox,
            _,
            unprocessed_messages,
            execution_context,
        ) = self.get_job_data(job_id, false)?;

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
            execution_context,
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
            Option<Vec<JobStepResult>>,
            Vec<String>,
            HashMap<String, String>,
        ),
        ShinkaiDBError,
    > {
        // Define cf names for all data we need to fetch
        let cf_job_id_name = format!("jobtopic_{}", job_id);
        let cf_job_id_scope_name = format!("{}_scope", job_id);

        // Get the needed cf handles
        let cf_job_id_scope = self
            .db
            .cf_handle(&cf_job_id_scope_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_job_id_scope_name))?;
        let cf_job_id = self
            .db
            .cf_handle(&cf_job_id_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_job_id_name))?;

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

        let conversation_inbox_value = self
            .db
            .get_cf(cf_job_id, JobInfo::ConversationInboxName.to_str().as_bytes())?
            .ok_or(ShinkaiDBError::DataNotFound)?;
        let inbox_name = std::str::from_utf8(&conversation_inbox_value)?.to_string();
        let conversation_inbox = Some(InboxName::new(inbox_name)?);

        // Reads all of the step history by iterating
        let step_history = self.get_step_history(job_id, fetch_step_history)?;

        // Reads all of the unprocessed messages by iterating
        let unprocessed_messages = self.get_unprocessed_messages(job_id)?;

        Ok((
            scope,
            is_finished,
            datetime_created,
            parent_agent_id,
            conversation_inbox.unwrap(),
            step_history,
            unprocessed_messages,
            self.get_job_execution_context(job_id)?,
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

    /// Updates the JobScope of a job given it's id
    pub fn update_job_scope(&mut self, job_id: String, scope: JobScope) -> Result<(), ShinkaiDBError> {
        // Define cf name for the scope we need to update
        let cf_job_id_scope_name = format!("{}_scope", &job_id);

        // Get the needed cf handle
        let cf_job_id_scope = self
            .db
            .cf_handle(&cf_job_id_scope_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_job_id_scope_name))?;

        // Convert the new scope to bytes
        let scope_bytes = scope.to_bytes()?;

        // Update the scope in the DB
        self.db.put_cf(cf_job_id_scope, job_id.as_bytes(), &scope_bytes)?;

        Ok(())
    }

    /// Fetches all jobs under a specific Agent
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

    /// Sets/updates the execution context for a Job in the DB
    pub fn set_job_execution_context(
        &mut self,
        job_id: String,
        context: HashMap<String, String>,
        message_key: Option<String>,
    ) -> Result<(), ShinkaiDBError> {
        let message_key = match message_key {
            Some(key) => key,
            None => {
                // Fetch the most recent message from the job's inbox
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone())?;
                let last_messages = self.get_last_messages_from_inbox(inbox_name.to_string(), 1, None)?;
                if let Some(message) = last_messages.first() {
                    if let Some(message) = message.first() {
                        message.calculate_message_hash()
                    } else {
                        return Err(ShinkaiDBError::SomeError("No messages found in the inbox".to_string()));
                    }
                } else {
                    return Err(ShinkaiDBError::SomeError("No messages found in the inbox".to_string()));
                }
            }
        };

        let cf_name = format!("{}_{}_execution_context", &job_id, &message_key);
        let current_time = ShinkaiStringTime::generate_time_now();

        // Convert the context to bytes
        let context_bytes = bincode::serialize(&context).map_err(|_| {
            ShinkaiDBError::SomeError("Failed converting execution context hashmap to bytes".to_string())
        })?;

        let cf_handle = match self.db.cf_handle(&cf_name) {
            Some(cf) => cf,
            None => {
                // Create Options for ColumnFamily
                let mut cf_opts = Options::default();
                cf_opts.create_if_missing(true);
                cf_opts.create_missing_column_families(true);

                // Create column family if it doesn't exist
                self.db.create_cf(&cf_name, &cf_opts)?;
                self.db
                    .cf_handle(&cf_name)
                    .ok_or(ShinkaiDBError::ProfileNameNonExistent(cf_name.clone()))?
            }
        };

        self.db.put_cf(cf_handle, current_time.as_bytes(), &context_bytes)?;

        Ok(())
    }

    /// Gets the execution context for a job
    pub fn get_job_execution_context(&self, job_id: &str) -> Result<HashMap<String, String>, ShinkaiDBError> {
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())?;
        let mut execution_context: HashMap<String, String> = HashMap::new();

        // Fetch the last message from the job's inbox
        let last_messages = self.get_last_messages_from_inbox(inbox_name.to_string(), 1, None)?;
        if let Some(message_path) = last_messages.first() {
            if let Some(message) = message_path.first() {
                let message_key = message.calculate_message_hash();
                let cf_name = format!("{}_{}_execution_context", job_id, message_key);
                if let Some(cf_handle) = self.db.cf_handle(&cf_name) {
                    // Get the last context (should be only one)
                    let mut iter = self.db.iterator_cf(cf_handle, IteratorMode::End);
                    if let Some(Ok((_, value))) = iter.next() {
                        let context: Result<HashMap<String, String>, ShinkaiDBError> = bincode::deserialize(&value)
                            .map_err(|_| {
                                ShinkaiDBError::SomeError(
                                    "Failed converting execution context bytes to hashmap".to_string(),
                                )
                            });
                        if let Ok(context) = context {
                            execution_context = context;
                        }
                    }
                }
            }
        }
        Ok(execution_context)
    }

    /// Fetches all unprocessed messages for a specific Job from the DB
    fn get_unprocessed_messages(&self, job_id: &str) -> Result<Vec<String>, ShinkaiDBError> {
        // Get the iterator
        let iter = self.get_unprocessed_messages_iterator(job_id)?;

        // Reads all of the unprocessed messages by iterating
        let mut unprocessed_messages: Vec<String> = Vec::new();
        for item in iter {
            let (_key, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
            let message = std::str::from_utf8(&value)?.to_string();
            unprocessed_messages.push(message);
        }

        Ok(unprocessed_messages)
    }

    /// Fetches an iterator over all unprocessed messages for a specific Job from the DB
    fn get_unprocessed_messages_iterator<'a>(
        &'a self,
        job_id: &str,
    ) -> Result<impl Iterator<Item = Result<(Box<[u8]>, Box<[u8]>), rocksdb::Error>> + 'a, ShinkaiDBError> {
        // Get the needed cf handle
        let cf_job_id_unprocessed_messages = self._get_unprocessed_messages_handle(job_id)?;

        // Get the iterator
        let iter = self.db.iterator_cf(cf_job_id_unprocessed_messages, IteratorMode::Start);

        Ok(iter)
    }

    /// Removes the oldest unprocessed message for a specific Job from the DB
    pub fn remove_oldest_unprocessed_message(&self, job_id: &str) -> Result<(), ShinkaiDBError> {
        // Get the needed cf handle
        let cf_job_id_unprocessed_messages = self._get_unprocessed_messages_handle(job_id)?;

        // Get the iterator
        let mut iter = self.get_unprocessed_messages_iterator(job_id)?;

        // Get the oldest message (first item in the iterator)
        if let Some(Ok((key, _))) = iter.next() {
            // Remove the oldest message from the DB
            self.db.delete_cf(cf_job_id_unprocessed_messages, key)?;
        }

        Ok(())
    }

    /// Fetches the column family handle for unprocessed messages of a specific Job
    fn _get_unprocessed_messages_handle(&self, job_id: &str) -> Result<&rocksdb::ColumnFamily, ShinkaiDBError> {
        let cf_job_id_unprocessed_messages_name = format!("{}_unprocessed_messages", job_id);

        // Get the needed cf handle
        let cf_job_id_unprocessed_messages =
            self.db
                .cf_handle(&cf_job_id_unprocessed_messages_name)
                .ok_or(ShinkaiDBError::ColumnFamilyNotFound(
                    cf_job_id_unprocessed_messages_name,
                ))?;

        Ok(cf_job_id_unprocessed_messages)
    }

    /// Updates the Job to being finished
    pub fn update_job_to_finished(&self, job_id: String) -> Result<(), ShinkaiDBError> {
        let cf_name = format!("jobtopic_{}", &job_id);
        let cf_handle = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::ProfileNameNonExistent(cf_name.clone()))?;

        // Check if the job is already finished
        let is_finished_value = self
            .db
            .get_cf(cf_handle, JobInfo::IsFinished.to_str().as_bytes())?
            .ok_or(ShinkaiDBError::DataNotFound)?;
        let is_finished = std::str::from_utf8(&is_finished_value)?.to_string() == "true";

        if is_finished {
            return Err(ShinkaiDBError::SomeError(format!("Job {} is already finished", job_id)));
        }

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
        let current_time = ShinkaiStringTime::generate_time_now();
        self.db.put_cf(cf_handle, current_time.as_bytes(), message.as_bytes())?;
        Ok(())
    }

    pub fn add_step_history(
        &mut self,
        job_id: String,
        user_message: String,
        agent_response: String,
        message_key: Option<String>,
    ) -> Result<(), ShinkaiDBError> {
        let message_key = match message_key {
            Some(key) => key,
            None => {
                // Fetch the most recent message from the job's inbox
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone())?;
                let last_messages = self.get_last_messages_from_inbox(inbox_name.to_string(), 1, None)?;
                if let Some(message) = last_messages.first() {
                    if let Some(message) = message.first() {
                        message.calculate_message_hash()
                    } else {
                        return Err(ShinkaiDBError::SomeError("No messages found in the inbox".to_string()));
                    }
                } else {
                    return Err(ShinkaiDBError::SomeError("No messages found in the inbox".to_string()));
                }
            }
        };

        let cf_name = format!("{}_{}_step_history", &job_id, &message_key);
        let current_time = ShinkaiStringTime::generate_time_now();

        // Create prompt & JobStepResult
        let mut prompt = Prompt::new();
        prompt.add_content(user_message, SubPromptType::User, 100);
        prompt.add_content(agent_response, SubPromptType::Assistant, 100);
        let mut job_step_result = JobStepResult::new();
        job_step_result.add_new_step_revision(prompt);

        // Convert to json and save to DB
        let json = job_step_result
            .to_json()
            .map_err(|e| ShinkaiDBError::DataConversionError(e.to_string()))?;

        let cf_handle = match self.db.cf_handle(&cf_name) {
            Some(cf) => cf,
            None => {
                // Create Options for ColumnFamily
                let mut cf_opts = Options::default();
                cf_opts.create_if_missing(true);
                cf_opts.create_missing_column_families(true);

                // Create column family if it doesn't exist
                self.db.create_cf(&cf_name, &cf_opts)?;
                self.db
                    .cf_handle(&cf_name)
                    .ok_or(ShinkaiDBError::ProfileNameNonExistent(cf_name.clone()))?
            }
        };

        self.db.put_cf(cf_handle, current_time.as_bytes(), json.as_bytes())?;

        // After adding the step to the history, fetch the updated step history
        // let step_history = self.get_step_history(&job_id, true)?;

        // You can print the step history or do something else with it here
        // eprintln!("Updated step history: {:?}", step_history);
        // eprintln!("\n\n");

        Ok(())
    }

    pub fn get_step_history(
        &self,
        job_id: &str,
        fetch_step_history: bool,
    ) -> Result<Option<Vec<JobStepResult>>, ShinkaiDBError> {
        if !fetch_step_history {
            return Ok(None);
        }

        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())?;
        let mut step_history: Vec<JobStepResult> = Vec::new();
        let mut until_offset_key: Option<String> = None;

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
                    let message_key = message.calculate_message_hash();
                    let cf_name = format!("{}_{}_step_history", job_id, message_key);
                    if let Some(cf_handle) = self.db.cf_handle(&cf_name) {
                        let iter = self.db.iterator_cf(cf_handle, IteratorMode::Start);
                        for item in iter {
                            let (_, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
                            let step_json_string = std::str::from_utf8(&value)?.to_string();
                            let step_res = JobStepResult::from_json(&step_json_string)?;
                            step_history.push(step_res);
                        }
                    }
                }
            }

            if let Some(last_message_path) = messages.last() {
                if let Some(last_message) = last_message_path.first() {
                    until_offset_key = Some(last_message.calculate_message_hash());
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

    pub fn is_job_inbox_empty(&self, job_id: &str) -> Result<bool, ShinkaiDBError> {
        let cf_conversation_inbox_name = format!("job_inbox::{}::false", job_id);
        let cf_handle = self
            .db
            .cf_handle(&cf_conversation_inbox_name)
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_conversation_inbox_name.clone()))?;

        let mut iter = self.db.iterator_cf(cf_handle, IteratorMode::Start);
        Ok(iter.next().is_none())
    }

    pub async fn add_message_to_job_inbox(
        &mut self,
        _: &str,
        message: &ShinkaiMessage,
        parent_message_key: Option<String>,
    ) -> Result<(), ShinkaiDBError> {
        self.unsafe_insert_inbox_message(&message, parent_message_key).await?;
        Ok(())
    }
}
