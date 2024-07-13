use crate::db::ShinkaiDB;
use crate::llm_provider::error::LLMProviderError;
use crate::llm_provider::execution::chains::inference_chain_trait::InferenceChain;
use crate::llm_provider::job::{Job, JobLike};
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::parsing_helper::ParsingHelper;
use crate::llm_provider::queue::job_queue_manager::JobForProcessing;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, ModelCapability};
use crate::network::ws_manager::WSUpdateHandler;
use crate::tools::js_toolkit::JSToolkit;
use crate::tools::tool_router::ToolRouter;
use crate::vector_fs::vector_fs::VectorFS;
use ed25519_dalek::SigningKey;
use shinkai_dsl::parser::parse_workflow;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::shinkai_utils::job_scope::{
    LocalScopeVRKaiEntry, LocalScopeVRPackEntry, ScopeEntry, VectorFSFolderScopeEntry, VectorFSItemScopeEntry,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::shinkai_message_schemas::JobMessage,
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use shinkai_tools_runner::built_in_tools;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::file_parser::file_parser::FileParser;
use shinkai_vector_resources::file_parser::unstructured_api::UnstructuredAPI;
use shinkai_vector_resources::source::DistributionInfo;
use shinkai_vector_resources::vector_resource::{VRPack, VRPath};
use std::result::Result::Ok;
use std::sync::Weak;
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

use tracing::instrument;

use super::chains::dsl_chain::dsl_inference_chain::DslChain;
use super::chains::inference_chain_trait::InferenceChainContext;
use super::user_message_parser::ParsedUserMessage;

impl JobManager {
    /// Processes a job message which will trigger a job step
    #[instrument(skip(
        identity_secret_key,
        generator,
        unstructured_api,
        vector_fs,
        db,
        ws_manager,
        tool_router
    ))]
    #[allow(clippy::too_many_arguments)]
    pub async fn process_job_message_queued(
        job_message: JobForProcessing,
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_profile_name: ShinkaiName,
        identity_secret_key: SigningKey,
        generator: RemoteEmbeddingGenerator,
        unstructured_api: UnstructuredAPI,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<Mutex<ToolRouter>>>,
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

        // Temporal
        if let Some(tool_router_arc) = &tool_router {
            let mut tool_router = tool_router_arc.lock().await;
            if !tool_router.is_started() {
                let start_time = Instant::now(); // Start time measurement

                // Remove all toolkits for the user
                db.remove_all_toolkits_for_user(&user_profile).unwrap();

                // Add the tools again
                let tools = built_in_tools::get_tools();
                for (name, definition) in tools {
                    let toolkit = JSToolkit::new(&name, vec![definition]);
                    db.add_jstoolkit(toolkit, user_profile.clone()).unwrap();
                }

                if let Err(e) = tool_router
                    .start(Box::new(generator.clone()), Arc::downgrade(&db), user_profile.clone())
                    .await
                {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to start tool router: {}", e),
                    );
                }
                let duration = start_time.elapsed();
                eprintln!("ToolRouter start call duration: {:?}", duration);
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
            unstructured_api.clone(),
        )
        .await;
        if let Err(e) = process_files_result {
            return Self::handle_error(&db, Some(user_profile), &job_id, &identity_secret_key, e, ws_manager).await;
        }

        // 2.- *If* a workflow is found, processing job message is taken over by this alternate logic
        let workflow_found_result = JobManager::should_process_workflow_for_tasks_take_over(
            db.clone(),
            vector_fs.clone(),
            &job_message.job_message,
            llm_provider_found.clone(),
            full_job.clone(),
            clone_signature_secret_key(&identity_secret_key),
            generator.clone(),
            user_profile.clone(),
            ws_manager.clone(),
            tool_router.clone(),
        )
        .await;

        let workflow_found = match workflow_found_result {
            Ok(found) => found,
            Err(e) => {
                return Self::handle_error(&db, Some(user_profile), &job_id, &identity_secret_key, e, ws_manager).await
            }
        };
        if workflow_found {
            return Ok(job_id);
        }

        // If a .jobkai file is found, processing job message is taken over by this alternate logic
        let jobkai_found_result = JobManager::should_process_job_files_for_tasks_take_over(
            db.clone(),
            vector_fs.clone(),
            &job_message.job_message,
            llm_provider_found.clone(),
            full_job.clone(),
            job_message.profile.clone(),
            clone_signature_secret_key(&identity_secret_key),
            unstructured_api.clone(),
            ws_manager.clone(),
        )
        .await;
        let jobkai_found = match jobkai_found_result {
            Ok(found) => found,
            Err(e) => {
                return Self::handle_error(&db, Some(user_profile), &job_id, &identity_secret_key, e, ws_manager).await
            }
        };
        if jobkai_found {
            return Ok(job_id);
        }

        // Otherwise proceed forward with rest of logic.
        let inference_chain_result = JobManager::process_inference_chain(
            db.clone(),
            vector_fs.clone(),
            clone_signature_secret_key(&identity_secret_key),
            job_message.job_message,
            full_job,
            llm_provider_found.clone(),
            user_profile.clone(),
            generator,
            ws_manager.clone(),
            tool_router.clone(),
        )
        .await;

        if let Err(e) = inference_chain_result {
            return Self::handle_error(&db, Some(user_profile), &job_id, &identity_secret_key, e, ws_manager).await;
        }

        Ok(job_id)
    }

    /// Handle errors by sending an error message to the job inbox
    async fn handle_error(
        db: &Arc<ShinkaiDB>,
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
    #[instrument(skip(identity_secret_key, db, vector_fs, generator, ws_manager, tool_router))]
    #[allow(clippy::too_many_arguments)]
    pub async fn process_inference_chain(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        identity_secret_key: SigningKey,
        job_message: JobMessage,
        full_job: Job,
        llm_provider_found: Option<SerializedLLMProvider>,
        user_profile: ShinkaiName,
        generator: RemoteEmbeddingGenerator,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<Mutex<ToolRouter>>>,
    ) -> Result<(), LLMProviderError> {
        let profile_name = user_profile.get_profile_name_string().unwrap_or_default();
        let job_id = full_job.job_id().to_string();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Inference chain - Processing Job: {:?}", full_job.job_id),
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
            prev_execution_context,
            generator,
            user_profile,
            ws_manager.clone(),
            tool_router.clone(),
        )
        .await?;
        let inference_response_content = inference_response.response;
        let new_execution_context = inference_response.new_job_execution_context;

        let duration = start.elapsed();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Time elapsed for inference chain processing is: {:?}", duration),
        );

        // Prepare data to save inference response to the DB
        let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            job_id.to_string(),
            inference_response_content.to_string(),
            "".to_string(),
            identity_secret_key_clone,
            profile_name.clone(),
            profile_name.clone(),
        )
        .map_err(|e| LLMProviderError::ShinkaiMessageBuilderError(e.to_string()))?;

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("process_inference_chain> shinkai_message: {:?}", shinkai_message).as_str(),
        );

        // Save response data to DB
        db.add_step_history(
            job_message.job_id.clone(),
            job_message.content,
            inference_response_content.to_string(),
            None,
        )?;
        db.add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message, None, ws_manager)
            .await?;
        db.set_job_execution_context(job_message.job_id.clone(), new_execution_context, None)?;

        Ok(())
    }

    /// Temporary function to process the files in the job message for workflows
    #[allow(clippy::too_many_arguments)]
    pub async fn should_process_workflow_for_tasks_take_over(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        job_message: &JobMessage,
        llm_provider_found: Option<SerializedLLMProvider>,
        full_job: Job,
        identity_secret_key: SigningKey,
        generator: RemoteEmbeddingGenerator,
        user_profile: ShinkaiName,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Option<Arc<Mutex<ToolRouter>>>,
    ) -> Result<bool, LLMProviderError> {
        if job_message.workflow_code.is_none() {
            return Ok(false);
        }

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Workflow Inference chain - Processing Job: {:?}", full_job),
        );

        // Setup initial data to get ready to call a specific inference chain
        let prev_execution_context = full_job.execution_context.clone();

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Prev Execution Context: {:?}", prev_execution_context),
        );

        let job_id = full_job.job_id().to_string();
        let llm_provider = llm_provider_found.ok_or(LLMProviderError::LLMProviderNotFound)?;
        let max_tokens_in_prompt = ModelCapabilitiesManager::get_max_input_tokens(&llm_provider.model);
        let parsed_user_message = ParsedUserMessage::new(job_message.content.to_string());
        let workflow = parse_workflow(&job_message.workflow_code.clone().unwrap())?;

        // eprintln!("should_process_workflow_for_tasks_take_over Full Job: {:?}", full_job);

        // Create the inference chain context
        let mut chain_context = InferenceChainContext::new(
            db.clone(),
            vector_fs.clone(),
            full_job,
            parsed_user_message,
            llm_provider,
            prev_execution_context,
            generator,
            user_profile.clone(),
            2,
            max_tokens_in_prompt,
            HashMap::new(),
            ws_manager.clone(),
            tool_router.clone(),
        );

        // Note: we do this once so we are not re-reading the files multiple times for each operation
        {
            let files = {
                let files_result = vector_fs.db.get_all_files_from_inbox(job_message.files_inbox.clone());
                // Check if there was an error getting the files
                match files_result {
                    Ok(files) => files,
                    Err(e) => return Err(LLMProviderError::VectorFS(e)),
                }
            };

            chain_context.update_raw_files(Some(files.into()));
        }

        // Available functions for the workflow
        let functions = HashMap::new();

        // TODO: read from tooling storage what we may have available

        // Call the inference chain router to choose which chain to use, and call it
        let mut dsl_inference = DslChain::new(Box::new(chain_context), workflow, functions);

        // Add the inference function to the functions map
        dsl_inference.add_inference_function();
        dsl_inference.add_inference_no_ws_function();
        dsl_inference.add_multi_inference_function();
        dsl_inference.add_all_generic_functions();
        dsl_inference.add_tools_from_router().await?;

        // Execute the workflow using run_chain
        let start = Instant::now();
        let inference_result = dsl_inference.run_chain().await?;
        let duration = start.elapsed();

        let response = inference_result.response;
        let new_execution_context = inference_result.new_job_execution_context;

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Time elapsed for inference chain processing is: {:?}", duration),
        );

        // Prepare data to save inference response to the DB
        let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);

        // TODO: can we extend it to add metadata somehow?
        // TODO: What should be the structre of this metadata?
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
            job_id,
            response.to_string(),
            "".to_string(),
            identity_secret_key_clone,
            user_profile.get_node_name_string(),
            user_profile.get_node_name_string(),
        )
        .map_err(|e| LLMProviderError::ShinkaiMessageBuilderError(e.to_string()))?;

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("process_inference_chain> shinkai_message: {:?}", shinkai_message).as_str(),
        );

        // Save response data to DB
        db.add_step_history(
            job_message.job_id.clone(),
            job_message.content.clone(),
            response.to_string(),
            None,
        )?;
        db.add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message, None, ws_manager)
            .await?;
        db.set_job_execution_context(job_message.job_id.clone(), new_execution_context, None)?;

        Ok(true)
    }

    /// Temporary function to process the files in the job message for tasks
    #[allow(clippy::too_many_arguments)]
    pub async fn should_process_job_files_for_tasks_take_over(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        job_message: &JobMessage,
        llm_provider_found: Option<SerializedLLMProvider>,
        full_job: Job,
        profile: ShinkaiName,
        identity_secret_key: SigningKey,
        _unstructured_api: UnstructuredAPI,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<bool, LLMProviderError> {
        if !job_message.files_inbox.is_empty() {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!(
                    "Searching for a .jobkai file in files: {}",
                    job_message.files_inbox.len()
                )
                .as_str(),
            );

            // Get the files from the DB
            let files = {
                let files_result = vector_fs.db.get_all_files_from_inbox(job_message.files_inbox.clone());
                // Check if there was an error getting the files
                match files_result {
                    Ok(files) => files,
                    Err(e) => return Err(LLMProviderError::VectorFS(e)),
                }
            };

            // Search for a .jobkai file
            for (filename, content) in files.into_iter() {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    &format!("Processing file: {}", filename),
                );

                let filename_lower = filename.to_lowercase();
                if filename_lower.ends_with(".png")
                    || filename_lower.ends_with(".jpg")
                    || filename_lower.ends_with(".jpeg")
                    || filename_lower.ends_with(".gif")
                {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Debug,
                        &format!("Found an image file: {}", filename),
                    );

                    let db_weak = Arc::downgrade(&db);
                    let agent_capabilities = ModelCapabilitiesManager::new(db_weak, profile.clone()).await;
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

                    let task = job_message.content.clone();
                    let file_extension = filename.split('.').last().unwrap_or("jpg");

                    // Call a new function
                    JobManager::handle_image_file(
                        db.clone(),
                        llm_provider_found.clone(),
                        full_job.clone(),
                        task,
                        content,
                        profile.clone(),
                        clone_signature_secret_key(&identity_secret_key),
                        file_extension.to_string(),
                        ws_manager.clone(),
                    )
                    .await?;
                    return Ok(true);
                }
            }
        }
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            "No .jobkai files found".to_string().as_str(),
        );
        Ok(false)
    }

    /// Processes the files sent together with the current job_message into Vector Resources,
    /// and saves them either into the local job scope, or the DB depending on `save_to_db_directly`.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_job_message_files_for_vector_resources(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        job_message: &JobMessage,
        agent_found: Option<SerializedLLMProvider>,
        full_job: &mut Job,
        profile: ShinkaiName,
        save_to_vector_fs_folder: Option<VRPath>,
        generator: RemoteEmbeddingGenerator,
        unstructured_api: UnstructuredAPI,
    ) -> Result<(), LLMProviderError> {
        if !job_message.files_inbox.is_empty() {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Processing files_map: ... files: {}", job_message.files_inbox.len()).as_str(),
            );
            eprintln!("Processing files_map: ... files: {}", job_message.files_inbox.len());
            {
                // Get the files from the DB
                let files = {
                    let files_result = vector_fs.db.get_all_files_from_inbox(job_message.files_inbox.clone());
                    // Check if there was an error getting the files
                    match files_result {
                        Ok(files) => files,
                        Err(e) => return Err(LLMProviderError::VectorFS(e)),
                    }
                };

                // Print out all the files
                for (filename, _) in &files {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Debug,
                        &format!("File found: {}", filename),
                    );
                    eprintln!("File found: {}", filename);
                }
            }
            // TODO: later we should able to grab errors and return them to the user
            let new_scope_entries_result = JobManager::process_files_inbox(
                db.clone(),
                vector_fs.clone(),
                agent_found,
                job_message.files_inbox.clone(),
                profile,
                save_to_vector_fs_folder,
                generator,
                unstructured_api,
            )
            .await;

            match new_scope_entries_result {
                Ok(new_scope_entries) => {
                    for (_, value) in new_scope_entries {
                        match value {
                            ScopeEntry::LocalScopeVRKai(local_entry) => {
                                if !full_job.scope.local_vrkai.contains(&local_entry) {
                                    full_job.scope.local_vrkai.push(local_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate LocalScopeVRKaiEntry detected",
                                    );
                                }
                            }
                            ScopeEntry::LocalScopeVRPack(local_entry) => {
                                if !full_job.scope.local_vrpack.contains(&local_entry) {
                                    full_job.scope.local_vrpack.push(local_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate LocalScopeVRPackEntry detected",
                                    );
                                }
                            }
                            ScopeEntry::VectorFSItem(fs_entry) => {
                                if !full_job.scope.vector_fs_items.contains(&fs_entry) {
                                    full_job.scope.vector_fs_items.push(fs_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate VectorFSScopeEntry detected",
                                    );
                                }
                            }
                            ScopeEntry::VectorFSFolder(fs_entry) => {
                                if !full_job.scope.vector_fs_folders.contains(&fs_entry) {
                                    full_job.scope.vector_fs_folders.push(fs_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate VectorFSScopeEntry detected",
                                    );
                                }
                            }
                            ScopeEntry::NetworkFolder(nf_entry) => {
                                if !full_job.scope.network_folders.contains(&nf_entry) {
                                    full_job.scope.network_folders.push(nf_entry);
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
                    db.update_job_scope(full_job.job_id().to_string(), full_job.scope.clone())?;
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
        }

        Ok(())
    }

    /// Processes the files in a given file inbox by generating VectorResources + job `ScopeEntry`s.
    /// If save_to_vector_fs_folder == true, the files will save to the DB and be returned as `VectorFSScopeEntry`s.
    /// Else, the files will be returned as LocalScopeEntries and thus held inside.
    #[allow(clippy::too_many_arguments)]
    pub async fn process_files_inbox(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        agent: Option<SerializedLLMProvider>,
        files_inbox: String,
        _profile: ShinkaiName,
        save_to_vector_fs_folder: Option<VRPath>,
        generator: RemoteEmbeddingGenerator,
        unstructured_api: UnstructuredAPI,
    ) -> Result<HashMap<String, ScopeEntry>, LLMProviderError> {
        // Create the RemoteEmbeddingGenerator instance
        let mut files_map: HashMap<String, ScopeEntry> = HashMap::new();

        // Get the files from the DB
        let files = {
            let files_result = vector_fs.db.get_all_files_from_inbox(files_inbox.clone());
            // Check if there was an error getting the files
            match files_result {
                Ok(files) => files,
                Err(e) => return Err(LLMProviderError::VectorFS(e)),
            }
        };

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

        let file_parser = match db.get_local_processing_preference()? {
            true => FileParser::Local,
            false => FileParser::Unstructured(unstructured_api),
        };

        let processed_vrkais =
            ParsingHelper::process_files_into_vrkai(dist_files, &generator, agent.clone(), file_parser).await?;

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
