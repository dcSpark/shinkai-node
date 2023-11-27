use super::chains::inference_chain_router::InferenceChain;
use super::job_prompts::JobPromptGenerator;
use crate::agent::agent::Agent;
use crate::agent::error::{self, AgentError};
use crate::agent::file_parsing::ParsingHelper;
use crate::agent::job::{Job, JobLike};
use crate::agent::job_manager::JobManager;
use crate::agent::queue::job_queue_manager::JobForProcessing;
use crate::cron_tasks::cron_manager::CronManagerError;
use crate::cron_tasks::web_scrapper::{CronTaskRequest, CronTaskResponse, WebScraper};
use crate::db::db_cron_task::CronTask;
use crate::db::db_errors::ShinkaiDBError;
use crate::db::ShinkaiDB;
use crate::planner::kai_files::{KaiJobFile, KaiSchemaType};
use blake3::Hasher;
use ed25519_dalek::SecretKey as SignatureStaticKey;
use rand::RngCore;
use serde_json::Value as JsonValue;
use serde_json::{to_string, Error};
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobCreationInfo;
use shinkai_message_primitives::shinkai_utils::job_scope::{DBScopeEntry, JobScope, LocalScopeEntry, ScopeEntry};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::utils::random_string;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::JobMessage},
    shinkai_utils::{shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key},
};
use shinkai_vector_resources::base_vector_resources::BaseVectorResource;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::source::{DocumentFileType, SourceFile, SourceFileType, VRSource};
use shinkai_vector_resources::vector_resource::VectorResource;
use std::result::Result::Ok;
use std::time::Instant;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use warp::filters::path::full;

impl JobManager {
    /// Processes a job message which will trigger a job step
    pub async fn process_job_message_queued(
        job_message: JobForProcessing,
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SignatureStaticKey,
    ) -> Result<String, AgentError> {
        let job_id = job_message.job_message.job_id.clone();
        // Fetch data we need to execute job step
        let (mut full_job, agent_found, profile_name, user_profile) =
            JobManager::fetch_relevant_job_data(&job_message.job_message.job_id, db.clone()).await?;

        // Added a new special file that if found it takes over
        let kai_found = JobManager::should_process_job_files_for_tasks_take_over(
            db.clone(),
            &job_message.job_message,
            agent_found.clone(),
            full_job.clone(),
            job_message.profile.clone(),
            clone_signature_secret_key(&identity_secret_key),
        )
        .await?;

        if kai_found {
            return Ok(job_id.clone());
        }

        // Processes any files which were sent with the job message
        JobManager::process_job_message_files_for_vector_resources(
            db.clone(),
            &job_message.job_message,
            agent_found.clone(),
            &mut full_job,
            job_message.profile,
            false,
        )
        .await?;

        match JobManager::process_inference_chain(
            db.clone(),
            clone_signature_secret_key(&identity_secret_key),
            job_message.job_message,
            full_job,
            agent_found.clone(),
            profile_name.clone(),
            user_profile,
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
                let shinkai_db = db.lock().await;
                shinkai_db.add_message_to_job_inbox(&job_id.clone(), &shinkai_message)?;
            }
        }

