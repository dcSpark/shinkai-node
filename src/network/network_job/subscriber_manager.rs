use crate::agent::queue::job_queue_manager::JobQueueManager;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::identity_manager::IdentityManagerTrait;
use crate::managers::IdentityManager;
use crate::network::network_job::subscriber_manager_error::SubscriberManagerError;
use crate::network::Node;
use crate::vector_fs::vector_fs::VectorFS;
use chrono::Utc;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionAction, ShinkaiSubscriptionRequest,
};
use std::result::Result::Ok;
use std::sync::Weak;
use std::{collections::HashMap, sync::Arc};
use std::{env, mem};
use tokio::sync::{Mutex, Semaphore};

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

        let thread_number = env::var("SUBSCRIBER_MANAGER_THREADS")
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

    // TODO: change u64 for a proper type for tokens
    pub async fn create_shareable_folder(
        &self,
        min_delegation: Option<u64>,
        min_montly: Option<u64>,
        path: String,
        profile: ShinkaiName,
    ) -> Result<bool, SubscriberManagerError> {
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;
        let vector_fs = vector_fs.lock().await;
        // TODO: we need to keep track of what's accessible here in SubscriberManager
        // TODO: Assume that we updated the vector_fs to have a new shareable folder
        Ok(true)
    }

    pub async fn unshare_folder(&self) -> Result<bool, SubscriberManagerError> {
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;
        let vector_fs = vector_fs.lock().await;
        // TODO: we need to keep track of what's accessible here in SubscriberManager
        // TODO: Assume that we updated the vector_fs to have a new shareable folder
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
