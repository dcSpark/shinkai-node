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

use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, Weak},
};

use chrono::{Timelike, Utc};
use ed25519_dalek::SigningKey;
use futures::Future;
use shinkai_db::db::{db_errors::ShinkaiDBError, ShinkaiDB};
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::{
    schemas::{
        cron_task::CronTask,
        inbox_name::{InboxName, InboxNameError},
        inbox_permission::InboxPermission,
        shinkai_name::ShinkaiName,
    },
    shinkai_message::shinkai_message_schemas::{JobCreationInfo, JobMessage},
    shinkai_utils::{
        job_scope::JobScope,
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::clone_signature_secret_key,
    },
};
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use tokio::sync::Mutex;

use crate::llm_provider::{error::LLMProviderError, job_manager::JobManager};

pub struct CronManager {
    pub db: Weak<ShinkaiDB>,
    pub node_profile_name: ShinkaiName,
    pub identity_secret_key: SigningKey,
    pub job_manager: Arc<Mutex<JobManager>>,
    pub cron_processing_task: Option<tokio::task::JoinHandle<()>>,
    pub ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
}

#[derive(Debug)]
pub enum CronManagerError {
    SomeError(String),
    JobCreationError(String),
    StrError(String),
    DBError(ShinkaiDBError),
    InboxError(InboxNameError),
}

impl From<LLMProviderError> for CronManagerError {
    fn from(error: LLMProviderError) -> Self {
        CronManagerError::JobCreationError(error.to_string())
    }
}

impl From<&str> for CronManagerError {
    fn from(error: &str) -> Self {
        CronManagerError::StrError(error.to_string())
    }
}

impl From<ShinkaiDBError> for CronManagerError {
    fn from(error: ShinkaiDBError) -> Self {
        CronManagerError::DBError(error)
    }
}

impl From<InboxNameError> for CronManagerError {
    fn from(error: InboxNameError) -> Self {
        CronManagerError::InboxError(error)
    }
}

