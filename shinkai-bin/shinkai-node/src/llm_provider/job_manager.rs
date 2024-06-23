use super::error::LLMProviderError;
use super::queue::job_queue_manager::{JobForProcessing, JobQueueManager};
use crate::llm_provider::llm_provider::LLMProvider;
use crate::llm_provider::job::JobLike;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::vector_fs::vector_fs::VectorFS;
use ed25519_dalek::SigningKey;
use futures::Future;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{
        shinkai_message::{MessageBody, MessageData, ShinkaiMessage},
        shinkai_message_schemas::{JobCreationInfo, JobMessage, MessageSchemaType},
    },
    shinkai_utils::signatures::clone_signature_secret_key,
};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::file_parser::unstructured_api::UnstructuredAPI;
use std::collections::HashSet;
use std::env;
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Weak;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, Semaphore};

const NUM_THREADS: usize = 4;

pub struct JobManager {
    pub jobs: Arc<Mutex<HashMap<String, Box<dyn JobLike>>>>,
    pub db: Weak<ShinkaiDB>,
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    pub llm_providers: Vec<Arc<Mutex<LLMProvider>>>,
    pub identity_secret_key: SigningKey,
    pub job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
    pub node_profile_name: ShinkaiName,
    pub job_processing_task: Option<tokio::task::JoinHandle<()>>,
    pub vector_fs: Weak<VectorFS>,
    // An EmbeddingGenerator initialized with the Node's default embedding model + server info
    pub embedding_generator: RemoteEmbeddingGenerator,
    /// Unstructured server connection
    pub unstructured_api: UnstructuredAPI,
    // Websocket manager for sending updates to the frontend
    pub ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
}

