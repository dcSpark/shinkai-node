use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::job_callback_manager::JobCallbackManager;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::llm_provider::parsing_helper::ParsingHelper;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, ModelCapability};
use crate::managers::sheet_manager::SheetManager;
use crate::managers::tool_router::ToolRouter;
use crate::network::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use crate::network::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use ed25519_dalek::SigningKey;
use shinkai_db::db::ShinkaiDB;
use shinkai_job_queue_manager::job_queue_manager::{JobForProcessing, JobQueueManager};
use shinkai_message_primitives::schemas::job::{Job, JobLike};
use shinkai_message_primitives::schemas::llm_providers::common_agent_llm_provider::ProviderOrAgent;
use shinkai_message_primitives::schemas::sheet::WorkflowSheetJobData;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{CallbackAction, MessageMetadata};
use shinkai_message_primitives::shinkai_utils::job_scope::{
    LocalScopeVRKaiEntry, LocalScopeVRPackEntry, ScopeEntry, VectorFSFolderScopeEntry, VectorFSItemScopeEntry,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::shinkai_message_schemas::JobMessage,
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use shinkai_sqlite::SqliteManager;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::source::{DistributionInfo, VRSourceReference};
use shinkai_vector_resources::vector_resource::{VRPack, VRPath};
use std::result::Result::Ok;
use std::sync::Weak;
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};

impl JobManager {
    /// Processes a job message which will trigger a job step
    #[allow(clippy::too_many_arguments)]
    pub async fn process_job_message_queued(
        job_message: JobForProcessing,
        db: Weak<RwLock<SqliteManager>>,
        vector_fs: Weak<VectorFS>,
        node_profile_name: ShinkaiName,
        identity_secret_key: SigningKey,
        generator: RemoteEmbeddingGenerator,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<ToolRouter>>,
        sheet_manager: Arc<Mutex<SheetManager>>,
        _callback_manager: Arc<Mutex<JobCallbackManager>>, // Note: we will use this later on
        job_queue_manager: Arc<Mutex<JobQueueManager<JobForProcessing>>>,
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        // sqlite_logger: Option<Arc<SqliteLogger>>,
        llm_stopper: Arc<LLMStopper>,
    ) -> Result<String, LLMProviderError> {
        let db = db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
        let vector_fs = vector_fs.upgrade().ok_or("Failed to upgrade vector_db").unwrap();
        let job_id = job_message.job_message.job_id.clone();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Processing job: {}", job_id),
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

        // Note: remove later on. This code is for the meantime only while we add embeddings to tools so they can get added at the first Shinkai start
        {
            if let Some(tool_router) = tool_router.clone() {
                let _ = tool_router.initialization(Box::new(generator.clone())).await;
            }
        }

        // 1.- Processes any files which were sent with the job message
        let process_files_result = JobManager::process_job_message_files_for_vector_resources(
            db.clone(),
            vector_fs.clone(),
            &job_message.job_message,
            llm_provider_found.clone(),
            &mut full_job,
            user_profile.clone(),
            None,
            generator.clone(),
            ws_manager.clone(),
        )
        .await;
        if let Err(e) = process_files_result {
            return Self::handle_error(&db, Some(user_profile), &job_id, &identity_secret_key, e, ws_manager).await;
        }

        // 2.- workflow flow removed for now

        // 3.- *If* a sheet job is found, processing job message is taken over by this alternate logic
        let sheet_job_found = JobManager::process_sheet_job(
            db.clone(),
            vector_fs.clone(),
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
            vector_fs.clone(),
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
        db: &Arc<RwLock<SqliteManager>>,
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
            .unwrap_or_else(|| ShinkaiName::new("@@localhost.arb-sep-shinkai".to_string()).unwrap())
            .node_name;

        let error_for_frontend = error.to_error_json();

        let identity_secret_key_clone = clone_signature_secret_key(identity_secret_key);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            job_id.to_string(),
            error_for_frontend.to_string(),
            "".to_string(),
            None,
            identity_secret_key_clone,
            node_name.clone(),
            node_name.clone(),
        )
        .expect("Failed to build error message");

        db.write()
            .await
            .add_message_to_job_inbox(job_id, &shinkai_message, None, ws_manager)
            .await
            .expect("Failed to add error message to job inbox");

        Err(error)
    }

