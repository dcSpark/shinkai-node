use super::error::AgentError;
use super::queue::job_queue_manager::{JobForProcessing, JobQueueManager};
use crate::agent::agent::Agent;
pub use crate::agent::execution::job_execution_core::*;
use crate::agent::job::{Job, JobId, JobLike};
use crate::db::{db_errors::ShinkaiDBError, ShinkaiDB};
use crate::managers::IdentityManager;
use crate::resources::bert_cpp::BertCPPProcess;
use ed25519_dalek::SecretKey as SignatureStaticKey;
use futures::Future;
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
use std::mem;
use std::pin::Pin;
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
    pub job_processing_task: Option<tokio::task::JoinHandle<()>>,
    _bert_process: BertCPPProcess,

    // TODO: remove them
    pub job_manager_receiver: Arc<Mutex<mpsc::Receiver<(Vec<JobPreMessage>, JobId)>>>,
    pub job_manager_sender: mpsc::Sender<(Vec<JobPreMessage>, JobId)>,
}

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
        let job_queue_manager = Arc::new(Mutex::new(job_queue));

        let _bert_process = BertCPPProcess::start().unwrap(); // Gets killed if out of scope
        // Start processing the job queue
        let job_queue_handler = JobManager::process_job_queue(
            job_queue_manager.clone(),
            db.clone(),
            NUM_THREADS,
            clone_signature_secret_key(&identity_secret_key),
            |job, db, identity_sk| Box::pin(JobManager::process_job_message_queued(job, db, identity_sk)),
        )
        .await;

        let job_manager = Self {
            db: db.clone(),
            job_manager_receiver: Arc::new(Mutex::new(job_manager_receiver)),
            job_manager_sender: job_manager_sender.clone(),
            identity_secret_key: clone_signature_secret_key(&identity_secret_key),
            node_profile_name,
            jobs: jobs_map,
            identity_manager,
            agents,
            job_queue_manager: job_queue_manager.clone(),
            job_processing_task: Some(job_queue_handler),
            _bert_process,
        };

        job_manager
    }

    pub async fn process_job_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
        db: Arc<Mutex<ShinkaiDB>>,
        max_parallel_jobs: usize,
        identity_sk: SignatureStaticKey,
        job_processing_fn: impl Fn(
                JobForProcessing,
                Arc<Mutex<ShinkaiDB>>,
                SignatureStaticKey,
            ) -> Pin<Box<dyn Future<Output = Result<String, AgentError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let job_queue_manager = Arc::clone(&job_queue_manager);
        let mut receiver = job_queue_manager.lock().await.subscribe_to_all().await;
        let db_clone = db.clone();
        let identity_sk = clone_signature_secret_key(&identity_sk);
        let job_processing_fn = Arc::new(job_processing_fn);

        let processing_jobs = Arc::new(Mutex::new(HashSet::new()));
        let semaphore = Arc::new(Semaphore::new(max_parallel_jobs));

        return tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                "Starting job queue processing loop",
            );

            let mut handles = Vec::new();
            loop {
                // Scope for acquiring and releasing the lock quickly
                let job_ids_to_process: Vec<String> = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_lock = job_queue_manager.lock().await;
                    let all_jobs = job_queue_manager_lock
                        .get_all_elements_interleave()
                        .await
                        .unwrap_or(Vec::new());
                    std::mem::drop(job_queue_manager_lock);

                    let jobs = all_jobs
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
                        .collect();

                    std::mem::drop(processing_jobs_lock);
                    jobs
                };

                // Spawn tasks based on filtered job IDs
                for job_id in job_ids_to_process {
                    let job_queue_manager = Arc::clone(&job_queue_manager);
                    let processing_jobs = Arc::clone(&processing_jobs);
                    let semaphore = Arc::clone(&semaphore);
                    let db_clone_2 = db_clone.clone();
                    let identity_sk_clone = clone_signature_secret_key(&identity_sk);
                    let job_processing_fn = Arc::clone(&job_processing_fn);

                    let handle = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.unwrap();

                        // Acquire the lock, dequeue the job, and immediately release the lock
                        let job = {
                            let mut job_queue_manager = job_queue_manager.lock().await;
                            let job = job_queue_manager.peek(&job_id).await;
                            job
                        };

                        match job {
                            Ok(Some(job)) => {
                                // Acquire the lock, process the job, and immediately release the lock
                                let result = {
                                    let result = job_processing_fn(job, db_clone_2, identity_sk_clone).await;
                                    if let Ok(Some(_)) = job_queue_manager.lock().await.dequeue(&job_id.clone()).await {
                                        result
                                    } else {
                                        Err(AgentError::JobDequeueFailed(job_id.clone()))
                                    }
                                };

                                match result {
                                    Ok(_) => {
                                        shinkai_log(
                                            ShinkaiLogOption::JobExecution,
                                            ShinkaiLogLevel::Debug,
                                            "Job processed successfully",
                                        );
                                    } // handle success case
                                    Err(e) => {} // handle error case
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                // Log the error
                            }
                        }
                        drop(_permit);
                        processing_jobs.lock().await.remove(&job_id);
                    });
                    handles.push(handle);
                }

                let handles_to_join = mem::replace(&mut handles, Vec::new());
                futures::future::join_all(handles_to_join).await;
                handles.clear();

                // Receive new jobs
                if let Some(new_job) = receiver.recv().await {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Info,
                        format!("Received new job {:?}", new_job.job_message.job_id).as_str(),
                    );
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

        let mut shinkai_db = self.db.lock().await;
        shinkai_db.add_message_to_job_inbox(&job_message.job_id.clone(), &message)?;
        std::mem::drop(shinkai_db);

        let job_for_processing = JobForProcessing::new(job_message.clone(), profile.clone());

        let mut job_queue_manager = self.job_queue_manager.lock().await;
        let _ = job_queue_manager.push(&job_message.job_id, job_for_processing).await;

        Ok(job_message.job_id.clone().to_string())
    }
}
