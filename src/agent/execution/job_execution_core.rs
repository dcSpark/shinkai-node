use crate::agent::error::AgentError;
use crate::agent::job::{Job, JobLike};
use crate::agent::job_manager::JobManager;
use crate::agent::queue::job_queue_manager::JobForProcessing;
use crate::db::ShinkaiDB;
use crate::managers::model_capabilities_manager::{ModelCapabilitiesManager, ModelCapability};
use crate::planner::kai_files::{KaiJobFile, KaiSchemaType};
use ed25519_dalek::SigningKey;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::shinkai_utils::job_scope::{LocalScopeEntry, ScopeEntry, VectorFSScopeEntry};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::shinkai_message_schemas::JobMessage,
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::source::{DocumentFileType, SourceFile, SourceFileType, TextChunkingStrategy, VRSource};
use shinkai_vector_resources::unstructured::unstructured_api::UnstructuredAPI;
use shinkai_vector_resources::vector_resource::VRPath;
use std::result::Result::Ok;
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::instrument;

impl JobManager {
    /// Processes a job message which will trigger a job step
    #[instrument(skip(identity_secret_key, db))]
    pub async fn process_job_message_queued(
        job_message: JobForProcessing,
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SigningKey,
        generator: RemoteEmbeddingGenerator,
        unstructured_api: UnstructuredAPI,
    ) -> Result<String, AgentError> {
        let job_id = job_message.job_message.job_id.clone();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Processing job: {}", job_id),
        );
        // Fetch data we need to execute job step
        let (mut full_job, agent_found, profile_name, user_profile) =
            JobManager::fetch_relevant_job_data(&job_message.job_message.job_id, db.clone()).await?;

        // If a .jobkai file is found, processing job message is taken over by this alternate logic
        let kai_found = JobManager::should_process_job_files_for_tasks_take_over(
            db.clone(),
            &job_message.job_message,
            agent_found.clone(),
            full_job.clone(),
            job_message.profile.clone(),
            clone_signature_secret_key(&identity_secret_key),
            unstructured_api.clone(),
        )
        .await?;
        if kai_found {
            return Ok(job_id.clone());
        }

        // Otherwise proceed forward with rest of logic.
        // Processes any files which were sent with the job message
        JobManager::process_job_message_files_for_vector_resources(
            db.clone(),
            &job_message.job_message,
            agent_found.clone(),
            &mut full_job,
            job_message.profile,
            None,
            generator.clone(),
            unstructured_api.clone(),
        )
        .await?;

        // Ensure the user profile exists before proceeding with inference chain
        let user_profile = &user_profile.clone().ok_or(AgentError::NoUserProfileFound)?;
        match JobManager::process_inference_chain(
            db.clone(),
            clone_signature_secret_key(&identity_secret_key),
            job_message.job_message,
            full_job,
            agent_found.clone(),
            profile_name.clone(),
            user_profile.clone(),
            generator,
        )
        .await
        {
            Ok(_) => (),
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Error,
                    &format!("Error processing inference chain: {}", e),
                );

                let error_for_user = format!("Error processing message. More info: {}", e);

