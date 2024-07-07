use crate::db::Topic;
use crate::network::subscription_manager::fs_entry_tree::FSEntryTree;
use crate::vector_fs::vector_fs::VectorFS;
use crate::{db::ShinkaiDB, llm_provider::queue::job_queue_manager::JobQueueManager};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_ENCODING};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_subscription::ShinkaiSubscription;
use shinkai_message_primitives::schemas::{shinkai_name::ShinkaiName, shinkai_subscription::SubscriptionId};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::vector_resource::{VRKai, VRPath};
use std::cmp::Ordering;
use std::env;
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, Semaphore};

use super::http_upload_manager::FileLink;

#[allow(dead_code)]
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

impl HttpDownloadJob {
    pub fn from_subscription_and_tree(subscription: ShinkaiSubscription, tree: &FSEntryTree) -> Result<Self, String> {
        let file_link = tree
            .web_link
            .as_ref()
            .ok_or_else(|| "WebLink is missing".to_string())
            .map(|web_link| FileLink {
                path: web_link.file.path.clone(),
                link: web_link.file.link.clone(),
                last_8_hash: web_link.file.last_8_hash.clone(),
                expiration: web_link.file.expiration,
            })?;

        Ok(HttpDownloadJob {
            subscription_id: subscription.subscription_id.clone(),
            info: file_link.clone(),
            url: file_link.link.clone(),
            date_created: chrono::Utc::now().to_rfc3339(),
        })
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

    #[allow(dead_code)]
    pub async fn process_download_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<HttpDownloadJob>>>,
        vector_fs: Weak<VectorFS>,
        db: Weak<ShinkaiDB>,
        max_parallel_downloads: usize,
    ) -> tokio::task::JoinHandle<()> {
        let job_queue_manager = Arc::clone(&job_queue_manager);
        let mut receiver = job_queue_manager.lock().await.subscribe_to_all().await;
        let semaphore = Arc::new(Semaphore::new(max_parallel_downloads));
        let mut handles = Vec::new();

        let is_testing = env::var("IS_TESTING").ok().map(|v| v == "1").unwrap_or(false);

        if is_testing {
            return tokio::spawn(async {});
        }

        tokio::spawn(async move {
            loop {
                let mut continue_immediately = false;

                // Call the extracted function to process the job queue
                let new_handles = HttpDownloadManager::process_job_queue(
                    Arc::clone(&job_queue_manager),
                    vector_fs.clone(),
                    db.clone(),
                    max_parallel_downloads,
                    Arc::clone(&semaphore),
                    &mut continue_immediately,
                )
                .await;
                handles.extend(new_handles);

                let handles_to_join = std::mem::take(&mut handles);
                futures::future::join_all(handles_to_join).await;
                handles.clear();

                // If job_ids_to_process was equal to max_parallel_downloads, loop again immediately
                // without waiting for a new job from receiver.recv().await
                if continue_immediately {
                    continue;
                }

                // Receive new jobs
                if let Some(new_job) = receiver.recv().await {
                    shinkai_log(
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Info,
                        format!(
                            "Received new job to download {:?}",
                            new_job.subscription_id.get_unique_id()
                        )
                        .as_str(),
                    );
                }
            }
        })
    }

    // Extracted function to process job queue
    #[allow(dead_code)]
    pub async fn process_job_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<HttpDownloadJob>>>,
        vector_fs: Weak<VectorFS>,
        db: Weak<ShinkaiDB>,
        max_parallel_downloads: usize,
        semaphore: Arc<Semaphore>,
        continue_immediately: &mut bool,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let mut new_handles = Vec::new();
        let job_ids_to_process: Vec<HttpDownloadJob> = {
            let job_queue = job_queue_manager.lock().await;
            let all_jobs = job_queue.get_all_elements_interleave().await.unwrap_or(Vec::new());
            let db_strong = db.upgrade();
            if db_strong.is_none() {
                println!("DB connection is lost, skipping this iteration.");
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                return new_handles; // Skip the rest of the loop iteration if the DB connection is lost
            }
            let db_strong = db_strong.unwrap();

            let filtered_jobs = all_jobs
                .into_iter()
                .filter(|job| {
                    let subscription = db_strong.get_my_subscription(job.subscription_id.get_unique_id());
                    if let Ok(subscription) = subscription {
                        subscription.subscription_id == job.subscription_id
                    } else {
                        false
                    }
                })
                .take(max_parallel_downloads)
                .collect::<Vec<HttpDownloadJob>>();

            // Check if the number of jobs to process is equal to max_parallel_downloads
            *continue_immediately = filtered_jobs.len() == max_parallel_downloads;
            filtered_jobs
        };

