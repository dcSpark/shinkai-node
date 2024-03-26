use crate::agent::queue::job_queue_manager::JobQueueManager;
use crate::db::db_errors::ShinkaiDBError;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::network::subscription_manager::fs_item_tree_generator::FSItemTreeGenerator;
use crate::network::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use crate::network::Node;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_error::VectorFSError;
use crate::vector_fs::vector_fs_permissions::ReadPermission;
use crate::vector_fs::vector_fs_types::{FSEntry, FSFolder, FSItem};
use chrono::NaiveDateTime;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use ed25519_dalek::SigningKey;
use futures::Future;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionStatus, SubscriptionId,
};
use shinkai_message_primitives::schemas::shinkai_subscription_req::{FolderSubscription, SubscriptionPayment};
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_vector_resources::vector_resource::VRPath;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Arc;
use std::sync::Weak;
use std::{env, mem};
use tokio::sync::{Mutex, MutexGuard, Semaphore};

use super::fs_item_tree::FSItemTree;
use super::my_subscription_manager::MySubscriptionsManager;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

/*
How to subscribe
- Node can scan multiple nodes and process what they offer. Endpoint: Node (External) -> Shareable stuff? (different than local node endpoint)
- User sees something that they like
- They subscribe to it
- Node validates and adds node to their subscriptors (maybe it should sync from the chain (?)) how do we know which subscription is which one?
  - can you be switching so you dont pay multiple subscriptions? -> maybe minimal time is good enough to avoid this
- Node processes the subscription and adds it to the queue
  - node ask the subscriber what they state
  - node calculates diff
  - node sends the diff to the subscriber
- Node checks for changes every X time and sends the diff to the subscriber in order to update the state
*/

// Message

// action: subscribe
// subscription_id: id
// vector_db: path
// state: [] or tree

// -> Validation ->

// it comes from the actual node
// vector_db path is valid and "shareable"
// state is valid
// delegation is enough

// -> Processing ->
// add node to the subscription_db
// schedule the job to be processed

// -> NetworkJobForProcessing -> (On Demand rather than calculated in advance)
// shinkai node name
// subscription_id
// vector_db
// state

/// Temp

// Who decides to split it? a cron? the SubscriberManager that checks the state? -> The subscriber manager

const NUM_THREADS: usize = 2;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SubscriptionWithTree {
    pub subscription: ShinkaiSubscription,
    pub subscriber_folder_tree: FSItemTree,
}

impl Ord for SubscriptionWithTree {
    fn cmp(&self, other: &Self) -> Ordering {
        self.subscription.cmp(&other.subscription)
    }
}

impl PartialOrd for SubscriptionWithTree {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SharedFolderInfo {
    pub path: String,
    pub permission: String,
    pub tree: FSItemTree,
    pub subscription_requirement: Option<FolderSubscription>,
}

pub struct ExternalSubscriberManager {
    pub db: Weak<Mutex<ShinkaiDB>>,
    pub vector_fs: Weak<Mutex<VectorFS>>,
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
    /// Maps subscription IDs to their sync status, where the `String` represents the folder path
    /// and the `usize` is the last sync version of the folder. The version is a counter that increments
    /// with each change in the folder, providing a non-deterministic but sequential tracking of updates.
    pub subscription_ids_are_sync: Arc<DashMap<String, (String, usize)>>,
    pub shared_folders_to_ephemeral_versioning: Arc<DashMap<String, usize>>,
    // todo: implement need fn that receives responses from subscribers to process and another one to update the state after successfully sync
    pub subscriptions_queue_manager: Arc<Mutex<JobQueueManager<SubscriptionWithTree>>>,
    pub subscription_processing_task: Option<tokio::task::JoinHandle<()>>,
}

impl ExternalSubscriberManager {
    pub async fn new(
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        identity_manager: Weak<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
    ) -> Self {
        let db_prefix = "subscriptions_abcprefix_"; // dont change it
        let subscriptions_queue = JobQueueManager::<SubscriptionWithTree>::new(
            db.clone(),
            Topic::AnyQueuesPrefixed.as_str(),
            Some(db_prefix.to_string()),
        )
        .await
        .unwrap();
        let subscriptions_queue_manager = Arc::new(Mutex::new(subscriptions_queue));
        let shared_folders_trees = Arc::new(DashMap::new());
        let subscription_ids_are_sync = Arc::new(DashMap::new());
        let shared_folders_to_ephemeral_versioning = Arc::new(DashMap::new());

        let thread_number = env::var("SUBSCRIBER_MANAGER_NETWORK_CONCURRENCY")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS); // Start processing the job queue

