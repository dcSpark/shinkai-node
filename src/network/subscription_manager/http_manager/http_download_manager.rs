// We may be a localhost subscriber so we may not be able to receive updates
// This file needs to keep track of updates by pinging the node every X time (hardcoded for now)

// We should be able to download 2-3 files at the same time but hopefully they are mixed from different subscriptions
// similar to jobs in a job queue

// they should be save to a specific folder + base folder

use crate::db::Topic;
use crate::vector_fs::vector_fs::VectorFS;
use crate::{agent::queue::job_queue_manager::JobQueueManager, db::ShinkaiDB};
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::{shinkai_name::ShinkaiName, shinkai_subscription::SubscriptionId};
use std::cmp::Ordering;
use std::env;
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, Semaphore};

use super::http_upload_manager::FileLink;

const NUM_THREADS: usize = 2;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct HttpDownloadJob {
    pub subscription_id: SubscriptionId,
    pub info: FileLink,
    pub url: String,
    pub date_created: String,
}

impl PartialOrd for HttpDownloadJob {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HttpDownloadJob {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date_created.cmp(&other.date_created)
    }
}

pub struct HttpDownloadManager {
    pub db: Weak<ShinkaiDB>,
    pub vector_fs: Weak<VectorFS>,
    pub node_profile_name: ShinkaiName,
    pub job_queue_manager: Arc<Mutex<JobQueueManager<HttpDownloadJob>>>,
    pub job_processing_task: Option<tokio::task::JoinHandle<()>>,
}

impl HttpDownloadManager {
    pub async fn new(db: Weak<ShinkaiDB>, vector_fs: Weak<VectorFS>, node_profile_name: ShinkaiName) -> Self {
        // TODO: we need to pass the subscription manager to this function to check if subscriptions are still valid?
        // or we can read it from the db and compare it
        let db_prefix = "http_downloader_manager_";
        let job_queue = JobQueueManager::<HttpDownloadJob>::new(
            db.clone(),
            Topic::AnyQueuesPrefixed.as_str(),
            Some(db_prefix.to_string()),
        )
        .await
        .unwrap();
        let job_queue_manager = Arc::new(Mutex::new(job_queue));

        let thread_number = env::var("HTTP_DOWNLOAD_MANAGER_THREADS")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS);

        let job_queue_handler = HttpDownloadManager::process_download_queue(
            Arc::clone(&job_queue_manager),
            vector_fs.clone(),
            db.clone(),
            thread_number,
        )
        .await;

        Self {
            db,
            vector_fs,
            node_profile_name,
            job_queue_manager,
            job_processing_task: Some(job_queue_handler),
        }
    }

    pub async fn process_download_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<HttpDownloadJob>>>,
        vector_fs: Weak<VectorFS>,
        db: Weak<ShinkaiDB>,
        max_parallel_downloads: usize,
    ) -> tokio::task::JoinHandle<()> {
        let semaphore = Arc::new(Semaphore::new(max_parallel_downloads));
        let mut handles = Vec::new();

        tokio::spawn(async move {
            loop {
                let mut continue_immediately = false;

                // Scope for acquiring and releasing the lock quickly
                let job_ids_to_process: Vec<HttpDownloadJob> = {
                    let job_queue = job_queue_manager.lock().await;
                    let all_jobs = job_queue.get_all_elements_interleave().await.unwrap_or(Vec::new());

                    let filtered_jobs = all_jobs
                        .into_iter()
                        .filter(|_job| {
                            // TODO: we should check here that the subscription is still valid
                            true
                        })
                        .take(max_parallel_downloads)
                        .collect::<Vec<HttpDownloadJob>>();

                    // Check if the number of jobs to process is equal to max_parallel_downloads
                    continue_immediately = filtered_jobs.len() == max_parallel_downloads;
                    filtered_jobs
                };

                // Spawn tasks based on filtered job IDs
                for job_id in job_ids_to_process {
                    let semaphore = Arc::clone(&semaphore);
                    let vector_fs_clone = vector_fs.clone();
                    let db_clone = db.clone();

                    let handle = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.unwrap();
                        // Simulate processing the job
                        println!("Processing job: {:?}", job_id);
                        // Here you would add your actual job processing logic
                        // TODO: download the file and save it to the vector fs
                        drop(_permit);
                    });
                    handles.push(handle);
                }

                let handles_to_join = std::mem::take(&mut handles);
                futures::future::join_all(handles_to_join).await;
                handles.clear();

                // If job_ids_to_process was equal to max_parallel_downloads, loop again immediately
                // without waiting for a new job from receiver.recv().await
                if continue_immediately {
                    continue;
                }

                // Simulate receiving new jobs
                // This is a placeholder for actual logic to wait for new jobs
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        })
    }
}