        // Spawn tasks based on filtered job IDs
        for job_id in job_ids_to_process {
            let job_queue_manager = Arc::clone(&job_queue_manager);
            let semaphore_clone = Arc::clone(&semaphore);
            let vector_fs_clone = vector_fs.clone();

            let handle = tokio::spawn(async move {
                let _permit = semaphore_clone.acquire().await.unwrap();
                // Call the new function to download and save the file
                if let Err(e) = HttpDownloadManager::download_and_save_file(job_id.clone(), vector_fs_clone).await {
                    println!("Error processing job {:?}: {}", job_id, e);
                }
                // Dequeue the job after processing
                if let Ok(Some(_)) = job_queue_manager
                    .lock()
                    .await
                    .dequeue(job_id.subscription_id.get_unique_id())
                    .await
                {
                    println!("Successfully dequeued job: {:?}", job_id);
                } else {
                    println!("Failed to dequeue job: {:?}", job_id);
                }
                drop(_permit);
            });
            new_handles.push(handle);
        }
        new_handles
    }

    // New static function to handle file download and saving
    #[allow(dead_code)]
    pub async fn download_and_save_file(
        job: HttpDownloadJob,
        vector_fs: Weak<VectorFS>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Upgrade the Weak pointer to a Strong one to use vector_fs
        if let Some(vector_fs) = vector_fs.upgrade() {
            // TODO: Update this so it's a tuple (VRKai + Checksum) and we validate the vrkai at the end
            // Ignore .checksum files for now
            if job.info.path.ends_with(".checksum") {
                return Ok(());
            }

            // Create HTTP client
            let client = Client::builder()
                .default_headers({
                    let mut headers = HeaderMap::new();
                    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
                    headers
                })
                .build()?;

            // Make the HTTP GET request to download the file
            let response = client.get(&job.url).send().await?;
            let content = response.bytes().await?;

            // Construct the full path where the file will be saved
            let requester = match job.subscription_id.extract_subscriber_node_with_profile() {
                Ok(requester) => requester,
                Err(e) => {
                    return Err(format!("Error extracting subscriber node and profile: {}", e).into());
                }
            };

            // Save the downloaded content to vector_fs
            let item_path = VRPath::from_string(&job.info.path)?;
            let writer = vector_fs
                .new_writer(requester.clone(), item_path.parent_path().clone(), requester.clone())
                .await?;

            let vrkai_file = match VRKai::from_bytes(&content) {
                Ok(vrkai) => vrkai,
                Err(e) => {
                    return Err(format!("Error creating VRKai from bytes: {}", e).into());
                }
            };

            let parent_folder = item_path.parent_path();
            vector_fs.create_new_folder_auto(&writer, parent_folder).await?;

            let save_result = vector_fs.save_vrkai_in_folder(&writer, vrkai_file).await;
            match save_result {
                Ok(_fs_item) => {
                    // If save is successful, do nothing or handle success case
                }
                Err(e) => {
                    return Err(format!("Error saving file in folder: {}", e).into());
                }
            }

            eprintln!("Downloaded and saved file: {:?}", job.info.path);
            Ok(())
        } else {
            Err("Failed to acquire vector_fs".into())
        }
    }

    // Function to add a new download job to the job queue
    #[allow(dead_code)]
    pub async fn add_job_to_download_queue(&self, job: HttpDownloadJob) -> Result<String, Box<dyn std::error::Error>> {
        // Create a mutable copy of the job
        let mut job = job.clone();

        // TODO: enable this when everything is working
        // Prepend "/subscription" to the path in the FileLink of the job copy
        // job.info.path = format!("/My_Subscriptions{}", job.info.path);

        let mut job_queue_manager = self.job_queue_manager.lock().await;
        let _ = job_queue_manager
            .push(job.subscription_id.get_unique_id(), job.clone())
            .await;

        Ok(job.subscription_id.get_unique_id().to_string())
    }

    #[allow(dead_code)]
    pub async fn test_process_job_queue(&self) -> Vec<tokio::task::JoinHandle<()>> {
        let thread_number = env::var("HTTP_DOWNLOAD_MANAGER_THREADS")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS);

        let semaphore = Arc::new(Semaphore::new(thread_number));
        let mut continue_immediately = false;

        HttpDownloadManager::process_job_queue(
            Arc::clone(&self.job_queue_manager),
            self.vector_fs.clone(),
            self.db.clone(),
            thread_number,
            Arc::clone(&semaphore),
            &mut continue_immediately,
        )
        .await
    }
}
