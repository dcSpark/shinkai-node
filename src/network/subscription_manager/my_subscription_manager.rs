use crate::agent::queue::job_queue_manager::JobQueueManager;
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
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionAction, ShinkaiSubscriptionRequest,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::{Mutex, MutexGuard};

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

const NUM_THREADS: usize = 2;

pub struct MySubscriptions {
    pub node: Weak<Mutex<Node>>,
    pub db: Weak<Mutex<ShinkaiDB>>,
    pub vector_fs: Weak<Mutex<VectorFS>>,
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub subscriptions_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>, //
    pub subscription_processing_task: Option<tokio::task::JoinHandle<()>>,             // Is it really needed?
    pub subscribed_folders_trees: HashMap<String, Arc<FSItemTree>>,
}

// Machine State for my subscriptions
// - should i notify my node about my subscriptions? or should it just follow them depending on the delegation?
// - for now, node -> wallet ->

// - join -> confirmation / error -> wait (if confirmed) -> receive
// - check my subscriptions of my wallet -> update local state (could go to 1)
// - updates -> send current state -> wait -> process -> receive
//

// impl MySubscriptions {
//     pub async fn new(
//         node: Weak<Mutex<Node>>,
//         db: Weak<Mutex<ShinkaiDB>>,
//         vector_fs: Weak<Mutex<VectorFS>>,
//         identity_manager: Weak<Mutex<IdentityManager>>,
//     ) -> Self {
//         let db_prefix = "subscriptions_abcprefix_";
//         let subscriptions_queue =
//             JobQueueManager::<ShinkaiSubscription>::new(db.clone(), Topic::AnyQueuesPrefixed.as_str(), Some(db_prefix.to_string()))
//                 .await
//                 .unwrap();
//         let subscriptions_queue_manager = Arc::new(Mutex::new(subscriptions_queue));

//         let thread_number = env::var("SUBSCRIBER_MANAGER_NETWORK_CONCURRENCY")
//             .unwrap_or(NUM_THREADS.to_string())
//             .parse::<usize>()
//             .unwrap_or(NUM_THREADS); // Start processing the job queue

//         let subscription_queue_handler = MySubscriptions::process_subscription_queue(
//             subscriptions_queue_manager.clone(),
//             db.clone(),
//             vector_fs.clone(),
//             thread_number,
//             node.clone(),
//             |job, db, vector_fs, node| MySubscriptions::process_job_message_queued(job, db, vector_fs, node),
//         )
//         .await;

//         MySubscriptions {
//             node,
//             db,
//             vector_fs,
//             identity_manager,
//             subscriptions_queue_manager,
//             subscription_processing_task: Some(subscription_queue_handler),
//             subscribed_folders_trees: HashMap::new(),
//         }
//     }
// }
