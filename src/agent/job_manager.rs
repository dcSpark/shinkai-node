use super::error::AgentError;
use super::queue::job_queue_manager::{JobForProcessing, JobQueueManager};
use crate::agent::agent::Agent;
pub use crate::agent::execution::job_execution_core::*;
use crate::agent::job::{Job, JobId, JobLike};
use crate::db::{db_errors::ShinkaiDBError, ShinkaiDB};
use crate::managers::IdentityManager;
use ed25519_dalek::SecretKey as SignatureStaticKey;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::{
    schemas::shinkai_name::{ShinkaiName, ShinkaiNameError},
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{JobCreationInfo, JobMessage, JobPreMessage, MessageSchemaType},
    },
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use std::collections::HashSet;
use std::result::Result::Ok;
use std::{collections::HashMap, error::Error, sync::Arc};
use tokio::sync::{mpsc, Mutex, Semaphore};

const NUM_THREADS: usize = 2;

pub struct JobManager {
    pub jobs: Arc<Mutex<HashMap<String, Box<dyn JobLike>>>>,
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    pub agents: Vec<Arc<Mutex<Agent>>>,
    pub identity_secret_key: SignatureStaticKey,
    pub job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
    pub node_profile_name: ShinkaiName,

    // TODO: remove them
    pub job_manager_receiver: Arc<Mutex<mpsc::Receiver<(Vec<JobPreMessage>, JobId)>>>,
    pub job_manager_sender: mpsc::Sender<(Vec<JobPreMessage>, JobId)>,
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
                let agent = Agent::from_serialized_agent(serialized_agent);
                agents.push(Arc::new(Mutex::new(agent)));
            }
        }

        let job_queue = JobQueueManager::<JobForProcessing>::new(db.clone()).await.unwrap();

        let mut job_manager = Self {
            db,
            job_manager_receiver: Arc::new(Mutex::new(job_manager_receiver)),
            job_manager_sender: job_manager_sender.clone(),
            identity_secret_key,
            node_profile_name,
            jobs: jobs_map,
            identity_manager,
            agents,
            job_queue_manager: Arc::new(Mutex::new(job_queue)),
        };
        // Start processing the job queue
        job_manager.process_job_queue(NUM_THREADS).await;
        job_manager
    }

    pub async fn process_job_queue(&mut self, max_parallel_jobs: usize) {
        let job_queue_manager = Arc::clone(&self.job_queue_manager);
        let mut receiver = job_queue_manager.lock().await.subscribe_to_all();
        let db_clone = self.db.clone();
        let identity_sk = clone_signature_secret_key(&self.identity_secret_key);

        let processing_jobs = Arc::new(Mutex::new(HashSet::new()));
        let semaphore = Arc::new(Semaphore::new(max_parallel_jobs));

        tokio::spawn(async move {
            eprintln!("Starting job queue processing loop");

            loop {
                // Scope for acquiring and releasing the lock quickly
                let job_ids_to_process: Vec<String> = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let all_jobs = job_queue_manager
                        .lock()
                        .await
                        .get_all_elements_interleave()
                        .await
                        .unwrap_or(Vec::new());

                    all_jobs
                        .into_iter()
                        .filter_map(|job| {
                            let job_id = job.job_message.job_id.clone().to_string();
                            if !processing_jobs_lock.contains(&job_id) {
                                processing_jobs_lock.insert(job_id.clone());
                                Some(job_id)
                            } else {
                                None
                            }
                        })
                        .collect()
                };

                // Spawn tasks based on filtered job IDs
                for job_id in job_ids_to_process {
                    eprintln!("job id: {:?}", job_id);
                    let job_queue_manager = Arc::clone(&job_queue_manager);
                    let processing_jobs = Arc::clone(&processing_jobs);
                    let semaphore = Arc::clone(&semaphore);
                    let db_clone_2 = db_clone.clone();
                    let identity_sk_clone = clone_signature_secret_key(&identity_sk);

                    tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.unwrap();
                        let mut job_queue_manager = job_queue_manager.lock().await;
                        match job_queue_manager.dequeue(&job_id).await {
                            Ok(Some(job)) => {
                                eprintln!("Processing job {:?}", job);
                                JobManager::process_job_message_queued(job, db_clone_2, identity_sk_clone).await;
                            }
                            Ok(None) => {}
                            Err(e) => {
                                // Log the error
                            }
                        }
                        processing_jobs.lock().await.remove(&job_id);
                    });
                }

                // Receive new jobs
                if let Some(new_job) = receiver.recv().await {
                    // let new_job_id = new_job.job_message.job_id.clone().to_string();
                    // let mut processing_jobs_lock = processing_jobs.lock().await;
                    // processing_jobs_lock.insert(new_job_id);
                }
            }
        });
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

    // From JobManager
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
                            let agent = Agent::from_serialized_agent(serialized_agent);
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

    pub async fn add_to_job_processing_queue(
        &mut self,
        message: ShinkaiMessage,
        job_message: JobMessage,
    ) -> Result<String, AgentError> {
        // TODO: save to the queue
        // Verify identity/profile match
        let sender_subidentity_result = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message.clone());
        let sender_subidentity = match sender_subidentity_result {
            Ok(subidentity) => subidentity,
            Err(e) => return Err(AgentError::InvalidSubidentity(e)),
        };
        let profile_result = sender_subidentity.extract_profile();
        let profile = match profile_result {
            Ok(profile) => profile,
            Err(e) => return Err(AgentError::InvalidProfileSubidentity(e.to_string())),
        };

        let job_for_processing = JobForProcessing::new(job_message.clone(), profile.clone());

        let mut job_queue_manager = self.job_queue_manager.lock().await;
        let _ = job_queue_manager.push(&job_message.job_id, job_for_processing).await;

        Ok(job_message.job_id.clone().to_string())
    }
}
