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

use std::{sync::Arc, pin::Pin, collections::HashSet, mem};

use futures::Future;
use shinkai_message_primitives::{schemas::shinkai_name::ShinkaiName, shinkai_utils::{signatures::clone_signature_secret_key, shinkai_logging::{shinkai_log, ShinkaiLogOption, ShinkaiLogLevel}}};
use tokio::sync::{Mutex, Semaphore};
use ed25519_dalek::SecretKey as SignatureStaticKey;

use crate::{db::{ShinkaiDB, db_cron_task::CronTask}, agent::queue::job_queue_manager::JobQueueManager};

pub struct CronManager {
    pub db: Arc<Mutex<ShinkaiDB>>,
    pub node_profile_name: ShinkaiName,
    pub identity_secret_key: SignatureStaticKey,
    pub cron_processing_task: Option<tokio::task::JoinHandle<()>>,
    // To avoid hitting the DB every minute, we will have a queue of jobs to process
    pub job_queue_manager: Arc<Mutex<JobQueueManager<CronTask>>>,
}

#[derive(Debug)]
pub enum CronManagerError {
    SomeError(String),
    JobDequeueFailed(String),
}

const NUM_THREADS: usize = 1;

impl CronManager {
    pub async fn new(
        db: Arc<Mutex<ShinkaiDB>>,
        identity_secret_key: SignatureStaticKey,
        node_profile_name: ShinkaiName,
    ) -> Self {

        let job_queue = JobQueueManager::<CronTask>::new(db.clone()).await.unwrap();
        let job_queue_manager = Arc::new(Mutex::new(job_queue));



        Self {
            db,
            identity_secret_key,
            node_profile_name,
            cron_processing_task: None,
            job_queue_manager,
        }
    }

    // // Note(Nico): this code is pretty much the same as the one in job_manager.rs
    // // One day I'll refactor this (trademark logo here)
    // pub async fn process_job_queue(
    //     job_queue_manager: Arc<Mutex<JobQueueManager<CronTask>>>,
    //     db: Arc<Mutex<ShinkaiDB>>,
    //     max_parallel_jobs: usize,
    //     identity_sk: SignatureStaticKey,
    //     job_processing_fn: impl Fn(
    //             CronTask,
    //             Arc<Mutex<ShinkaiDB>>,
    //             SignatureStaticKey,
    //             // TODO: update Error to a custom one
    //         ) -> Pin<Box<dyn Future<Output = Result<String, CronManagerError>> + Send>>
    //         + Send
    //         + Sync
    //         + 'static,
    // ) -> tokio::task::JoinHandle<()> {
    //     let job_queue_manager = Arc::clone(&job_queue_manager);
    //     let mut receiver = job_queue_manager.lock().await.subscribe_to_all().await;
    //     let db_clone = db.clone();
    //     let identity_sk = clone_signature_secret_key(&identity_sk);
    //     let job_processing_fn = Arc::new(job_processing_fn);

    //     let processing_jobs = Arc::new(Mutex::new(HashSet::new()));
    //     let semaphore = Arc::new(Semaphore::new(max_parallel_jobs));

    //     return tokio::spawn(async move {
    //         shinkai_log(
    //             ShinkaiLogOption::JobExecution,
    //             ShinkaiLogLevel::Info,
    //             "Starting job queue processing loop",
    //         );

    //         let mut handles = Vec::new();
    //         loop {
    //             // Scope for acquiring and releasing the lock quickly
    //             let job_ids_to_process: Vec<String> = {
    //                 let mut processing_jobs_lock = processing_jobs.lock().await;
    //                 let job_queue_manager_lock = job_queue_manager.lock().await;
    //                 let all_jobs = job_queue_manager_lock
    //                     .get_all_elements_interleave()
    //                     .await
    //                     .unwrap_or(Vec::new());
    //                 std::mem::drop(job_queue_manager_lock);

    //                 let jobs = all_jobs
    //                     .into_iter()
    //                     .filter_map(|job| {
    //                         let job_id = job.job_message.job_id.clone().to_string();
    //                         if !processing_jobs_lock.contains(&job_id) {
    //                             processing_jobs_lock.insert(job_id.clone());
    //                             Some(job_id)
    //                         } else {
    //                             None
    //                         }
    //                     })
    //                     .collect();

    //                 std::mem::drop(processing_jobs_lock);
    //                 jobs
    //             };

    //             // Spawn tasks based on filtered job IDs
    //             for job_id in job_ids_to_process {
    //                 let job_queue_manager = Arc::clone(&job_queue_manager);
    //                 let processing_jobs = Arc::clone(&processing_jobs);
    //                 let semaphore = Arc::clone(&semaphore);
    //                 let db_clone_2 = db_clone.clone();
    //                 let identity_sk_clone = clone_signature_secret_key(&identity_sk);
    //                 let job_processing_fn = Arc::clone(&job_processing_fn);

    //                 let handle = tokio::spawn(async move {
    //                     let _permit = semaphore.acquire().await.unwrap();

    //                     // Acquire the lock, dequeue the job, and immediately release the lock
    //                     let job = {
    //                         let mut job_queue_manager = job_queue_manager.lock().await;
    //                         let job = job_queue_manager.peek(&job_id).await;
    //                         job
    //                     };

    //                     match job {
    //                         Ok(Some(job)) => {
    //                             // Acquire the lock, process the job, and immediately release the lock
    //                             let result = {
    //                                 let result = job_processing_fn(job, db_clone_2, identity_sk_clone).await;
    //                                 if let Ok(Some(_)) = job_queue_manager.lock().await.dequeue(&job_id.clone()).await {
    //                                     result
    //                                 } else {
    //                                     Err(CronManagerError::JobDequeueFailed(job_id.clone()))
    //                                 }
    //                             };

    //                             match result {
    //                                 Ok(_) => {
    //                                     shinkai_log(
    //                                         ShinkaiLogOption::JobExecution,
    //                                         ShinkaiLogLevel::Debug,
    //                                         "Job processed successfully",
    //                                     );
    //                                 } // handle success case
    //                                 Err(e) => {} // handle error case
    //                             }
    //                         }
    //                         Ok(None) => {}
    //                         Err(e) => {
    //                             // Log the error
    //                         }
    //                     }
    //                     drop(_permit);
    //                     processing_jobs.lock().await.remove(&job_id);
    //                 });
    //                 handles.push(handle);
    //             }

    //             let handles_to_join = mem::replace(&mut handles, Vec::new());
    //             futures::future::join_all(handles_to_join).await;
    //             handles.clear();

    //             // Receive new jobs
    //             if let Some(new_job) = receiver.recv().await {
    //                 shinkai_log(
    //                     ShinkaiLogOption::JobExecution,
    //                     ShinkaiLogLevel::Info,
    //                     format!("Received new cron job {:?}", new_job.job_message.job_id).as_str(),
    //                 );
    //             }
    //         }
    //     });
    // }
}