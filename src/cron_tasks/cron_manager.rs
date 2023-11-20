/*

Stories

a) Add new Task
User creates a new cron tab by explaining in natural language what they want to do, to what (URL) and when. I will use two inputs to make it easier for the LLM for now.

Note: No navigation for now.

It will show back the cron tab in code and natural language from that (use JS library), and ask for confirmation.

b) See all Tasks (and update or remove them)

The user is able to see all the tasks, update them or remove them.

c) Execute Task

- Have a thread that runs every minute and checks if there are any tasks to execute
- (Option B) sleep until the next cycle, then check all the tasks, calculate when is the next one to execute, and sleep until then
- Execute task

*/

use core::panic;
use std::{
    collections::{HashMap, HashSet},
    mem,
    pin::Pin,
    sync::Arc,
};

use chrono::{DateTime, Timelike, Utc};
use cron_parser::parse;
use ed25519_dalek::SecretKey as SignatureStaticKey;
use futures::Future;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_message::shinkai_message_schemas::{JobCreationInfo, JobMessage},
    shinkai_utils::{
        job_scope::JobScope,
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        signatures::clone_signature_secret_key,
    },
};
use std::str::FromStr;
use tokio::sync::{Mutex, Semaphore};

use crate::{
    agent::{error::AgentError, job_manager::JobManager, queue::job_queue_manager::JobQueueManager},
    cron_tasks::web_scrapper::WebScraper,
    db::{db_cron_task::CronTask, ShinkaiDB},
};

use super::youtube_checker::YoutubeChecker;

pub struct CronManager {
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub node_profile_name: ShinkaiName,
    pub identity_secret_key: SignatureStaticKey,
    pub job_manager: Arc<Mutex<JobManager>>,
    pub cron_processing_task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
pub enum CronManagerError {
    SomeError(String),
    JobDequeueFailed(String),
    JobCreationError(String),
}

impl From<AgentError> for CronManagerError {
    fn from(error: AgentError) -> Self {
        CronManagerError::JobCreationError(error.to_string())
    }
}

const NUM_THREADS: usize = 2;
const CRON_INTERVAL_TIME: u64 = 60 * 1;

impl CronManager {
    pub async fn new(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SignatureStaticKey,
        node_profile_name: ShinkaiName,
        job_manager: Arc<Mutex<JobManager>>,
    ) -> Self {
        let cron_processing_task = CronManager::process_job_queue(
            db.clone(),
            node_profile_name.clone(),
            clone_signature_secret_key(&identity_secret_key),
            CRON_INTERVAL_TIME,
            job_manager.clone(),
            |job, db, identity_sk, job_manager, node_profile_name| {
                Box::pin(CronManager::process_job_message_queued(
                    job,
                    db,
                    identity_sk,
                    job_manager,
                    node_profile_name,
                ))
            },
        );

        Self {
            db,
            identity_secret_key,
            node_profile_name,
            job_manager,
            cron_processing_task: Some(cron_processing_task),
        }
    }

    pub fn process_job_queue(
        db: Arc<Mutex<ShinkaiDB>>,
        node_profile_name: ShinkaiName,
        identity_sk: SignatureStaticKey,
        cron_time_interval: u64,
        job_manager: Arc<Mutex<JobManager>>,
        job_processing_fn: impl Fn(
                CronTask,
                Arc<Mutex<ShinkaiDB>>,
                SignatureStaticKey,
                Arc<Mutex<JobManager>>,
                ShinkaiName,
            ) -> Pin<Box<dyn Future<Output = Result<bool, CronManagerError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let job_processing_fn = Arc::new(job_processing_fn);

        tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::CronExecution,
                ShinkaiLogLevel::Info,
                "Starting cron job queue processing loop",
            );

            let is_testing = std::env::var("IS_TESTING").unwrap_or_else(|_| String::from("false")) != "false";

