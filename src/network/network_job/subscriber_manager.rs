use crate::agent::queue::job_queue_manager::JobQueueManager;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::network::network_job::subscriber_manager_error::SubscriberManagerError;
use crate::network::Node;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_permissions::ReadPermission;
use crate::vector_fs::vector_fs_types::{FSEntry, FSFolder, FSItem};
use chrono::{DateTime, Utc};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionAction, ShinkaiSubscriptionRequest,
};
use shinkai_message_primitives::schemas::shinkai_subscription_req::ShinkaiSubscriptionReq;
use shinkai_vector_resources::vector_resource::VRPath;
use std::collections::HashMap;
use std::env;
use std::result::Result::Ok;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::{Mutex, MutexGuard};

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
#[derive(Debug, Clone)]
pub struct FSItemTree {
    pub name: String,
    pub path: String,
    pub last_modified: DateTime<Utc>,
    pub children: HashMap<String, Arc<FSItemTree>>,
}
/// - Temp

// Who decides to split it? a cron? the SubscriberManager that checks the state? -> The subscriber manager

const NUM_THREADS: usize = 2;

pub struct SubscriberManager {
    pub node: Weak<Mutex<Node>>,
    pub db: Weak<Mutex<ShinkaiDB>>,
    pub vector_fs: Weak<Mutex<VectorFS>>,
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub subscriptions_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
    pub subscription_processing_task: Option<tokio::task::JoinHandle<()>>,
}

impl SubscriberManager {
    pub async fn new(
        node: Weak<Mutex<Node>>,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        identity_manager: Weak<Mutex<IdentityManager>>,
    ) -> Self {
        let subscriptions_queue =
            JobQueueManager::<ShinkaiSubscription>::new(db.clone(), Topic::Subscriptions.as_str())
                .await
                .unwrap();
        let subscriptions_queue_manager = Arc::new(Mutex::new(subscriptions_queue));

        let thread_number = env::var("SUBSCRIBER_MANAGER_NETWORK_CONCURRENCY")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS); // Start processing the job queue

        let subscription_queue_handler = SubscriberManager::process_subscription_queue(
            subscriptions_queue_manager.clone(),
            db.clone(),
            vector_fs.clone(),
            thread_number,
            node.clone(),
            |job, db, vector_fs, node| SubscriberManager::process_job_message_queued(job, db, vector_fs, node),
        )
        .await;

