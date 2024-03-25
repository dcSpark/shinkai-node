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
use std::cmp::Ordering;
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
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
    pub node_name: ShinkaiName,
    pub subscriptions_queue_manager: Arc<Mutex<JobQueueManager<SubscriptionWithTree>>>,
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
        let subscriptions_queue = JobQueueManager::<SubscriptionWithTree>::new(
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

    fn process_job_message_queued(
        subscription_with_tree: SubscriptionWithTree,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, SubscriberManagerError>> + Send + 'static>> {
        Box::pin(async move {
            // Placeholder logic for processing a queued job message
            println!("Processing job: {:?}", subscription_with_tree.subscription.subscription_id);

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
        shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
        thread_number: usize,
        process_job: impl Fn(
                SubscriptionWithTree,
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
                        Ok(subscriptions) => {
                            // Note: this extract and sort subscriptions by folder
                            // to avoid just focusing in the one user with multiple subscriptions
                            // this also open the doors for us to implement some cache for specific sub-folders and items
                            let mut subscriptions_with_folders: Vec<(String, SubscriptionId)> = subscriptions
                                .into_iter()
                                .map(|s| {
                                    // Extract the shared folder for each subscription ID
                                    let shared_folder = s.subscription_id.extract_shared_folder().unwrap_or_default();
                                    (shared_folder, s.subscription_id)
                                })
                                .collect();

                            // Sort the vector by shared_folder
                            subscriptions_with_folders.sort_by(|a, b| a.0.cmp(&b.0));

                            // Extract and collect the SubscriptionId in sorted order
                            subscriptions_with_folders.into_iter().map(|(_, id)| id).collect()
                        }
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

                // Process each subscription ID
                for subscription_id in subscriptions_ids_to_process {
                    let semaphore = semaphore.clone();
                    let db = db.clone();
                    let vector_fs = vector_fs.clone();
                    let shared_folders_trees = shared_folders_trees.clone();
                    let process_job = process_job.clone();

                    let handle = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.expect("Failed to acquire semaphore permit");

                        let subscription = {
                            let db = match db.upgrade() {
                                Some(db) => db,
                                None => {
                                    shinkai_log(
                                        ShinkaiLogOption::ExtSubscriptions,
                                        ShinkaiLogLevel::Error,
                                        "Database instance is not available",
                                    );
                                    return;
                                }
                            };
                            let db = db.lock().await;
                            match db.get_subscription_by_id(&subscription_id) {
                                Ok(subscription) => subscription,
                                Err(e) => {
                                    shinkai_log(
                                        ShinkaiLogOption::ExtSubscriptions,
                                        ShinkaiLogLevel::Error,
                                        &format!("Failed to fetch subscription: {:?}", e),
                                    );
                                    return;
                                }
                            }
                        };

                        let subscription_with_tree = SubscriptionWithTree {
                            subscription,
                            subscriber_folder_tree: FSItemTree {
                                name: "/".to_string(),
                                path: "/".to_string(),
                                last_modified: Utc::now(),
                                children: HashMap::new(),
                            },
                        };

                        let _ = process_job(
                            subscription_with_tree,
                            db.clone(),
                            vector_fs.clone(),
                            shared_folders_trees.clone(),
                        )
                        .await
                        .unwrap_or_else(|e| {
                            shinkai_log(
                                ShinkaiLogOption::ExtSubscriptions,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to process job: {:?}", e),
                            );
                            "Failed to process job".to_string()
                        });
                        drop(_permit);
                    });
                    handles.push(handle);
                }

                let handles_to_join = mem::replace(&mut handles, Vec::new());
                futures::future::join_all(handles_to_join).await;
                handles.clear();

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
