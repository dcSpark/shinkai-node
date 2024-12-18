use super::error::LLMProviderError;
use super::job_callback_manager::JobCallbackManager;
use super::llm_stopper::LLMStopper;
use crate::managers::sheet_manager::SheetManager;
use crate::managers::tool_router::ToolRouter;
use crate::managers::IdentityManager;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use ed25519_dalek::SigningKey;
use futures::Future;
use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_job_queue_manager::job_queue_manager::{JobForProcessing, JobQueueManager};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job::JobLike;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::AssociatedUI;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{JobCreationInfo, JobMessage, MessageSchemaType},
    },
    shinkai_utils::signatures::clone_signature_secret_key,
};
use shinkai_sqlite::SqliteManager;

use std::collections::HashSet;
use std::env;
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Weak;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock, Semaphore};

const NUM_THREADS: usize = 4;

pub trait JobManagerTrait {
    fn create_job<'a>(
        &'a mut self,
        job_creation_info: JobCreationInfo,
        user_profile: &'a ShinkaiName,
        agent_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;

    fn queue_job_message<'a>(
        &'a mut self,
        job_message: &'a JobMessage,
        user_profile: &'a ShinkaiName,
        message_hash_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>;
}

pub struct JobManager {
    pub jobs: Arc<Mutex<HashMap<String, Box<dyn JobLike>>>>,
    pub db: Weak<SqliteManager>,
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    pub identity_secret_key: SigningKey,
    pub job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
    pub node_profile_name: ShinkaiName,
    pub job_processing_task: Option<tokio::task::JoinHandle<()>>,
    // Websocket manager for sending updates to the frontend
    pub ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
}

impl JobManager {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        db: Weak<SqliteManager>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        identity_secret_key: SigningKey,
        node_profile_name: ShinkaiName,
        embedding_generator: RemoteEmbeddingGenerator,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        sheet_manager: Arc<Mutex<SheetManager>>,
        callback_manager: Arc<Mutex<JobCallbackManager>>,
        my_agent_payments_manager: Arc<Mutex<MyAgentOfferingsManager>>,
        ext_agent_payments_manager: Arc<Mutex<ExtAgentOfferingsManager>>,
        // sqlite_logger: Option<Arc<SqliteLogger>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Self {
        let jobs_map = Arc::new(Mutex::new(HashMap::new()));
        {
            let db_arc = db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
            let all_jobs = db_arc.get_all_jobs().unwrap();
            let mut jobs = jobs_map.lock().await;
            for job in all_jobs {
                jobs.insert(job.job_id().to_string(), job);
            }
        }

        let db_prefix = "job_manager_abcdeprefix_";
        let job_queue = JobQueueManager::<JobForProcessing>::new(db.clone(), Some(db_prefix.to_string()))
            .await
            .unwrap();
        let job_queue_manager = Arc::new(Mutex::new(job_queue));

