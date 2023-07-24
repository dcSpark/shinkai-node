use crate::{
    schemas::{
        inbox_name::InboxName,
        message_schemas::{JobCreation, JobMessage, JobPreMessage, JobScope, MessageSchemaType},
    },
    shinkai_message::{
        shinkai_message_extension::{ParsedContent, ShinkaiMessageWrapper},
        shinkai_message_handler::ShinkaiMessageHandler,
    },
    shinkai_message_proto::ShinkaiMessage, db::{ShinkaiDB, db_errors::ShinkaiDBError},
};
use std::{collections::HashMap, error::Error, sync::Arc};
use std::{fmt, thread};
use tokio::sync::{mpsc, Mutex};

use super::agent::Agent;

pub trait JobLike: Send + Sync {
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
    jobs: Arc<Mutex<HashMap<String, Box<dyn JobLike>>>>,
    db: Arc<Mutex<ShinkaiDB>>,
    job_manager_sender: mpsc::Sender<ShinkaiMessage>,
    agents: Vec<Arc<Mutex<Agent>>>,
}

impl JobManager {
    pub async fn new(db: Arc<Mutex<ShinkaiDB>>) -> Self {
        let jobs_map = Arc::new(Mutex::new(HashMap::new()));
        let (sender, receiver) = mpsc::channel(100);
        {
            let shinkai_db = db.lock().await;
            let all_jobs = shinkai_db.get_all_jobs().unwrap();
            let mut jobs = jobs_map.lock().await;
            for job in all_jobs {
                jobs.insert(job.job_id().to_string(), job);
            }
        }
        Self {
            jobs: jobs_map,
            db,
            job_manager_sender: sender,
            agents: Vec::new(),
        }
    }

    pub fn is_job_message(&mut self, message: ShinkaiMessage) -> bool {
        match MessageSchemaType::from_str(&message.body.unwrap().internal_metadata.unwrap().message_schema_type) {
            Some(MessageSchemaType::JobCreationSchema)
            | Some(MessageSchemaType::JobMessageSchema)
            | Some(MessageSchemaType::PreMessageSchema) => true,
            _ => false,
        }
    }

    pub async fn process_job_message(
        &mut self,
        shinkai_message: ShinkaiMessage,
        job_id: Option<String>,
    ) -> Result<String, JobManagerError> {
        let message = ShinkaiMessageWrapper::from(&shinkai_message);
        let body = message.body;
        let message_type_str = &body.internal_metadata.message_schema_type;
        let message_type =
            MessageSchemaType::from_str(message_type_str).ok_or(JobManagerError::MessageTypeParseFailed)?;

        match message_type {
            MessageSchemaType::JobCreationSchema => {
                if let ParsedContent::JobCreation(job_creation) = body.parsed_content {
                    let agent_subidentity = &body.internal_metadata.recipient_subidentity;
                    // TODO: check if valid recipient_subidentity if not return an error agent not found
                    let job_id = format!("jobid_{}", uuid::Uuid::new_v4());
                    {
                        let mut shinkai_db = self.db.lock().await;
                        match shinkai_db.create_new_job(job_id.clone(), agent_subidentity.clone(), job_creation.scope) {
                            Ok(_) => (),
                            Err(err) => return Err(JobManagerError::ShinkaiDB(err)),
                        };

                        match shinkai_db.get_job(&job_id) {
                            Ok(job) => {
                                self.jobs.lock().await.insert(job_id.clone(), Box::new(job));

                                // TODO: Start job by calling
                                if let Some(agent) = self.agents.first() {
                                    // or use some logic to select the right agent
                                    let agent = agent.lock().await;
                                    // TODO: check that process_message is not awaited
                                    agent.process_message(shinkai_message);
                                }

                                return Ok(job_id.clone());
                            }
                            Err(err) => {
                                return Err(JobManagerError::ShinkaiDB(err));
                            }
                        }
                    }
                } else {
                    return Err(JobManagerError::JobCreationDeserializationFailed);
                }
            }
            MessageSchemaType::JobMessageSchema => {
                if let ParsedContent::JobMessage(job_message) = body.parsed_content {
                    // Check if the job exists
                    if let Some(job) = self.jobs.lock().await.get(&job_message.job_id) {
                        // Clone the job for use within async block
                        let job = job.clone();
                        // Perform some logic related to the JobMessageSchema message type
                        // The decision phase
                        let decision_phase_output = self.decision_phase(&**job).await?;
                        // The execution phase
                        let execution_phase_output = self.execution_phase(decision_phase_output).await;
                        return Ok(job_message.job_id.clone());
                    } else {
                        return Err(JobManagerError::JobNotFound);
                    }
                } else {
                    return Err(JobManagerError::JobMessageDeserializationFailed);
                }
            }
            MessageSchemaType::PreMessageSchema => {
                if let ParsedContent::PreMessage(pre_message) = body.parsed_content {
                    // Perform some logic related to the PreMessageSchema message type
                    // This is just a placeholder logic
                    // TODO: implement the real logic
                    Ok(String::new())
                } else {
                    return Err(JobManagerError::JobPreMessageDeserializationFailed);
                }
            }
            _ => return Err(JobManagerError::NotAJobMessage),
        }
    }

