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
use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;
use shinkai_job_queue_manager::job_queue_manager::{JobForProcessing, JobQueueManager};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::job::JobLike;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::AssociatedUI;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName, shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage}, shinkai_message_schemas::{JobCreationInfo, JobMessage, MessageSchemaType}
    }, shinkai_utils::signatures::clone_signature_secret_key
};
use shinkai_sqlite::SqliteManager;
use std::collections::HashSet;
use std::env;
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Weak;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, Semaphore};

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
    pub job_queue_manager_normal: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
    pub job_queue_manager_immediate: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
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

        // Create a manager for normal jobs
        let db_prefix_normal = "job_manager_normal_";
        let job_queue_result_normal =
            JobQueueManager::<JobForProcessing>::new(db.clone(), Some(db_prefix_normal.to_string())).await;
        if let Err(ref e) = job_queue_result_normal {
            eprintln!("Error initializing normal JobQueueManager: {:?}", e);
        }
        let job_queue_normal = Arc::new(Mutex::new(job_queue_result_normal.unwrap()));

        // Create a manager for immediate jobs
        let db_prefix_immediate = "job_manager_immediate_";
        let job_queue_result_immediate =
            JobQueueManager::<JobForProcessing>::new(db.clone(), Some(db_prefix_immediate.to_string())).await;
        if let Err(ref e) = job_queue_result_immediate {
            eprintln!("Error initializing immediate JobQueueManager: {:?}", e);
        }
        let job_queue_immediate = Arc::new(Mutex::new(job_queue_result_immediate.unwrap()));

        let max_jobs = env::var("JOB_MANAGER_THREADS")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS);

        // Start processing both queues
        let job_queue_handler = JobManager::process_job_queue(
            job_queue_normal.clone(),
            job_queue_immediate.clone(),
            db.clone(),
            node_profile_name.clone(),
            max_jobs,
            clone_signature_secret_key(&identity_secret_key),
            embedding_generator.clone(),
            ws_manager.clone(),
            tool_router.clone(),
            sheet_manager.clone(),
            callback_manager.clone(),
            Some(my_agent_payments_manager.clone()),
            Some(ext_agent_payments_manager.clone()),
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
            job_queue_manager_normal: job_queue_normal,
            job_queue_manager_immediate: job_queue_immediate,
            job_processing_task: Some(job_queue_handler),
            ws_manager,
        }
    }

    pub async fn process_job_queue(
        queue_normal: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
        queue_immediate: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
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
                Arc<LLMStopper>,
            ) -> Pin<Box<dyn Future<Output = Result<String, LLMProviderError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let mut rx_normal = queue_normal.lock().await.subscribe_to_all().await;
        let mut rx_immediate = queue_immediate.lock().await.subscribe_to_all().await;
        let db_clone = db.clone();
        let identity_sk = clone_signature_secret_key(&identity_sk);
        let job_processing_fn = Arc::new(job_processing_fn);
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
                // First try to process immediate jobs
                let immediate_jobs_to_process: Vec<(String, JobForProcessing)> = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_lock = queue_immediate.lock().await;
                    let all_jobs = match job_queue_manager_lock.get_all_elements_interleave().await {
                        Ok(jobs) => jobs,
                        Err(_) => Vec::new(),
                    };
                    std::mem::drop(job_queue_manager_lock);

                    all_jobs
                        .into_iter()
                        .filter_map(|job| {
                            let job_id = job.job_message.job_id.clone();
                            if !processing_jobs_lock.contains(&job_id) {
                                processing_jobs_lock.insert(job_id.clone());
                                Some((job_id, job))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                };

                if !immediate_jobs_to_process.is_empty() {
                    // Process immediate jobs
                    for (job_id, job) in immediate_jobs_to_process {
                        let permit = semaphore.clone().acquire_owned().await.unwrap();
                        let job_processing_fn = Arc::clone(&job_processing_fn);
                        let db_clone = db_clone.clone();
                        let node_profile_name = node_profile_name.clone();
                        let identity_sk = clone_signature_secret_key(&identity_sk);
                        let generator = generator.clone();
                        let ws_manager = ws_manager.clone();
                        let tool_router = tool_router.clone();
                        let sheet_manager = sheet_manager.clone();
                        let callback_manager = callback_manager.clone();
                        let queue_immediate = Arc::clone(&queue_immediate);
                        let my_agent_payments_manager = my_agent_payments_manager.clone();
                        let ext_agent_payments_manager = ext_agent_payments_manager.clone();
                        let llm_stopper = Arc::clone(&llm_stopper);
                        let processing_jobs = Arc::clone(&processing_jobs);

                        tokio::spawn(async move {
                            let result = (job_processing_fn)(
                                job,
                                db_clone,
                                node_profile_name,
                                identity_sk,
                                generator,
                                ws_manager,
                                tool_router,
                                sheet_manager,
                                callback_manager,
                                queue_immediate.clone(),
                                my_agent_payments_manager,
                                ext_agent_payments_manager,
                                llm_stopper,
                            )
                            .await;

                            if result.is_ok() {
                                let _ = queue_immediate.lock().await.dequeue(&job_id).await;
                            }

                            let mut processing_jobs = processing_jobs.lock().await;
                            processing_jobs.remove(&job_id);
                            drop(permit);
                        });
                    }
                    continue;
                }

                // If no immediate jobs, process normal jobs
                let normal_jobs_to_process: Vec<(String, JobForProcessing)> = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_lock = queue_normal.lock().await;
                    let all_jobs = match job_queue_manager_lock.get_all_elements_interleave().await {
                        Ok(jobs) => jobs,
                        Err(_) => Vec::new(),
                    };
                    std::mem::drop(job_queue_manager_lock);

                    all_jobs
                        .into_iter()
                        .filter_map(|job| {
                            let job_id = job.job_message.job_id.clone();
                            if !processing_jobs_lock.contains(&job_id) {
                                processing_jobs_lock.insert(job_id.clone());
                                Some((job_id, job))
                            } else {
                                None
                            }
                        })
                        .take(max_parallel_jobs)
                        .collect::<Vec<_>>()
                };

                if normal_jobs_to_process.is_empty() {
                    // No jobs in either queue: wait for new jobs
                    tokio::select! {
                        maybe_imm = rx_immediate.recv() => {
                            if maybe_imm.is_none() {
                                eprintln!("rx_immediate closed, shutting down...");
                                break;
                            }
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Info,
                                &format!("Received new immediate job {:?}", maybe_imm.unwrap().job_message.job_id),
                            );
                        }
                        maybe_norm = rx_normal.recv() => {
                            if maybe_norm.is_none() {
                                eprintln!("rx_normal closed, shutting down...");
                                break;
                            }
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Info,
                                &format!("Received new normal job {:?}", maybe_norm.unwrap().job_message.job_id),
                            );
                        }
                    }
                } else {
                    // Process normal jobs
                    for (job_id, job) in normal_jobs_to_process {
                        // Check for immediate jobs before processing each normal job
                        let immediate_jobs_to_process: Vec<(String, JobForProcessing)> = {
                            let mut processing_jobs_lock = processing_jobs.lock().await;
                            let job_queue_manager_lock = queue_immediate.lock().await;
                            let all_jobs = match job_queue_manager_lock.get_all_elements_interleave().await {
                                Ok(jobs) => jobs,
                                Err(_) => Vec::new(),
                            };
                            std::mem::drop(job_queue_manager_lock);

                            all_jobs
                                .into_iter()
                                .filter_map(|job| {
                                    let job_id = job.job_message.job_id.clone();
                                    if !processing_jobs_lock.contains(&job_id) {
                                        processing_jobs_lock.insert(job_id.clone());
                                        Some((job_id, job))
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                        };

                        // If we found immediate jobs, process them first
                        if !immediate_jobs_to_process.is_empty() {
                            for (imm_job_id, imm_job) in immediate_jobs_to_process {
                                let permit = semaphore.clone().acquire_owned().await.unwrap();
                                let job_processing_fn = Arc::clone(&job_processing_fn);
                                let db_clone = db_clone.clone();
                                let node_profile_name = node_profile_name.clone();
                                let identity_sk = clone_signature_secret_key(&identity_sk);
                                let generator = generator.clone();
                                let ws_manager = ws_manager.clone();
                                let tool_router = tool_router.clone();
                                let sheet_manager = sheet_manager.clone();
                                let callback_manager = callback_manager.clone();
                                let queue_immediate = Arc::clone(&queue_immediate);
                                let my_agent_payments_manager = my_agent_payments_manager.clone();
                                let ext_agent_payments_manager = ext_agent_payments_manager.clone();
                                let llm_stopper = Arc::clone(&llm_stopper);
                                let processing_jobs = Arc::clone(&processing_jobs);

                                tokio::spawn(async move {
                                    let result = (job_processing_fn)(
                                        imm_job,
                                        db_clone,
                                        node_profile_name,
                                        identity_sk,
                                        generator,
                                        ws_manager,
                                        tool_router,
                                        sheet_manager,
                                        callback_manager,
                                        queue_immediate.clone(),
                                        my_agent_payments_manager,
                                        ext_agent_payments_manager,
                                        llm_stopper,
                                    )
                                    .await;

                                    if result.is_ok() {
                                        let _ = queue_immediate.lock().await.dequeue(&imm_job_id).await;
                                    }

                                    let mut processing_jobs = processing_jobs.lock().await;
                                    processing_jobs.remove(&imm_job_id);
                                    drop(permit);
                                });
                            }
                        }

                        // Now process the normal job
                        let permit = semaphore.clone().acquire_owned().await.unwrap();
                        let job_processing_fn = Arc::clone(&job_processing_fn);
                        let db_clone = db_clone.clone();
                        let node_profile_name = node_profile_name.clone();
                        let identity_sk = clone_signature_secret_key(&identity_sk);
                        let generator = generator.clone();
                        let ws_manager = ws_manager.clone();
                        let tool_router = tool_router.clone();
                        let sheet_manager = sheet_manager.clone();
                        let callback_manager = callback_manager.clone();
                        let queue_normal = Arc::clone(&queue_normal);
                        let my_agent_payments_manager = my_agent_payments_manager.clone();
                        let ext_agent_payments_manager = ext_agent_payments_manager.clone();
                        let llm_stopper = Arc::clone(&llm_stopper);
                        let processing_jobs = Arc::clone(&processing_jobs);

                        tokio::spawn(async move {
                            let result = (job_processing_fn)(
                                job,
                                db_clone,
                                node_profile_name,
                                identity_sk,
                                generator,
                                ws_manager,
                                tool_router,
                                sheet_manager,
                                callback_manager,
                                queue_normal.clone(),
                                my_agent_payments_manager,
                                ext_agent_payments_manager,
                                llm_stopper,
                            )
                            .await;

                            if result.is_ok() {
                                let _ = queue_normal.lock().await.dequeue(&job_id).await;
                            }

                            let mut processing_jobs = processing_jobs.lock().await;
                            processing_jobs.remove(&job_id);
                            drop(permit);
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

    pub async fn process_job_message(
        &mut self,
        message: ShinkaiMessage,
        high_priority: bool,
    ) -> Result<String, LLMProviderError> {
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
                                    self.add_to_job_processing_queue(message, job_message, high_priority)
                                        .await
                                }
                                _ => {
                                    // Handle Empty message type if needed, or return an error if it's not a valid
                                    // job message
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

            // Create the job
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

    async fn update_job_folder_name(
        &self,
        job_id: &str,
        content: &str,
        db_arc: &SqliteManager,
    ) -> Result<(), LLMProviderError> {
        // Parse the inbox name to check if it's a job inbox
        let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.to_string())?;

        // Get the current folder name before updating
        let old_folder = db_arc
            .get_job_folder_name(job_id)
            .map_err(|e| LLMProviderError::ShinkaiDB(e))?;

        // Update the inbox name
        let mut truncated_content = content.to_string();
        if truncated_content.chars().count() > 120 {
            truncated_content = format!("{}...", truncated_content.chars().take(120).collect::<String>());
        }
        db_arc
            .unsafe_update_smart_inbox_name(&inbox_name.to_string(), &truncated_content)
            .map_err(|e| LLMProviderError::ShinkaiDB(e))?;

        // Get the new folder name after updating
        let new_folder = db_arc
            .get_job_folder_name(job_id)
            .map_err(|e| LLMProviderError::ShinkaiDB(e))?;

        // Move the folder if it exists
        if old_folder.exists() {
            ShinkaiFileManager::move_folder(old_folder, new_folder, db_arc)
                .map_err(|e| LLMProviderError::SomeError(format!("Failed to move folder: {}", e)))?;
        }

        Ok(())
    }

    pub async fn add_to_job_processing_queue(
        &mut self,
        message: ShinkaiMessage,
        job_message: JobMessage,
        high_priority: bool,
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
            self.update_job_folder_name(&job_message.job_id, &job_message.content, &db_arc)
                .await?;
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
        self.add_job_message_to_job_queue(&job_message, &profile, Some(message_hash_id), high_priority)
            .await?;

        Ok(job_message.job_id.clone().to_string())
    }

    pub async fn add_job_message_to_job_queue(
        &mut self,
        job_message: &JobMessage,
        profile: &ShinkaiName,
        message_hash_id: Option<String>,
        high_priority: bool,
    ) -> Result<String, LLMProviderError> {
        let job_for_processing = JobForProcessing::new(job_message.clone(), profile.clone(), message_hash_id);

        if high_priority {
            let mut imm = self.job_queue_manager_immediate.lock().await;
            let _ = imm.push(&job_message.job_id, job_for_processing).await;
        } else {
            let mut norm = self.job_queue_manager_normal.lock().await;
            let _ = norm.push(&job_message.job_id, job_for_processing).await;
        }

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
            self.add_job_message_to_job_queue(job_message, user_profile, message_hash_id_option, false)
                .await
                .map_err(|e| e.to_string())
        })
    }
}