                // Prepare data to save inference response to the DB
                let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);
                let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
                    job_id.to_string(),
                    error_for_user.to_string(),
                    "".to_string(),
                    identity_secret_key_clone,
                    profile_name.clone(),
                    profile_name.clone(),
                )
                .unwrap();

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("process_inference_chain> shinkai_message: {:?}", shinkai_message).as_str(),
                );

                // Save response data to DB
                let mut shinkai_db = db.lock().await;
                shinkai_db
                    .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, None)
                    .await?;
            }
        }

        return Ok(job_id.clone());
    }

    /// Processes the provided message & job data, routes them to a specific inference chain,
    /// and then parses + saves the output result to the DB.
    pub async fn process_inference_chain(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SigningKey,
        job_message: JobMessage,
        full_job: Job,
        agent_found: Option<SerializedAgent>,
        profile_name: String,
        user_profile: ShinkaiName,
        generator: RemoteEmbeddingGenerator,
    ) -> Result<(), AgentError> {
        let job_id = full_job.job_id().to_string();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Inference chain - Processing Job: {:?}", full_job),
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
        let (inference_response_content, new_execution_context) = JobManager::inference_chain_router(
            db.clone(),
            agent_found,
            full_job,
            job_message.clone(),
            prev_execution_context,
            &generator,
            user_profile,
        )
        .await?;
        let duration = start.elapsed();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Time elapsed for inference chain processing is: {:?}", duration),
        );

        // Prepare data to save inference response to the DB
        let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);
        let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
            job_id.to_string(),
            inference_response_content.to_string(),
            "".to_string(),
            identity_secret_key_clone,
            profile_name.clone(),
            profile_name.clone(),
        )
        .unwrap();

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("process_inference_chain> shinkai_message: {:?}", shinkai_message).as_str(),
        );

        // Save response data to DB
        let mut shinkai_db = db.lock().await;
        shinkai_db.add_step_history(
            job_message.job_id.clone(),
            job_message.content,
            inference_response_content.to_string(),
            None,
        )?;
        shinkai_db
            .add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message, None)
            .await?;
        shinkai_db.set_job_execution_context(job_message.job_id.clone(), new_execution_context, None)?;

        Ok(())
    }

    /// Temporary function to process the files in the job message for tasks
    pub async fn should_process_job_files_for_tasks_take_over(
        db: Arc<Mutex<ShinkaiDB>>,
        job_message: &JobMessage,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        profile: ShinkaiName,
        identity_secret_key: SigningKey,
        unstructured_api: UnstructuredAPI,
    ) -> Result<bool, AgentError> {
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
                let shinkai_db = db.lock().await;
                let files_result = shinkai_db.get_all_files_from_inbox(job_message.files_inbox.clone());
                // Check if there was an error getting the files
                match files_result {
                    Ok(files) => files,
                    Err(e) => return Err(AgentError::ShinkaiDB(e)),
                }
            };

            // Search for a .jobkai file
            for (filename, content) in files.into_iter() {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    &format!("Processing file: {}", filename),
                );

                if filename.ends_with(".jobkai") {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Debug,
                        &format!("Found a .jobkai file: {}", filename),
                    );

                    let content_str = String::from_utf8(content.clone()).unwrap();
                    let kai_file_result: Result<KaiJobFile, serde_json::Error> =
                        KaiJobFile::from_json_str(&content_str);
                    let kai_file = match kai_file_result {
                        Ok(kai_file) => kai_file,
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                &format!("Error parsing KaiJobFile: {}", e),
                            );
                            return Err(AgentError::AgentNotFound);
                        }
                    };
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Debug,
                        format!("KaiJobFile: {:?}", kai_file).as_str(),
                    );
                    match kai_file.schema {
                        KaiSchemaType::CronJobRequest(cron_task_request) => {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Debug,
                                format!("CronJobRequest: {:?}", cron_task_request).as_str(),
                            );
                            // Handle CronJobRequest
                            JobManager::handle_cron_job_request(
                                db.clone(),
                                agent_found.clone(),
                                full_job.clone(),
                                job_message.clone(),
                                cron_task_request,
                                profile.clone(),
                                clone_signature_secret_key(&identity_secret_key),
                            )
                            .await?;
                            return Ok(true);
                        }
                        KaiSchemaType::CronJob(cron_task) => {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Debug,
                                format!("CronJob: {:?}", cron_task).as_str(),
                            );
                            // Handle CronJob
                            JobManager::handle_cron_job(
                                db.clone(),
                                agent_found.clone(),
                                full_job.clone(),
                                cron_task,
                                profile.clone(),
                                clone_signature_secret_key(&identity_secret_key),
                                unstructured_api,
                            )
                            .await?;
                            return Ok(true);
                        }
                        _ => {
                            shinkai_log(
                                ShinkaiLogOption::JobExecution,
                                ShinkaiLogLevel::Error,
                                "Unexpected schema type in KaiJobFile",
                            );
                            return Err(AgentError::AgentNotFound);
                        }
                    }
                } else if filename.ends_with(".png")
                    || filename.ends_with(".jpg")
                    || filename.ends_with(".jpeg")
                    || filename.ends_with(".gif")
                {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Debug,
                        &format!("Found an image file: {}", filename),
                    );

                    let agent_capabilities = ModelCapabilitiesManager::new(db.clone(), profile.clone()).await;
                    let has_image_analysis = agent_capabilities.has_capability(ModelCapability::ImageAnalysis).await;

                    if !has_image_analysis {
                        shinkai_log(
                            ShinkaiLogOption::JobExecution,
                            ShinkaiLogLevel::Error,
                            "Agent does not have ImageAnalysis capability",
                        );
                        return Err(AgentError::AgentMissingCapabilities(
                            "Agent does not have ImageAnalysis capability".to_string(),
                        ));
                    }

                    let task = job_message.content.clone();
                    let file_extension = filename.split('.').last().unwrap_or("jpg");

                    // Call a new function
                    JobManager::handle_image_file(
                        db.clone(),
                        agent_found.clone(),
                        full_job.clone(),
                        task,
                        content,
                        profile.clone(),
                        clone_signature_secret_key(&identity_secret_key),
                        file_extension.to_string(),
                    )
                    .await?;
                    return Ok(true);
                }
            }
        }
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("No .jobkai files found").as_str(),
        );
        Ok(false)
    }

    /// Processes the files sent together with the current job_message into Vector Resources,
    /// and saves them either into the local job scope, or the DB depending on `save_to_db_directly`.
    pub async fn process_job_message_files_for_vector_resources(
        db: Arc<Mutex<ShinkaiDB>>,
        job_message: &JobMessage,
        agent_found: Option<SerializedAgent>,
        full_job: &mut Job,
        profile: ShinkaiName,
        save_to_vector_fs_folder: Option<VRPath>,
        generator: RemoteEmbeddingGenerator,
        unstructured_api: UnstructuredAPI,
    ) -> Result<(), AgentError> {
        if !job_message.files_inbox.is_empty() {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                format!("Processing files_map: ... files: {}", job_message.files_inbox.len()).as_str(),
            );
            // TODO: later we should able to grab errors and return them to the user
            let new_scope_entries_result = JobManager::process_files_inbox(
                db.clone(),
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
                            ScopeEntry::Local(local_entry) => {
                                if !full_job.scope.local.contains(&local_entry) {
                                    full_job.scope.local.push(local_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate LocalScopeEntry detected",
                                    );
                                }
                            }
                            ScopeEntry::VectorFS(fs_entry) => {
                                if !full_job.scope.vector_fs.contains(&fs_entry) {
                                    full_job.scope.vector_fs.push(fs_entry);
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
                    let mut shinkai_db = db.lock().await;
                    shinkai_db.update_job_scope(full_job.job_id().to_string(), full_job.scope.clone())?;
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
    /// If save_to_db_directly == true, the files will save to the DB and be returned as `VectorFSScopeEntry`s.
    /// Else, the files will be returned as `LocalScopeEntry`s and thus held inside.
    pub async fn process_files_inbox(
        db: Arc<Mutex<ShinkaiDB>>,
        agent: Option<SerializedAgent>,
        files_inbox: String,
        profile: ShinkaiName,
        save_to_vector_fs_folder: Option<VRPath>,
        generator: RemoteEmbeddingGenerator,
        unstructured_api: UnstructuredAPI,
    ) -> Result<HashMap<String, ScopeEntry>, AgentError> {
        // Handle the None case if the agent is not found
        let agent = match agent {
            Some(agent) => agent,
            None => return Err(AgentError::AgentNotFound),
        };

        // Create the RemoteEmbeddingGenerator instance
        let mut files_map: HashMap<String, ScopeEntry> = HashMap::new();

        // Get the files from the DB
        let files = {
            let shinkai_db = db.lock().await;
            let files_result = shinkai_db.get_all_files_from_inbox(files_inbox.clone());
            // Check if there was an error getting the files
            match files_result {
                Ok(files) => files,
                Err(e) => return Err(AgentError::ShinkaiDB(e)),
            }
        };

        // Start processing the files
        for (filename, content) in files.into_iter() {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Debug,
                &format!("Processing file: {}", filename),
            );
            let resource = JobManager::parse_file_into_resource_gen_desc(
                content.clone(),
                &generator,
                filename.clone(),
                &vec![],
                agent.clone(),
                400,
                unstructured_api.clone(),
            )
            .await?;

            // Now create Local/VectorFSScopeEntry depending on setting
            let text_chunking_strategy = TextChunkingStrategy::V1;
            if let Some(folder_path) = &save_to_vector_fs_folder {
                // TODO: Save to VectorFS
                let resource_header = resource.as_trait_object().generate_resource_header();
                let fs_scope_entry = VectorFSScopeEntry {
                    resource_header: resource_header,
                    vector_fs_path: folder_path.clone(),
                };
                files_map.insert(filename, ScopeEntry::VectorFS(fs_scope_entry));
            } else {
                let local_scope_entry = LocalScopeEntry {
                    resource: resource,
                    source: SourceFile::new_standard_source_file(
                        filename.clone(),
                        SourceFileType::detect_file_type(&filename)?,
                        content,
                        None,
                    ),
                };
                files_map.insert(filename, ScopeEntry::Local(local_scope_entry));
            }
        }

        Ok(files_map)
    }
}