impl JobManager {
    pub async fn new(
        db: Weak<ShinkaiDB>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        identity_secret_key: SigningKey,
        node_profile_name: ShinkaiName,
        vector_fs: Weak<VectorFS>,
        embedding_generator: RemoteEmbeddingGenerator,
        unstructured_api: UnstructuredAPI,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
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

        // Get all serialized_llm_providers and convert them to LLM Providers
        let mut llm_providers: Vec<Arc<Mutex<LLMProvider>>> = Vec::new();
        {
            let identity_manager = identity_manager.lock().await;
            let serialized_llm_providers = identity_manager.get_all_llm_providers().await.unwrap();
            for serialized_agent in serialized_llm_providers {
                let llm_provider = LLMProvider::from_serialized_llm_provider(serialized_agent);
                llm_providers.push(Arc::new(Mutex::new(llm_provider)));
            }
        }

        let db_prefix = "job_manager_abcdeprefix_";
        let job_queue = JobQueueManager::<JobForProcessing>::new(
            db.clone(),
            Topic::AnyQueuesPrefixed.as_str(),
            Some(db_prefix.to_string()),
        )
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
            vector_fs.clone(),
            node_profile_name.clone(),
            thread_number,
            clone_signature_secret_key(&identity_secret_key),
            embedding_generator.clone(),
            unstructured_api.clone(),
            |job, db, vector_fs, node_profile_name, identity_sk, generator, unstructured_api| {
                Box::pin(JobManager::process_job_message_queued(
                    job,
                    db,
                    vector_fs,
                    node_profile_name,
                    identity_sk,
                    generator,
                    unstructured_api,
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
            llm_providers,
            job_queue_manager: job_queue_manager.clone(),
            job_processing_task: Some(job_queue_handler),
            vector_fs,
            embedding_generator,
            unstructured_api,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_job_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_profile_name: ShinkaiName,
        max_parallel_jobs: usize,
        identity_sk: SigningKey,
        generator: RemoteEmbeddingGenerator,
        unstructured_api: UnstructuredAPI,
        job_processing_fn: impl Fn(
                JobForProcessing,
                Weak<ShinkaiDB>,
                Weak<VectorFS>,
                ShinkaiName,
                SigningKey,
                RemoteEmbeddingGenerator,
                UnstructuredAPI,
            ) -> Pin<Box<dyn Future<Output = Result<String, LLMProviderError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let job_queue_manager = Arc::clone(&job_queue_manager);
        let mut receiver = job_queue_manager.lock().await.subscribe_to_all().await;
        let db_clone = db.clone();
        let vector_fs_clone = vector_fs.clone();
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
                let mut continue_immediately = false;

                // Scope for acquiring and releasing the lock quickly
                let job_ids_to_process: Vec<String> = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_lock = job_queue_manager.lock().await;
                    let all_jobs = job_queue_manager_lock
                        .get_all_elements_interleave()
                        .await
                        .unwrap_or(Vec::new());
                    std::mem::drop(job_queue_manager_lock);

                    let filtered_jobs = all_jobs
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
                        .collect::<Vec<_>>();

                    // Check if the number of jobs to process is equal to max_parallel_jobs
                    continue_immediately = filtered_jobs.len() == max_parallel_jobs;

                    std::mem::drop(processing_jobs_lock);
                    filtered_jobs
                };

                // Spawn tasks based on filtered job IDs
                for job_id in job_ids_to_process {
                    let job_queue_manager = Arc::clone(&job_queue_manager);
                    let processing_jobs = Arc::clone(&processing_jobs);
                    let semaphore = Arc::clone(&semaphore);
                    let db_clone_2 = db_clone.clone();
                    let vector_fs_clone_2 = vector_fs_clone.clone();
                    let identity_sk_clone = clone_signature_secret_key(&identity_sk);
                    let job_processing_fn = Arc::clone(&job_processing_fn);
                    let cloned_generator = generator.clone();
                    let cloned_unstructured_api = unstructured_api.clone();
                    let node_profile_name = node_profile_name.clone();

                    let handle = tokio::spawn(async move {
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
                                    let result = job_processing_fn(
                                        job,
                                        db_clone_2,
                                        vector_fs_clone_2,
                                        node_profile_name,
                                        identity_sk_clone,
                                        cloned_generator,
                                        cloned_unstructured_api,
                                    )
                                    .await;
                                    if let Ok(Some(_)) = job_queue_manager.lock().await.dequeue(&job_id.clone()).await {
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
                                }
                            }
                            Ok(None) => {}
                            Err(_) => {
                                // Log the error
                            }
                        }
                        drop(_permit);
                        processing_jobs.lock().await.remove(&job_id);
                    });
                    handles.push(handle);
                }

                let handles_to_join = std::mem::take(&mut handles);
                futures::future::join_all(handles_to_join).await;
                handles.clear();

                // If job_ids_to_process was equal to max_parallel_jobs, loop again immediately
                // without waiting for a new job from receiver.recv().await
                if continue_immediately {
                    continue;
                }

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
                                    let agent_id =
                                        agent_name.get_agent_name_string().ok_or(LLMProviderError::LLMProviderNotFound)?;
                                    let job_creation: JobCreationInfo = serde_json::from_str(&data.message_raw_content)
                                        .map_err(|_| LLMProviderError::ContentParseFailed)?;
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
        profile: &ShinkaiName,
        llm_provider_id: &String,
    ) -> Result<String, LLMProviderError> {
        // TODO: add job_id to agent so it's aware
        let job_id = format!("jobid_{}", uuid::Uuid::new_v4());
        {
            let db_arc = self.db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
            let is_hidden = job_creation.is_hidden.unwrap_or(false);
            match db_arc.create_new_job(job_id.clone(), llm_provider_id.clone(), job_creation.scope, is_hidden) {
                Ok(_) => (),
                Err(err) => return Err(LLMProviderError::ShinkaiDB(err)),
            };

            match db_arc.get_job(&job_id) {
                Ok(job) => {
                    std::mem::drop(db_arc); // require to avoid deadlock
                    self.jobs.lock().await.insert(job_id.clone(), Box::new(job));
                    let mut llm_provider_found = None;
                    for agent in &self.llm_providers {
                        let locked_agent = agent.lock().await;
                        if &locked_agent.id == llm_provider_id {
                            llm_provider_found = Some(agent.clone());
                            break;
                        }
                    }

                    if llm_provider_found.is_none() {
                        let identity_manager = self.identity_manager.lock().await;
                        if let Some(serialized_agent) = identity_manager.search_local_llm_provider(llm_provider_id, profile).await {
                            let agent = LLMProvider::from_serialized_llm_provider(serialized_agent);
                            llm_provider_found = Some(Arc::new(Mutex::new(agent)));
                            if let Some(agent) = llm_provider_found.clone() {
                                self.llm_providers.push(agent);
                            }
                        }
                    }

                    let job_id_to_return = match llm_provider_found {
                        Some(_) => Ok(job_id.clone()),
                        None => Err(anyhow::Error::new(LLMProviderError::LLMProviderNotFound)),
                    };

                    job_id_to_return.map_err(|_| LLMProviderError::LLMProviderNotFound)
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
            if content.chars().count() > 30 {
                let truncated_content: String = content.chars().take(30).collect();
                content = format!("{}...", truncated_content);
            }
            let inbox_name = InboxName::get_job_inbox_name_from_params(job_message.job_id.to_string())?.to_string();
            db_arc.update_smart_inbox_name(&inbox_name.to_string(), &content)?;
        }

        db_arc
            .add_message_to_job_inbox(&job_message.job_id.clone(), &message, job_message.parent.clone())
            .await?;
        std::mem::drop(db_arc);

        self.add_job_message_to_job_queue(&job_message, &profile).await?;

        Ok(job_message.job_id.clone().to_string())
    }

    pub async fn add_job_message_to_job_queue(
        &mut self,
        job_message: &JobMessage,
        profile: &ShinkaiName,
    ) -> Result<String, LLMProviderError> {
        let job_for_processing = JobForProcessing::new(job_message.clone(), profile.clone());

        let mut job_queue_manager = self.job_queue_manager.lock().await;
        let _ = job_queue_manager.push(&job_message.job_id, job_for_processing).await;

        Ok(job_message.job_id.clone().to_string())
    }
}