    /// Processes the provided message & job data, routes them to a specific inference chain,
    /// and then parses + saves the output result to the DB.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_inference_chain(
        db: Arc<RwLock<SqliteManager>>,
        vector_fs: Arc<VectorFS>,
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
        my_agent_payments_manager: Option<Arc<Mutex<MyAgentOfferingsManager>>>,
        ext_agent_payments_manager: Option<Arc<Mutex<ExtAgentOfferingsManager>>>,
        // sqlite_logger: Option<Arc<SqliteLogger>>,
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
        let image_files = JobManager::get_image_files_from_message(vector_fs.clone(), &job_message).await?;
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

        // Setup initial data to get ready to call a specific inference chain
        let prev_execution_context = full_job.execution_context.clone();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Prev Execution Context: {:?}", prev_execution_context),
        );
        let start = Instant::now();

        // Call the inference chain router to choose which chain to use, and call it
        let inference_response = JobManager::inference_chain_router(
            db.clone(),
            vector_fs.clone(),
            llm_provider_found,
            full_job,
            job_message.clone(),
            message_hash_id,
            image_files.clone(),
            prev_execution_context,
            generator,
            user_profile.clone(),
            ws_manager.clone(),
            tool_router.clone(),
            sheet_manager.clone(),
            my_agent_payments_manager.clone(),
            ext_agent_payments_manager.clone(),
            // sqlite_logger.clone(),
            llm_stopper.clone(),
        )
        .await?;
        let inference_response_content = inference_response.response.clone();
        let new_execution_context = inference_response.new_job_execution_context.clone();

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
            "".to_string(),
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

        // Save response data to DB
        db.write().await.add_step_history(
            job_message.job_id.clone(),
            job_message.content,
            Some(image_files),
            inference_response_content.to_string(),
            None,
            None,
        )?;
        db.write()
            .await
            .add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message, None, ws_manager)
            .await?;
        db.write()
            .await
            .set_job_execution_context(job_message.job_id.clone(), new_execution_context, None)?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_sheet_job(
        db: Arc<RwLock<SqliteManager>>,
        vector_fs: Arc<VectorFS>,
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

                Self::process_specified_files_for_vector_resources(
                    db.clone(),
                    vector_fs.clone(),
                    files_inbox.first().unwrap().clone(),
                    file_names,
                    None,
                    &mut mutable_job,
                    user_profile.clone(),
                    None,
                    generator.clone(),
                    ws_manager.clone(),
                )
                .await?;
            }

            eprintln!("full_job: {:?}", mutable_job);

            for (local_file_path, local_file_name) in &input_string.local_files {
                let vector_fs_entry = VectorFSItemScopeEntry {
                    name: local_file_name.clone(),
                    path: VRPath::from_string(local_file_path)
                        .map_err(|e| LLMProviderError::InvalidVRPath(e.to_string()))?,
                    source: VRSourceReference::None,
                };

                // Unwrap the scope_with_files since you are sure it is always Some
                mutable_job
                    .scope_with_files
                    .as_mut()
                    .unwrap()
                    .vector_fs_items
                    .push(vector_fs_entry);
            }

            let mut job_message = job_message.clone();
            job_message.content = input_string.content;

            let empty_files = HashMap::new();

            let inference_result = JobManager::inference_chain_router(
                db.clone(),
                vector_fs.clone(),
                llm_provider_found,
                mutable_job.clone(),
                job_message.clone(),
                message_hash_id,
                empty_files,
                HashMap::new(), // Assuming prev_execution_context is an empty HashMap
                generator,
                user_profile.clone(),
                ws_manager.clone(),
                tool_router.clone(),
                Some(sheet_manager.clone()),
                my_agent_payments_manager.clone(),
                ext_agent_payments_manager.clone(),
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

    /// Helper function to process files and update the job scope.
    async fn process_files_and_update_scope(
        db: Arc<RwLock<SqliteManager>>,
        vector_fs: Arc<VectorFS>,
        files: Vec<(String, Vec<u8>)>,
        agent_found: Option<ProviderOrAgent>,
        full_job: &mut Job,
        profile: ShinkaiName,
        save_to_vector_fs_folder: Option<VRPath>,
        generator: RemoteEmbeddingGenerator,
        _ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), LLMProviderError> {
        // TODO: sync with Paul about format
        // // Send WS message indicating file processing is starting
        // if let Some(ref manager) = ws_manager {
        //     let m = manager.lock().await;
        //     let metadata = WSMetadata {
        //         id: Some(full_job.job_id().to_string()),
        //         is_done: false,
        //         done_reason: Some("Starting file processing".to_string()),
        //         total_duration: None,
        //         eval_count: None,
        //     };

        //     let ws_message_type = WSMessageType::Metadata(metadata);
        //     let _ = m
        //         .queue_message(
        //             WSTopic::Inbox,
        //             full_job.job_id().to_string(),
        //             "Processing files...".to_string(),
        //             ws_message_type,
        //             false,
        //         )
        //         .await;
        // }

        // Start timing the file processing
        let start_time = Instant::now();

        // Process the files
        let new_scope_entries_result = JobManager::process_files_inbox(
            db.clone(),
            vector_fs.clone(),
            agent_found,
            files.clone(),
            profile,
            save_to_vector_fs_folder,
            generator,
        )
        .await;

        // Calculate processing duration
        let processing_duration = start_time.elapsed();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("File processing took: {:?}", processing_duration),
        );

        // // Send WS message indicating file processing is complete
        // if let Some(ref manager) = ws_manager {
        //     let m = manager.lock().await;
        //     let metadata = WSMetadata {
        //         id: Some(full_job.job_id().to_string()),
        //         is_done: false,
        //         done_reason: Some("File processing complete".to_string()),
        //         total_duration: None,
        //         eval_count: None,
        //     };

        //     let ws_message_type = WSMessageType::Metadata(metadata);
        //     let _ = m
        //         .queue_message(
        //             WSTopic::Inbox,
        //             full_job.job_id().to_string(),
        //             "Files processed successfully".to_string(),
        //             ws_message_type,
        //             true,
        //         )
        //         .await;
        // }

        match new_scope_entries_result {
            Ok(new_scope_entries) => {
                // Safely unwrap the scope_with_files
                let job_id = full_job.job_id().to_string();
                if let Some(ref mut scope_with_files) = full_job.scope_with_files {
                    // Update the job scope with new entries
                    for (_filename, scope_entry) in new_scope_entries {
                        match scope_entry {
                            ScopeEntry::LocalScopeVRKai(local_entry) => {
                                if !scope_with_files.local_vrkai.contains(&local_entry) {
                                    scope_with_files.local_vrkai.push(local_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate LocalScopeVRKaiEntry detected",
                                    );
                                }
                            }
                            ScopeEntry::LocalScopeVRPack(local_entry) => {
                                if !scope_with_files.local_vrpack.contains(&local_entry) {
                                    scope_with_files.local_vrpack.push(local_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate LocalScopeVRPackEntry detected",
                                    );
                                }
                            }
                            ScopeEntry::VectorFSItem(fs_entry) => {
                                if !scope_with_files.vector_fs_items.contains(&fs_entry) {
                                    scope_with_files.vector_fs_items.push(fs_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate VectorFSScopeEntry detected",
                                    );
                                }
                            }
                            ScopeEntry::VectorFSFolder(fs_entry) => {
                                if !scope_with_files.vector_fs_folders.contains(&fs_entry) {
                                    scope_with_files.vector_fs_folders.push(fs_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate VectorFSScopeEntry detected",
                                    );
                                }
                            }
                        }
                    }
                    db.write().await.update_job_scope(job_id, scope_with_files.clone())?;
                } else {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Error,
                        "No scope_with_files found in full_job",
                    );
                }
            }
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    format!("Error processing files: {}", e).as_str(),
                );
                return Err(e);
            }
        }

        Ok(())
    }

    /// Processes the files sent together with the current job_message into Vector Resources.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_job_message_files_for_vector_resources(
        db: Arc<RwLock<SqliteManager>>,
        vector_fs: Arc<VectorFS>,
        job_message: &JobMessage,
        agent_found: Option<ProviderOrAgent>,
        full_job: &mut Job,
        profile: ShinkaiName,
        save_to_vector_fs_folder: Option<VRPath>,
        generator: RemoteEmbeddingGenerator,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), LLMProviderError> {
        eprintln!("full_job: {:?}", full_job);
        eprintln!("job_message: {:?}", job_message);

        if !job_message.files_inbox.is_empty() {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Processing files_map: ... files: {}", job_message.files_inbox.len()).as_str(),
            );

            // Get the files from the DB
            let files = {
                let files_result = vector_fs.db.get_all_files_from_inbox(job_message.files_inbox.clone());
                match files_result {
                    Ok(files) => files,
                    Err(e) => return Err(LLMProviderError::VectorFS(e)),
                }
            };

            // Process the files and update the job scope
            Self::process_files_and_update_scope(
                db,
                vector_fs,
                files,
                agent_found,
                full_job,
                profile,
                save_to_vector_fs_folder,
                generator,
                ws_manager,
            )
            .await?;
        }

        Ok(())
    }

    /// Processes the specified files into Vector Resources.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_specified_files_for_vector_resources(
        db: Arc<RwLock<SqliteManager>>,
        vector_fs: Arc<VectorFS>,
        files_inbox: String,
        file_names: Vec<String>,
        agent_found: Option<ProviderOrAgent>,
        full_job: &mut Job,
        profile: ShinkaiName,
        save_to_vector_fs_folder: Option<VRPath>,
        generator: RemoteEmbeddingGenerator,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), LLMProviderError> {
        eprintln!("full_job: {:?}", full_job);
        eprintln!("files_inbox: {:?}", files_inbox);
        eprintln!("file_names: {:?}", file_names);

        if !file_names.is_empty() {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Processing specified files: {:?}", file_names).as_str(),
            );

            // Get the files from the DB
            let files = {
                let files_result = vector_fs.db.get_all_files_from_inbox(files_inbox.clone());
                match files_result {
                    Ok(files) => files,
                    Err(e) => return Err(LLMProviderError::VectorFS(e)),
                }
            };

            // Filter files based on the provided file names
            let specified_files: Vec<(String, Vec<u8>)> = files
                .into_iter()
                .filter(|(name, _)| file_names.contains(name))
                .collect();

            // Process the specified files and update the job scope
            Self::process_files_and_update_scope(
                db,
                vector_fs,
                specified_files,
                agent_found,
                full_job,
                profile,
                save_to_vector_fs_folder,
                generator,
                ws_manager,
            )
            .await?;
        }

        Ok(())
    }

    /// Retrieves image files associated with a job message and converts them to base64
    pub async fn get_image_files_from_message(
        vector_fs: Arc<VectorFS>,
        job_message: &JobMessage,
    ) -> Result<HashMap<String, String>, LLMProviderError> {
        if job_message.files_inbox.is_empty() {
            return Ok(HashMap::new());
        }

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("Retrieving files for job message: {}", job_message.job_id).as_str(),
        );

        let files_vec = vector_fs
            .db
            .get_all_files_from_inbox(job_message.files_inbox.clone())
            .map_err(LLMProviderError::VectorFS)?;

        let image_files: HashMap<String, String> = files_vec
            .into_iter()
            .filter_map(|(filename, content)| {
                let filename_lower = filename.to_lowercase();
                if filename_lower.ends_with(".png")
                    || filename_lower.ends_with(".jpg")
                    || filename_lower.ends_with(".jpeg")
                    || filename_lower.ends_with(".gif")
                {
                    // Note: helpful for later, when we add other types like audio, video, etc.
                    // let file_extension = filename.split('.').last().unwrap_or("jpg");
                    let base64_content = base64::encode(&content);
                    Some((filename, base64_content))
                } else {
                    None
                }
            })
            .collect();

        Ok(image_files)
    }

    /// Processes the files in a given file inbox by generating VectorResources + job `ScopeEntry`s.
    /// If save_to_vector_fs_folder == true, the files will save to the DB and be returned as `VectorFSScopeEntry`s.
    /// Else, the files will be returned as LocalScopeEntries and thus held inside.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_files_inbox(
        db: Arc<RwLock<SqliteManager>>,
        _vector_fs: Arc<VectorFS>,
        agent: Option<ProviderOrAgent>,
        files: Vec<(String, Vec<u8>)>,
        _profile: ShinkaiName,
        save_to_vector_fs_folder: Option<VRPath>,
        generator: RemoteEmbeddingGenerator,
    ) -> Result<HashMap<String, ScopeEntry>, LLMProviderError> {
        // Create the RemoteEmbeddingGenerator instance
        let mut files_map: HashMap<String, ScopeEntry> = HashMap::new();

        // Filter out image files
        // TODO: Eventually we will add extra embeddings that support images
        let files: Vec<(String, Vec<u8>)> = files
            .into_iter()
            .filter(|(name, _)| {
                let name_lower = name.to_lowercase();
                !name_lower.ends_with(".png")
                    && !name_lower.ends_with(".jpg")
                    && !name_lower.ends_with(".jpeg")
                    && !name_lower.ends_with(".gif")
            })
            .collect();

        // Sort out the vrpacks from the rest
        #[allow(clippy::type_complexity)]
        let (vr_packs, other_files): (Vec<(String, Vec<u8>)>, Vec<(String, Vec<u8>)>) =
            files.into_iter().partition(|(name, _)| name.ends_with(".vrpack"));

        // TODO: Decide how frontend relays distribution info so it can be properly added
        // For now attempting basic auto-detection of distribution origin based on filename, and setting release date to none
        let mut dist_files = vec![];
        for file in other_files {
            let distribution_info = DistributionInfo::new_auto(&file.0, None);
            dist_files.push((file.0, file.1, distribution_info));
        }

        let processed_vrkais =
            ParsingHelper::process_files_into_vrkai(dist_files, &generator, agent.clone(), db.clone()).await?;

        // Save the vrkai into scope (and potentially VectorFS)
        for (filename, vrkai) in processed_vrkais {
            // Now create Local/VectorFSScopeEntry depending on setting
            if let Some(folder_path) = &save_to_vector_fs_folder {
                let fs_scope_entry = VectorFSItemScopeEntry {
                    name: vrkai.resource.as_trait_object().name().to_string(),
                    path: folder_path.clone(),
                    source: vrkai.resource.as_trait_object().source().clone(),
                };

                // TODO: Save to the vector_fs if `save_to_vector_fs_folder` not None
                // let vector_fs = self.v

                files_map.insert(filename, ScopeEntry::VectorFSItem(fs_scope_entry));
            } else {
                let local_scope_entry = LocalScopeVRKaiEntry { vrkai };
                files_map.insert(filename, ScopeEntry::LocalScopeVRKai(local_scope_entry));
            }
        }

        // Save the vrpacks into scope (and potentially VectorFS)
        for (filename, vrpack_bytes) in vr_packs {
            let vrpack = VRPack::from_bytes(&vrpack_bytes)?;
            // Now create Local/VectorFSScopeEntry depending on setting
            if let Some(folder_path) = &save_to_vector_fs_folder {
                let fs_scope_entry = VectorFSFolderScopeEntry {
                    name: vrpack.name.clone(),
                    path: folder_path.push_cloned(vrpack.name.clone()),
                };

                // TODO: Save to the vector_fs if `save_to_vector_fs_folder` not None
                // let vector_fs = self.v

                files_map.insert(filename, ScopeEntry::VectorFSFolder(fs_scope_entry));
            } else {
                let local_scope_entry = LocalScopeVRPackEntry { vrpack };
                files_map.insert(filename, ScopeEntry::LocalScopeVRPack(local_scope_entry));
            }
        }

        Ok(files_map)
    }
}
