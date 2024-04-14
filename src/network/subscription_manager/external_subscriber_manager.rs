use crate::agent::queue::job_queue_manager::JobQueueManager;
use crate::db::db_errors::ShinkaiDBError;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::network::subscription_manager::fs_entry_tree_generator::FSEntryTreeGenerator;
use crate::network::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use crate::network::Node;
use crate::schemas::identity::StandardIdentity;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_permissions::ReadPermission;
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
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    MessageSchemaType, SubscriptionGenericResponse, SubscriptionResponseStatus,
};
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_vector_resources::vector_resource::{VRPack, VRPath};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::env;
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::{Mutex, Semaphore};

use super::fs_entry_tree::FSEntryTree;
use super::my_subscription_manager::MySubscriptionsManager;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

const NUM_THREADS: usize = 2;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SubscriptionWithTree {
    pub subscription: ShinkaiSubscription,
    pub subscriber_folder_tree: FSEntryTree,
    pub symmetric_key: String,
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
    pub tree: FSEntryTree,
    pub subscription_requirement: Option<FolderSubscription>,
}

pub struct ExternalSubscriberManager {
    pub db: Weak<ShinkaiDB>,
    pub vector_fs: Weak<VectorFS>,
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
    pub last_refresh: Arc<Mutex<DateTime<Utc>>>,
    /// Maps subscription IDs to their sync status, where the `String` represents the folder path
    /// and the `usize` is the last sync version of the folder. The version is a counter that increments
    /// with each change in the folder, providing a non-deterministic but sequential tracking of updates.
    pub subscription_ids_are_sync: Arc<DashMap<String, (String, usize)>>,
    pub shared_folders_to_ephemeral_versioning: Arc<DashMap<String, usize>>,
    pub subscriptions_queue_manager: Arc<Mutex<JobQueueManager<SubscriptionWithTree>>>,
    pub subscription_processing_task: Option<tokio::task::JoinHandle<()>>,
    pub process_state_updates_queue_handler: Option<tokio::task::JoinHandle<()>>,
}

impl ExternalSubscriberManager {
    pub async fn new(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
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
        let last_refresh = Arc::new(Mutex::new(Utc::now()));

        let thread_number = env::var("SUBSCRIBER_MANAGER_NETWORK_CONCURRENCY")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS); // Start processing the job queue