impl CronManager {
    pub async fn new(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        identity_secret_key: SigningKey,
        node_name: ShinkaiName,
        job_manager: Arc<Mutex<JobManager>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Self {
        let cron_processing_task = CronManager::process_job_queue(
            db.clone(),
            vector_fs.clone(),
            node_name.clone(),
            clone_signature_secret_key(&identity_secret_key),
            Self::cron_interval_time(),
            job_manager.clone(),
            ws_manager.clone(),
            |job, db, vector_fs, identity_sk, job_manager, node_name, profile, ws_manager| {
                Box::pin(CronManager::process_job_message_queued(
                    job,
                    db,
                    vector_fs,
                    identity_sk,
                    job_manager,
                    node_name,
                    profile,
                    ws_manager.clone(),
                ))
            },
        );

        Self {
            db,
            identity_secret_key,
            node_profile_name: node_name,
            job_manager,
            cron_processing_task: Some(cron_processing_task),
            ws_manager,
        }
    }

    fn cron_interval_time() -> u64 {
        std::env::var("CRON_INTERVAL_TIME")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process_job_queue(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_profile_name: ShinkaiName,
        identity_sk: SigningKey,
        cron_time_interval: u64,
        job_manager: Arc<Mutex<JobManager>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        job_processing_fn: impl Fn(
                CronTask,
                Weak<ShinkaiDB>,
                Weak<VectorFS>,
                SigningKey,
                Arc<Mutex<JobManager>>,
                ShinkaiName,
                String,
                Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
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
                let jobs_to_process: HashMap<String, Vec<(String, CronTask)>> = {
                    let db_arc = db.upgrade();
                    if db_arc.is_none() {
                        shinkai_log(
                            ShinkaiLogOption::CronExecution,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade Weak reference to Arc for DB access. Exiting job queue processing loop.",
                        );
                        return;
                    }
                    let db_arc = db_arc.unwrap();
                    db_arc
                        .get_all_cron_tasks_from_all_profiles(node_profile_name.clone())
                        .unwrap_or_default()
                };
                if !jobs_to_process.is_empty() {
                    shinkai_log(
                        ShinkaiLogOption::CronExecution,
                        ShinkaiLogLevel::Debug,
                        format!("Cron Jobs retrieved from DB: {:?}", jobs_to_process.len()).as_str(),
                    );
                }
                let mut handles = Vec::new();

                // Spawn tasks based on filtered job IDs
                for (profile, tasks) in jobs_to_process {
                    for (_, cron_task) in tasks {
                        if !is_testing && !Self::should_execute_cron_task(&cron_task, cron_time_interval) {
                            shinkai_log(
                                ShinkaiLogOption::CronExecution,
                                ShinkaiLogLevel::Debug,
                                format!("Cron Job not ready to be executed: {:?}", cron_task).as_str(),
                            );
                            continue;
                        }

                        let db_clone = db.clone();
                        let vector_fs_clone = vector_fs.clone();
                        let identity_sk_clone = clone_signature_secret_key(&identity_sk);
                        let job_manager_clone = job_manager.clone();
                        let node_profile_name_clone = node_profile_name.clone();
                        let job_processing_fn_clone = Arc::clone(&job_processing_fn);
                        let profile_clone = profile.clone();
                        let ws_manager = ws_manager.clone();

                        let handle = tokio::spawn(async move {
                            let result = job_processing_fn_clone(
                                cron_task,
                                db_clone,
                                vector_fs_clone,
                                identity_sk_clone,
                                job_manager_clone,
                                node_profile_name_clone,
                                profile_clone,
                                ws_manager,
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
                }
                futures::future::join_all(handles).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(cron_time_interval)).await;
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_job_message_queued(
        cron_job: CronTask,
        db: Weak<ShinkaiDB>,
        _vector_fs: Weak<VectorFS>,
        identity_secret_key: SigningKey,
        job_manager: Arc<Mutex<JobManager>>,
        node_profile_name: ShinkaiName,
        profile: String,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<bool, CronManagerError> {
        shinkai_log(
            ShinkaiLogOption::CronExecution,
            ShinkaiLogLevel::Debug,
            format!("Processing job: {:?}", cron_job).as_str(),
        );

        let shinkai_profile = ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), profile)?;

        let job_creation = JobCreationInfo {
            scope: JobScope::new_default(),
            is_hidden: Some(false),
            associated_ui: None,
        };

        // Create Job
        let job_id = job_manager
            .lock()
            .await
            .process_job_creation(job_creation, &shinkai_profile, &cron_job.llm_provider_id)
            .await?;

        let message_hash_id: String;
        {
            // Get the inbox name
            let inbox_name = InboxName::get_job_inbox_name_from_params(job_id.clone())?;

            // Add permission
            let db_arc = db.upgrade().unwrap();
            db_arc.add_permission_with_profile(
                inbox_name.to_string().as_str(),
                shinkai_profile.clone(),
                InboxPermission::Admin,
            )?;

            let cron_request_message = format!(
                "My scheduled job \"{}\" created on \"{}\" is ready to be executed",
                cron_job.prompt, cron_job.created_at
            );
            let shinkai_message = ShinkaiMessageBuilder::job_message_from_llm_provider(
                job_id.to_string(),
                cron_request_message.to_string(),
                "".to_string(),
                None,
                identity_secret_key,
                node_profile_name.node_name.clone(),
                node_profile_name.node_name.clone(),
            )
            .unwrap();

            message_hash_id = shinkai_message.calculate_message_hash_for_pagination();
            db_arc
                .add_message_to_job_inbox(&job_id.clone(), &shinkai_message, None, ws_manager)
                .await?;
            db_arc.update_smart_inbox_name(inbox_name.to_string().as_str(), cron_job.prompt.as_str())?;
        }

        // Add Message to Job Queue
        let job_message = JobMessage {
            job_id: job_id.clone(),
            content: "".to_string(),
            files_inbox: "".to_string(), // TODO placeholder
            parent: None,
            workflow_code: None,
            workflow_name: None,
            callback: None,
            sheet_job_data: None,
            metadata: None,
        };

        job_manager
            .lock()
            .await
            .add_job_message_to_job_queue(&job_message, &node_profile_name, Some(message_hash_id.clone()))
            .await?;

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
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub async fn add_cron_task(
        &self,
        profile: ShinkaiName,
        task_id: String,
        cron: String,
        prompt: String,
        subprompt: String,
        url: String,
        crawl_links: bool,
        llm_provider_id: String,
    ) -> tokio::task::JoinHandle<Result<(), CronManagerError>> {
        let db = self.db.clone();
        // Note: needed to avoid a deadlock
        tokio::spawn(async move {
            let db_arc = db.upgrade().unwrap();
            db_arc
                .add_cron_task(
                    profile,
                    task_id,
                    cron,
                    prompt,
                    subprompt,
                    url,
                    crawl_links,
                    llm_provider_id,
                )
                .map_err(|e| CronManagerError::SomeError(e.to_string()))
        })
    }
}
