use std::{
    collections::HashMap, fmt, pin::Pin, sync::{Arc, Weak}
};

use chrono::{Local, Utc};
use ed25519_dalek::SigningKey;
use futures::Future;
use shinkai_message_primitives::{
    schemas::{
        crontab::{CronTask, CronTaskAction}, inbox_name::InboxNameError, shinkai_name::ShinkaiName, ws_types::WSUpdateHandler
    }, shinkai_message::shinkai_message_schemas::{AssociatedUI, JobMessage}, shinkai_utils::{
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption}, signatures::clone_signature_secret_key
    }
};
use shinkai_sqlite::{errors::SqliteManagerError, SqliteManager};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{
    llm_provider::{error::LLMProviderError, job_manager::JobManager}, managers::IdentityManager, network::{node_error::NodeError, Node}
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

impl fmt::Display for CronManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CronManagerError::SomeError(msg) => write!(f, "SomeError: {}", msg),
            CronManagerError::JobCreationError(msg) => write!(f, "JobCreationError: {}", msg),
            CronManagerError::StrError(msg) => write!(f, "StrError: {}", msg),
            CronManagerError::DBError(err) => write!(f, "DBError: {}", err),
            CronManagerError::InboxError(err) => write!(f, "InboxError: {}", err),
        }
    }
}

pub struct CronManager {
    pub db: Weak<SqliteManager>,
    pub node_profile_name: ShinkaiName,
    pub identity_secret_key: SigningKey,
    pub job_manager: Arc<Mutex<JobManager>>,
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    pub node_encryption_sk: EncryptionStaticKey,
    pub node_encryption_pk: EncryptionPublicKey,
    pub _cron_processing_task: Option<tokio::task::JoinHandle<()>>,
    pub ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
}

