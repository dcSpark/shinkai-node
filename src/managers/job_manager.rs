use std::{collections::HashMap, error::Error, sync::Arc};

use crate::{
    db::{ShinkaiMessageDB, db_errors::ShinkaiMessageDBError},
    schemas::{inbox_name::InboxName, job_schemas::{JobScope, JobCreation, JobMessage, JobPreMessage}, message_schemas::MessageSchemaType},
    shinkai_message_proto::ShinkaiMessage, shinkai_message::shinkai_message_handler::ShinkaiMessageHandler,
};
use tokio::sync::Mutex;

pub trait JobLike {
    fn job_id(&self) -> &str;
    fn datetime_created(&self) -> &str;
    fn is_finished(&self) -> bool;
    fn parent_agent_id(&self) -> &str;
    fn scope(&self) -> &JobScope;
    fn conversation_inbox_name(&self) -> &InboxName;
}

#[derive(Clone)]
pub struct Job {
    // based on uuid
    pub job_id: String,
    // Format: "20230702T20533481346" or Utc::now().format("%Y%m%dT%H%M%S%f").to_string();
    pub datetime_created: String,
    // determines if the job is finished or not
    pub is_finished: bool,
    // identity of the parent agent. We just use a full identity name for simplicity
    pub parent_agent_id: String,
    // what storage buckets and/or documents are accessible to the LLM via vector search
    // and/or direct querying based off bucket name/key
    pub scope: JobScope,
    // an inbox where messages to the agent from the user and messages from the agent are stored,
    // enabling each job to have a classical chat/conversation UI
    pub conversation_inbox_name: InboxName,
    // A step history (an ordered list of all messages submitted to the LLM which triggered a step to execute,
    // including everything in the conversation inbox + any messages from the agent recursively calling itself or otherwise)
    pub step_history: Vec<String>,
}

impl JobLike for Job {
    fn job_id(&self) -> &str {
        &self.job_id
    }

    fn datetime_created(&self) -> &str {
        &self.datetime_created
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn parent_agent_id(&self) -> &str {
        &self.parent_agent_id
    }

    fn scope(&self) -> &JobScope {
        &self.scope
    }

    fn conversation_inbox_name(&self) -> &InboxName {
        &self.conversation_inbox_name
    }
}

pub struct JobManager {
    jobs: HashMap<String, Box<dyn JobLike>>,
    db: Arc<Mutex<ShinkaiMessageDB>>,
}

impl JobManager {
    pub async fn new(db: Arc<Mutex<ShinkaiMessageDB>>) -> Self {
        let mut jobs_map = HashMap::new();
        {
            let shinkai_db = db.lock().await;
            let jobs = shinkai_db.get_all_jobs().unwrap();
            for job in jobs {
                jobs_map.insert(job.job_id().to_string(), job);
            }
        }
        Self { db, jobs: jobs_map }
    }

    pub fn is_job_message(&mut self, message: ShinkaiMessage) -> bool {
        match MessageSchemaType::from_str(&message.body.unwrap().internal_metadata.unwrap().message_schema_type) {
            Some(MessageSchemaType::JobCreationSchema)
            | Some(MessageSchemaType::JobMessageSchema)
            | Some(MessageSchemaType::PreMessageSchema) => true,
            _ => false,
        }
    }

    pub async fn process_job_message(&mut self, message: ShinkaiMessage) -> Result<String, Box<dyn Error>> {
        if !self.is_job_message(message.clone()) {
            return Err("Message is not a job message.".into());
        }
        // Unwrap the message_schema_type
        let message_type_str = &message.clone().body.unwrap().internal_metadata.unwrap().message_schema_type;
        // Parse it into a MessageSchemaType
        let message_type = MessageSchemaType::from_str(message_type_str).ok_or("Could not parse message type.")?;

        match message_type {
            MessageSchemaType::JobCreationSchema => {
                let job_creation: JobCreation = serde_json::from_str(&message.clone().body.unwrap().content)
                    .map_err(|_| "Failed to deserialize JobCreation message.")?;

                let agent_subidentity = message.clone().body.unwrap().internal_metadata.unwrap().recipient_subidentity;
                // TODO: check if valid recipient_subidentity if not return an error agent not found
                let job_id = format!("jobid_{}", uuid::Uuid::new_v4());
                { 
                    let mut shinkai_db = self.db.lock().await;
                    shinkai_db.create_new_job(job_id.clone(), agent_subidentity.clone(), job_creation.scope).unwrap();
                    // get job

                    match shinkai_db.get_job(&job_id) {
                        Ok(job) => {
                            self.jobs.insert(job_id.clone(), Box::new(job));
                            return Ok(job_id.clone())
                        },
                        Err(e) => {
                            assert_eq!(e, ShinkaiMessageDBError::ProfileNameNonExistent);
                            return Err::<String, Box<dyn std::error::Error>>(Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Job not found")))
                        },
                    }
                }
            }
            MessageSchemaType::JobMessageSchema => {
                let job_message: JobMessage = serde_json::from_str(&message.clone().body.unwrap().content)
                    .map_err(|_| "Failed to deserialize JobMessage.")?;

                // Perform some logic related to the JobMessageSchema message type
                // TODO: implement the real logic
                if let Some(job) = self.jobs.get_mut(&job_message.job_id) {
                    // job.step_history.push(job_message.content);
                    return Ok(job_message.job_id.clone());
                } else {
                    return Err("Job not found.".into());
                }
            }
            MessageSchemaType::PreMessageSchema => {
                let body = &message.clone().body.unwrap();
                let pre_message: JobPreMessage = serde_json::from_str(&body.content)
                    .map_err(|_| "Failed to deserialize JobPreMessage.")?;

                // Perform some logic related to the PreMessageSchema message type
                // This is just a placeholder logic
                // TODO: implement the real logic
                return Ok(String::new());
            }
            _ => return Err("Message is not a job message.".into()),
        }
    }

    pub fn execute_job(&self, job_id: String) -> Result<(), Box<dyn Error>> {
        let job = self.jobs.get(&job_id).ok_or("Job id does not exist.")?;
        // job execution logic here
        Ok(())
    }
}