        let process_state_updates_queue_handler =
            ExternalSubscriberManager::process_subscription_request_state_updates(
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
            )
            .await;

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
            |subscription_with_tree,
             db,
             vector_fs,
             node_name,
             my_signature_secret_key,
             my_encryption_secret_key,
             identity_manager,
             shared_folders_trees,
             subscription_ids_are_sync,
             shared_folders_to_ephemeral_versioning| {
                ExternalSubscriberManager::process_subscription_job_message_queued(
                    subscription_with_tree,
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

        let mut manager = ExternalSubscriberManager {
            db,
            vector_fs,
            last_refresh,
            identity_manager,
            subscriptions_queue_manager,
            subscription_processing_task: Some(subscription_queue_handler),
            process_state_updates_queue_handler: Some(process_state_updates_queue_handler),
            shared_folders_trees,
            subscription_ids_are_sync,
            shared_folders_to_ephemeral_versioning,
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
        };

        let result = manager.update_shared_folders().await;
        shinkai_log(
            ShinkaiLogOption::ExtSubscriptions,
            ShinkaiLogLevel::Info,
            format!("ExternalSubscriberManager::update_shared_folders result: {:?}", result).as_str(),
        );

        manager
    }

    pub async fn get_cached_shared_folder_tree(&mut self, path: &str) -> Vec<SharedFolderInfo> {
        let now = Utc::now();
        {
            let last_refresh_lock = self.last_refresh.lock().await;
            if now.signed_duration_since(*last_refresh_lock).num_minutes() >= 5 {
                // Drop the lock explicitly before the mutable borrow
                drop(last_refresh_lock);
                // Now `self` can be mutably borrowed because the immutable borrow has ended
                let _ = self.update_shared_folders().await;
                // Re-acquire the lock to update the last refresh time
                *self.last_refresh.lock().await = now;
            }
        } // The lock is dropped here if not already dropped

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
                .unwrap_or_default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_subscription_request_state_updates(
        job_queue_manager: Arc<Mutex<JobQueueManager<SubscriptionWithTree>>>,
        db: Weak<ShinkaiDB>,
        _: Weak<VectorFS>, // vector_fs
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        identity_manager: Weak<Mutex<IdentityManager>>,
        _: Arc<DashMap<String, SharedFolderInfo>>, // shared_folders_tree
        subscription_ids_are_sync: Arc<DashMap<String, (String, usize)>>,
        shared_folders_to_ephemeral_versioning: Arc<DashMap<String, usize>>,
        _: usize, // tread_number
    ) -> tokio::task::JoinHandle<()> {
        let job_queue_manager = Arc::clone(&job_queue_manager);
        let interval_minutes = env::var("SUBSCRIPTION_PROCESS_INTERVAL_MINUTES")
            .unwrap_or("5".to_string()) // Default to 5 minutes if not set
            .parse::<u64>()
            .unwrap_or(5);

        let is_testing = env::var("IS_TESTING").ok().map(|v| v == "1").unwrap_or(false);

        tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Info,
                "process_subscription_request_state_updates> Starting subscribers processing loop",
            );

            if is_testing {
                // Wait until subscription_ids_are_sync has at least 1 value
                loop {
                    if !subscription_ids_are_sync.is_empty() {
                        shinkai_log(
                            ShinkaiLogOption::ExtSubscriptions,
                            ShinkaiLogLevel::Debug,
                            format!(
                                "IS TESTING> subscription_ids_are_sync is not empty: {:?}. Starting loop...",
                                subscription_ids_are_sync
                            )
                            .as_str(),
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        break;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
            }

            loop {
                // Game Plan:
                // Phase 1
                // 1. Find all subscriptions
                // 2. Filter out subscriptions that are already on sync
                // 3. Request subscribers folder state (async)

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
                    // print node name
                    eprintln!(
                        ">> (process_subscription_request_state_updates) Sending request to subscriber: {:?}",
                        subscription_id.get_unique_id().to_string()
                    );
                    eprintln!("Node name: {:?}", node_name);
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

                // Note: maybe we could treat every send as its own future and then join them all in group batches
                // let handles_to_join = mem::replace(&mut handles, Vec::new());
                // futures::future::join_all(handles_to_join).await;
                // handles.clear();

                // Wait for interval_minutes before the next iteration
                tokio::time::sleep(tokio::time::Duration::from_secs(interval_minutes * 60)).await;
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn process_subscription_job_message_queued(
        subscription_with_tree: SubscriptionWithTree,
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        maybe_identity_manager: Weak<Mutex<IdentityManager>>,
        shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
        subscription_ids_are_sync: Arc<DashMap<String, (String, usize)>>,
        shared_folders_to_ephemeral_versioning: Arc<DashMap<String, usize>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, SubscriberManagerError>> + Send + 'static>> {
        Box::pin(async move {
            let shared_folder = subscription_with_tree.subscription.shared_folder.clone();
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Debug,
                format!(
                    "Processing subscription: {:?} with shared folder: {:?}",
                    subscription_with_tree.subscription.subscription_id.get_unique_id(),
                    shared_folder
                )
                .as_str(),
            );

            let local_shared_folder_state = {
                let key_shared_folder = format!(
                    "{}{}",
                    subscription_with_tree.subscription.streaming_profile, shared_folder
                );
                shinkai_log(
                    ShinkaiLogOption::ExtSubscriptions,
                    ShinkaiLogLevel::Debug,
                    format!(
                        "process_subscription_job_message_queued Key shared folder: {:?}",
                        key_shared_folder
                    )
                    .as_str(),
                );
                let local_shared_folder_state = shared_folders_trees
                    .get(&key_shared_folder)
                    .map_or(FSEntryTree::new_empty(), |entry| entry.value().tree.clone());
                if local_shared_folder_state.is_empty() {
                    shinkai_log(
                        ShinkaiLogOption::ExtSubscriptions,
                        ShinkaiLogLevel::Error,
                        "Local shared folder state is empty",
                    );
                    return Err(SubscriberManagerError::InvalidRequest(
                        "Local shared folder state is empty".to_string(),
                    ));
                } else {
                    local_shared_folder_state
                }
            };

            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Debug,
                format!("Local shared folder state: {:?}", local_shared_folder_state).as_str(),
            );
            // Calculate diff
            let diff = FSEntryTreeGenerator::compare_fs_item_trees(
                &subscription_with_tree.subscriber_folder_tree,
                &local_shared_folder_state,
            );
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Debug,
                format!("Diff: {:?}", diff).as_str(),
            );

            // If at least one diff was found, retrieve the VRPack for the path
            if !diff.children.is_empty() {
                shinkai_log(
                    ShinkaiLogOption::ExtSubscriptions,
                    ShinkaiLogLevel::Debug,
                    "Diff found, sending VRPack to subscriber",
                );
                // Note: for now we are just going to get everything
                // TODO: In a later update we will be more granular

                let vector_fs_inst = vector_fs.upgrade().ok_or(SubscriberManagerError::VectorFSNotAvailable(
                    "VectorFS instance is not available".to_string(),
                ))?;

                let vr_path_shared_folder = VRPath::from_string(&shared_folder)
                    .map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

                println!("Shared folder path: {}", vr_path_shared_folder);

                // Use the origin profile subidentity for both Reader inputs to only fetch all paths with public (or whitelist later) read perms without issues.
                let subscription_id = subscription_with_tree.subscription.subscription_id.clone();
                shinkai_log(
                    ShinkaiLogOption::ExtSubscriptions,
                    ShinkaiLogLevel::Debug,
                    format!("Processing subscription: {:?}", subscription_id).as_str(),
                );
                let streamer = subscription_id.extract_streamer_node_with_profile()?;
                let subscriber = subscription_id.extract_subscriber_node_with_profile()?;

                let reader = vector_fs_inst
                    .new_reader(subscriber.clone(), vr_path_shared_folder, streamer.clone())
                    .await?;
                let vr_pack = vector_fs_inst.retrieve_vrpack(&reader).await.unwrap();

                if let Some(identity_manager_lock) = maybe_identity_manager.upgrade() {
                    let identity_manager = identity_manager_lock.lock().await;
                    let standard_identity = identity_manager
                        .external_profile_to_global_identity(
                            &subscription_with_tree
                                .subscription
                                .subscriber_node
                                .get_node_name_string(),
                        )
                        .await?;
                    drop(identity_manager);

                    let result = Self::send_vr_pack_to_peer(
                        vr_pack,
                        subscription_id.clone(),
                        standard_identity,
                        subscription_with_tree.symmetric_key,
                    )
                    .await;

                    shinkai_log(
                        ShinkaiLogOption::ExtSubscriptions,
                        ShinkaiLogLevel::Debug,
                        format!(
                            "Sending VRPack to subscriber: {:?} with result: {:?}",
                            subscription_id, result
                        )
                        .as_str(),
                    );

                    // TODO: Update db with last modified or something?
                } else {
                    shinkai_log(
                        ShinkaiLogOption::ExtSubscriptions,
                        ShinkaiLogLevel::Error,
                        "Identity manager is not available",
                    );
                    return Err(SubscriberManagerError::IdentityManagerUnavailable);
                }
            }

            Ok(format!(
                "Job {} processed successfully",
                subscription_with_tree.subscription.subscription_id.get_unique_id()
            ))
        })
    }

    pub async fn send_vr_pack_to_peer(
        vr_pack: VRPack,
        subscription_id: SubscriptionId,
        receiver_identity: StandardIdentity,
        symmetric_key: String,
    ) -> Result<(), SubscriberManagerError> {
        // Extract the receiver's socket address and profile name from the StandardIdentity
        let receiver_socket_addr = receiver_identity.addr.ok_or_else(|| {
            SubscriberManagerError::AddressUnavailable(
                format!(
                    "Shinkai ID doesn't have a valid socket address: {}",
                    receiver_identity.full_identity_name.extract_node()
                )
                .to_string(),
            )
        })?;

        shinkai_log(
            ShinkaiLogOption::ExtSubscriptions,
            ShinkaiLogLevel::Info,
            format!("Sending VRPack to subscriber: {:?}", subscription_id).as_str(),
        );

        // Call the send_encrypted_vrkaipath_pairs function
        Node::send_encrypted_vrpack(vr_pack, subscription_id, symmetric_key, receiver_socket_addr).await;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn process_subscription_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<SubscriptionWithTree>>>,
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
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
                Weak<ShinkaiDB>,
                Weak<VectorFS>,
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
        let mut receiver = job_queue_manager.lock().await.subscribe_to_all().await;
        let processing_jobs = Arc::new(Mutex::new(HashSet::new()));
        let semaphore = Arc::new(Semaphore::new(thread_number));
        let process_job = Arc::new(process_job);

        tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Info,
                "process_subscription_queue> Starting subscribers processing loop",
            );

            let mut handles = Vec::new();
            loop {
                let mut continue_immediately = false;
                // Phase 2
                // 4. Calc. diff and schedule network requests (async) -> Process

                // Start Phase 2: We check current jobs that are ready to go
                let job_ids_to_perform_comparisons_and_send_files = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_lock = job_queue_manager.lock().await;
                    let all_jobs = job_queue_manager_lock.get_all_elements_interleave().await;
                    drop(job_queue_manager_lock);

                    let filtered_jobs = all_jobs
                        .unwrap_or(Vec::new())
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
                    eprintln!(
                        ">> (process_subscription_queue) Processing subscription_id: {:?}",
                        subscription_id
                    );
                    let job_queue_manager = Arc::clone(&job_queue_manager);
                    let processing_jobs = Arc::clone(&processing_jobs);
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
                            job_queue_manager.peek(&subscription_id).await
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
                                        .dequeue(job.subscription.subscription_id.clone().get_unique_id())
                                        .await
                                    {
                                        result
                                    } else {
                                        Err(SubscriberManagerError::OperationFailed(format!(
                                            "Failed to dequeue job: {}",
                                            job.subscription.subscription_id.clone().get_unique_id()
                                        )))
                                    }
                                };
                                match result {
                                    Ok(_) => {
                                        shinkai_log(
                                            ShinkaiLogOption::ExtSubscriptions,
                                            ShinkaiLogLevel::Debug,
                                            "process_subscription_queue: Job processed successfully",
                                        );
                                    } // handle success case
                                    Err(_) => {
                                        shinkai_log(
                                            ShinkaiLogOption::ExtSubscriptions,
                                            ShinkaiLogLevel::Error,
                                            "Job processing failed",
                                        );
                                    } // handle error case
                                }
                            }
                            Ok(None) => {}
                            Err(_) => {
                                // Log the error
                            }
                        }
                        drop(_permit);
                        processing_jobs.lock().await.remove(&subscription_id);
                    });
                    handles.push(handle);
                }

                let handles_to_join = std::mem::take(&mut handles);
                futures::future::join_all(handles_to_join).await;
                handles.clear();

                // If job_ids_to_process was equal to max_parallel_jobs, loop again immediately
                // without waiting for a new job from receiver.recv().await
                if continue_immediately {
                    continue;
                }

                // Receive new jobs
                if let Some(new_job) = receiver.recv().await {
                    shinkai_log(
                        ShinkaiLogOption::ExtSubscriptions,
                        ShinkaiLogLevel::Info,
                        format!(
                            "Received new subscription job {:?}",
                            new_job.subscription.subscription_id.clone().get_unique_id().to_string()
                        )
                        .as_str(),
                    );
                }
            }
        })
    }

    pub async fn update_shared_folders(&mut self) -> Result<(), SubscriberManagerError> {
        let profiles = {
            let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;
            let identities = db
                .get_all_profiles(self.node_name.clone())
                .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;
            identities
                .iter()
                .filter_map(|i| i.full_identity_name.clone().get_profile_name_string())
                .collect::<Vec<String>>()
        };

        for profile in profiles {
            let result = self
                .available_shared_folders(
                    self.node_name.clone(),
                    profile.clone(),
                    self.node_name.clone(),
                    profile.clone(),
                    "/".to_string(),
                )
                .await;
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Debug,
                format!(
                    "ExternalSubscriberManager::update_shared_folders for profile {:?} result: {:?}",
                    profile, result
                )
                .as_str(),
            );
        }

        Ok(())
    }

    pub async fn available_shared_folders(
        &mut self,
        streamer_node: ShinkaiName,
        streamer_profile: String,
        requester_node: ShinkaiName,
        requester_profile: String,
        path: String,
    ) -> Result<Vec<SharedFolderInfo>, SubscriberManagerError> {
        if streamer_profile.is_empty() {
            return Err(SubscriberManagerError::InvalidRequest(
                "Streamer profile cannot be empty".to_string(),
            ));
        };

        let full_requester_profile_subidentity =
            ShinkaiName::from_node_and_profile_names(requester_node.node_name, requester_profile)?;
        let full_streamer_profile_subidentity =
            ShinkaiName::from_node_and_profile_names(streamer_node.node_name, streamer_profile.clone())?;

        let mut converted_results = Vec::new();
        {
            let vector_fs = self
                .vector_fs
                .upgrade()
                .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                    "VectorFS instance is not available".to_string(),
                ))?;

            let vr_path =
                VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

            // Use the origin profile subidentity for both Reader inputs to only fetch all paths with public (or whitelist later) read perms without issues.
            let perms_reader = vector_fs
                .new_reader(
                    full_requester_profile_subidentity.clone(),
                    vr_path,
                    full_streamer_profile_subidentity.clone(),
                )
                .await
                .map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;
            let results = vector_fs
                .find_paths_with_read_permissions(&perms_reader, vec![ReadPermission::Public])
                .await?;
            eprintln!("available_shared_folders> results: {:?}", results);

            // Use the new function to filter results to only include top-level folders
            let filtered_results = FSEntryTreeGenerator::filter_to_top_level_folders(results);
            eprintln!("available_shared_folders> filtered_results: {:?}", filtered_results);

            // Drop the lock on vector_fs before proceeding
            drop(vector_fs);

            let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;

            for (path, permission) in filtered_results {
                let path_str = path.to_string();
                let permission_str = format!("{:?}", permission);
                let subscription_requirement = match db.get_folder_requirements(&path_str) {
                    Ok(req) => Some(req),
                    Err(_) => None, // Instead of erroring out, we return None for folders without requirements
                };
                let tree = FSEntryTreeGenerator::shared_folders_to_tree(
                    self.vector_fs.clone(),
                    full_streamer_profile_subidentity.clone(),
                    full_requester_profile_subidentity.clone(),
                    path_str.clone(),
                )
                .await?;

                let result = SharedFolderInfo {
                    path: path_str.clone(),
                    permission: permission_str,
                    tree,
                    subscription_requirement,
                };
                eprintln!("SharedFolderInfo> result: {:?}", result);

                let shared_folder_key = format!("{}{}", streamer_profile, path_str);

                // Check if the value of shared_folders_trees is different than the new value inserted
                let should_update_version = self
                    .shared_folders_trees
                    .get(&shared_folder_key)
                    .map_or(true, |existing| *existing.value() != result);

                if should_update_version {
                    // Update shared_folders_to_ephemeral_versioning
                    self.shared_folders_to_ephemeral_versioning
                        .entry(shared_folder_key.clone())
                        .and_modify(|e| *e += 1)
                        .or_insert(1); // the first version starts at one
                }

                converted_results.push(result.clone());

                self.shared_folders_trees.insert(shared_folder_key, result);
                eprintln!(
                    "available_shared_folders> shared_folders_trees: {:?}",
                    self.shared_folders_trees
                );
            }
        }
        eprintln!("available_shared_folders> converted_results: {:?}", converted_results);

        Ok(converted_results)
    }

    pub async fn update_shareable_folder_requirements(
        &self,
        path: String,
        requester_shinkai_identity: ShinkaiName,
        subscription_requirement: FolderSubscription,
    ) -> Result<bool, SubscriberManagerError> {
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;

        let vr_path = VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;
        let result = vector_fs
            .get_path_permission_for_paths(requester_shinkai_identity.clone(), vec![vr_path])
            .await?;

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
        {
            let vector_fs = self
                .vector_fs
                .upgrade()
                .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                    "VectorFS instance is not available".to_string(),
                ))?;

            let vr_path =
                VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;
            let writer = vector_fs
                .new_writer(
                    requester_shinkai_identity.clone(),
                    vr_path.clone(),
                    requester_shinkai_identity.clone(),
                )
                .await?;

            // Retrieve the current write permissions for the path
            let permissions_vector = vector_fs
                .get_path_permission_for_paths(requester_shinkai_identity.clone(), vec![vr_path.clone()])
                .await?;

            if permissions_vector.is_empty() {
                return Err(SubscriberManagerError::InvalidRequest(
                    "Path does not exist".to_string(),
                ));
            }

            let (_, current_permissions) = permissions_vector.into_iter().next().unwrap();

            // Set the read permissions to Public while reusing the write permissions
            let result = vector_fs
                .update_permissions_recursively(&writer, ReadPermission::Public, current_permissions.write_permission)
                .await;
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Debug,
                format!(
                    "create_shareable_folder> update_permissions_recursively result: {:?}",
                    result
                )
                .as_str(),
            );
            result?
        }
        {
            // Assuming we have validated the admin and permissions, we proceed to update the DB
            let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;

            db.set_folder_requirements(&path, subscription_requirement)
                .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;
        }

        // Trigger a refresh of the shareable folders cache
        let requester_profile = requester_shinkai_identity.get_profile_name_string().ok_or(
            SubscriberManagerError::IdentityProfileNotFound("Profile name not found".to_string()),
        )?;

        let _ = self
            .available_shared_folders(
                requester_shinkai_identity.clone(),
                requester_profile.clone(),
                requester_shinkai_identity.clone(),
                requester_profile.clone(),
                "/".to_string(),
            )
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

            // Retrieve the current permissions for the path
            let permissions_vector = vector_fs
                .get_path_permission_for_paths(requester_shinkai_identity.clone(), vec![VRPath::from_string(&path)?])
                .await?;

            if permissions_vector.is_empty() {
                return Err(SubscriberManagerError::InvalidRequest(
                    "Path does not exist".to_string(),
                ));
            }

            let (vr_path, current_permissions) = permissions_vector.into_iter().next().unwrap();

            // Create a writer for the path
            let writer = vector_fs
                .new_writer(
                    requester_shinkai_identity.clone(),
                    vr_path,
                    requester_shinkai_identity.clone(),
                )
                .await?;

            // Set the read permissions to Private while reusing the write permissions using update_permissions_recursively
            vector_fs
                .update_permissions_recursively(&writer, ReadPermission::Private, current_permissions.write_permission)
                .await?;
        }
        {
            // Assuming we have validated the admin and permissions, we proceed to update the DB
            let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;
            db.remove_folder_requirements(&path)
                .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;
        }

        if let Some(profile_name) = requester_shinkai_identity.get_profile_name_string() {
            let key_shared_folder = format!("{}{}", profile_name, path);
            self.shared_folders_trees.remove(&key_shared_folder);
            Ok(true)
        } else {
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Error,
                format!("Profile name not found for {:?}", requester_shinkai_identity).as_str(),
            );
            Err(SubscriberManagerError::VectorFSNotAvailable(
                "Profile name not found".to_string(),
            ))
        }
    }

    pub async fn subscribe_to_shared_folder(
        &mut self,
        requester_shinkai_identity: ShinkaiName,
        streamer_shinkai_identity: ShinkaiName,
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

        let requester_profile = requester_shinkai_identity.get_profile_name_string().ok_or(
            SubscriberManagerError::IdentityProfileNotFound("Profile name not found for requester".to_string()),
        )?;
        let streamer_profile = streamer_shinkai_identity.get_profile_name_string().ok_or(
            SubscriberManagerError::IdentityProfileNotFound("Profile name not found for origin".to_string()),
        )?;

        let subscription_id = SubscriptionId::new(
            streamer_shinkai_identity.extract_node(),
            streamer_profile.clone(),
            shared_folder.clone(),
            requester_shinkai_identity.extract_node(),
            requester_profile.clone(),
        );

        // The requester has passed the validation checks
        // Proceed to add the requester to the list of subscribers
        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;

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
            streamer_shinkai_identity.extract_node(),
            streamer_profile,
            requester_shinkai_identity.extract_node(),
            requester_profile,
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

    /// Unsubscribe from a shared folder
    /// This function will remove the subscription from the database, but will not remove already scheduled actions.
    pub async fn unsubscribe_from_shared_folder(
        &mut self,
        requester_shinkai_identity: ShinkaiName,
        streamer_shinkai_identity: ShinkaiName,
        shared_folder: String,
    ) -> Result<bool, SubscriberManagerError> {
        let requester_profile = requester_shinkai_identity.get_profile_name_string().ok_or(
            SubscriberManagerError::IdentityProfileNotFound("Profile name not found for requester".to_string()),
        )?;
        let streamer_profile = streamer_shinkai_identity.get_profile_name_string().ok_or(
            SubscriberManagerError::IdentityProfileNotFound("Profile name not found for streamer".to_string()),
        )?;

        let subscription_id = SubscriptionId::new(
            streamer_shinkai_identity.extract_node(),
            streamer_profile.clone(),
            shared_folder.clone(),
            requester_shinkai_identity.extract_node(),
            requester_profile.clone(),
        );

        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;

        match db.remove_subscriber(&subscription_id) {
            Ok(_) => {
                // Successfully unsubscribed
                shinkai_log(
                    ShinkaiLogOption::ExtSubscriptions,
                    ShinkaiLogLevel::Info,
                    &format!(
                        "Successfully unsubscribed from shared folder: {} for subscription ID: {}",
                        shared_folder,
                        subscription_id.get_unique_id()
                    ),
                );

                // Optionally, remove from the subscription_ids_are_sync map if needed
                let subscription_id_str = subscription_id.get_unique_id().to_string();
                self.subscription_ids_are_sync.remove(&subscription_id_str);

                Ok(true)
            }
            Err(e) => {
                // Handle error
                shinkai_log(
                    ShinkaiLogOption::ExtSubscriptions,
                    ShinkaiLogLevel::Error,
                    &format!(
                        "Failed to unsubscribe from shared folder: {}. Error: {:?}",
                        shared_folder, e
                    ),
                );
                Err(SubscriberManagerError::DatabaseError(e.to_string()))
            }
        }
    }

    pub async fn create_and_send_request_updated_state(
        subscription_id: SubscriptionId,
        db: Weak<ShinkaiDB>,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        node_name: ShinkaiName,
        maybe_identity_manager: Weak<Mutex<IdentityManager>>,
    ) -> Result<(), SubscriberManagerError> {
        let subscription = {
            let db = db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;

            let subscription = db.get_subscription_by_id(&subscription_id).map_err(|e| match e {
                ShinkaiDBError::DataNotFound => SubscriberManagerError::SubscriptionNotFound(format!(
                    "Subscription with ID {} not found",
                    subscription_id.get_unique_id()
                )),
                _ => SubscriberManagerError::DatabaseError(e.to_string()),
            });
            subscription?
        };

        // Create message to request updated state
        if let Some(identity_manager_lock) = maybe_identity_manager.upgrade() {
            let subscriber_node = subscription.subscriber_node.clone();
            let identity_manager = identity_manager_lock.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&subscriber_node.get_node_name_string())
                .await?;
            drop(identity_manager);

            let receiver_public_key = standard_identity.node_encryption_public_key;

            // Update to use SubscriptionRequiresTreeUpdateResponse instead
            let msg_request_subscription = ShinkaiMessageBuilder::vecfs_request_share_current_shared_folder_state(
                subscription.shared_folder.clone(),
                clone_static_secret_key(&my_encryption_secret_key),
                clone_signature_secret_key(&my_signature_secret_key),
                receiver_public_key,
                node_name.get_node_name_string(),
                subscription.streaming_profile.clone(),
                subscriber_node.get_node_name_string(),
                subscription.subscriber_profile.clone(),
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

    pub async fn get_node_subscribers(
        &self,
        path: Option<String>,
    ) -> Result<HashMap<String, Vec<ShinkaiSubscription>>, SubscriberManagerError> {
        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;

        let subscriptions = if let Some(ref path) = path {
            if path != "/" {
                // Use db.all_subscribers_for_folder when path is defined and not "/"
                db.all_subscribers_for_folder(path)
                    .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?
            } else {
                // When path is "/", treat it the same as if path is None
                db.all_subscribers_subscription()
                    .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?
            }
        } else {
            // When path is None, get all subscriptions and then group by folder
            db.all_subscribers_subscription()
                .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?
        };

        let mut subscribers_by_path: HashMap<String, Vec<ShinkaiSubscription>> = HashMap::new();

        for subscription in subscriptions {
            subscribers_by_path
                .entry(subscription.shared_folder.clone())
                .or_default()
                .push(subscription);
        }

        Ok(subscribers_by_path)
    }

    pub async fn subscriber_current_state_response(
        &self,
        subscription_unique_id: String,
        subscriber_folder_tree: FSEntryTree,
        subscriber_node: ShinkaiName,
        subscriber_profile: String,
        symmetric_key: String,
    ) -> Result<(), SubscriberManagerError> {
        shinkai_log(
            ShinkaiLogOption::ExtSubscriptions,
            ShinkaiLogLevel::Debug,
            &format!(
                "Received current state response for subscription ID: {}, from subscriber: {}. Tree: {:?}",
                subscription_unique_id, subscriber_node, subscriber_folder_tree
            ),
        );

        let subscription = {
            // Validate Subscription Exists and that Requesting Node matches the subscription
            let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                "Database instance is not available".to_string(),
            ))?;

            let subscription_id = SubscriptionId::from_unique_id(subscription_unique_id.clone());
            db.get_subscription_by_id(&subscription_id).map_err(|e| match e {
                ShinkaiDBError::DataNotFound => SubscriberManagerError::SubscriptionNotFound(format!(
                    "Subscription with ID {} not found",
                    subscription_unique_id
                )),
                _ => SubscriberManagerError::DatabaseError(e.to_string()),
            })?
        };

        if subscription.subscriber_node.get_node_name_string() != subscriber_node.get_node_name_string() {
            return Err(SubscriberManagerError::InvalidSubscriber(
                "Subscriber node does not match the subscription".to_string(),
            ));
        }
        if subscription.subscriber_profile != subscriber_profile {
            return Err(SubscriberManagerError::InvalidSubscriber(
                "Subscriber profile does not match the subscription".to_string(),
            ));
        }

        let subscription_id_clone = subscription.subscription_id.clone();
        let unique_id = subscription_id_clone.get_unique_id();
        let subscription_with_tree = SubscriptionWithTree {
            subscription,
            subscriber_folder_tree,
            symmetric_key,
        };

        {
            let mut queue_manager = self.subscriptions_queue_manager.lock().await;
            if queue_manager.peek(unique_id).await?.is_none() {
                let _ = queue_manager.push(unique_id, subscription_with_tree).await;
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
        assert!(!subscription_requirement.is_free);
    }
}
