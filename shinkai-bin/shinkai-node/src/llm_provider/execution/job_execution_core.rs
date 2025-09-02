use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChainResult;
use crate::llm_provider::job_callback_manager::JobCallbackManager;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;

use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, ModelCapability};
use crate::managers::tool_router::ToolRouter;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use ed25519_dalek::SigningKey;

use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;
use shinkai_job_queue_manager::job_queue_manager::{JobForProcessing, JobQueueManager};
use shinkai_message_primitives::schemas::job::{Job, JobLike};
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{CallbackAction, MessageMetadata};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName, shinkai_message::shinkai_message_schemas::JobMessage, shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key}
};
use shinkai_sqlite::SqliteManager;
use base64::Engine;
use std::result::Result::Ok;
use std::sync::Weak;
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

impl JobManager {
    /// Processes a job message which will trigger a job step
    #[allow(clippy::too_many_arguments)]
    pub async fn process_job_message_queued(
        job_message: JobForProcessing,
        db: Weak<SqliteManager>,
        node_profile_name: ShinkaiName,
        identity_secret_key: SigningKey,
        generator: RemoteEmbeddingGenerator,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        job_callback_manager: Arc<Mutex<JobCallbackManager>>,
        _job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        // sqlite_logger: Option<Arc<SqliteLogger>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<String, LLMProviderError> {
        let db = db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
        let job_id = job_message.job_message.job_id.clone();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("Processing job: {} with JobMessage: {:?}", job_id, job_message),
        );

        // Fetch data we need to execute job step
        let fetch_data_result = JobManager::fetch_relevant_job_data(&job_message.job_message.job_id, db.clone()).await;
        let (full_job, llm_provider_found, _, user_profile) = match fetch_data_result {
            Ok(data) => data,
            Err(e) => return Self::handle_error(&db, None, &job_id, &identity_secret_key, e, ws_manager).await,
        };

        // Ensure the user profile exists before proceeding with inference chain
        let user_profile = match user_profile {
            Some(profile) => profile,
            None => {
                return Self::handle_error(
                    &db,
                    None,
                    &job_id,
                    &identity_secret_key,
                    LLMProviderError::NoUserProfileFound,
                    ws_manager,
                )
                .await
            }
        };

        let user_profile = ShinkaiName::from_node_and_profile_names(
            node_profile_name.node_name,
            user_profile.profile_name.unwrap_or_default(),
        )
        .unwrap();

        let inference_chain_result = JobManager::process_inference_chain(
            db.clone(),
            clone_signature_secret_key(&identity_secret_key),
            job_message.job_message,
            job_message.message_hash_id.clone(),
            full_job,
            llm_provider_found.clone(),
            user_profile.clone(),
            generator,
            ws_manager.clone(),
            tool_router.clone(),
            job_callback_manager.clone(),
            my_agent_payments_manager.clone(),
            ext_agent_payments_manager.clone(),
            // sqlite_logger.clone(),
            llm_stopper.clone(),
        )
        .await;

        if let Err(e) = inference_chain_result {
            return Self::handle_error(&db, Some(user_profile), &job_id, &identity_secret_key, e, ws_manager).await;
        }