            loop {
                let jobs_to_process: HashMap<String, CronTask> = {
                    let mut db_lock = db.lock().await;

                    db_lock
                        .get_all_cron_tasks_from_all_profiles()
                        .unwrap_or(HashMap::new())
                };
                eprintln!("Cron Jobs to process: {:?}", jobs_to_process);
                let mut handles = Vec::new();

                // Spawn tasks based on filtered job IDs
                for (_, cron_task) in jobs_to_process {
                    if !is_testing && !Self::should_execute_cron_task(&cron_task, cron_time_interval) {
                        eprintln!("Cron Job not ready to be executed: {:?}", cron_task);
                        continue;
                    }

                    let db_clone = db.clone();
                    let identity_sk_clone = clone_signature_secret_key(&identity_sk);
                    let job_manager_clone = job_manager.clone();
                    let node_profile_name_clone = node_profile_name.clone();
                    let job_processing_fn_clone = Arc::clone(&job_processing_fn);

                    let handle = tokio::spawn(async move {
                        let result = job_processing_fn_clone(
                            cron_task,
                            db_clone,
                            identity_sk_clone,
                            job_manager_clone,
                            node_profile_name_clone,
                        )
                        .await;
                        match result {
                            Ok(_) => {
                                shinkai_log(
                                    ShinkaiLogOption::JobExecution,
                                    ShinkaiLogLevel::Debug,
                                    "Cron Job processed successfully",
                                );
                            }
                            Err(e) => {
                                shinkai_log(
                                    ShinkaiLogOption::CronExecution,
                                    ShinkaiLogLevel::Error,
                                    format!("Cron Job processing failed: {:?}", e).as_str(),
                                );
                            }
                        }
                    });

                    handles.push(handle);
                }
                futures::future::join_all(handles).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(cron_time_interval)).await;
            }
        })
    }

    pub async fn process_job_message_queued(
        cron_job: CronTask,
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SignatureStaticKey,
        job_manager: Arc<Mutex<JobManager>>,
        node_profile_name: ShinkaiName,
    ) -> Result<bool, CronManagerError> {
        // TODO: it needs to create a new job per cron task
        // Should it download the stuff and then connect it to the job? (very likely)
        eprintln!("Processing job: {:?}", cron_job);

        // Create a new instance of the WebScraper
        let scraper = WebScraper {
            task: cron_job.clone(),
            // TODO: Move to ENV
            api_url: "https://internal.shinkai.com/x-unstructured-api/general/v0/general".to_string(),
        };

        // Call the download_and_parse method of the WebScraper
        let mut structured_results = Vec::new();
        // let mut unfiltered_results = Vec::new();
        match scraper.download_and_parse().await {
            Ok(content) => {
                shinkai_log(
                    ShinkaiLogOption::JobExecution,
                    ShinkaiLogLevel::Debug,
                    "Web scraping completed successfully",
                );
                structured_results.push(content.clone());

                // If crawl_links is true, scan for all the links in content and download_and_parse them as well
                if cron_job.crawl_links {
                    let links = WebScraper::extract_links(&content.unfiltered);
                    eprintln!("Links #: {:?}", links.len());
                    for link in links.into_iter().take(2) {
                        // TODO: remove .into_iter().take(2)
                        let mut scraper_for_link = scraper.clone();
                        scraper_for_link.task.url = link.clone();
                        match scraper_for_link.download_and_parse().await {
                            Ok(content) => {
                                structured_results.push(content);
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
                eprintln!("Web scraping failed: {:?}", e);
                return Err(CronManagerError::SomeError(format!("Web scraping failed: {:?}", e)));
            }
        }

        eprintln!("Results #: {:?}", structured_results.len());
        eprintln!("Creating job");
        let job_creation = JobCreationInfo {
            scope: JobScope::new_default(),
        };

        eprintln!("Job Creation: {:?}", job_creation);
        eprintln!("Cron job: {:?}", cron_job);

        // Create Job
        let job_id = job_manager
            .lock()
            .await
            .process_job_creation(job_creation, &cron_job.agent_id)
            .await?;

        eprintln!("Results: {:?} \n\n\n\n\n", structured_results);
        for result in structured_results {
            // Concatenate the result with cron_job.prompt
            let content = format!("{} (try to extract their links from the content) --- website content --- {} --- end website content ---", cron_job.prompt, result.structured);
            eprintln!("Content: {:?}", content);
            // panic!("Job ID: {:?}", job_id);

            // Add Message to Job Queue
            let job_message = JobMessage {
                job_id: job_id.clone(),
                content,
                files_inbox: "".to_string(),
            };

            job_manager
                .lock()
                .await
                .add_job_message_to_job_queue(&job_message, &node_profile_name)
                .await?;
        }

        Ok(true)
    }

    pub fn should_execute_cron_task(cron_task: &CronTask, cron_time_interval: u64) -> bool {
        // Calculate the current time and the end of the interval
        let now = Utc::now();
        let now_rounded = now.with_second(0).unwrap().with_nanosecond(0).unwrap();
        let end_of_interval = now_rounded + chrono::Duration::seconds(cron_time_interval as i64);

        // Parse the cron expression
        let next_execution_time = match cron_parser::parse(&cron_task.cron, &now_rounded) {
            Ok(datetime) => datetime,
            Err(_) => {
                eprintln!("Invalid cron expression: {}", &cron_task.cron);
                shinkai_log(
                    ShinkaiLogOption::CronExecution,
                    ShinkaiLogLevel::Error,
                    format!("Invalid cron expression: {}", &cron_task.cron).as_str(),
                );
                return false;
            }
        };

        // Check if the next execution time falls within the range of now and now + cron_time_interval
        next_execution_time >= now && next_execution_time <= end_of_interval
    }

    pub fn is_valid_cron_expression(cron_expression: &str) -> bool {
        cron_parser::parse(cron_expression, &Utc::now()).is_ok()
    }

    // TODO: rename this or refactor it to a manager
    pub async fn add_cron_task(
        &self,
        profile: String,
        task_id: String,
        cron: String,
        prompt: String,
        subprompt: String,
        url: String,
        crawl_links: bool,
        agent_id: String,
    ) -> tokio::task::JoinHandle<Result<(), CronManagerError>> {
        let db = self.db.clone();
        // Note: needed to avoid a deadlock
        tokio::spawn(async move {
            let mut db_lock = db.lock().await;
            db_lock
                .add_cron_task(profile, task_id, cron, prompt, subprompt, url, crawl_links, agent_id)
                .map_err(|e| CronManagerError::SomeError(e.to_string()))
        })
    }
}
