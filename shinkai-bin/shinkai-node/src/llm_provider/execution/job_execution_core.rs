use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChainResult;
use crate::llm_provider::job_callback_manager::JobCallbackManager;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;

use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, ModelCapability};
use crate::managers::sheet_manager::SheetManager;
use crate::managers::tool_router::ToolRouter;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use ed25519_dalek::SigningKey;

use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_fs::shinkai_file_manager::ShinkaiFileManager;
use shinkai_job_queue_manager::job_queue_manager::{JobForProcessing, JobQueueManager};
use shinkai_message_primitives::schemas::job::{Job, JobLike};
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::sheet::WorkflowSheetJobData;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{CallbackAction, MessageMetadata};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName, shinkai_message::shinkai_message_schemas::JobMessage, shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key}
};
use shinkai_sqlite::SqliteManager;
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
        sheet_manager: Arc<Mutex<SheetManager>>,
        job_callback_manager: Arc<Mutex<JobCallbackManager>>,
        job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
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
        let (mut full_job, llm_provider_found, _, user_profile) = match fetch_data_result {
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

        // 1.- *If* a sheet job is found, processing job message is taken over by this alternate logic
        let sheet_job_found = JobManager::process_sheet_job(
            db.clone(),
            &job_message.job_message,
            job_message.message_hash_id.clone(),
            llm_provider_found.clone(),
            full_job.clone(),
            user_profile.clone(),
            generator.clone(),
            ws_manager.clone(),
            Some(sheet_manager.clone()),
            tool_router.clone(),
            job_queue_manager.clone(),
            my_agent_payments_manager.clone(),
            ext_agent_payments_manager.clone(),
            job_callback_manager.clone(),
            // sqlite_logger.clone(),
            llm_stopper.clone(),
        )
        .await?;
        if sheet_job_found {
            return Ok(job_id);
        }

        // Otherwise proceed forward with rest of logic.
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
            Some(sheet_manager.clone()),
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
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
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

        if image_files.len() > 0 {
            let db_weak = Arc::downgrade(&db);
            let agent_capabilities = ModelCapabilitiesManager::new(db_weak, user_profile.clone()).await;
            let has_image_analysis = agent_capabilities.has_capability(ModelCapability::ImageAnalysis).await;

            if !has_image_analysis {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    "Agent does not have ImageAnalysis capability",
                );
                return Err(LLMProviderError::LLMProviderMissingCapabilities(
                    "Agent does not have ImageAnalysis capability".to_string(),
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
            generator,
            user_profile.clone(),
            ws_manager.clone(),
            tool_router.clone(),
            sheet_manager.clone(),
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
            vec![],
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

    #[allow(clippy::too_many_arguments)]
    pub async fn process_sheet_job(
        db: Arc<SqliteManager>,
        job_message: &JobMessage,
        message_hash_id: Option<String>,
        llm_provider_found: Option<ProviderOrAgent>,
        full_job: Job,
        user_profile: ShinkaiName,
        generator: RemoteEmbeddingGenerator,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        sheet_manager: Option<Arc<Mutex<SheetManager>>>,
        tool_router: Option<Arc<ToolRouter>>,
        job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        job_callback_manager: Arc<Mutex<JobCallbackManager>>,
        // sqlite_logger: Option<Arc<SqliteLogger>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<bool, LLMProviderError> {
        if let Some(sheet_job_data) = &job_message.sheet_job_data {
            let sheet_job_data: WorkflowSheetJobData = serde_json::from_str(sheet_job_data)
                .map_err(|e| LLMProviderError::SerializationError(e.to_string()))?;

            // Check SheetManager for the latest inputs
            let sheet_manager = sheet_manager.ok_or(LLMProviderError::SheetManagerNotFound)?;

            // Get the processed input string within the lock scope
            let input_string = {
                let sheet_manager = sheet_manager.lock().await;
                let sheet = sheet_manager.get_sheet(&sheet_job_data.sheet_id)?;
                sheet
                    .get_processed_input(sheet_job_data.row.clone(), sheet_job_data.col.clone())
                    .ok_or(LLMProviderError::InputProcessingError(format!("{:?}", sheet_job_data)))?
            };

            // Create a mutable copy of full_job
            let mut mutable_job = full_job.clone();

            if input_string.uploaded_files.len() > 0 {
                // Decompose the uploaded_files into two separate vectors
                let (files_inbox, file_names): (Vec<String>, Vec<String>) =
                    input_string.uploaded_files.iter().cloned().unzip();

                unimplemented!();

                // TODO: fix this
                // Self::process_specified_files_for_vector_resources(
                //     db.clone(),
                //     files_inbox.first().unwrap().clone(),
                //     file_names,
                //     None,
                //     &mut mutable_job,
                //     user_profile.clone(),
                //     None,
                //     generator.clone(),
                //     ws_manager.clone(),
                // )
                // .await?;
            }

            for (local_file_path, local_file_name) in &input_string.local_files {
                let path = ShinkaiPath::from_string(local_file_path.to_string());

                // Unwrap the scope_with_files since you are sure it is always Some
                mutable_job.scope.vector_fs_items.push(path);
            }

            let mut job_message = job_message.clone();
            job_message.content = input_string.content;

            let empty_files = HashMap::new();

            let inference_result = JobManager::inference_chain_router(
                db.clone(),
                llm_provider_found,
                mutable_job.clone(),
                job_message.clone(),
                message_hash_id,
                empty_files,
                generator,
                user_profile.clone(),
                ws_manager.clone(),
                tool_router.clone(),
                Some(sheet_manager.clone()),
                my_agent_payments_manager.clone(),
                ext_agent_payments_manager.clone(),
                job_callback_manager.clone(),
                // sqlite_logger.clone(),
                llm_stopper.clone(),
            )
            .await?;

            let response = inference_result.response;

            // Update the sheet using the callback manager. "sheet" is just a local copy
            // In { } to avoid locking the mutex for too long
            {
                let mut sheet_manager = sheet_manager.lock().await;
                sheet_manager
                    .set_cell_value(
                        &sheet_job_data.sheet_id,
                        sheet_job_data.row.clone(),
                        sheet_job_data.col.clone(),
                        response.clone(),
                    )
                    .await
                    .map_err(|e| LLMProviderError::SheetManagerError(e.to_string()))?;
            }

            // Check for callbacks and add them to the JobManagerQueue if required
            if let Some(callback) = &job_message.callback {
                if let CallbackAction::Sheet(sheet_action) = callback.as_ref() {
                    if let Some(next_job_message) = &sheet_action.job_message_next {
                        let mut job_queue_manager = job_queue_manager.lock().await;
                        job_queue_manager
                            .push(
                                &next_job_message.job_id,
                                JobForProcessing::new(next_job_message.clone(), user_profile, None),
                            )
                            .await?;
                    }
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
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
                            let base64_content = base64::encode(&content);
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
                                let base64_content = base64::encode(&content);
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
}
