use crate::agent::queue::job_queue_manager::JobQueueManager;
use crate::db::db_errors::ShinkaiDBError;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::network::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use crate::network::Node;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_error::VectorFSError;
use crate::vector_fs::vector_fs_permissions::ReadPermission;
use crate::vector_fs::vector_fs_types::{FSEntry, FSFolder, FSItem};
use chrono::NaiveDateTime;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures::Future;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionStatus, SubscriptionId,
};
use shinkai_message_primitives::schemas::shinkai_subscription_req::{FolderSubscription, SubscriptionPayment};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_vector_resources::vector_resource::VRPath;
use std::collections::HashMap;
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Arc;
use std::sync::Weak;
use std::{env, mem};
use tokio::sync::{Mutex, MutexGuard, Semaphore};

use super::fs_item_tree::FSItemTree;

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
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
    pub node_name: ShinkaiName,
    pub subscriptions_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
    pub subscription_processing_task: Option<tokio::task::JoinHandle<()>>,
}

impl ExternalSubscriberManager {
    pub async fn new(
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        identity_manager: Weak<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
    ) -> Self {
        let db_prefix = "subscriptions_abcprefix_"; // dont change it
        let subscriptions_queue = JobQueueManager::<ShinkaiSubscription>::new(
            db.clone(),
            Topic::AnyQueuesPrefixed.as_str(),
            Some(db_prefix.to_string()),
        )
        .await
        .unwrap();
        let subscriptions_queue_manager = Arc::new(Mutex::new(subscriptions_queue));
        let shared_folders_trees = Arc::new(DashMap::new());

        let thread_number = env::var("SUBSCRIBER_MANAGER_NETWORK_CONCURRENCY")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS); // Start processing the job queue

        let subscription_queue_handler = ExternalSubscriberManager::process_subscription_queue(
            subscriptions_queue_manager.clone(),
            db.clone(),
            vector_fs.clone(),
            shared_folders_trees.clone(),
            thread_number,
            |job, db, vector_fs, shared_folders_trees| {
                ExternalSubscriberManager::process_job_message_queued(job, db, vector_fs, shared_folders_trees)
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
            node_name,
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

    // Placeholder for process_job_message_queued
    // Correct the return type of the function to match the expected type
    // TODO: Remove? I don't think I need this. it could be helpful for testing?
    fn process_job_message_queued(
        job: ShinkaiSubscription,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, SubscriberManagerError>> + Send + 'static>> {
        Box::pin(async move {
            // Placeholder logic for processing a queued job message
            println!("Processing job: {:?}", job.subscription_id);

            // Simulate some processing work
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Log completion of job processing
            println!("Completed processing job: {:?}", job.subscription_id);

            // Assuming the processing is successful, return Ok with a message
            // Adjust according to actual logic and possible error conditions
            Ok(format!(
                "Job {} processed successfully",
                job.subscription_id.get_unique_id()
            ))
        })
    }

    pub async fn process_subscription_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
        thread_number: usize,
        // TODO: probably we want to pass a Weak Mutex of something in memory that we can use to store the state of the shared_folders
        process_job: impl Fn(
                ShinkaiSubscription,
                Weak<Mutex<ShinkaiDB>>,
                Weak<Mutex<VectorFS>>,
                Arc<DashMap<String, SharedFolderInfo>>,
            ) -> Pin<Box<dyn Future<Output = Result<String, SubscriberManagerError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        // let job_queue_manager = Arc::clone(&job_queue_manager); // not needed?
        let semaphore = Arc::new(Semaphore::new(thread_number));
        let process_job = Arc::new(process_job);

        let interval_minutes = env::var("SUBSCRIPTION_PROCESS_INTERVAL_MINUTES")
            .unwrap_or("5".to_string()) // Default to 5 minutes if not set
            .parse::<u64>()
            .unwrap_or(5);

        tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Info,
                "Starting subscribers processing loop",
            );

            let mut handles = Vec::new();
            loop {
                // TODO: we want to get all the subscribers id and process them

                // // Scope for acquiring and releasing the lock quickly
                // let subscriptions_ids_to_process: Vec<String> = {
                //     // acquire the lock of the db
                //     let shinkai_db = db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
                //         "Database instance is not available".to_string(),
                //     ))?;
                //     let db = shinkai_db.lock().await;
                //     let subscribers = db.get_subscribers().unwrap_or_default();
                //     subscribers.iter().map(|subscriber| subscriber.id.clone()).collect()

                // };

                for _ in 0..thread_number {
                    let job_queue_manager = job_queue_manager.clone();
                    let semaphore = semaphore.clone();
                    let db = db.clone();
                    let vector_fs = vector_fs.clone();
                    let shared_folders_trees = shared_folders_trees.clone();
                    let process_job = process_job.clone();

                    let handle = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.expect("Failed to acquire semaphore permit");

                        // match job_queue_manager.lock().await.dequeue("some_key").await {
                        //     Ok(Some(job)) => {
                        //         let result = process_job(job, db, vector_fs, shared_folders_trees).await;
                        //         if let Ok(Some(_)) = job_queue_manager.lock().await.dequeue(&job_id.clone()).await {
                        //             result
                        //         } else {
                        //             Err(SubscriberManagerError::)
                        //         }
                        //     }
                        //     Ok(None) => {
                        //         // No job to process, release the permit and exit the loop
                        //         drop(_permit);
                        //         return;
                        //     }
                        //     Err(err) => {
                        //         eprintln!("Error dequeuing job: {:?}", err);
                        //         // Error handling, release the permit and exit the loop
                        //         drop(_permit);
                        //         return;
                        //     }
                        // }
                    });
                    handles.push(handle);
                }

