use super::error::AgentError;
use super::queue::job_queue_manager::{JobForProcessing, JobQueueManager};
use crate::agent::agent::Agent;
pub use crate::agent::execution::job_execution_core::*;
use crate::agent::job::{Job, JobId, JobLike};
use crate::db::{db_errors::ShinkaiDBError, ShinkaiDB};
use crate::managers::IdentityManager;
use ed25519_dalek::SecretKey as SignatureStaticKey;
use shinkai_message_primitives::{
    schemas::shinkai_name::{ShinkaiName, ShinkaiNameError},
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{JobCreationInfo, JobMessage, JobPreMessage, MessageSchemaType},
    },
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use std::result::Result::Ok;
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex};

const NUM_THREADS: usize = 2;

pub struct JobManager {
    pub jobs: Arc<Mutex<HashMap<String, Box<dyn JobLike>>>>,
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    pub job_manager_receiver: Arc<Mutex<mpsc::Receiver<(Vec<JobPreMessage>, JobId)>>>,
    pub job_manager_sender: mpsc::Sender<(Vec<JobPreMessage>, JobId)>,
    pub agents: Vec<Arc<Mutex<Agent>>>,
    pub identity_secret_key: SignatureStaticKey,
    pub job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
    pub node_profile_name: ShinkaiName,
    pub thread_pool: rayon::ThreadPool,
}

// TODO: Maybe remove this altogether?
impl JobManager {
    pub async fn new(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        identity_secret_key: SignatureStaticKey,
        node_profile_name: ShinkaiName,
    ) -> Self {
        let (job_manager_sender, job_manager_receiver) = mpsc::channel(100);

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

        // NUM_THREADS is an arbitrary number, we can change it whenever we have a better sense on how to set it up
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(NUM_THREADS)
            .build()
            .unwrap();

        let job_queue = JobQueueManager::<JobForProcessing>::new(db.clone()).await.unwrap();

        let job_manager = Self {
            db,
            job_manager_receiver: Arc::new(Mutex::new(job_manager_receiver)),
            job_manager_sender: job_manager_sender.clone(),
            identity_secret_key,
            node_profile_name,
            thread_pool,
            jobs: jobs_map,
            identity_manager,
            agents,
            job_queue_manager: Arc::new(Mutex::new(job_queue)),
        };
        job_manager
    }

    pub async fn process_job_message(&mut self, message: ShinkaiMessage) -> Result<String, AgentError> {
        if self.is_job_message(message.clone()) {
            match message.clone().body {
                MessageBody::Unencrypted(body) => {
                    match body.message_data {
                        MessageData::Unencrypted(data) => {
                            let message_type = data.message_content_schema;
                            match message_type {
                                MessageSchemaType::JobCreationSchema => {
                                    let agent_name =
                                        ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;
                                    let agent_id = agent_name.get_agent_name().ok_or(AgentError::AgentNotFound)?;
                                    let job_creation: JobCreationInfo = serde_json::from_str(&data.message_raw_content)
                                        .map_err(|_| AgentError::ContentParseFailed)?;
                                    self.process_job_creation(job_creation, &agent_id).await
                                }
                                MessageSchemaType::JobMessageSchema => {
                                    let job_message: JobMessage = serde_json::from_str(&data.message_raw_content)
                                        .map_err(|_| AgentError::ContentParseFailed)?;
                                    self.add_to_job_processing_queue(message, job_message).await
                                }
                                _ => {
                                    // Handle Empty message type if needed, or return an error if it's not a valid job message
                                    Err(AgentError::NotAJobMessage)
                                }
                            }
                        }
                        _ => Err(AgentError::NotAJobMessage),
                    }
                }
                _ => Err(AgentError::NotAJobMessage),
            }
        } else {
            Err(AgentError::NotAJobMessage)
        }
    }

    // From AgentManager
    /// Checks that the provided ShinkaiMessage is an unencrypted job message
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

    /// Processes a job creation message
    pub async fn process_job_creation(
        &mut self,
        job_creation: JobCreationInfo,
        agent_id: &String,
    ) -> Result<String, AgentError> {
        // TODO: add job_id to agent so it's aware
        let job_id = format!("jobid_{}", uuid::Uuid::new_v4());
        {
            let mut shinkai_db = self.db.lock().await;
            match shinkai_db.create_new_job(job_id.clone(), agent_id.clone(), job_creation.scope) {
                Ok(_) => (),
                Err(err) => return Err(AgentError::ShinkaiDB(err)),
            };

            match shinkai_db.get_job(&job_id) {
                Ok(job) => {
                    std::mem::drop(shinkai_db); // require to avoid deadlock
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
                        None => Err(anyhow::Error::new(AgentError::AgentNotFound)),
                    };

                    job_id_to_return.map_err(|_| AgentError::AgentNotFound)
                }
                Err(err) => {
                    return Err(AgentError::ShinkaiDB(err));
                }
            }
        }
    }

    /// Adds pre-message to job inbox
    pub async fn handle_pre_message_schema(
        &mut self,
        pre_message: JobPreMessage,
        job_id: String,
        shinkai_message: ShinkaiMessage,
    ) -> Result<String, AgentError> {
        self.db
            .lock()
            .await
            .add_message_to_job_inbox(job_id.as_str(), &shinkai_message)?;
        Ok(String::new())
    }
}