    async fn decision_phase(&self, job: &dyn JobLike) -> Result<Vec<JobPreMessage>, Box<dyn Error>> {
        // Write to the LLM inbox with the job step history
        // TODO: do agents are constantly pulling messages? or do they have to be notified?

        // Make sure the output is valid
        // If not valid, keep calling the LLM until a valid output is produced
        // Return the output
        unimplemented!()
    }

    async fn execution_phase(&self, pre_messages: Vec<JobPreMessage>) -> Result<Vec<ShinkaiMessage>, Box<dyn Error>> {
        // For each Premessage:
        // 1. Call the necessary tools to fill out the contents
        // 2. Convert the Premessage into a Message
        // Return the list of Messages
        unimplemented!()
    }

    pub fn add_agent(&mut self, id: String) {
        // let agent = Agent::new(id, self.job_manager_sender.clone());
        // self.agents.push(agent);
    }

    pub fn start_agents(&self) {
        let agents_clone = self.agents.clone();
        for agent in &agents_clone {
            let agent_clone = agent.clone();
            let handler = async move {
                // let mut agent_locked = agent_clone.lock().unwrap();
                // agent_locked.start().await;
            };
            tokio::spawn(handler);
        }
    }
}

#[derive(Debug)]
pub enum JobManagerError {
    NotAJobMessage,
    JobNotFound,
    JobCreationDeserializationFailed,
    JobMessageDeserializationFailed,
    JobPreMessageDeserializationFailed,
    MessageTypeParseFailed,
    IO(String),
    ShinkaiDB(ShinkaiDBError),
}

impl fmt::Display for JobManagerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            JobManagerError::NotAJobMessage => write!(f, "Message is not a job message"),
            JobManagerError::JobNotFound => write!(f, "Job not found"),
            JobManagerError::JobCreationDeserializationFailed => write!(f, "Failed to deserialize JobCreation message"),
            JobManagerError::JobMessageDeserializationFailed => write!(f, "Failed to deserialize JobMessage"),
            JobManagerError::JobPreMessageDeserializationFailed => write!(f, "Failed to deserialize JobPreMessage"),
            JobManagerError::MessageTypeParseFailed => write!(f, "Could not parse message type"),
            JobManagerError::IO(err) => write!(f, "IO error: {}", err),
            JobManagerError::ShinkaiDB(err) => write!(f, "Shinkai DB error: {}", err),
        }
    }
}

impl std::error::Error for JobManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JobManagerError::ShinkaiDB(err) => Some(err),
            _ => None,
        }
    }
}

impl From<Box<dyn std::error::Error>> for JobManagerError {
    fn from(err: Box<dyn std::error::Error>) -> JobManagerError {
        JobManagerError::IO(err.to_string())
    }
}