        let thread_number = env::var("JOB_MANAGER_THREADS")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS);

        // Start processing the job queue
        let job_queue_handler = JobManager::process_job_queue(
            job_queue_manager.clone(),
            db.clone(),
            node_profile_name.clone(),
            thread_number,
            clone_signature_secret_key(&identity_secret_key),
            embedding_generator.clone(),
            ws_manager.clone(),
            tool_router.clone(),
            sheet_manager.clone(),
            callback_manager.clone(),
            Some(my_agent_payments_manager.clone()),
            Some(ext_agent_payments_manager.clone()),
            // sqlite_logger.clone(),
            llm_stopper.clone(),
            |job,
             db,
             node_profile_name,
             identity_sk,
             generator,
             ws_manager,
             tool_router,
             sheet_manager,
             callback_manager,
             job_queue_manager,
             my_agent_payments_manager,
             ext_agent_payments_manager,
             //  sqlite_logger,
             llm_stopper| {
                Box::pin(JobManager::process_job_message_queued(
                    job,
                    db,
                    node_profile_name,
                    identity_sk,
                    generator,
                    ws_manager,
                    tool_router,
                    sheet_manager,
                    callback_manager,
                    job_queue_manager,
                    my_agent_payments_manager.clone(),
                    ext_agent_payments_manager.clone(),
                    // sqlite_logger.clone(),
                    llm_stopper.clone(),
                ))
            },
        )
        .await;

        Self {
            db: db.clone(),
            identity_secret_key: clone_signature_secret_key(&identity_secret_key),
            node_profile_name,
            jobs: jobs_map,
            identity_manager,
            job_queue_manager: job_queue_manager.clone(),
            job_processing_task: Some(job_queue_handler),
            ws_manager,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_job_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
        db: Weak<SqliteManager>,
        node_profile_name: ShinkaiName,
        max_parallel_jobs: usize,
        identity_sk: SigningKey,
        generator: RemoteEmbeddingGenerator,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        sheet_manager: Arc<Mutex<SheetManager>>,
        callback_manager: Arc<Mutex<JobCallbackManager>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        // sqlite_logger: Option<Arc<SqliteLogger>>,
        llm_stopper: Arc<LLMStopper>,
        job_processing_fn: impl Fn(
                JobForProcessing,
                Weak<SqliteManager>,
                ShinkaiName,
                SigningKey,
                RemoteEmbeddingGenerator,
                Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
                Option<Arc<ToolRouter>>,
                Arc<Mutex<SheetManager>>,
                Arc<Mutex<JobCallbackManager>>,
                Arc<Mutex<JobQueueManager<JobForProcessing>>>,
                Option<Arc<Mutex<MyAgentOfferingsManager>>>,
                Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
                // Option<Arc<SqliteLogger>>,
                Arc<LLMStopper>,
            ) -> Pin<Box<dyn Future<Output = Result<String, LLMProviderError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let job_queue_manager = Arc::clone(&job_queue_manager);
        let mut receiver = job_queue_manager.lock().await.subscribe_to_all().await;
        let db_clone = db.clone();
        let identity_sk = clone_signature_secret_key(&identity_sk);
        let job_processing_fn = Arc::new(job_processing_fn);
        // let sqlite_logger = sqlite_logger.clone();
        let llm_stopper = Arc::clone(&llm_stopper);
        let processing_jobs = Arc::new(Mutex::new(HashSet::new()));
        let semaphore = Arc::new(Semaphore::new(max_parallel_jobs));

        tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                "Starting job queue processing loop",
            );

            loop {
                // Fetch jobs to process
                let job_ids_to_process: Vec<String> = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_lock = job_queue_manager.lock().await;
                    let all_jobs = match job_queue_manager_lock.get_all_elements_interleave().await {
                        Ok(jobs) => jobs,
                        Err(_) => Vec::new(),
                    };
                    std::mem::drop(job_queue_manager_lock);

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
                        .take(max_parallel_jobs)
                        .collect::<Vec<_>>()
                };

                if job_ids_to_process.is_empty() {
                    // No jobs to process, wait for new jobs
                    if let Some(new_job) = receiver.recv().await {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Info,
                            &format!("Received new job {:?}", new_job.job_message.job_id),
                        );
                    } else {
                        // Receiver closed, exit the loop
                        eprintln!("Receiver closed, exiting job queue processing loop");
                        break;
                    }
                } else {
                    // Spawn tasks for the jobs
                    for job_id in job_ids_to_process {
                        let job_queue_manager = Arc::clone(&job_queue_manager);
                        let processing_jobs = Arc::clone(&processing_jobs);
                        let semaphore = Arc::clone(&semaphore);
                        let db_clone_2 = db_clone.clone();
                        let identity_sk_clone = clone_signature_secret_key(&identity_sk);
                        let job_processing_fn = Arc::clone(&job_processing_fn);
                        let cloned_generator = generator.clone();
                        let node_profile_name = node_profile_name.clone();
                        let ws_manager = ws_manager.clone();
                        let tool_router = tool_router.clone();
                        let sheet_manager = sheet_manager.clone();
                        let callback_manager = callback_manager.clone();
                        let my_agent_payments_manager = my_agent_payments_manager.clone();
                        let ext_agent_payments_manager = ext_agent_payments_manager.clone();
                        // let sqlite_logger = sqlite_logger.clone();
                        let llm_stopper = Arc::clone(&llm_stopper);

                        tokio::spawn(async move {
                            let _permit = semaphore.acquire().await.unwrap();

                            // Acquire the lock, dequeue the job, and immediately release the lock
                            let job = {
                                let job_queue_manager = job_queue_manager.lock().await;
                                job_queue_manager.peek(&job_id).await
                            };

                            match job {
                                Ok(Some(job)) => {
                                    // Acquire the lock, process the job, and immediately release the lock
                                    let result = {
                                        let result = (job_processing_fn)(
                                            job,
                                            db_clone_2,
                                            node_profile_name,
                                            identity_sk_clone,
                                            cloned_generator,
                                            ws_manager,
                                            tool_router,
                                            sheet_manager,
                                            callback_manager,
                                            job_queue_manager.clone(),
                                            my_agent_payments_manager,
                                            ext_agent_payments_manager,
                                            // sqlite_logger,
                                            llm_stopper,
                                        )
                                        .await;
                                        if let Ok(Some(_)) = job_queue_manager.lock().await.dequeue(&job_id).await {
                                            result
                                        } else {
                                            Err(LLMProviderError::JobDequeueFailed(job_id.clone()))
                                        }
                                    };

                                    if result.is_ok() {
                                        shinkai_log(
                                            ShinkaiLogOption::JobExecution,
                                            ShinkaiLogLevel::Debug,
                                            "Job processed successfully",
                                        );
                                    } else {
                                        shinkai_log(
                                            ShinkaiLogOption::JobExecution,
                                            ShinkaiLogLevel::Error,
                                            "Job processing failed",
                                        );
                                    }
                                }
                                Ok(None) => {
                                    // Job not found, possibly already processed
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Debug,
                                        &format!("Job {} not found", job_id),
                                    );
                                }
                                Err(e) => {
                                    // Log the error
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        &format!("Error peeking job {}: {:?}", job_id, e),
                                    );
                                }
                            }
                            drop(_permit);
                            processing_jobs.lock().await.remove(&job_id);
                        });
                    }
                }
            }

            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                "Job queue processing loop has been terminated",
            );
        })
    }

    pub async fn process_job_message(&mut self, message: ShinkaiMessage) -> Result<String, LLMProviderError> {
        let profile = ShinkaiName::from_shinkai_message_using_recipient_subidentity(&message)?;

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
                                    let agent_id = agent_name
                                        .get_agent_name_string()
                                        .ok_or(LLMProviderError::LLMProviderNotFound)?;
                                    let mut job_creation: JobCreationInfo =
                                        serde_json::from_str(&data.message_raw_content)
                                            .map_err(|_| LLMProviderError::ContentParseFailed)?;

                                    // Delete later
                                    // Treat empty string in associated_ui as None
                                    if let Some(AssociatedUI::Sheet(ref sheet)) = job_creation.associated_ui {
                                        if sheet.is_empty() {
                                            job_creation.associated_ui = None;
                                        }
                                    }

                                    self.process_job_creation(job_creation, &profile, &agent_id).await
                                }
                                MessageSchemaType::JobMessageSchema => {
                                    let job_message: JobMessage = serde_json::from_str(&data.message_raw_content)
                                        .map_err(|_| LLMProviderError::ContentParseFailed)?;
                                    self.add_to_job_processing_queue(message, job_message).await
                                }
                                _ => {
                                    // Handle Empty message type if needed, or return an error if it's not a valid job message
                                    Err(LLMProviderError::NotAJobMessage)
                                }
                            }
                        }
                        _ => Err(LLMProviderError::NotAJobMessage),
                    }
                }
                _ => Err(LLMProviderError::NotAJobMessage),
            }
        } else {
            Err(LLMProviderError::NotAJobMessage)
        }
    }

    // From JobManager
    /// Checks that the provided ShinkaiMessage is an unencrypted job message
    pub fn is_job_message(&mut self, message: ShinkaiMessage) -> bool {
        matches!(
            &message.body,
            MessageBody::Unencrypted(body) if matches!(
                &body.message_data,
                MessageData::Unencrypted(data) if matches!(
                    data.message_content_schema,
                    MessageSchemaType::JobCreationSchema | MessageSchemaType::JobMessageSchema
                )
            )
        )
    }

    /// Processes a job creation message
    pub async fn process_job_creation(
        &mut self,
        job_creation: JobCreationInfo,
        _profile: &ShinkaiName,
        llm_or_agent_provider_id: &String,
    ) -> Result<String, LLMProviderError> {
        let job_id = format!("jobid_{}", uuid::Uuid::new_v4());
        {
            let db_arc = self.db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
            let is_hidden = job_creation.is_hidden.unwrap_or(false);
            match db_arc.create_new_job(
                job_id.clone(),
                llm_or_agent_provider_id.clone(),
                job_creation.scope,
                is_hidden,
                job_creation.associated_ui,
                None,
            ) {
                Ok(_) => (),
                Err(err) => return Err(LLMProviderError::ShinkaiDB(err)),
            };

            match db_arc.get_job(&job_id) {
                Ok(job) => {
                    std::mem::drop(db_arc);
                    self.jobs.lock().await.insert(job_id.clone(), Box::new(job));
                    Ok(job_id.clone())
                }
                Err(err) => Err(LLMProviderError::ShinkaiDB(err)),
            }
        }
    }

    pub async fn add_to_job_processing_queue(
        &mut self,
        message: ShinkaiMessage,
        job_message: JobMessage,
    ) -> Result<String, LLMProviderError> {
        // Verify identity/profile match
        let sender_subidentity_result = ShinkaiName::from_shinkai_message_using_sender_subidentity(&message.clone());
        let sender_subidentity = match sender_subidentity_result {
            Ok(subidentity) => subidentity,
            Err(e) => return Err(LLMProviderError::InvalidSubidentity(e)),
        };
        let profile_result = sender_subidentity.extract_profile();
        let profile = match profile_result {
            Ok(profile) => profile,
            Err(e) => return Err(LLMProviderError::InvalidProfileSubidentity(e.to_string())),
        };

        let db_arc = self.db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
        let is_empty = db_arc.is_job_inbox_empty(&job_message.job_id.clone())?;
        if is_empty {
            let mut content = job_message.clone().content;
            if content.chars().count() > 120 {
                let truncated_content: String = content.chars().take(120).collect();
                content = format!("{}...", truncated_content);
            }
            let inbox_name = InboxName::get_job_inbox_name_from_params(job_message.job_id.to_string())?.to_string();
            db_arc.update_smart_inbox_name(&inbox_name.to_string(), &content)?;
        }

        db_arc
            .add_message_to_job_inbox(
                &job_message.job_id.clone(),
                &message,
                job_message.parent.clone(),
                self.ws_manager.clone(),
            )
            .await?;
        std::mem::drop(db_arc);

        let message_hash_id = message.calculate_message_hash_for_pagination();
        self.add_job_message_to_job_queue(&job_message, &profile, Some(message_hash_id))
            .await?;

        Ok(job_message.job_id.clone().to_string())
    }

    pub async fn add_job_message_to_job_queue(
        &mut self,
        job_message: &JobMessage,
        profile: &ShinkaiName,
        message_hash_id: Option<String>,
    ) -> Result<String, LLMProviderError> {
        let job_for_processing = JobForProcessing::new(job_message.clone(), profile.clone(), message_hash_id);

        let mut job_queue_manager = self.job_queue_manager.lock().await;
        let _ = job_queue_manager.push(&job_message.job_id, job_for_processing).await;

        Ok(job_message.job_id.clone().to_string())
    }
}

impl JobManagerTrait for JobManager {
    fn create_job<'a>(
        &'a mut self,
        job_creation_info: JobCreationInfo,
        user_profile: &'a ShinkaiName,
        agent_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            self.process_job_creation(job_creation_info, user_profile, &agent_id.to_string())
                .await
                .map_err(|e| e.to_string())
        })
    }

    fn queue_job_message<'a>(
        &'a mut self,
        job_message: &'a JobMessage,
        user_profile: &'a ShinkaiName,
        message_hash_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
        Box::pin(async move {
            let message_hash_id_option = if message_hash_id.is_empty() {
                None
            } else {
                Some(message_hash_id.to_string())
            };
            self.add_job_message_to_job_queue(job_message, user_profile, message_hash_id_option)
                .await
                .map_err(|e| e.to_string())
        })
    }
}