        let subscription_queue_handler = ExternalSubscriberManager::process_subscription_queue(
            subscriptions_queue_manager.clone(),
            db.clone(),
            vector_fs.clone(),
            node_name.clone(),
            my_signature_secret_key.clone(),
            my_encryption_secret_key.clone(),
            identity_manager.clone(),
            shared_folders_trees.clone(),
            subscription_ids_are_sync.clone(),
            shared_folders_to_ephemeral_versioning.clone(),
            thread_number,
            |job,
             db,
             vector_fs,
             node_name,
             my_signature_secret_key,
             my_encryption_secret_key,
             identity_manager,
             shared_folders_trees,
             subscription_ids_are_sync,
             shared_folders_to_ephemeral_versioning| {
                ExternalSubscriberManager::process_job_message_queued(
                    job,
                    db,
                    vector_fs,
                    node_name,
                    my_signature_secret_key,
                    my_encryption_secret_key,
                    identity_manager,
                    shared_folders_trees,
                    subscription_ids_are_sync,
                    shared_folders_to_ephemeral_versioning,
                )
            },
        )
        .await;

        ExternalSubscriberManager {
            db,
            vector_fs,
            identity_manager,
            subscriptions_queue_manager,
            subscription_processing_task: Some(subscription_queue_handler),
            shared_folders_trees,
            subscription_ids_are_sync,
            shared_folders_to_ephemeral_versioning,
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
        }
    }

    pub async fn get_cached_shared_folder_tree(&self, path: &str) -> Vec<SharedFolderInfo> {
        if path == "/" {
            // Collect all values into a Vec
            self.shared_folders_trees
                .iter()
                .map(|entry| entry.value().clone())
                .collect()
        } else {
            // Attempt to get a single value and wrap it in a Vec if it exists
            self.shared_folders_trees
                .get(path)
                .map(|value| vec![value.clone()])
                .unwrap_or_else(Vec::new)
        }
    }

    fn process_job_message_queued(
        subscription_with_tree: SubscriptionWithTree,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        identity_manager: Weak<Mutex<IdentityManager>>,
        shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
        subscription_ids_are_sync: Arc<DashMap<String, (String, usize)>>,
        shared_folders_to_ephemeral_versioning: Arc<DashMap<String, usize>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, SubscriberManagerError>> + Send + 'static>> {
        Box::pin(async move {
            // Placeholder logic for processing a queued job message
            println!(
                "Processing job: {:?}",
                subscription_with_tree.subscription.subscription_id
            );

            // TODO: access shared_folders_trees for the subscription path

            // Assuming the processing is successful, return Ok with a message
            // Adjust according to actual logic and possible error conditions
            Ok(format!(
                "Job {} processed successfully",
                subscription_with_tree.subscription.subscription_id.get_unique_id()
            ))
        })
    }

    pub async fn process_subscription_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<SubscriptionWithTree>>>,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        identity_manager: Weak<Mutex<IdentityManager>>,
        shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
        subscription_ids_are_sync: Arc<DashMap<String, (String, usize)>>,
        shared_folders_to_ephemeral_versioning: Arc<DashMap<String, usize>>,
        thread_number: usize,
        process_job: impl Fn(
                SubscriptionWithTree,
                Weak<Mutex<ShinkaiDB>>,
                Weak<Mutex<VectorFS>>,
                ShinkaiName,
                SigningKey,
                EncryptionStaticKey,
                Weak<Mutex<IdentityManager>>,
                Arc<DashMap<String, SharedFolderInfo>>,
                Arc<DashMap<String, (String, usize)>>,
                Arc<DashMap<String, usize>>,
            ) -> Pin<Box<dyn Future<Output = Result<String, SubscriberManagerError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let job_queue_manager = Arc::clone(&job_queue_manager);
        let processing_jobs = Arc::new(Mutex::new(HashSet::new()));
        let semaphore = Arc::new(Semaphore::new(thread_number));
        let process_job = Arc::new(process_job);