impl CronManager {
    pub async fn new(
        db: Weak<SqliteManager>,
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
            identity_secret_key,
            node_profile_name: node_name,
            job_manager,
            identity_manager,
            node_encryption_sk,
            node_encryption_pk,
            _cron_processing_task: Some(cron_processing_task),
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
        db: Weak<SqliteManager>,
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
                Weak<SqliteManager>,
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
                    let db_arc = db.upgrade();
                    if db_arc.is_none() {
                        shinkai_log(
                            ShinkaiLogOption::CronExecution,
                            ShinkaiLogLevel::Error,
                            "Failed to upgrade Weak reference to Arc for SqliteManager access. Exiting job queue processing loop.",
                        );
                        return;
                    }
                    let db_arc = db_arc.unwrap();
                    db_arc
                        .get_all_cron_tasks()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|task| !task.paused)
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
                for (_time_created, tasks) in jobs_to_process {
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
                        let identity_sk_clone = clone_signature_secret_key(&identity_sk);
                        let job_manager_clone = job_manager.clone();
                        let identity_manager_clone = identity_manager.clone();
                        let node_encryption_sk_clone = node_encryption_sk.clone();
                        let node_encryption_pk_clone = node_encryption_pk.clone();
                        let node_profile_name_clone = node_profile_name.clone();
                        let job_processing_fn_clone = Arc::clone(&job_processing_fn);
                        let profile_clone = node_profile_name.clone().get_profile_name_string().unwrap_or_default();
                        let ws_manager = ws_manager.clone();

                        let handle = tokio::spawn(async move {
                            let result = job_processing_fn_clone(
                                cron_task,
                                db_clone,
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
        db: Weak<SqliteManager>,
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
        let db = db.upgrade().unwrap();

        // Update the last executed time
        {
            let current_time = Utc::now().to_rfc3339();
            db.update_cron_task_last_executed(cron_job.task_id.into(), &current_time)?;
        }

        let shinkai_profile = ShinkaiName::from_node_and_profile_names(node_profile_name.to_string(), profile)?;

        match cron_job.action {
            CronTaskAction::CreateJobWithConfigAndMessage {
                config,
                message,
                job_creation_info,
                llm_provider,
            } => {
                // Clone job_creation_info and set is_hidden to true if not defined
                let mut job_creation_info_clone = job_creation_info.clone();
                if job_creation_info_clone.is_hidden.is_none() {
                    job_creation_info_clone.is_hidden = Some(true);
                }
                // Set the associated UI to Cron with the task ID
                job_creation_info_clone.associated_ui = Some(AssociatedUI::Cron(cron_job.task_id.to_string()));

                let job_id = job_manager
                    .lock()
                    .await
                    .process_job_creation(job_creation_info_clone, &shinkai_profile, &llm_provider)
                    .await?;

                // Update the job configuration
                db.update_job_config(&job_id, config)?;

                let mut job_message = message.clone();
                job_message.job_id = job_id;

                // Use send_job_message_with_bearer instead of ShinkaiMessageBuilder
                Self::send_job_message_with_bearer(
                    db.clone(),
                    node_profile_name.clone(),
                    identity_manager.clone(),
                    job_manager.clone(),
                    job_message,
                    node_encryption_sk.clone(),
                    node_encryption_pk.clone(),
                    identity_secret_key.clone(),
                    cron_job.task_id.into(),
                )
                .await?;
            }
            CronTaskAction::SendMessageToJob { job_id: _, message } => {
                // Use send_job_message_with_bearer instead of ShinkaiMessageBuilder
                Self::send_job_message_with_bearer(
                    db.clone(),
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
        let now = Local::now();
        let end_of_interval = now + chrono::Duration::seconds(cron_time_interval as i64);

        println!("Evaluating cron task:");
        println!("  Cron expression: {}", cron_task.cron);
        println!("  Current time: {}", now.to_rfc3339());
        println!("  End of interval: {}", end_of_interval.to_rfc3339());

        // Validate that the cron expression has exactly 5 fields
        if cron_task.cron.split_whitespace().count() != 5 {
            println!("  Invalid cron expression: wrong number of fields");
            return false;
        }

        let next_execution_time = match cron_parser::parse(&cron_task.cron, &now) {
            Ok(datetime) => {
                println!("  Next execution time: {}", datetime);
                datetime
            }
            Err(e) => {
                println!("  Failed to parse cron expression: {}", e);
                return false;
            }
        };

        let is_after_now = next_execution_time >= now;
        let is_before_end = next_execution_time <= end_of_interval;

        println!("  Conditions:");
        println!("    Is after current time? {}", is_after_now);
        println!("    Is before interval end? {}", is_before_end);

        let result = is_after_now && is_before_end;
        println!("  Final result: {}", result);

        result
    }

    async fn log_success_to_sqlite(db: &Arc<SqliteManager>, task_id: i64, job_id: Option<String>) {
        let execution_time = Local::now().to_rfc3339();
        let db = db;
        if let Err(err) = db.add_cron_task_execution(task_id, &execution_time, true, None, job_id) {
            eprintln!("Failed to log success to SQLite: {}", err);
        }
    }

    async fn send_job_message_with_bearer(
        db: Arc<SqliteManager>,
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
                Self::log_error_to_sqlite(&db, task_id, "Bearer token not found", None).await;
                return Ok(());
            }
            Err(err) => {
                Self::log_error_to_sqlite(&db, task_id, &format!("Failed to retrieve bearer token: {}", err), None)
                    .await;
                return Ok(());
            }
        };

        // Create a local channel for the response
        let (res_tx, res_rx) = async_channel::bounded(1);

        // Send the job message
        if let Err(err) = Node::v2_job_message(
            db.clone(),
            node_name_clone,
            identity_manager_clone,
            job_manager_clone,
            bearer,
            job_message_clone.clone(),
            encryption_secret_key_clone,
            encryption_public_key_clone,
            signing_secret_key_clone,
            None,
            res_tx,
        )
        .await
        {
            Self::log_error_to_sqlite(
                &db,
                task_id,
                &format!("Failed to send job message: {}", err),
                Some(job_message_clone.job_id),
            )
            .await;
            return Ok(());
        }

        // Handle the response only if sending was successful
        if let Err(err) = res_rx.recv().await {
            Self::log_error_to_sqlite(
                &db,
                task_id,
                &format!("Failed to receive response: {}", err),
                Some(job_message_clone.job_id),
            )
            .await;
        } else {
            // Log success if the response is received successfully
            Self::log_success_to_sqlite(&db, task_id, Some(job_message_clone.job_id)).await;
        }

        Ok(())
    }

    async fn log_error_to_sqlite(db: &Arc<SqliteManager>, task_id: i64, error_message: &str, job_id: Option<String>) {
        let execution_time = Local::now().to_rfc3339();
        let db = db;
        if let Err(err) = db.add_cron_task_execution(task_id, &execution_time, false, Some(error_message), job_id) {
            eprintln!("Failed to log error to SQLite: {}", err);
        }
    }

    pub async fn execute_cron_task_immediately(&self, cron_task: CronTask) -> Result<(), CronManagerError> {
        let db = self.db.clone();
        let identity_secret_key = self.identity_secret_key.clone();
        let job_manager = self.job_manager.clone();
        let identity_manager = self.identity_manager.clone();
        let node_encryption_sk = self.node_encryption_sk.clone();
        let node_encryption_pk = self.node_encryption_pk.clone();
        let node_profile_name = self.node_profile_name.clone();
        let profile = node_profile_name.get_profile_name_string().unwrap_or_default();
        let ws_manager = self.ws_manager.clone();

        CronManager::process_job_message_queued(
            cron_task,
            db,
            identity_secret_key,
            job_manager,
            identity_manager,
            node_encryption_sk,
            node_encryption_pk,
            node_profile_name,
            profile,
            ws_manager,
        )
        .await?;

        Ok(())
    }

    /// Returns a schedule of when each active cron task is approximately going
    /// to be executed next.
    ///
    /// We do this by:
    /// 1. Fetching all active (non-paused) cron tasks from the DB.
    /// 2. For each task, parse the cron expression to find the next scheduled
    ///    time after "now".
    /// 3. We then find the iteration interval in which this next scheduled time
    ///    falls. If a task is scheduled to run within the next
    ///    `cron_interval_time` seconds from some iteration, it will actually be
    ///    executed at the start of that iteration (due to how we batch checks).
    ///
    /// Note: This will only approximate when tasks are executed since they only
    /// run at discrete intervals.
    pub async fn get_cron_schedule(&self) -> Result<Vec<(CronTask, chrono::DateTime<Local>)>, CronManagerError> {
        let cron_time_interval = Self::cron_interval_time();
        let now = Local::now();

        let db = self
            .db
            .upgrade()
            .ok_or_else(|| CronManagerError::SomeError("DB reference lost".to_string()))?;

        // Fetch all cron tasks
        let tasks = db.get_all_cron_tasks().map_err(CronManagerError::DBError)?;

        let mut schedule = Vec::new();

        for task in tasks {
            if task.paused {
                continue;
            }

            // Parse the cron expression to find the next scheduled time
            let parsed_next_time = cron_parser::parse(&task.cron, &now);
            let next_time = match parsed_next_time {
                Ok(t) => t,
                Err(_) => {
                    // If we fail to parse or determine next time, skip
                    continue;
                }
            };

            // The cron might be scheduled right now or in the future. If the next_time is
            // in the future, we need to find the iteration window in which it falls.
            // Our iteration runs every `cron_time_interval` seconds.
            //
            // Essentially, on each iteration (starting at `now`), we check tasks that are
            // due within the next `cron_time_interval` seconds. If `next_time`
            // is within that window, the task will run at the start of that
            // iteration. If not, we check subsequent intervals until we find
            // the one that includes `next_time`.

            // We'll find the first iteration `k` where:
            // iteration_start = now + k * cron_time_interval
            // iteration_end   = iteration_start + cron_time_interval
            // and next_time is in [iteration_start, iteration_end]

            let mut iteration_count = 0;
            let approximate_run_time = loop {
                let iteration_start = now + chrono::Duration::seconds((iteration_count * cron_time_interval) as i64);
                let iteration_end = iteration_start + chrono::Duration::seconds(cron_time_interval as i64);

                if next_time >= iteration_start && next_time <= iteration_end {
                    // The task will run at iteration_start, since we only run tasks at iteration
                    // boundaries.
                    break iteration_start;
                }

                iteration_count += 1;
                // Safety break to avoid infinite loop in abnormal situations
                if iteration_count > 1_000 {
                    // If we can't find a match within some large number of iterations, skip.
                    break now;
                }
            };

            schedule.push((task, approximate_run_time.with_timezone(&Local)));
        }

        Ok(schedule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;
    use shinkai_message_primitives::schemas::crontab::CronTaskAction;

    fn create_test_cron_task(cron: &str) -> CronTask {
        let job_message = JobMessage {
            job_id: "job_id".to_string(),
            content: "message".to_string(),
            parent: None,
            sheet_job_data: None,
            callback: None,
            metadata: None,
            tool_key: None,
            fs_files_paths: vec![],
            job_filenames: vec![],
        };

        CronTask {
            name: "Test Task".to_string(),
            description: Some("Test Description".to_string()),
            task_id: 1,
            cron: cron.to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            last_modified: "2024-01-01T00:00:00Z".to_string(),
            action: CronTaskAction::SendMessageToJob {
                job_id: "test".to_string(),
                message: job_message,
            },
            paused: false,
        }
    }

    #[test]
    fn test_should_execute_every_minute() {
        let task = create_test_cron_task("* * * * *");
        assert!(CronManager::should_execute_cron_task(&task, 60));
    }

    #[test]
    fn test_should_execute_specific_minute() {
        let now = Local::now();
        let next_minute = (now.minute() + 1) % 60;
        let cron = format!("{} * * * *", next_minute);
        let task = create_test_cron_task(&cron);

        // Should execute within the next interval
        assert!(CronManager::should_execute_cron_task(&task, 120));
    }

    #[test]
    fn test_should_not_execute_past_time() {
        let now = Local::now();
        let past_minute = if now.minute() == 0 { 59 } else { now.minute() - 1 };
        let cron = format!("{} * * * *", past_minute);
        let task = create_test_cron_task(&cron);

        // Should not execute as the time has passed
        assert!(!CronManager::should_execute_cron_task(&task, 60));
    }

    #[test]
    fn test_invalid_cron_expression() {
        // Using an invalid cron expression with only 4 fields instead of 5
        let task = create_test_cron_task("* * * *");

        // The should_execute_cron_task function should return false for invalid
        // expressions as it already handles the error case in its
        // implementation
        assert!(!CronManager::should_execute_cron_task(&task, 60));

        // We can also test another invalid expression
        let task_invalid_values = create_test_cron_task("60 24 32 13 8");
        assert!(!CronManager::should_execute_cron_task(&task_invalid_values, 60));
    }

    #[test]
    fn test_should_execute_within_interval() {
        let now = Local::now();
        let next_minute = (now.minute() + 1) % 60;

        // Create a cron expression for the next minute, any hour/day/month
        let cron = format!("{} * * * *", next_minute);
        println!("Current time: {:?}", now);
        println!("Cron expression: {}", cron);

        let task = create_test_cron_task(&cron);

        // Get the next execution time for debugging
        let next_execution = cron_parser::parse(&cron, &now).unwrap();
        println!("Next execution time: {:?}", next_execution);
        println!("Interval end: {:?}", now + chrono::Duration::seconds(120));

        // Use a 2-minute interval to ensure we catch the next minute
        assert!(CronManager::should_execute_cron_task(&task, 120));
    }

    #[test]
    fn test_should_not_execute_outside_interval() {
        let now = Local::now();
        let future_minute = (now.minute() + 2) % 60;
        let cron = format!("{} * * * *", future_minute);
        let task = create_test_cron_task(&cron);

        // Should not execute as it's outside the interval
        assert!(!CronManager::should_execute_cron_task(&task, 60));
    }
}