        Ok(job_id)
    }

    /// Handle errors by sending an error message to the job inbox
    async fn handle_error(
        db: &Arc<SqliteManager>,
        user_profile: Option<ShinkaiName>,
        job_id: &str,
        identity_secret_key: &SigningKey,
        error: LLMProviderError,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<String, LLMProviderError> {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Error,
            &format!("Error processing job: {}", error),
        );

        let node_name = user_profile
            .unwrap_or_else(|| ShinkaiName::new("@@localhost.sep-shinkai".to_string()).unwrap())
            .node_name;

        let error_json = error.to_error_message();
        let error_for_frontend = format!("{}", error_json);

        let identity_secret_key_clone = clone_signature_secret_key(identity_secret_key);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            job_id.to_string(),
            error_for_frontend.to_string(),
            vec![],
            None,
            identity_secret_key_clone,
            node_name.clone(),
            node_name.clone(),
        )
        .expect("Failed to build error message");

        db.add_message_to_job_inbox(job_id, &shinkai_message, None, ws_manager)
            .await
            .expect("Failed to add error message to job inbox");

        Err(error)
    }

    /// Processes the provided message & job data, routes them to a specific inference chain,
    /// and then parses + saves the output result to the DB.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_inference_chain(
        db: Arc<SqliteManager>,
        identity_secret_key: SigningKey,
        job_message: JobMessage,
        message_hash_id: Option<String>,
        full_job: Job,
        llm_provider_found: Option<ProviderOrAgent>,
        user_profile: ShinkaiName,
        generator: RemoteEmbeddingGenerator,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        job_callback_manager: Arc<Mutex<JobCallbackManager>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<(), LLMProviderError> {
        let job_id = full_job.job_id().to_string();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Inference chain - Processing Job: {:?}", full_job.job_id),
        );

        eprintln!("Full job: {:?}", full_job);

        // Retrieve image files from the message
        // Note: this could be other type of files later on e.g. video, audio, etc.
        let image_files = JobManager::get_image_files_from_message(db.clone(), &job_message).await?;
        eprintln!("# of images: {:?}", image_files.len());
        
        // Retrieve video files from the message
        let video_files = JobManager::get_video_files_from_message(db.clone(), &job_message).await?;
        eprintln!("# of videos: {:?}", video_files.len());
        
        // Retrieve audio files from the message
        let audio_files = JobManager::get_audio_files_from_message(db.clone(), &job_message).await?;
        eprintln!("# of audios: {:?}", audio_files.len());

        if image_files.len() > 0 {
            // Check if the specific LLM provider being used has ImageAnalysis capability
            let has_image_analysis = if let Some(provider) = &llm_provider_found {
                // Get the specific model for this provider
                let model = match provider {
                    ProviderOrAgent::LLMProvider(llm_provider) => llm_provider.model.clone(),
                    ProviderOrAgent::Agent(agent) => {
                        // For agents, get the underlying LLM provider
                        let llm_id = &agent.llm_provider_id;
                        if let Ok(Some(llm_provider)) = db.get_llm_provider(llm_id, &user_profile) {
                            llm_provider.model.clone()
                        } else {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                "Could not retrieve LLM provider for agent",
                            );
                            return Err(LLMProviderError::LLMProviderNotFound);
                        }
                    }
                };
                
                // Check if this specific model has ImageAnalysis capability
                ModelCapabilitiesManager::get_llm_provider_capabilities(&model)
                    .contains(&ModelCapability::ImageAnalysis)
            } else {
                false
            };

            if !has_image_analysis {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    "The specific LLM provider being used does not have ImageAnalysis capability",
                );
                return Err(LLMProviderError::LLMProviderMissingCapabilities(
                    "The specific LLM provider being used does not have ImageAnalysis capability".to_string(),
                ));
            }
        }

        if video_files.len() > 0 {
            // Check if the specific LLM provider being used has VideoAnalysis capability
            let has_video_analysis = if let Some(provider) = &llm_provider_found {
                // Get the specific model for this provider
                let model = match provider {
                    ProviderOrAgent::LLMProvider(llm_provider) => llm_provider.model.clone(),
                    ProviderOrAgent::Agent(agent) => {
                        // For agents, get the underlying LLM provider
                        let llm_id = &agent.llm_provider_id;
                        if let Ok(Some(llm_provider)) = db.get_llm_provider(llm_id, &user_profile) {
                            llm_provider.model.clone()
                        } else {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                "Could not retrieve LLM provider for agent",
                            );
                            return Err(LLMProviderError::LLMProviderNotFound);
                        }
                    }
                };
                
                // Check if this specific model has VideoAnalysis capability
                ModelCapabilitiesManager::get_llm_provider_capabilities(&model)
                    .contains(&ModelCapability::VideoAnalysis)
            } else {
                false
            };

            if !has_video_analysis {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    "The specific LLM provider being used does not have VideoAnalysis capability",
                );
                return Err(LLMProviderError::LLMProviderMissingCapabilities(
                    "The specific LLM provider being used does not have VideoAnalysis capability".to_string(),
                ));
            }
        }

        if audio_files.len() > 0 {
            // Check if the specific LLM provider being used has AudioAnalysis capability
            let has_audio_analysis = if let Some(provider) = &llm_provider_found {
                // Get the specific model for this provider
                let model = match provider {
                    ProviderOrAgent::LLMProvider(llm_provider) => llm_provider.model.clone(),
                    ProviderOrAgent::Agent(agent) => {
                        // For agents, get the underlying LLM provider
                        let llm_id = &agent.llm_provider_id;
                        if let Ok(Some(llm_provider)) = db.get_llm_provider(llm_id, &user_profile) {
                            llm_provider.model.clone()
                        } else {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                "Could not retrieve LLM provider for agent",
                            );
                            return Err(LLMProviderError::LLMProviderNotFound);
                        }
                    }
                };
                
                // Check if this specific model has AudioAnalysis capability
                ModelCapabilitiesManager::get_llm_provider_capabilities(&model)
                    .contains(&ModelCapability::AudioAnalysis)
            } else {
                false
            };

            if !has_audio_analysis {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    "The specific LLM provider being used does not have AudioAnalysis capability",
                );
                return Err(LLMProviderError::LLMProviderMissingCapabilities(
                    "The specific LLM provider being used does not have AudioAnalysis capability".to_string(),
                ));
            }
        }

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Retrieved {} image files", image_files.len()),
        );

        let start = Instant::now();

        // Call the inference chain router to choose which chain to use, and call it
        let (inference_response, inference_response_content) = match JobManager::inference_chain_router(
            db.clone(),
            llm_provider_found,
            full_job,
            job_message.clone(),
            message_hash_id,
            image_files.clone(),
            video_files.clone(),
            audio_files.clone(),
            generator,
            user_profile.clone(),
            ws_manager.clone(),
            tool_router.clone(),
            my_agent_payments_manager.clone(),
            ext_agent_payments_manager.clone(),
            job_callback_manager.clone(),
            // sqlite_logger.clone(),
            llm_stopper.clone(),
        )
        .await
        {
            Ok(response) => (response.clone(), response.response),
            Err(e) => {
                let error_message = format!("{}", e);
                // Create a minimal inference response with the error message
                let error_response = InferenceChainResult {
                    response: error_message.clone(),
                    tps: None,
                    answer_duration: None,
                    tool_calls: None,
                    generated_files: Vec::new(),
                };
                (error_response, error_message)
            }
        };

        let duration = start.elapsed();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Time elapsed for inference chain processing is: {:?}", duration),
        );

        let message_metadata = MessageMetadata {
            tps: inference_response.tps.clone(),
            duration_ms: inference_response.answer_duration.clone(),
            function_calls: inference_response.tool_calls_metadata(),
        };

        // Prepare data to save inference response to the DB
        let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            job_id.to_string(),
            inference_response_content.to_string(),
            inference_response.generated_files.clone(),
            Some(message_metadata),
            identity_secret_key_clone,
            user_profile.node_name.clone(),
            user_profile.node_name.clone(),
        )
        .map_err(|e| LLMProviderError::ShinkaiMessageBuilderError(e.to_string()))?;

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("process_inference_chain> shinkai_message: {:?}", shinkai_message).as_str(),
        );

        db.add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message, None, ws_manager)
            .await?;

        // Check for callbacks and add them to the JobManagerQueue if required
        if let Some(callback) = &job_message.callback {
            if let CallbackAction::ImplementationCheck(tool_type, available_tools) = callback.as_ref() {
                job_callback_manager
                    .lock()
                    .await
                    .handle_implementation_check_callback(
                        db.clone(),
                        tool_type.clone(),
                        inference_response_content.to_string(),
                        available_tools.clone(),
                        &identity_secret_key,
                        &user_profile,
                        &job_id,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    /// Retrieves image files associated with a job message and converts them to base64
    pub async fn get_image_files_from_message(
        db: Arc<SqliteManager>,
        job_message: &JobMessage,
    ) -> Result<HashMap<String, String>, LLMProviderError> {
        if job_message.fs_files_paths.is_empty() && job_message.job_filenames.is_empty() {
            return Ok(HashMap::new());
        }

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("Retrieving files for job message: {}", job_message.job_id).as_str(),
        );

        let mut image_files = HashMap::new();

        // Process fs_files_paths
        for file_path in &job_message.fs_files_paths {
            if let Some(file_name) = file_path.path.file_name() {
                let filename_lower = file_name.to_string_lossy().to_lowercase();
                if filename_lower.ends_with(".png")
                    || filename_lower.ends_with(".jpg")
                    || filename_lower.ends_with(".jpeg")
                    || filename_lower.ends_with(".gif")
                {
                    // Retrieve the file content
                    match ShinkaiFileManager::get_file_content(file_path.clone()) {
                        Ok(content) => {
                            let base64_content = base64::engine::general_purpose::STANDARD.encode(&content);
                            image_files.insert(file_path.relative_path().to_string(), base64_content);
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        // Process job_filenames
        for filename in &job_message.job_filenames {
            let filename_lower = filename.to_lowercase();
            if filename_lower.ends_with(".png")
                || filename_lower.ends_with(".jpg")
                || filename_lower.ends_with(".jpeg")
                || filename_lower.ends_with(".gif")
            {
                // Construct the job file path
                match ShinkaiFileManager::construct_job_file_path(&job_message.job_id, filename, &db) {
                    Ok(file_path) => {
                        // Retrieve the file content
                        match ShinkaiFileManager::get_file_content(file_path.clone()) {
                            Ok(content) => {
                                let base64_content = base64::engine::general_purpose::STANDARD.encode(&content);
                                image_files.insert(filename.clone(), base64_content);
                            }
                            Err(_) => continue,
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        Ok(image_files)
    }

    /// Retrieves video files associated with a job message and converts them to base64
    pub async fn get_video_files_from_message(
        db: Arc<SqliteManager>,
        job_message: &JobMessage,
    ) -> Result<HashMap<String, String>, LLMProviderError> {
        if job_message.fs_files_paths.is_empty() && job_message.job_filenames.is_empty() {
            return Ok(HashMap::new());
        }

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("Retrieving video files for job message: {}", job_message.job_id).as_str(),
        );

        let mut video_files = HashMap::new();

        // Process fs_files_paths
        for file_path in &job_message.fs_files_paths {
            if let Some(file_name) = file_path.path.file_name() {
                let filename_lower = file_name.to_string_lossy().to_lowercase();
                if filename_lower.ends_with(".mp4")
                    || filename_lower.ends_with(".mov")
                    || filename_lower.ends_with(".avi")
                    || filename_lower.ends_with(".webm")
                    || filename_lower.ends_with(".mkv")
                    || filename_lower.ends_with(".wmv")
                    || filename_lower.ends_with(".flv")
                {
                    // Retrieve the file content
                    match ShinkaiFileManager::get_file_content(file_path.clone()) {
                        Ok(content) => {
                            let base64_content = base64::engine::general_purpose::STANDARD.encode(&content);
                            video_files.insert(file_path.relative_path().to_string(), base64_content);
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        // Process job_filenames
        for filename in &job_message.job_filenames {
            let filename_lower = filename.to_lowercase();
            if filename_lower.ends_with(".mp4")
                || filename_lower.ends_with(".mov")
                || filename_lower.ends_with(".avi")
                || filename_lower.ends_with(".webm")
                || filename_lower.ends_with(".mkv")
                || filename_lower.ends_with(".wmv")
                || filename_lower.ends_with(".flv")
            {
                // Construct the job file path
                match ShinkaiFileManager::construct_job_file_path(&job_message.job_id, filename, &db) {
                    Ok(file_path) => {
                        // Retrieve the file content
                        match ShinkaiFileManager::get_file_content(file_path.clone()) {
                            Ok(content) => {
                                let base64_content = base64::engine::general_purpose::STANDARD.encode(&content);
                                video_files.insert(filename.clone(), base64_content);
                            }
                            Err(_) => continue,
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        Ok(video_files)
    }

    /// Retrieves audio files associated with a job message and converts them to base64
    pub async fn get_audio_files_from_message(
        db: Arc<SqliteManager>,
        job_message: &JobMessage,
    ) -> Result<HashMap<String, String>, LLMProviderError> {
        if job_message.fs_files_paths.is_empty() && job_message.job_filenames.is_empty() {
            return Ok(HashMap::new());
        }

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("Retrieving audio files for job message: {}", job_message.job_id).as_str(),
        );

        let mut audio_files = HashMap::new();

        // Process fs_files_paths
        for file_path in &job_message.fs_files_paths {
            if let Some(file_name) = file_path.path.file_name() {
                let filename_lower = file_name.to_string_lossy().to_lowercase();
                if filename_lower.ends_with(".mp3")
                    || filename_lower.ends_with(".wav")
                    || filename_lower.ends_with(".flac")
                    || filename_lower.ends_with(".ogg")
                    || filename_lower.ends_with(".m4a")
                    || filename_lower.ends_with(".aiff")
                    || filename_lower.ends_with(".wma")
                    || filename_lower.ends_with(".aac")
                {
                    // Retrieve the file content
                    match ShinkaiFileManager::get_file_content(file_path.clone()) {
                        Ok(content) => {
                            let base64_content = base64::engine::general_purpose::STANDARD.encode(&content);
                            audio_files.insert(file_path.relative_path().to_string(), base64_content);
                        }
                        Err(_) => continue,
                    }
                }
            }
        }

        // Process job_filenames
        for filename in &job_message.job_filenames {
            let filename_lower = filename.to_lowercase();
            if filename_lower.ends_with(".mp3")
                || filename_lower.ends_with(".wav")
                || filename_lower.ends_with(".flac")
                || filename_lower.ends_with(".ogg")
                || filename_lower.ends_with(".m4a")
                || filename_lower.ends_with(".aiff")
                || filename_lower.ends_with(".wma")
                || filename_lower.ends_with(".aac")
            {
                // Construct the job file path
                match ShinkaiFileManager::construct_job_file_path(&job_message.job_id, filename, &db) {
                    Ok(file_path) => {
                        // Retrieve the file content
                        match ShinkaiFileManager::get_file_content(file_path.clone()) {
                            Ok(content) => {
                                let base64_content = base64::engine::general_purpose::STANDARD.encode(&content);
                                audio_files.insert(filename.clone(), base64_content);
                            }
                            Err(_) => continue,
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        Ok(audio_files)
    }
}
