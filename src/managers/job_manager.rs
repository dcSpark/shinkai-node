use crate::db::{db_errors::ShinkaiDBError, ShinkaiDB};
use chrono::Utc;
use reqwest::Identity;
use shinkai_message_wasm::{
    schemas::inbox_name::InboxName,
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{JobCreation, JobMessage, JobPreMessage, JobScope, MessageSchemaType},
    },
    ShinkaiMessageWrapper,
};
use std::result::Result::Ok;
use std::{collections::HashMap, error::Error, sync::Arc};
use std::{fmt, thread};
use tokio::sync::{mpsc, Mutex};
use warp::path::full;

use super::{agent::Agent, IdentityManager};

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
    pub agent_manager: Arc<Mutex<AgentManager>>,
    pub job_manager_receiver: Option<mpsc::Receiver<Vec<JobPreMessage>>>,
}

impl JobManager {
    pub async fn new(db: Arc<Mutex<ShinkaiDB>>, identity_manager: Arc<Mutex<IdentityManager>>) -> Self {
        let (job_manager_sender, job_manager_receiver) = tokio::sync::mpsc::channel(100);
        let agent_manager = AgentManager::new(db, identity_manager, job_manager_sender).await;

        let mut job_manager = Self {
            agent_manager: Arc::new(Mutex::new(agent_manager)),
            job_manager_receiver: Some(job_manager_receiver),
        };
        job_manager.process_received_messages().await;
        job_manager
    }

    pub async fn process_job_message(
        &mut self,
        shinkai_message: ShinkaiMessage,
        job_id: Option<String>,
    ) -> Result<String, JobManagerError> {
        if self.agent_manager.lock().await.is_job_message(shinkai_message.clone()) {
            self.agent_manager
                .lock()
                .await
                .process_job_message(shinkai_message, job_id)
                .await
        } else {
            Err(JobManagerError::NotAJobMessage)
        }
    }

    pub async fn process_received_messages(&mut self) {
        if let Some(mut receiver) = self.job_manager_receiver.take() {
            let agent_manager = Arc::clone(&self.agent_manager);
            tokio::spawn(async move {
                while let Some(messages) = receiver.recv().await {
                    println!("process_received_messages> messages: {:?}", messages);
                    for message in messages {
                        let mut agent_manager = agent_manager.lock().await;
                        println!("calling handle_pre_message_schema> message: {:?}", message);
                        if let Err(err) = agent_manager.handle_pre_message_schema(message).await {
                            eprintln!("Error while handling pre message schema: {:?}", err);
                        }
                    }
                }
            });
        }
    }

    pub async fn decision_phase(&self, job: &dyn JobLike) -> Result<(), Box<dyn Error>> {
        self.agent_manager.lock().await.decision_phase(job).await
    }

    pub async fn execution_phase(
        &self,
        pre_messages: Vec<JobPreMessage>,
    ) -> Result<Vec<ShinkaiMessage>, Box<dyn Error>> {
        self.agent_manager.lock().await.execution_phase(pre_messages).await
    }
}

pub struct AgentManager {
    jobs: Arc<Mutex<HashMap<String, Box<dyn JobLike>>>>,
    db: Arc<Mutex<ShinkaiDB>>,
    identity_manager: Arc<Mutex<IdentityManager>>,
    job_manager_sender: mpsc::Sender<Vec<JobPreMessage>>,
    agents: Vec<Arc<Mutex<Agent>>>,
}