        return Ok(job_id.clone());
    }

    /// Inserts a KaiJobFile into a specific inbox
    pub async fn insert_kai_job_file_into_inbox(
        db: Arc<Mutex<ShinkaiDB>>,
        file_name_no_ext: String,
        kai_file: KaiJobFile,
    ) -> Result<String, AgentError> {
        let inbox_name = random_string();

        // Lock the database
        let mut db = db.lock().await;

        // Create the inbox
        match db.create_files_message_inbox(inbox_name.clone()) {
            Ok(_) => {
                // Convert the KaiJobFile to a JSON string
                let kai_file_json = to_string(&kai_file)?;

                // Convert the JSON string to bytes
                let kai_file_bytes = kai_file_json.into_bytes();

                // Save the KaiJobFile to the inbox
                let _ = db.add_file_to_files_message_inbox(
                    inbox_name.clone(),
                    format!("{}.jobkai", file_name_no_ext).to_string(),
                    kai_file_bytes,
                )?;
                return Ok(inbox_name);
            }
            Err(err) => return Err(AgentError::ShinkaiDB(ShinkaiDBError::RocksDBError(err))),
        }
    }

    /// Processes the provided message & job data, routes them to a specific inference chain,
    /// and then parses + saves the output result to the DB.
    pub async fn process_inference_chain(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SignatureStaticKey,
        job_message: JobMessage,
        full_job: Job,
        agent_found: Option<SerializedAgent>,
        profile_name: String,
        user_profile: Option<ShinkaiName>,
    ) -> Result<(), AgentError> {
        let job_id = full_job.job_id().to_string();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Processing job: {}", job_id),
        );
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Processing Job: {:?}", full_job),
        );

        // Setup initial data to get ready to call a specific inference chain
        let prev_execution_context = full_job.execution_context.clone();
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            &format!("Prev Execution Context: {:?}", prev_execution_context),
        );
        let generator = RemoteEmbeddingGenerator::new_default();
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
        let shinkai_db = db.lock().await;
        shinkai_db.add_step_history(
            job_message.job_id.clone(),
            job_message.content,
            inference_response_content.to_string(),
        )?;
        shinkai_db.add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message)?;
        shinkai_db.set_job_execution_context(&job_message.job_id.clone(), new_execution_context)?;

        Ok(())
    }

    /// Temporary function to process the files in the job message for tasks
    pub async fn should_process_job_files_for_tasks_take_over(
        db: Arc<Mutex<ShinkaiDB>>,
        job_message: &JobMessage,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        profile: ShinkaiName,
        identity_secret_key: SignatureStaticKey,
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
                    // new code
                    match kai_file.schema {
                        KaiSchemaType::CronJobRequest(cron_task_request) => {
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
                            eprintln!("CronJob: {:?}", cron_task);
                            // Handle CronJob
                            JobManager::handle_cron_job(
                                db.clone(),
                                agent_found.clone(),
                                full_job.clone(),
                                cron_task,
                                profile.clone(),
                                clone_signature_secret_key(&identity_secret_key),
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

    /// Processes the provided message & job data, routes them to a specific inference chain,
    pub async fn handle_cron_job_request(
        db: Arc<Mutex<ShinkaiDB>>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        job_message: JobMessage,
        cron_task_request: CronTaskRequest,
        profile: ShinkaiName,
        identity_secret_key: SignatureStaticKey,
    ) -> Result<bool, AgentError> {
        // Setup initial data to get ready to call a specific inference chain
        let prev_execution_context = full_job.execution_context.clone();

        // Call the inference chain router to choose which chain to use, and call it
        let (inference_response_content, new_execution_context) = Self::alt_inference_chain_router(
            db.clone(),
            agent_found.clone(),
            full_job.clone(),
            job_message.clone(),
            cron_task_request.clone(),
            prev_execution_context,
            Some(profile.clone()),
        )
        .await?;

        // Prepare data to save inference response to the DB
        let cron_task_response = CronTaskResponse {
            cron_task_request: cron_task_request,
            cron_description: inference_response_content.cron_expression.to_string(),
            pddl_plan_problem: inference_response_content.pddl_plan_problem.to_string(),
            pddl_plan_domain: Some(inference_response_content.pddl_plan_domain.to_string()),
        };

        let agg_response = cron_task_response.to_string();
        let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);
        let agent = agent_found.ok_or(AgentError::AgentNotFound)?;

        let kai_file = KaiJobFile {
            schema: KaiSchemaType::CronJobResponse(cron_task_response.clone()),
            shinkai_profile: Some(profile.clone()),
            agent_id: agent.id.clone(),
        };

        let inbox_name_result =
            Self::insert_kai_job_file_into_inbox(db.clone(), "cron_request".to_string(), kai_file).await;

        match inbox_name_result {
            Ok(inbox_name) => {
                let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
                    full_job.job_id.to_string(),
                    agg_response.clone(),
                    inbox_name,
                    identity_secret_key_clone,
                    profile.node_name.clone(),
                    profile.node_name.clone(),
                )
                .unwrap();

                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    format!("process_inference_chain> shinkai_message: {:?}", shinkai_message).as_str(),
                );

                // Save response data to DB
                let shinkai_db = db.lock().await;
                shinkai_db.add_step_history(job_message.job_id.clone(), job_message.content, agg_response)?;
                shinkai_db.add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message)?;
                shinkai_db.set_job_execution_context(&job_message.job_id.clone(), new_execution_context)?;

                Ok(true)
            }
            Err(err) => Err(err),
        }
    }

    /// Processes the provided message & job data, routes them to a specific inference chain,
    pub async fn handle_cron_job(
        db: Arc<Mutex<ShinkaiDB>>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        cron_job: CronTask,
        profile: ShinkaiName,
        identity_secret_key: SignatureStaticKey,
    ) -> Result<(), AgentError> {
        let prev_execution_context = full_job.execution_context.clone();

        // Create a new instance of the WebScraper
        let scraper = WebScraper {
            task: cron_job.clone(),
            // TODO: Move to ENV
            api_url: "https://internal.shinkai.com/x-unstructured-api/general/v0/general".to_string(),
        };

        // Call the download_and_parse method of the WebScraper
        match scraper.download_and_parse().await {
            Ok(content) => {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    "Web scraping completed successfully",
                );
                shinkai_log(
                    ShinkaiLogOption::CronExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Content: {:?}", content.structured).as_str(),
                );
                let links = WebScraper::extract_links(&content.unfiltered);

                let (inference_response_content, new_execution_context) =
                    JobManager::cron_inference_chain_router_summary(
                        db.clone(),
                        agent_found.clone(),
                        full_job.clone(),
                        cron_job.prompt.clone(),
                        content.structured.clone(),
                        links,
                        prev_execution_context.clone(),
                        Some(profile.clone()),
                        // TODO(Nico): probably the router should do the inference but we are not clear how yet
                        InferenceChain::CronExecutionChainMainTask,
                    )
                    .await?;
                shinkai_log(
                    ShinkaiLogOption::CronExecution,
                    ShinkaiLogLevel::Debug,
                    format!("Crawl links: {}", cron_job.crawl_links).as_str(),
                );
                // Create Job
                let job_id = full_job.job_id().to_string();
                let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone())?;
                let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
                    full_job.clone().job_id.to_string(),
                    inference_response_content.clone(),
                    inbox_name.to_string(),
                    clone_signature_secret_key(&identity_secret_key),
                    profile.node_name.clone(),
                    profile.node_name.clone(),
                )
                .unwrap();

                // Save response data to DB
                {
                    let shinkai_db = db.lock().await;
                    shinkai_db.add_step_history(job_id.clone(), "".to_string(), inference_response_content.clone())?;
                    shinkai_db.add_message_to_job_inbox(&job_id.clone(), &shinkai_message)?;
                    shinkai_db.set_job_execution_context(&job_id.clone(), new_execution_context)?;
                }

                // If crawl_links is true, scan for all the links in content and download_and_parse them as well
                if cron_job.crawl_links {
                    let links = WebScraper::extract_links(&inference_response_content);
                    eprintln!("Extracted Links: {:?}", links);

                    for link in links {
                        let mut scraper_for_link = scraper.clone();
                        scraper_for_link.task.url = link.clone();
                        match scraper_for_link.download_and_parse().await {
                            Ok(content) => {
                                eprintln!("Link: {:?}", link);
                                eprintln!("web scrapping result {:?}", content.structured);
                                let (inference_response_content, new_execution_context) =
                                    JobManager::cron_inference_chain_router_summary(
                                        db.clone(),
                                        agent_found.clone(),
                                        full_job.clone(),
                                        cron_job.prompt.clone(),
                                        content.structured.clone(),
                                        vec![],
                                        prev_execution_context.clone(),
                                        Some(profile.clone()),
                                        InferenceChain::CronExecutionChainSubtask,
                                    )
                                    .await?;

                                eprintln!("Inference response content: {:?}", inference_response_content);

                                let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
                                    full_job.clone().job_id.to_string(),
                                    inference_response_content.clone(),
                                    inbox_name.to_string(),
                                    clone_signature_secret_key(&identity_secret_key),
                                    profile.node_name.clone(),
                                    profile.node_name.clone(),
                                )
                                .unwrap();

                                // Save response data to DB
                                let shinkai_db = db.lock().await;
                                shinkai_db.add_step_history(
                                    job_id.clone(),
                                    "".to_string(),
                                    inference_response_content.clone(),
                                )?;
                                shinkai_db.add_message_to_job_inbox(&job_id.clone(), &shinkai_message)?;
                                shinkai_db.set_job_execution_context(&job_id.clone(), new_execution_context)?;
                            }
                            Err(e) => {
                                eprintln!("Web scraping failed for link: {:?}, error: {:?}", link, e);
                                shinkai_log(
                                    ShinkaiLogOption::CronExecution,
                                    ShinkaiLogLevel::Error,
                                    format!("Web scraping failed for link: {:?}, error: {:?}", link, e).as_str(),
                                );
                            }
                        }
                    }
                }
            }
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::CronExecution,
                    ShinkaiLogLevel::Error,
                    format!("Web scraping failed: {:?}", e).as_str(),
                );
                return Err(AgentError::WebScrapingFailed("Your error message".into()));
            }
        }
        Ok(())
    }

    /// Processes the files sent together with the current job_message into Vector Resources,
    /// and saves them either into the local job scope, or the DB depending on `save_to_db_directly`.
    pub async fn process_job_message_files_for_vector_resources(
        db: Arc<Mutex<ShinkaiDB>>,
        job_message: &JobMessage,
        agent_found: Option<SerializedAgent>,
        full_job: &mut Job,
        profile: ShinkaiName,
        save_to_db_directly: bool,
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
                save_to_db_directly,
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
                            ScopeEntry::Database(db_entry) => {
                                if !full_job.scope.database.contains(&db_entry) {
                                    full_job.scope.database.push(db_entry);
                                } else {
                                    shinkai_log(
                                        ShinkaiLogOption::JobExecution,
                                        ShinkaiLogLevel::Error,
                                        "Duplicate DBScopeEntry detected",
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
        } else {
            // TODO: move this somewhere else
            let mut shinkai_db = db.lock().await;
            match shinkai_db.init_profile_resource_router(&profile) {
                Ok(_) => std::mem::drop(shinkai_db), // required to avoid deadlock
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Error,
                        format!("Error initializing profile resource router: {}", e).as_str(),
                    );
                    return Err(AgentError::ShinkaiDB(e));
                }
            }
        }

        Ok(())
    }

    /// Processes the files in a given file inbox by generating VectorResources + job `ScopeEntry`s.
    /// If save_to_db_directly == true, the files will save to the DB and be returned as `DBScopeEntry`s.
    /// Else, the files will be returned as `LocalScopeEntry`s and thus held inside.
    pub async fn process_files_inbox(
        db: Arc<Mutex<ShinkaiDB>>,
        agent: Option<SerializedAgent>,
        files_inbox: String,
        profile: ShinkaiName,
        save_to_db_directly: bool,
    ) -> Result<HashMap<String, ScopeEntry>, AgentError> {
        // Handle the None case if the agent is not found
        let agent = match agent {
            Some(agent) => agent,
            None => return Err(AgentError::AgentNotFound),
        };

        // Create the RemoteEmbeddingGenerator instance
        let generator = Arc::new(RemoteEmbeddingGenerator::new_default());
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
                &*generator,
                filename.clone(),
                &vec![],
                agent.clone(),
                400,
            )
            .await?;

            // Now create Local/DBScopeEntry depending on setting
            if save_to_db_directly {
                let resource_header = resource.as_trait_object().generate_resource_header();
                let shinkai_db = db.lock().await;
                shinkai_db.init_profile_resource_router(&profile)?;
                shinkai_db.save_resource(resource, &profile).unwrap();

                let db_scope_entry = DBScopeEntry {
                    resource_header: resource_header,
                    source: VRSource::from_file(&filename, &content)?,
                };
                files_map.insert(filename, ScopeEntry::Database(db_scope_entry));
            } else {
                let local_scope_entry = LocalScopeEntry {
                    resource: resource,
                    source: SourceFile::new(
                        filename.clone(),
                        SourceFileType::Document(DocumentFileType::Pdf),
                        content,
                    ),
                };
                files_map.insert(filename, ScopeEntry::Local(local_scope_entry));
            }
        }

        Ok(files_map)
    }
}
