use crate::agent::queue::job_queue_manager::JobQueueManager;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::network::network_job::subscriber_manager_error::SubscriberManagerError;
use crate::network::Node;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_permissions::ReadPermission;
use chrono::Utc;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionAction, ShinkaiSubscriptionRequest,
};
use shinkai_message_primitives::schemas::shinkai_subscription_req::ShinkaiSubscriptionReq;
use shinkai_vector_resources::vector_resource::VRPath;
use std::env;
use std::result::Result::Ok;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::Mutex;

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
        // TODO: how do I send stuff? I can't call up to the Node. or can I pass a weak reference of the parent?
        let subscriptions_queue = JobQueueManager::<ShinkaiSubscription>::new(db.clone(), Topic::JobQueues.as_str())
            .await
            .unwrap();
        let subscriptions_queue_manager = Arc::new(Mutex::new(subscriptions_queue));

        let thread_number = env::var("SUBSCRIBER_MANAGER_NETWORK_CONCURRENCY")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS); // Start processing the job queue

        // let job_queue_handler = SubscriberManager::process_job_queue(
        //     job_queue_manager.clone(),
        //     db.clone(),
        //     vector_fs.clone(),
        //     thread_number,
        //     node.clone(),
        //     |job, db, vector_fs, node| {
        //         Box::pin(SubscriberManager::process_job_message_queued(
        //             job,
        //             db,
        //             vector_fs,
        //             node
        //         ))
        //     },
        // )
        // .await;

        SubscriberManager {
            node,
            db,
            vector_fs,
            identity_manager,
            subscriptions_queue_manager,
            subscription_processing_task: None, // TODO: Update
        }
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
        let mut vector_fs = vector_fs.lock().await;

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

    pub async fn unshare_folder(&self, path: String, requester_shinkai_identity: ShinkaiName) -> Result<bool, SubscriberManagerError> {
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;
        let mut vector_fs = vector_fs.lock().await;

        // Retrieve the current permissions for the path
        let permissions_vector = vector_fs.get_path_permission_for_paths(
            requester_shinkai_identity.clone(),
            vec![VRPath::from_string(&path)?],
        )?;

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
