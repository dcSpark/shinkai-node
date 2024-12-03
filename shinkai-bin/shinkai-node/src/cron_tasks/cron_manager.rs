use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, Weak},
};

use chrono::{Timelike, Utc};
use ed25519_dalek::SigningKey;
use futures::Future;
use shinkai_db::{
    db::{db_errors::ShinkaiDBError, ShinkaiDB},
    schemas::ws_types::WSUpdateHandler,
};
use shinkai_message_primitives::{
    schemas::{
        crontab::{CronTask, CronTaskAction},
        inbox_name::InboxNameError,
        shinkai_name::ShinkaiName,
    },
    shinkai_message::shinkai_message_schemas::JobMessage,
    shinkai_utils::{
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        signatures::clone_signature_secret_key,
    },
};
use shinkai_sqlite::{SqliteManager, SqliteManagerError};
use tokio::sync::{Mutex, RwLock};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{
    llm_provider::{error::LLMProviderError, job_manager::JobManager},
    managers::IdentityManager,
    network::{node_error::NodeError, Node},
};

#[derive(Debug)]
pub enum CronManagerError {
    SomeError(String),
    JobCreationError(String),
    StrError(String),
    DBError(SqliteManagerError),
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

impl From<SqliteManagerError> for CronManagerError {
    fn from(error: SqliteManagerError) -> Self {
        CronManagerError::DBError(error)
    }
}

impl From<InboxNameError> for CronManagerError {
    fn from(error: InboxNameError) -> Self {
        CronManagerError::InboxError(error)
    }
}

impl From<NodeError> for CronManagerError {
    fn from(error: NodeError) -> Self {
        CronManagerError::SomeError(error.to_string())
    }
}

impl From<SqliteManagerError> for CronManagerError {
    fn from(error: SqliteManagerError) -> Self {
        CronManagerError::SomeError(error.to_string())
    }
}

pub struct CronManager {
    pub db: Weak<ShinkaiDB>,
    pub sqlite_manager: Weak<RwLock<SqliteManager>>,
    pub node_profile_name: ShinkaiName,
    pub identity_secret_key: SigningKey,
    pub job_manager: Arc<Mutex<JobManager>>,
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    pub node_encryption_sk: EncryptionStaticKey,
    pub node_encryption_pk: EncryptionPublicKey,
    pub cron_processing_task: Option<tokio::task::JoinHandle<()>>,
    pub ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
}

impl CronManager {
    pub async fn new(
        db: Weak<ShinkaiDB>,
        sqlite_manager: Weak<RwLock<SqliteManager>>,
        identity_secret_key: SigningKey,
        node_name: ShinkaiName,
        job_manager: Arc<Mutex<JobManager>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Self {
        let cron_processing_task = CronManager::process_job_queue(
            db.clone(),
            sqlite_manager.clone(),
            node_name.clone(),
            clone_signature_secret_key(&identity_secret_key),
            Self::cron_interval_time(),
            job_manager.clone(),
            identity_manager.clone(),
            node_encryption_sk.clone(),
            node_encryption_pk.clone(),
            ws_manager.clone(),
            |job,
             db,
             sqlite_manager,
             identity_sk,
             job_manager,
             identity_manager,
             node_encryption_sk,
             node_encryption_pk,
             node_name,
             profile,
             ws_manager| {
                Box::pin(CronManager::process_job_message_queued(
                    job,
                    db,
                    sqlite_manager,
                    identity_sk,
                    job_manager,
                    identity_manager,
                    node_encryption_sk,
                    node_encryption_pk,
                    node_name,
                    profile,
                    ws_manager.clone(),
                ))
            },
        );

        Self {
            db,
            sqlite_manager,
            identity_secret_key,
            node_profile_name: node_name,
            job_manager,
            identity_manager,
            node_encryption_sk,
            node_encryption_pk,
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
        sqlite_manager: Weak<RwLock<SqliteManager>>,
        node_profile_name: ShinkaiName,
        identity_sk: SigningKey,
        cron_time_interval: u64,
        job_manager: Arc<Mutex<JobManager>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        job_processing_fn: impl Fn(
                CronTask,
                Weak<ShinkaiDB>,
                Weak<RwLock<SqliteManager>>,
                SigningKey,
                Arc<Mutex<JobManager>>,
                Arc<Mutex<IdentityManager>>,
                EncryptionStaticKey,
                EncryptionPublicKey,
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
                    let sqlite_manager_arc = sqlite_manager.upgrade();
                    if sqlite_manager_arc.is_none() {
                        shinkai_log(
                            ShinkaiLogOption::CronExecution,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade Weak reference to Arc for SqliteManager access. Exiting job queue processing loop.",
                        );
                        return;
                    }
                    let sqlite_manager_arc = sqlite_manager_arc.unwrap();
                    let sqlite_manager = sqlite_manager_arc.read().await;
                    sqlite_manager
                        .get_all_cron_tasks()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|task| (task.created_at.clone(), vec![(task.task_id.to_string(), task)]))
                        .collect()
                };
                if !jobs_to_process.is_empty() {
                    shinkai_log(
                        ShinkaiLogOption::CronExecution,
                        ShinkaiLogLevel::Debug,
                        format!("Cron Jobs retrieved from SqliteManager: {:?}", jobs_to_process.len()).as_str(),
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
                        let sqlite_manager_clone = sqlite_manager.clone();
                        let identity_sk_clone = clone_signature_secret_key(&identity_sk);
                        let job_manager_clone = job_manager.clone();
                        let identity_manager_clone = identity_manager.clone();
                        let node_encryption_sk_clone = node_encryption_sk.clone();
                        let node_encryption_pk_clone = node_encryption_pk.clone();
                        let node_profile_name_clone = node_profile_name.clone();
                        let job_processing_fn_clone = Arc::clone(&job_processing_fn);
                        let profile_clone = profile.clone();
                        let ws_manager = ws_manager.clone();

                        let handle = tokio::spawn(async move {
                            let result = job_processing_fn_clone(
                                cron_task,
                                db_clone,
                                sqlite_manager_clone,
                                identity_sk_clone,
                                job_manager_clone,
                                identity_manager_clone,
                                node_encryption_sk_clone,
                                node_encryption_pk_clone,
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
        sqlite_manager: Weak<RwLock<SqliteManager>>,
        identity_secret_key: SigningKey,
        job_manager: Arc<Mutex<JobManager>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        node_encryption_sk: EncryptionStaticKey,
        node_encryption_pk: EncryptionPublicKey,
        node_profile_name: ShinkaiName,
        profile: String,
        _ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<bool, CronManagerError> {
        shinkai_log(
            ShinkaiLogOption::CronExecution,
            ShinkaiLogLevel::Debug,
            format!("Processing job: {:?}", cron_job).as_str(),
        );
        let sqlite_manager = sqlite_manager.upgrade().unwrap();

        // Update the last executed time
        {
            let current_time = Utc::now().to_rfc3339();
            let sqlite_manager = sqlite_manager.read().await;
            sqlite_manager.update_cron_task_last_executed(cron_job.task_id.into(), &current_time)?;
        }

        let shinkai_profile = ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), profile)?;

        match cron_job.action {
            CronTaskAction::CreateJobWithConfigAndMessage {
                config,
                message,
                job_creation_info,
            } => {
                let job_id = job_manager
                    .lock()
                    .await
                    .process_job_creation(job_creation_info, &shinkai_profile, &cron_job.task_id.to_string())
                    .await?;

                // Update the job configuration
                let db_arc = db.upgrade().unwrap();
                db_arc.update_job_config(&job_id, config)?;

                // Use send_job_message_with_bearer instead of ShinkaiMessageBuilder
                Self::send_job_message_with_bearer(
                    db_arc.clone(),
                    sqlite_manager.clone(),
                    node_profile_name.clone(),
                    identity_manager.clone(),
                    job_manager.clone(),
                    message.clone(), // Use the message directly
                    node_encryption_sk.clone(),
                    node_encryption_pk.clone(),
                    identity_secret_key.clone(),
                    cron_job.task_id.into(),
                )
                .await?;
            }
            CronTaskAction::SendMessageToJob { job_id, message } => {
                let db_arc = db.upgrade().unwrap();

                // Use send_job_message_with_bearer instead of ShinkaiMessageBuilder
                Self::send_job_message_with_bearer(
                    db_arc.clone(),
                    sqlite_manager.clone(),
                    node_profile_name.clone(),
                    identity_manager.clone(),
                    job_manager.clone(),
                    message.clone(), // Use the message directly
                    node_encryption_sk.clone(),
                    node_encryption_pk.clone(),
                    identity_secret_key.clone(),
                    cron_job.task_id.into(),
                )
                .await?;
            }
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

    async fn send_job_message_with_bearer(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<RwLock<SqliteManager>>,
        node_name_clone: ShinkaiName,
        identity_manager_clone: Arc<Mutex<IdentityManager>>,
        job_manager_clone: Arc<Mutex<JobManager>>,
        job_message_clone: JobMessage,
        encryption_secret_key_clone: EncryptionStaticKey,
        encryption_public_key_clone: EncryptionPublicKey,
        signing_secret_key_clone: SigningKey,
        task_id: i64,
    ) -> Result<(), NodeError> {
        // Retrieve the bearer token from the database
        let bearer = match db.read_api_v2_key() {
            Ok(Some(token)) => token,
            Ok(None) => {
                Self::log_error_to_sqlite(&sqlite_manager, task_id, "Bearer token not found").await;
                return Ok(());
            }
            Err(err) => {
                Self::log_error_to_sqlite(
                    &sqlite_manager,
                    task_id,
                    &format!("Failed to retrieve bearer token: {}", err),
                )
                .await;
                return Ok(());
            }
        };

        // Create a local channel for the response
        let (res_tx, res_rx) = async_channel::bounded(1);

        // Send the job message
        if let Err(err) = Node::v2_job_message(
            db,
            node_name_clone,
            identity_manager_clone,
            job_manager_clone,
            bearer,
            job_message_clone,
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
            res_tx,
        )
        .await
        {
            Self::log_error_to_sqlite(
                &sqlite_manager,
                task_id,
                &format!("Failed to send job message: {}", err),
            )
            .await;
        }

        // Handle the response
        if let Err(err) = res_rx.recv().await {
            Self::log_error_to_sqlite(
                &sqlite_manager,
                task_id,
                &format!("Failed to receive response: {}", err),
            )
            .await;
        }

        Ok(())
    }

    async fn log_error_to_sqlite(sqlite_manager: &Arc<RwLock<SqliteManager>>, task_id: i64, error_message: &str) {
        let execution_time = chrono::Utc::now().to_rfc3339();
        let sqlite_manager = sqlite_manager.write().await;
        if let Err(err) = sqlite_manager.add_cron_task_execution(task_id, &execution_time, false, Some(error_message)) {
            eprintln!("Failed to log error to SQLite: {}", err);
        }
    }
}
