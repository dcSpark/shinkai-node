use std::{sync::Arc, time::Instant};

use ed25519_dalek::SecretKey as SignatureStaticKey;
use serde_json::to_string;
use shinkai_message_primitives::{
    schemas::{agents::serialized_agent::SerializedAgent, inbox_name::InboxName, shinkai_name::ShinkaiName},
    shinkai_message::shinkai_message_schemas::JobMessage,
    shinkai_utils::{
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::clone_signature_secret_key,
        utils::random_string,
    },
};
use tokio::sync::Mutex;

use crate::{
    agent::{
        error::AgentError, execution::chains::inference_chain_router::InferenceChain, job::Job, job_manager::JobManager,
    },
    cron_tasks::web_scrapper::{CronTaskRequest, CronTaskRequestResponse, WebScraper},
    db::{db_cron_task::CronTask, db_errors::ShinkaiDBError, ShinkaiDB},
    planner::{kai_files::{KaiJobFile, KaiSchemaType}, kai_manager::KaiJobFileManager},
};

impl JobManager {
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
        let cron_task_response = CronTaskRequestResponse {
            cron_task_request: cron_task_request,
            cron_description: inference_response_content.cron_expression.to_string(),
            pddl_plan_problem: inference_response_content.pddl_plan_problem.to_string(),
            pddl_plan_domain: Some(inference_response_content.pddl_plan_domain.to_string()),
        };

        let agg_response = cron_task_response.to_string();
        let identity_secret_key_clone = clone_signature_secret_key(&identity_secret_key);
        let agent = agent_found.ok_or(AgentError::AgentNotFound)?;

        let kai_file = KaiJobFile {
            schema: KaiSchemaType::CronJobRequestResponse(cron_task_response.clone()),
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
                let job_id = full_job.job_id.to_string();
                let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
                    full_job.clone().job_id.to_string(),
                    inference_response_content.clone(),
                    "".to_string(),
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
                                    "".to_string(),
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

    /// Processes the provided image file
    pub async fn handle_image_file(
        db: Arc<Mutex<ShinkaiDB>>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        task: String,
        content: Vec<u8>,
        profile: ShinkaiName,
        identity_secret_key: SignatureStaticKey,
        file_extension: String,
    ) -> Result<(), AgentError> {
        let prev_execution_context = full_job.execution_context.clone();
        let base64_image = format!("data:image/{};base64,{}", file_extension, base64::encode(&content));

        // TODO: fix the new_execution_context
        let (inference_response_content, new_execution_context) = JobManager::image_analysis_chain(
            db.clone(),
            full_job.clone(),
            agent_found.clone(),
            prev_execution_context.clone(),
            Some(profile.clone()),
            task.clone(),
            base64_image,
            0,
            3,
        )
        .await?;

        let shinkai_message = ShinkaiMessageBuilder::job_message_from_agent(
            full_job.job_id.to_string(),
            inference_response_content.clone().to_string(),
            "".to_string(),
            clone_signature_secret_key(&identity_secret_key),
            profile.node_name.clone(),
            profile.node_name.clone(),
        )
        .unwrap();

        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Debug,
            format!("process_image_file> shinkai_message: {:?}", shinkai_message).as_str(),
        );

        // Save response data to DB
        let shinkai_db = db.lock().await;
        shinkai_db.add_step_history(
            full_job.job_id.clone(),
            "".to_string(),
            inference_response_content.to_string(),
        )?;
        shinkai_db.add_message_to_job_inbox(&full_job.job_id.clone(), &shinkai_message)?;
        shinkai_db.set_job_execution_context(&full_job.job_id.clone(), prev_execution_context)?;

        Ok(())
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
}