        SubscriberManager {
            node,
            db,
            vector_fs,
            identity_manager,
            subscriptions_queue_manager,
            subscription_processing_task: Some(subscription_queue_handler),
        }
    }

    // WIP
    fn build_tree(items: &[FSItem], parent_path: &str) -> FSItemTree {
        let mut children: HashMap<String, Arc<FSItemTree>> = HashMap::new();

        for item in items {
            let item_path = item.path.to_string();
            if item_path.starts_with(parent_path) && item_path != parent_path {
                let child_name = item_path
                    .strip_prefix(parent_path)
                    .unwrap()
                    .trim_start_matches('/')
                    .to_string();
                let child_path = item_path;
                let last_modified = item.last_written_datetime;

                if child_name.contains('/') {
                    let child_name = child_name.split('/').next().unwrap().to_string();
                    if !children.contains_key(&child_name) {
                        children.insert(
                            child_name.clone(),
                            Arc::new(FSItemTree {
                                name: child_name.clone(),
                                path: format!("{}/{}", parent_path, child_name),
                                last_modified,
                                children: HashMap::new(),
                            }),
                        );
                    }
                } else {
                    children.insert(
                        child_name.clone(),
                        Arc::new(FSItemTree {
                            name: child_name,
                            path: child_path,
                            last_modified,
                            children: HashMap::new(),
                        }),
                    );
                }
            }
        }

        for child in children.values_mut() {
            *child = Arc::new(Self::build_tree(items, &child.path));
        }

        FSItemTree {
            name: parent_path.to_string(),
            path: parent_path.to_string(),
            last_modified: items
                .iter()
                .find(|item| item.path.to_string() == parent_path)
                .map(|item| item.last_written_datetime)
                .unwrap_or_else(|| Utc::now()),
            children,
        }
    }

    pub async fn shared_folders_to_tree(
        &self,
        requester_shinkai_identity: ShinkaiName,
        path: String,
    ) -> Result<FSItemTree, SubscriberManagerError> {
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;
        let mut vector_fs = vector_fs.lock().await;

        let vr_path = VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;
        let reader = vector_fs
            .new_reader(
                requester_shinkai_identity.clone(),
                vr_path,
                requester_shinkai_identity.clone(),
            )
            .map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

        // Step 1: Obtain all the shared folders
        let shared_folders = vector_fs
            .find_paths_with_read_permissions(&reader, vec![ReadPermission::Public, ReadPermission::Whitelist])?;

        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;
        let db = db.lock().await;

        let mut fs_items = Vec::new();

        for (path, _permission) in shared_folders {
            let path_str = path.to_string();

            // Step 2: Check the content of each shared folder
            let reader = vector_fs
                .new_reader(
                    requester_shinkai_identity.clone(),
                    path.clone(),
                    requester_shinkai_identity.clone(),
                )
                .map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

            let result = vector_fs.retrieve_fs_entry(&reader);
            let fs_entry = match result {
                Ok(fs_entry) => fs_entry,
                Err(e) => {
                    return Err(SubscriberManagerError::InvalidRequest(format!(
                        "Failed to retrieve fs entry: {}",
                        e
                    )));
                }
            };

            match fs_entry {
                FSEntry::Root(fs_root) => {
                    // Handle the root entry if needed
                }
                FSEntry::Folder(fs_folder) => {
                    // Recursively process the folder and its contents
                    let items = Self::process_folder(&fs_folder)?;
                    fs_items.extend(items);
                }
                FSEntry::Item(fs_item) => {
                    fs_items.push(fs_item);
                }
            }
        }

        // Step 3: Form the tree based on the obtained items
        let tree = Self::build_tree(&fs_items, &path);

        Ok(tree)
    }

    fn process_folder(fs_folder: &FSFolder) -> Result<Vec<FSItem>, SubscriberManagerError> {
        let mut items = Vec::new();

        // Recursively process the folder's child folders
        for child_folder in &fs_folder.child_folders {
            let sub_items = Self::process_folder(child_folder)?;
            items.extend(sub_items);
        }

        // Add the folder's child items
        items.extend(fs_folder.child_items.clone());

        Ok(items)
    }
    // end WIP

    // Placeholder for process_job_message_queued
    // Correct the return type of the function to match the expected type
    fn process_job_message_queued(
        job: ShinkaiSubscription,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        node: Weak<Mutex<Node>>,
    ) -> Box<dyn std::future::Future<Output = ()> + Send + 'static> {
        Box::new(async move {
            // Placeholder logic for processing a queued job message
            println!("Processing job: {:?}", job.subscription_id);

            // Simulate some processing work
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Log completion of job processing
            println!("Completed processing job: {:?}", job.subscription_id);
        })
    }

    pub async fn process_subscription_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        thread_number: usize,
        node: Weak<Mutex<Node>>,
        process_job: impl Fn(
            ShinkaiSubscription,
            Weak<Mutex<ShinkaiDB>>,
            Weak<Mutex<VectorFS>>,
            Weak<Mutex<Node>>,
        ) -> Box<dyn std::future::Future<Output = ()> + Send + 'static>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut handles = Vec::new();
            for _ in 0..thread_number {
                let job_queue_manager = job_queue_manager.clone();
                let db = db.clone();
                let vector_fs = vector_fs.clone();
                let node = node.clone();
                let handle = tokio::spawn(async move {
                    loop {
                        match job_queue_manager.lock().await.dequeue("some_key").await {
                            Ok(Some(job)) => {
                                "hey_replace_me".to_string();
                            }
                            Ok(None) => break,
                            Err(err) => {
                                eprintln!("Error dequeuing job: {:?}", err);
                                break;
                            }
                        }
                    }
                });
                handles.push(handle);
            }
            for handle in handles {
                handle.await.unwrap();
            }
        })
    }

    pub async fn available_shared_folders(
        &self,
        requester_shinkai_identity: ShinkaiName,
        path: String,
    ) -> Result<Vec<(String, String, Option<ShinkaiSubscriptionReq>)>, SubscriberManagerError> {
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;
        let mut vector_fs = vector_fs.lock().await;

        let vr_path = VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

        let reader = vector_fs
            .new_reader(
                requester_shinkai_identity.clone(),
                vr_path,
                requester_shinkai_identity.clone(),
            )
            .map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

        // Note: double-check that the Whitelist is correct here under these assumptions
        let results = vector_fs
            .find_paths_with_read_permissions(&reader, vec![ReadPermission::Public, ReadPermission::Whitelist])?;

        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;
        let db = db.lock().await;

        let mut converted_results = Vec::new();
        for (path, permission) in results {
            let path_str = path.to_string();
            let permission_str = format!("{:?}", permission);
            let subscription_requirement = match db.get_folder_requirements(&path_str) {
                Ok(req) => Some(req),
                Err(_) => None, // Instead of erroring out, we return None for folders without requirements
            };
            converted_results.push((path_str, permission_str, subscription_requirement));
        }

        // TODO: should we return a tree? so you can see the structure of the folders?
        Ok(converted_results)
    }

    pub async fn update_shareable_folder_requirements(
        &self,
        path: String,
        requester_shinkai_identity: ShinkaiName,
        subscription_requirement: ShinkaiSubscriptionReq,
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
        &self,
        path: String,
        requester_shinkai_identity: ShinkaiName,
        subscription_requirement: ShinkaiSubscriptionReq,
    ) -> Result<bool, SubscriberManagerError> {
        // TODO: check that you are actually an admin of the folder
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;
        let mut vector_fs = vector_fs.lock().await;

        let vr_path = VRPath::from_string(&path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;
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
        vector_fs
            .set_path_permission(&writer, ReadPermission::Public, current_permissions.write_permission)
            .map_err(|e| SubscriberManagerError::VectorFSError(e.to_string()))?;

        // Assuming we have validated the admin and permissions, we proceed to update the DB
        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;
        let mut db = db.lock().await;

        db.set_folder_requirements(&path, subscription_requirement)
            .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;

        Ok(true)
    }

    pub async fn unshare_folder(
        &self,
        path: String,
        requester_shinkai_identity: ShinkaiName,
    ) -> Result<bool, SubscriberManagerError> {
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

        // Set the read permissions to Private while reusing the write permissions
        vector_fs.set_path_permission(&writer, ReadPermission::Private, current_permissions.write_permission)?;

        // Assuming we have validated the admin and permissions, we proceed to update the DB
        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseNotAvailable(
            "Database instance is not available".to_string(),
        ))?;
        let mut db = db.lock().await;

        db.remove_folder_requirements(&path)
            .map_err(|e| SubscriberManagerError::DatabaseError(e.to_string()))?;

        Ok(true)
    }

    pub async fn call_subscriber_test_fn(&self) -> Result<String, SubscriberManagerError> {
        let node_lock = self.node.upgrade().ok_or(SubscriberManagerError::NodeNotAvailable(
            "Node instance is not available".to_string(),
        ))?;
        let node = node_lock.lock().await;
        Ok(node.subscriber_test_fn())
    }

    // Schedule the job to be processed. Update the last time it was processed.
    pub async fn process_action(
        &mut self,
        action: ShinkaiSubscriptionRequest,
        profile: ShinkaiName,
    ) -> Result<String, SubscriberManagerError> {
        // TODO: Transform request to ShinkaiSubscription if it passes validation
        // -> Validation ->

        // it comes from the actual node (API side) -> it should be validated before it gets here
        // vector_db path is valid and "shareable" (here)

        match action.action {
            ShinkaiSubscriptionAction::Subscribe => {
                // Transform request to ShinkaiSubscription if it passes validation
                let subscription = ShinkaiSubscription {
                    action: action.action,
                    subscription_id: action.subscription_id,
                    vector_db_path: action.vector_db_path.ok_or(SubscriberManagerError::InvalidRequest(
                        "vector_db_path is required".to_string(),
                    ))?,
                    subscriber_identity: profile,
                    state: action.state,
                    date_created: Utc::now(),
                    last_modified: Utc::now(),
                    last_sync: None,
                };

                // TODO: vector_db path is valid and public
                let vector_fs = self
                    .vector_fs
                    .upgrade()
                    .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                        "VectorFS instance is not available".to_string(),
                    ))?;
                let vector_fs = vector_fs.lock().await;

                // it's not already registered (here)
                // state is valid (here)
                // delegation is enough (here -> yeah we need to do logic about what is what in terms of shareables)

                // TODO: Add fn to add allowed vector_db paths for externals

                // -> Processing ->
                // add node to the subscription_db
                // schedule the job to be processed
                // Further processing and validation here

                Ok("Subscription processed".to_string())
            }
            // Handle other actions as needed
            _ => Err(SubscriberManagerError::InvalidRequest(
                "Unsupported action type".to_string(),
            )),
        }
    }
}
