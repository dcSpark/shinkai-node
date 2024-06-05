use std::sync::Arc;

use ed25519_dalek::SigningKey;
use serde_json::to_string;
use shinkai_message_primitives::{
    schemas::{
        agents::serialized_agent::{AgentLLMInterface, SerializedAgent},
        shinkai_name::ShinkaiName,
    },
    shinkai_message::shinkai_message_schemas::JobMessage,
    shinkai_utils::{
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::clone_signature_secret_key,
    },
};
use shinkai_vector_resources::{file_parser::unstructured_api::UnstructuredAPI, utils::random_string};

use crate::{
    agent::{
        error::AgentError, execution::chains::inference_chain_router::InferenceChainDecision, job::Job,
        job_manager::JobManager,
    },
    cron_tasks::web_scrapper::{CronTaskRequest, CronTaskRequestResponse, WebScraper},
    db::{db_cron_task::CronTask, db_errors::ShinkaiDBError, ShinkaiDB},
    planner::kai_files::{KaiJobFile, KaiSchemaType},
    vector_fs::vector_fs::VectorFS,
};

impl JobManager {
    /// Processes the provided message & job data, routes them to a specific inference chain,
    pub async fn handle_cron_job_request(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        job_message: JobMessage,
        cron_task_request: CronTaskRequest,
        profile: ShinkaiName,
        identity_secret_key: SigningKey,
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
            cron_task_request,
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
            Self::insert_kai_job_file_into_inbox(db.clone(), vector_fs.clone(), "cron_request".to_string(), kai_file)
                .await;

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
                db.add_step_history(job_message.job_id.clone(), job_message.content, agg_response, None)?;
                db.add_message_to_job_inbox(&job_message.job_id.clone(), &shinkai_message, None)
                    .await?;
                db.set_job_execution_context(job_message.job_id.clone(), new_execution_context, None)?;

                Ok(true)
            }
            Err(err) => Err(err),
        }
    }

    /// Processes the provided message & job data, routes them to a specific inference chain,
    pub async fn handle_cron_job(
        db: Arc<ShinkaiDB>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        cron_job: CronTask,
        profile: ShinkaiName,
        identity_secret_key: SigningKey,
        unstructured_api: UnstructuredAPI,
    ) -> Result<(), AgentError> {
        let prev_execution_context = full_job.execution_context.clone();

        // Create a new instance of the WebScraper
        let scraper = WebScraper {
            task: cron_job.clone(),
            unstructured_api,
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
                        InferenceChainDecision::new_no_results("CronExecutionChainMainTask".to_string()),
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
                    db.add_step_history(job_id.clone(), "".to_string(), inference_response_content.clone(), None)?;
                    db.add_message_to_job_inbox(&job_id.clone(), &shinkai_message, None)
                        .await?;
                    db.set_job_execution_context(job_id.clone(), new_execution_context, None)?;
                }

                // If crawl_links is true, scan for all the links in content and download_and_parse them as well
                if cron_job.crawl_links {
                    let links = WebScraper::extract_links(&inference_response_content);

                    for link in links {
                        let mut scraper_for_link = scraper.clone();
                        scraper_for_link.task.url.clone_from(&link);
                        match scraper_for_link.download_and_parse().await {
                            Ok(content) => {
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
                                        InferenceChainDecision::new_no_results("CronExecutionChainSubtask".to_string()),
                                    )
                                    .await?;

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
                                db.add_step_history(
                                    job_id.clone(),
                                    "".to_string(),
                                    inference_response_content.clone(),
                                    None,
                                )?;
                                db.add_message_to_job_inbox(&job_id.clone(), &shinkai_message, None)
                                    .await?;
                                db.set_job_execution_context(job_id.clone(), new_execution_context, None)?;
                            }
                            Err(e) => {
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
        db: Arc<ShinkaiDB>,
        agent_found: Option<SerializedAgent>,
        full_job: Job,
        task: String,
        content: Vec<u8>,
        profile: ShinkaiName,
        identity_secret_key: SigningKey,
        file_extension: String,
    ) -> Result<(), AgentError> {
        let prev_execution_context = full_job.execution_context.clone();

        let base64_image = match &agent_found {
            Some(agent) => match agent.model {
                AgentLLMInterface::OpenAI(_) => {
                    format!("data:image/{};base64,{}", file_extension, base64::encode(&content))
                }
                AgentLLMInterface::ShinkaiBackend(_) => {
                    format!("data:image/{};base64,{}", file_extension, base64::encode(&content))
                }
                _ => base64::encode(&content),
            },
            None => base64::encode(&content),
        };

        // TODO: fix the new_execution_context
        let (inference_response_content, _) = JobManager::image_analysis_chain(
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
        db.add_step_history(
            full_job.job_id.clone(),
            "".to_string(),
            inference_response_content.to_string(),
            None,
        )?;
        db.add_message_to_job_inbox(&full_job.job_id.clone(), &shinkai_message, None)
            .await?;
        db.set_job_execution_context(full_job.job_id.clone(), prev_execution_context, None)?;

        Ok(())
    }

    /// Inserts a KaiJobFile into a specific inbox
    pub async fn insert_kai_job_file_into_inbox(
        db: Arc<ShinkaiDB>,
        vector_fs: Arc<VectorFS>,
        file_name_no_ext: String,
        kai_file: KaiJobFile,
    ) -> Result<String, AgentError> {
        let inbox_name = random_string();

        // Create the inbox
        match db.create_files_message_inbox(inbox_name.clone()) {
            Ok(_) => {
                // Convert the KaiJobFile to a JSON string
                let kai_file_json = to_string(&kai_file)?;

                // Convert the JSON string to bytes
                let kai_file_bytes = kai_file_json.into_bytes();

                // Save the KaiJobFile to the inbox
                vector_fs.db.add_file_to_files_message_inbox(
                    inbox_name.clone(),
                    format!("{}.jobkai", file_name_no_ext).to_string(),
                    kai_file_bytes,
                )?;
                Ok(inbox_name)
            }
            Err(err) => Err(AgentError::ShinkaiDB(ShinkaiDBError::RocksDBError(err))),
        }
    }
}