        let interval_minutes = env::var("SUBSCRIPTION_PROCESS_INTERVAL_MINUTES")
            .unwrap_or("5".to_string()) // Default to 5 minutes if not set
            .parse::<u64>()
            .unwrap_or(5);

        let is_testing = env::var("IS_TESTING").ok().map(|v| v == "1").unwrap_or(false);

        tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Info,
                "Starting subscribers processing loop",
            );

            let mut handles = Vec::new();
            if is_testing {
                // Wait until subscription_ids_are_sync has at least 1 value
                loop {
                    if !subscription_ids_are_sync.is_empty() {
                        eprintln!(
                            ">> subscription_ids_are_sync moving to the loop in 5s: {:?}",
                            subscription_ids_are_sync
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }

            loop {
                let mut continue_immediately = false;
                eprintln!("Starting subscribers processing loop");
                // Game Plan:
                // Phase 1
                // 1. Find all subscriptions
                // 2. Filter out subscriptions that are already on sync
                // 3. Request subscribers folder state (async)
                // Phase 2
                // 4. Calc. diff and schedule network requests (async) -> Process

                // Scope for acquiring and releasing the lock quickly
                let subscriptions_ids_to_process: Vec<SubscriptionId> = {
                    let db = match db.upgrade() {
                        Some(db) => db,
                        None => {
                            shinkai_log(
                                ShinkaiLogOption::ExtSubscriptions,
                                ShinkaiLogLevel::Error,
                                "Database instance is not available",
                            );
                            break; // or continue based on your error handling policy
                        }
                    };
                    let db = db.lock().await;
                    match db.all_subscribers_subscription() {
                        Ok(subscriptions) => subscriptions.into_iter().map(|s| s.subscription_id).collect(),
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::ExtSubscriptions,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to fetch subscriptions: {:?}", e),
                            );
                            Vec::new() // Continue the loop, even if fetching subscriptions failed
                        }
                    }
                };
                eprintln!(">> Subscriptions to process: {:?}", subscriptions_ids_to_process);

                // We keep a ephemeral versioning of the shared folders to know if a specific subscription is already sync
                // This is useful to avoid processing the same subscription multiple times
                let filtered_subscription_ids = subscriptions_ids_to_process
                    .into_iter()
                    .filter(|subscription_id| {
                        let subscription_id_str = subscription_id.get_unique_id().to_string();
                        if let Some(ref arc_tuple) = subscription_ids_are_sync.get(&subscription_id_str) {
                            let folder_path = &arc_tuple.0;
                            let last_sync_version = &arc_tuple.1;
                            // Use the cloned version here
                            if let Some(current_version_arc) = shared_folders_to_ephemeral_versioning.get(folder_path) {
                                let current_version = *current_version_arc.value();
                                return current_version != *last_sync_version;
                            }
                        }
                        true
                    })
                    .collect::<Vec<SubscriptionId>>();

                // Check if a job with this subscription_id is already queued in the job_manager
                let mut post_filtered_subscription_ids = Vec::new();
                for subscription_id in filtered_subscription_ids {
                    let subscription_id_str = subscription_id.get_unique_id().to_string();
                    // Perform the asynchronous check
                    let is_not_queued = {
                        let job_queue_manager_clone = Arc::clone(&job_queue_manager);
                        let result = job_queue_manager_clone
                        .lock()
                        .await
                        .peek(&subscription_id_str)
                        .await
                        .map_or(true, |opt| opt.is_none());
                        result
                    };
                    if is_not_queued {
                        post_filtered_subscription_ids.push(subscription_id);
                    }
                }
                // Now we send requests to the subscribers to get their current state
                for subscription_id in post_filtered_subscription_ids.clone() {
                    let _ = Self::create_and_send_request_updated_state(
                        subscription_id,
                        db.clone(),
                        my_encryption_secret_key.clone(),
                        my_signature_secret_key.clone(),
                        node_name.clone(),
                        identity_manager.clone(),
                    )
                    .await;
                }

                // End Phase 1
                // Start Phase 2: We check current jobs that are ready to go
                let job_ids_to_perform_comparisons_and_send_files = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_clone = Arc::clone(&job_queue_manager);
                    let job_queue_manager_lock = job_queue_manager_clone.lock().await;
                    let all_jobs = job_queue_manager_lock
                        .get_all_elements_interleave()
                        .await
                        .unwrap_or(Vec::new());
                    drop(job_queue_manager_lock);
                    std::mem::drop(job_queue_manager_clone);

                    let filtered_jobs = all_jobs
                        .into_iter()
                        .filter_map(|job| {
                            let job_id = job.subscription.subscription_id.clone().get_unique_id().to_string();
                            if !processing_jobs_lock.contains(&job_id) {
                                processing_jobs_lock.insert(job_id.clone());
                                Some(job_id)
                            } else {
                                None
                            }
                        })
                        .take(thread_number)
                        .collect::<Vec<_>>();

                    // Check if the number of jobs to process is equal to max_parallel_jobs
                    continue_immediately = filtered_jobs.len() == thread_number;

                    std::mem::drop(processing_jobs_lock);
                    filtered_jobs
                };

                for subscription_id in job_ids_to_perform_comparisons_and_send_files {
                    let job_queue_manager = Arc::clone(&job_queue_manager);
                    let semaphore = semaphore.clone();
                    let db = db.clone();
                    let vector_fs = vector_fs.clone();
                    let node_name = node_name.clone();
                    let my_signature_secret_key = my_signature_secret_key.clone();
                    let my_encryption_secret_key = my_encryption_secret_key.clone();
                    let identity_manager = identity_manager.clone();
                    let shared_folders_trees = shared_folders_trees.clone();
                    let subscription_ids_are_sync = subscription_ids_are_sync.clone();
                    let shared_folders_to_ephemeral_versioning_clone = shared_folders_to_ephemeral_versioning.clone();
                    let process_job = process_job.clone();

                    let handle = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.expect("Failed to acquire semaphore permit");

                        // Acquire the lock, dequeue the job, and immediately release the lock
                        let subscription_with_tree = {
                            let job_queue_manager = job_queue_manager.lock().await;
                            let subscription_with_tree = job_queue_manager.peek(&subscription_id).await;
                            subscription_with_tree
                        };

                        match subscription_with_tree {
                            Ok(Some(job)) => {
                                // Acquire the lock, process the job, and immediately release the lock
                                let result = {
                                    let result = process_job(
                                        job.clone(),
                                        db.clone(),
                                        vector_fs.clone(),
                                        node_name.clone(),
                                        my_signature_secret_key.clone(),
                                        my_encryption_secret_key.clone(),
                                        identity_manager.clone(),
                                        shared_folders_trees.clone(),
                                        subscription_ids_are_sync.clone(),
                                        shared_folders_to_ephemeral_versioning_clone.clone(),
                                    )
                                    .await;
                                    if let Ok(Some(_)) = job_queue_manager
                                        .lock()
                                        .await
                                        .dequeue(&job.subscription.subscription_id.clone().get_unique_id().to_string())
                                        .await
                                    {
                                        result
                                    } else {
                                        Err(SubscriberManagerError::OperationFailed(format!(
                                            "Failed to dequeue job: {}",
                                            job.subscription.subscription_id.clone().get_unique_id().to_string()
                                        )))
                                    }
                                };
                                match result {
                                    Ok(_) => {
                                        shinkai_log(
                                            ShinkaiLogOption::JobExecution,
                                            ShinkaiLogLevel::Debug,
                                            "Job processed successfully",
                                        );
                                    } // handle success case
                                    Err(_) => {} // handle error case
                                }
                            }
                            Ok(None) => {}
                            Err(_) => {
                                // Log the error
                            }
                        }

                        drop(_permit);
                    });
                    handles.push(handle);
                }

                let handles_to_join = mem::replace(&mut handles, Vec::new());
                futures::future::join_all(handles_to_join).await;
                handles.clear();

                // Wait for interval_minutes before the next iteration
                tokio::time::sleep(tokio::time::Duration::from_secs(interval_minutes * 60)).await;

                // TODO: the continue_immediately logic is a temporary solution to avoid waiting for the interval
            }
        })
    }

    /// The return type is (shareable_path, permission, tree, subscription_requirement)
    pub async fn available_shared_folders(
        &mut self,
        requester_shinkai_identity: ShinkaiName,
        path: String,
    ) -> Result<Vec<SharedFolderInfo>, SubscriberManagerError> {
        let mut converted_results = Vec::new();
        {
            let vector_fs = self
                .vector_fs
                .upgrade()
                .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                    "VectorFS instance is not available".to_string(),
                ))?;
            let mut vector_fs = vector_fs.lock().await;

            let vr_path =
                VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

            let reader = vector_fs
                .new_reader(
                    requester_shinkai_identity.clone(),
                    vr_path,
                    requester_shinkai_identity.clone(),
                )
                .map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

            // Note: double-check that the Whitelist is correct here under these assumptions
            let results = vector_fs.find_paths_with_read_permissions(&reader, vec![ReadPermission::Public])?; // everything is whitelisted. I think it should be Private by default ReadPermission::Whitelist

            // Use the new function to filter results to only include top-level folders
            let filtered_results = FSItemTreeGenerator::filter_to_top_level_folders(results);

            // Drop the lock on vector_fs before proceeding
            drop(vector_fs);

            let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;
            let db = db.lock().await;

            for (path, permission) in filtered_results {
                let path_str = path.to_string();
                let permission_str = format!("{:?}", permission);
                let subscription_requirement = match db.get_folder_requirements(&path_str) {
                    Ok(req) => Some(req),
                    Err(_) => None, // Instead of erroring out, we return None for folders without requirements
                };
                let tree = FSItemTreeGenerator::shared_folders_to_tree(
                    self.vector_fs.clone(),
                    requester_shinkai_identity.clone(),
                    path_str.clone(),
                )
                .await?;

                let result = SharedFolderInfo {
                    path: path_str.clone(),
                    permission: permission_str,
                    tree,
                    subscription_requirement,
                };

                // Check if the value of shared_folders_trees is different than the new value inserted
                let should_update_version = self
                    .shared_folders_trees
                    .get(&path_str)
                    .map_or(true, |existing| *existing.value() != result);

                if should_update_version {
                    // Update shared_folders_to_ephemeral_versioning
                    self.shared_folders_to_ephemeral_versioning
                        .entry(path_str.clone())
                        .and_modify(|e| *e += 1)
                        .or_insert(1); // the first version starts at one
                }

                converted_results.push(result.clone());
                self.shared_folders_trees.insert(path_str, result);
            }
        }

        // TODO: convert eprintlns to shinkai_logs
        eprintln!(
            "Node: {} Converted results: {:?}",
            self.node_name.clone(),
            converted_results
        );

        Ok(converted_results)
    }

    pub async fn update_shareable_folder_requirements(
        &self,
        path: String,
        requester_shinkai_identity: ShinkaiName,
        subscription_requirement: FolderSubscription,
    ) -> Result<bool, SubscriberManagerError> {
        // TODO: check that you are actually an admin of the folder
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;
        let vector_fs = vector_fs.lock().await;

        let vr_path = VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;
        let result = vector_fs.get_path_permission_for_paths(requester_shinkai_identity.clone(), vec![vr_path])?;

        // Checks that the permission is valid (Whitelist or Public)
        for (_, path_permission) in &result {
            if path_permission.read_permission != ReadPermission::Public
                && path_permission.read_permission != ReadPermission::Whitelist
            {
                return Err(SubscriberManagerError::InvalidRequest(
                    "Permission is not valid".to_string(),
                ));
            }
        }

        // Assuming we have validated the admin and permissions, we proceed to update the DB
        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;
        let mut db = db.lock().await;

        db.set_folder_requirements(&path, subscription_requirement)
            .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;

        Ok(true)
    }

    pub async fn create_shareable_folder(
        &mut self,
        path: String,
        requester_shinkai_identity: ShinkaiName,
        subscription_requirement: FolderSubscription,
    ) -> Result<bool, SubscriberManagerError> {
        // TODO: check that you are actually an admin of the folder
        // Note: I think is done automatically
        {
            let vector_fs = self
                .vector_fs
                .upgrade()
                .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                    "VectorFS instance is not available".to_string(),
                ))?;
            let mut vector_fs = vector_fs.lock().await;

            let vr_path =
                VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;
            let writer = vector_fs.new_writer(
                requester_shinkai_identity.clone(),
                vr_path.clone(),
                requester_shinkai_identity.clone(),
            )?;

            // Retrieve the current write permissions for the path
            let permissions_vector =
                vector_fs.get_path_permission_for_paths(requester_shinkai_identity.clone(), vec![vr_path.clone()])?;

            if permissions_vector.is_empty() {
                return Err(SubscriberManagerError::InvalidRequest(
                    "Path does not exist".to_string(),
                ));
            }

            let (_, current_permissions) = permissions_vector.into_iter().next().unwrap();

            // Set the read permissions to Public while reusing the write permissions
            vector_fs.update_permissions_recursively(
                &writer,
                ReadPermission::Public,
                current_permissions.write_permission,
            )?;
        }
        {
            // Assuming we have validated the admin and permissions, we proceed to update the DB
            let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;
            let mut db = db.lock().await;

            db.set_folder_requirements(&path, subscription_requirement)
                .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;
        }

        // Trigger a refresh of the shareable folders cache
        let _ = self
            .available_shared_folders(requester_shinkai_identity.clone(), "/".to_string())
            .await;

        Ok(true)
    }

    pub async fn unshare_folder(
        &mut self,
        path: String,
        requester_shinkai_identity: ShinkaiName,
    ) -> Result<bool, SubscriberManagerError> {
        {
            let vector_fs = self
                .vector_fs
                .upgrade()
                .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                    "VectorFS instance is not available".to_string(),
                ))?;
            let mut vector_fs = vector_fs.lock().await;

            // Retrieve the current permissions for the path
            let permissions_vector = vector_fs
                .get_path_permission_for_paths(requester_shinkai_identity.clone(), vec![VRPath::from_string(&path)?])?;

            if permissions_vector.is_empty() {
                return Err(SubscriberManagerError::InvalidRequest(
                    "Path does not exist".to_string(),
                ));
            }

            let (vr_path, current_permissions) = permissions_vector.into_iter().next().unwrap();

            // Create a writer for the path
            let writer = vector_fs.new_writer(
                requester_shinkai_identity.clone(),
                vr_path,
                requester_shinkai_identity.clone(),
            )?;

            // Set the read permissions to Private while reusing the write permissions using update_permissions_recursively
            vector_fs.update_permissions_recursively(
                &writer,
                ReadPermission::Private,
                current_permissions.write_permission,
            )?;
        }
        {
            // Assuming we have validated the admin and permissions, we proceed to update the DB
            let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;
            let mut db = db.lock().await;
            db.remove_folder_requirements(&path)
                .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;
        }

        self.shared_folders_trees.remove(&path);
        Ok(true)
    }

    pub async fn subscribe_to_shared_folder(
        &mut self,
        requester_shinkai_identity: ShinkaiName,
        shared_folder: String,
        subscription_requirement: SubscriptionPayment,
    ) -> Result<bool, SubscriberManagerError> {
        // Validate that the requester actually did the alleged payment
        match subscription_requirement.clone() {
            SubscriptionPayment::Free => {
                // No validation required
            }
            SubscriptionPayment::DirectDelegation => {
                // Validate direct delegation logic here
                // If validation fails, you can return early with an error
                // Placeholder for validation check
                let is_valid_delegation = false; // This should be replaced with actual validation logic
                if !is_valid_delegation {
                    return Err(SubscriberManagerError::SubscriptionFailed(
                        "Direct delegation validation failed".to_string(),
                    ));
                }
            }
            SubscriptionPayment::Payment(payment_details) => {
                // Validate payment logic here
                // If validation fails, you can return early with an error
                // Placeholder for payment validation
                let is_valid_payment = false; // This should be replaced with actual payment validation logic
                if !is_valid_payment {
                    return Err(SubscriberManagerError::SubscriptionFailed(format!(
                        "Payment validation failed: {}",
                        payment_details
                    )));
                }
            }
        }

        let subscription_id = SubscriptionId::new(
            self.node_name.extract_node(),
            shared_folder.clone(),
            requester_shinkai_identity.extract_node(),
        );

        // The requester has passed the validation checks
        // Proceed to add the requester to the list of subscribers
        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;
        let mut db = db.lock().await;

        match db.get_subscription_by_id(&subscription_id) {
            Ok(_) => {
                // If subscription exists, return an error or a specific message indicating already subscribed
                return Err(SubscriberManagerError::AlreadySubscribed(
                    "Requester is already subscribed to this folder".to_string(),
                ));
            }
            Err(ShinkaiDBError::DataNotFound) => {
                // If subscription does not exist, proceed with adding the subscription
            }
            Err(e) => {
                // Handle other database errors
                return Err(SubscriberManagerError::DatabaseError(e.to_string()));
            }
        }

        let subscription = ShinkaiSubscription::new(
            shared_folder.clone(),
            self.node_name.extract_node(),
            requester_shinkai_identity.extract_node(),
            ShinkaiSubscriptionStatus::SubscriptionConfirmed,
            Some(subscription_requirement),
        );

        db.add_subscriber_subscription(subscription)
            .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;

        // Check if we are in testing mode and update subscription_ids_are_sync accordingly
        if std::env::var("IS_TESTING").unwrap_or_default() == "1" {
            let subscription_id_str = subscription_id.get_unique_id().to_string();
            // Assuming the initial version for a new subscription should be 0
            // and using the shared_folder as the path. Adjust as necessary.
            self.subscription_ids_are_sync
                .insert(subscription_id_str, (shared_folder.clone(), 0));
        }
        Ok(true)
    }

    pub async fn create_and_send_request_updated_state(
        subscription_id: SubscriptionId,
        db: Weak<Mutex<ShinkaiDB>>,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        node_name: ShinkaiName,
        maybe_identity_manager: Weak<Mutex<IdentityManager>>,
    ) -> Result<(), SubscriberManagerError> {
        let subscription = {
            let db = db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;
            let db = db.lock().await;

            let subscription = db.get_subscription_by_id(&subscription_id).map_err(|e| match e {
                ShinkaiDBError::DataNotFound => SubscriberManagerError::SubscriptionNotFound(format!(
                    "Subscription with ID {} not found",
                    subscription_id.get_unique_id()
                )),
                _ => SubscriberManagerError::DatabaseError(e.to_string()),
            })?;
            subscription
        };

        // create message to request updated state

        if let Some(identity_manager_lock) = maybe_identity_manager.upgrade() {
            let subscriber_node_name = subscription.subscriber_identity.clone();
            let identity_manager = identity_manager_lock.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&subscriber_node_name.get_node_name())
                .await?;
            drop(identity_manager);

            let receiver_public_key = standard_identity.node_encryption_public_key;

            // Update to use SubscriptionRequiresTreeUpdateResponse instead
            let msg_request_subscription = ShinkaiMessageBuilder::vecfs_request_share_current_shared_folder_state(
                subscription.shared_folder.clone(),
                clone_static_secret_key(&my_encryption_secret_key),
                clone_signature_secret_key(&my_signature_secret_key),
                receiver_public_key,
                node_name.get_node_name(),
                // Note: the other node doesn't care about the sender's profile in this context
                "".to_string(),
                subscriber_node_name.get_node_name(),
                "".to_string(),
            )
            .map_err(|e| SubscriberManagerError::MessageProcessingError(e.to_string()))?;

            // TODO: move send_message_to_peer to a separate file
            MySubscriptionsManager::send_message_to_peer(
                msg_request_subscription,
                db.clone(),
                standard_identity,
                my_encryption_secret_key.clone(),
                maybe_identity_manager.clone(),
            )
            .await?;
        } else {
            return Err(SubscriberManagerError::IdentityManagerUnavailable);
        }

        Ok(())
    }

    pub async fn subscriber_current_state_response(
        &self,
        subscription_unique_id: String,
        subscriber_folder_tree: FSItemTree,
        subscriber_node_name: ShinkaiName,
    ) -> Result<(), SubscriberManagerError> {
        shinkai_log(
            ShinkaiLogOption::ExtSubscriptions,
            ShinkaiLogLevel::Debug,
            &format!(
                "Received current state response for subscription ID: {}, from subscriber: {}. Tree: {:?}",
                subscription_unique_id, subscriber_node_name, subscriber_folder_tree
            ),
        );

        // Validate Subscription Exists and that Requesting Node matches the subscription
        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;
        let db = db.lock().await;

        let subscription = db
            .get_subscription_by_id(&SubscriptionId::from_unique_id(subscription_unique_id.clone()))
            .map_err(|e| match e {
                ShinkaiDBError::DataNotFound => SubscriberManagerError::SubscriptionNotFound(format!(
                    "Subscription with ID {} not found",
                    subscription_unique_id
                )),
                _ => SubscriberManagerError::DatabaseError(e.to_string()),
            })?;

        if subscription.subscriber_identity.get_node_name() != subscriber_node_name.get_node_name() {
            return Err(SubscriberManagerError::InvalidSubscriber(
                "Subscriber does not match the subscription".to_string(),
            ));
        }

        let subscription_id_clone = subscription.subscription_id.clone();
        let unique_id = subscription_id_clone.get_unique_id();

        let subscription_with_tree = SubscriptionWithTree {
            subscription,
            subscriber_folder_tree,
        };

        {
            let mut queue_manager = self.subscriptions_queue_manager.lock().await;
            if queue_manager.peek(&unique_id).await?.is_none() {
                eprintln!("Adding to queue: {:?}", subscription_with_tree);
                let _ = queue_manager.push(&unique_id, subscription_with_tree).await;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::from_str;
    use shinkai_message_primitives::schemas::shinkai_subscription_req::PaymentOption;

    #[test]
    fn test_convert_string_to_shared_folder_info() {
        let json_str = r#"[{"path":"/shared_test_folder","permission":"Public","tree":{"name":"/","path":"/shared_test_folder","last_modified":"2024-03-24T00:11:29.958427+00:00","children":{"crypto":{"name":"crypto","path":"/shared_test_folder/crypto","last_modified":"2024-03-24T00:11:27.905905+00:00","children":{"shinkai_intro":{"name":"shinkai_intro","path":"/shared_test_folder/crypto/shinkai_intro","last_modified":"2024-02-26T23:06:00.019065981+00:00","children":{}}}}}},"subscription_requirement":{"minimum_token_delegation":100,"minimum_time_delegated_hours":100,"monthly_payment":{"USD":10.0},"is_free":false}}]"#;

        let shared_folder_info: Vec<SharedFolderInfo> = from_str(json_str).unwrap();

        assert_eq!(shared_folder_info.len(), 1);
        let folder_info = &shared_folder_info[0];
        assert_eq!(folder_info.path, "/shared_test_folder");
        assert_eq!(folder_info.permission, "Public");
        assert!(folder_info.subscription_requirement.is_some());
        let subscription_requirement = folder_info.subscription_requirement.as_ref().unwrap();
        assert_eq!(subscription_requirement.minimum_token_delegation, Some(100));
        assert_eq!(subscription_requirement.minimum_time_delegated_hours, Some(100));
        assert_eq!(
            match subscription_requirement.monthly_payment {
                Some(PaymentOption::USD(amount)) => Some(amount),
                _ => None,
            },
            Some(10.0)
        );
        assert_eq!(subscription_requirement.is_free, false);
    }
}
