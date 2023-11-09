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

use ed25519_dalek::SecretKey as SignatureStaticKey;
use futures::Future;
use shinkai_message_primitives::{
    schemas::shinkai_name::ShinkaiName,
    shinkai_utils::{
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        signatures::clone_signature_secret_key,
    },
};
use tokio::sync::{Mutex, Semaphore};

use crate::{
    agent::queue::job_queue_manager::JobQueueManager,
    db::{db_cron_task::CronTask, ShinkaiDB},
};

use super::youtube_checker::YoutubeChecker;

pub struct CronManager {
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub node_profile_name: ShinkaiName,
    pub identity_secret_key: SignatureStaticKey,
    pub cron_processing_task: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Debug)]
pub enum CronManagerError {
    SomeError(String),
    JobDequeueFailed(String),
}

const NUM_THREADS: usize = 4;
const CRON_INTERVAL_TIME: u64 = 60 * 10;

impl CronManager {
    pub async fn new(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SignatureStaticKey,
        node_profile_name: ShinkaiName,
    ) -> Self {
        let cron_processing_task = CronManager::process_job_queue(
            db.clone(),
            node_profile_name.clone(),
            clone_signature_secret_key(&identity_secret_key),
            CRON_INTERVAL_TIME,
            |job, db, identity_sk| Box::pin(CronManager::process_job_message_queued(job, db, identity_sk)),
        );

        Self {
            db,
            identity_secret_key,
            node_profile_name,
            cron_processing_task: Some(cron_processing_task),
        }
    }

    pub fn process_job_queue(
        db: Arc<Mutex<ShinkaiDB>>,
        node_profile_name: ShinkaiName,
        identity_sk: SignatureStaticKey,
        cron_time_interval: u64,
        job_processing_fn: impl Fn(
                CronTask,
                Arc<Mutex<ShinkaiDB>>,
                SignatureStaticKey,
            ) -> Pin<Box<dyn Future<Output = Result<bool, CronManagerError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let job_processing_fn = Arc::new(job_processing_fn);

        tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                "Starting job queue processing loop",
            );

            loop {
                let jobs_to_process: HashMap<String, CronTask> = {
                    let mut db_lock = db.lock().await;
                    db_lock
                        .get_all_cron_tasks(node_profile_name.clone().get_profile_name().unwrap())
                        .unwrap_or(HashMap::new())
                };
                eprintln!("jobs_to_process: {:?}", jobs_to_process);

                let mut handles = Vec::new();

                // Spawn tasks based on filtered job IDs
                for (_, cron_task) in jobs_to_process {
                    let db_clone = db.clone();
                    let identity_sk_clone = clone_signature_secret_key(&identity_sk);
                    let job_processing_fn_clone = Arc::clone(&job_processing_fn);

                    let handle = tokio::spawn(async move {
                        let result = job_processing_fn_clone(cron_task, db_clone, identity_sk_clone).await;
                        match result {
                            Ok(_) => {
                                shinkai_log(
                                    ShinkaiLogOption::JobExecution,
                                    ShinkaiLogLevel::Debug,
                                    "Job processed successfully",
                                );
                            }
                            Err(e) => {
                                shinkai_log(
                                    ShinkaiLogOption::CronExecution,
                                    ShinkaiLogLevel::Error,
                                    format!("Job processing failed: {:?}", e).as_str(),
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

    // #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    // pub struct CronTask {
    //     pub task_id: String,
    //     pub cron: String,
    //     pub prompt: String,
    //     pub url: String,
    // }

    pub async fn process_job_message_queued(
        cron_job: CronTask,
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SignatureStaticKey,
    ) -> Result<bool, CronManagerError> {
        // TODO: it needs to create a new job per cron task
        // Should it download the stuff and then connect it to the job? (very likely)
        eprintln!("Processing job: {:?}", cron_job);
        // let youtube_checker = YoutubeChecker::new();
        // youtube_checker.check_new_videos(&cron_job.url).await;

        Ok(true)
    }
}