                // let handles_to_join = mem::replace(&mut handles, Vec::new());
                // futures::future::join_all(handles).await;
                // handles.clear();

                // Wait for interval_minutes before the next iteration
                tokio::time::sleep(tokio::time::Duration::from_secs(interval_minutes * 60)).await;
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
            let filtered_results = self.filter_to_top_level_folders(results);

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
                let tree = self
                    .shared_folders_to_tree(requester_shinkai_identity.clone(), path_str.clone())
                    .await?;

                let result = SharedFolderInfo {
                    path: path_str.clone(),
                    permission: permission_str,
                    tree,
                    subscription_requirement,
                };
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
            shared_folder,
            self.node_name.extract_node(),
            requester_shinkai_identity.extract_node(),
            ShinkaiSubscriptionStatus::SubscriptionConfirmed,
            Some(subscription_requirement),
        );

        db.add_subscriber_subscription(subscription)
            .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;

        Ok(true)
    }

    // Schedule the job to be processed. Update the last time it was processed.
    pub async fn process_action(
        &mut self,
        action: ShinkaiSubscription,
        profile: ShinkaiName,
    ) -> Result<String, SubscriberManagerError> {
        // TODO: Transform request to ShinkaiSubscription if it passes validation
        // -> Validation ->

        // it comes from the actual node (API side) -> it should be validated before it gets here
        // vector_db path is valid and "shareable" (here)

        // match action.action {
        //     ShinkaiSubscriptionStatus::SubscriptionRequested => {
        //         // Transform request to ShinkaiSubscription if it passes validation
        //         let subscription = ShinkaiSubscription {
        //             action: action.action,
        //             subscription_id: action.subscription_id,
        //             vector_db_path: action.vector_db_path.ok_or(SubscriberManagerError::InvalidRequest(
        //                 "vector_db_path is required".to_string(),
        //             ))?,
        //             subscriber_identity: profile,
        //             state: action.state,
        //             date_created: Utc::now(),
        //             last_modified: Utc::now(),
        //             last_sync: None,
        //         };

        //         // TODO: vector_db path is valid and public
        //         let vector_fs = self
        //             .vector_fs
        //             .upgrade()
        //             .ok_or(SubscriberManagerError::VectorFSNotAvailable(
        //                 "VectorFS instance is not available".to_string(),
        //             ))?;
        //         let vector_fs = vector_fs.lock().await;

        // it's not already registered (here)
        // state is valid (here)
        // delegation is enough (here -> yeah we need to do logic about what is what in terms of shareables)

        // TODO: Add fn to add allowed vector_db paths for externals

        // -> Processing ->
        // add node to the subscription_db
        // schedule the job to be processed
        // Further processing and validation here

        Ok("Subscription processed".to_string())
        // }
        // Handle other actions as needed
        // _ => Err(SubscriberManagerError::InvalidRequest(
        //     "Unsupported action type".to_string(),
        // )),
        // }
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