impl AgentManager {
    pub async fn new(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        job_manager_sender: mpsc::Sender<Vec<JobPreMessage>>,
    ) -> Self {
        let jobs_map = Arc::new(Mutex::new(HashMap::new()));
        {
            let shinkai_db = db.lock().await;
            let all_jobs = shinkai_db.get_all_jobs().unwrap();
            let mut jobs = jobs_map.lock().await;
            for job in all_jobs {
                jobs.insert(job.job_id().to_string(), job);
            }
        }

        // Get all serialized_agents and convert them to Agents
        let mut agents: Vec<Arc<Mutex<Agent>>> = Vec::new();
        {
            let identity_manager = identity_manager.lock().await;
            let serialized_agents = identity_manager.get_all_agents().await.unwrap();
            for serialized_agent in serialized_agents {
                let agent = Agent::from_serialized_agent(serialized_agent, job_manager_sender.clone());
                agents.push(Arc::new(Mutex::new(agent)));
            }
        }

        let mut job_manager = Self {
            jobs: jobs_map,
            db,
            job_manager_sender,
            identity_manager,
            agents,
        };
        job_manager
    }

    pub fn is_job_message(&mut self, message: ShinkaiMessage) -> bool {
        match &message.body {
            MessageBody::Unencrypted(body) => match &body.message_data {
                MessageData::Unencrypted(data) => match data.message_content_schema {
                    MessageSchemaType::JobCreationSchema | MessageSchemaType::JobMessageSchema => true,
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        }
    }

    pub async fn handle_job_creation_schema(
        &mut self,
        job_creation: JobCreation,
        agent_id: &String,
    ) -> Result<String, JobManagerError> {
        let job_id = format!("jobid_{}", uuid::Uuid::new_v4());
        {
            let mut shinkai_db = self.db.lock().await;
            match shinkai_db.create_new_job(job_id.clone(), agent_id.clone(), job_creation.scope) {
                Ok(_) => (),
                Err(err) => return Err(JobManagerError::ShinkaiDB(err)),
            };

            match shinkai_db.get_job(&job_id) {
                Ok(job) => {
                    self.jobs.lock().await.insert(job_id.clone(), Box::new(job));

                    let mut agent_found = None;
                    for agent in &self.agents {
                        let locked_agent = agent.lock().await;
                        if &locked_agent.id == agent_id {
                            agent_found = Some(agent.clone());
                            break;
                        }
                    }

                    if agent_found.is_none() {
                        let identity_manager = self.identity_manager.lock().await;
                        if let Some(serialized_agent) = identity_manager.search_local_agent(&agent_id).await {
                            let agent = Agent::from_serialized_agent(serialized_agent, self.job_manager_sender.clone());
                            agent_found = Some(Arc::new(Mutex::new(agent)));
                            self.agents.push(agent_found.clone().unwrap());
                        }
                    }

                    let job_id_to_return = match agent_found {
                        Some(_) => Ok(job_id.clone()),
                        None => Err(anyhow::Error::new(JobManagerError::AgentNotFound)),
                    };

                    job_id_to_return.map_err(|_| JobManagerError::AgentNotFound)
                }
                Err(err) => {
                    return Err(JobManagerError::ShinkaiDB(err));
                }
            }
        }
    }

    pub async fn handle_job_message_schema(&mut self, job_message: JobMessage) -> Result<String, JobManagerError> {
        if let Some(job) = self.jobs.lock().await.get(&job_message.job_id) {
            let job = job.clone();

            let decision_phase_output = self.decision_phase(&**job).await?;

            return Ok(job_message.job_id.clone());
        } else {
            return Err(JobManagerError::JobNotFound);
        }
    }

    pub async fn handle_pre_message_schema(&mut self, pre_message: JobPreMessage) -> Result<String, JobManagerError> {
        // Placeholder logic
        println!("handle_pre_message_schema> pre_message: {:?}", pre_message);
        Ok(String::new())
    }

    pub async fn process_job_message(
        &mut self,
        message: ShinkaiMessage,
        job_id: Option<String>,
    ) -> Result<String, JobManagerError> {
        match message.body {
            MessageBody::Unencrypted(body) => {
                let internal_metadata = body.internal_metadata;
                let agent_id = &internal_metadata.recipient_subidentity;

                match body.message_data {
                    MessageData::Unencrypted(data) => {
                        let message_type = data.message_content_schema;
                        match message_type {
                            MessageSchemaType::JobCreationSchema => {
                                let job_creation: JobCreation =
                                    serde_json::from_str(&data.message_raw_content)
                                        .map_err(|_| JobManagerError::ContentParseFailed)?;
                                self.handle_job_creation_schema(job_creation, agent_id).await
                            }
                            MessageSchemaType::JobMessageSchema => {
                                let job_message: JobMessage =
                                    serde_json::from_str(&data.message_raw_content)
                                        .map_err(|_| JobManagerError::ContentParseFailed)?;
                                self.handle_job_message_schema(job_message).await
                            }
                            MessageSchemaType::PreMessageSchema => {
                                let pre_message: JobPreMessage =
                                    serde_json::from_str(&data.message_raw_content)
                                        .map_err(|_| JobManagerError::ContentParseFailed)?;
                                self.handle_pre_message_schema(pre_message).await
                            }
                            _ => {
                                // Handle Empty message type if needed, or return an error if it's not a valid job message
                                Err(JobManagerError::NotAJobMessage)
                            }
                        }
                    }
                    _ => Err(JobManagerError::NotAJobMessage),
                }
            }
            _ => Err(JobManagerError::NotAJobMessage),
        }
    }

    async fn decision_phase(&self, job: &dyn JobLike) -> Result<(), Box<dyn Error>> {
        // When a new message is supplied to the job, the decision phase of the new step begins running
        // (with its existing step history as context) which triggers calling the Agent's LLM.
        {
            // Add current time as ISO8601 to step history
            let time_with_comment = format!("{}: {}", "Current datetime in RFC3339", Utc::now().to_rfc3339());
            self.db
                .lock()
                .await
                .add_step_history(job.job_id().to_string(), time_with_comment)
                .unwrap();
        }

        let full_job = { self.db.lock().await.get_job(job.job_id()).unwrap() };
        let context = full_job.step_history;

        let agent_id = full_job.parent_agent_id;
        let mut agent_found = None;
        for agent in &self.agents {
            let locked_agent = agent.lock().await;
            if locked_agent.id == agent_id {
                agent_found = Some(agent.clone());
                break;
            }
        }

        let response = match agent_found {
            Some(agent) => {
                // Create a new async task where the agent's execute method will run
                // Note: agent execute run in a separate thread
                tokio::spawn(async move {
                    let mut agent = agent.lock().await;
                    agent.execute("test".to_string(), context).await;
                })
                .await?;
                Ok(())
            }
            None => Err(Box::new(JobManagerError::AgentNotFound)),
        };
        println!("decision_phase> response: {:?}", response);

        // TODO: update this fn so it allows for recursion
        // let is_valid = self.is_decision_phase_output_valid().await;
        // if is_valid == false {
        //     self.decision_phase(job).await?;
        // }

        // The expected output from the LLM is one or more `Premessage`s (a message that potentially
        // still has computation that needs to be performed via tools to fill out its contents).
        // If the output from the LLM does not fit the expected structure, then the LLM is queried again
        // with the exact same inputs until a valid output is provided (potentially supplying extra text
        // each time to the LLM clarifying the previous result was invalid with an example/error message).

        // Make sure the output is valid
        // If not valid, keep calling the LLM until a valid output is produced
        // Return the output
        Ok(())
    }

    async fn is_decision_phase_output_valid(&self) -> bool {
        // Check if the output is valid
        // If not valid, return false
        // If valid, return true
        unimplemented!()
    }

    async fn execution_phase(&self, pre_messages: Vec<JobPreMessage>) -> Result<Vec<ShinkaiMessage>, Box<dyn Error>> {
        // For each Premessage:
        // 1. Call the necessary tools to fill out the contents
        // 2. Convert the Premessage into a Message
        // Return the list of Messages
        unimplemented!()
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
    AgentNotFound,
    ContentParseFailed,
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
            JobManagerError::AgentNotFound => write!(f, "Agent not found"),
            JobManagerError::ContentParseFailed => write!(f, "Failed to parse content"),
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
