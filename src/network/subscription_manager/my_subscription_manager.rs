use crate::agent::queue::job_queue_manager::JobQueueManager;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::network::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use crate::network::Node;
use crate::schemas::identity::StandardIdentity;
use crate::vector_fs::vector_fs::VectorFS;
use crate::vector_fs::vector_fs_error::VectorFSError;
use crate::vector_fs::vector_fs_permissions::ReadPermission;
use crate::vector_fs::vector_fs_types::{FSEntry, FSFolder, FSItem};
use chrono::NaiveDateTime;
use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use lru::LruCache;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionAction, ShinkaiSubscriptionRequest,
};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::{Mutex, MutexGuard};

use super::fs_item_tree::FSItemTree;
use super::shared_folder_sm::{ExternalNodeState, SharedFoldersExternalNodeSM};
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

const NUM_THREADS: usize = 2;
const LRU_CAPACITY: usize = 100;
const REFRESH_THRESHOLD_MINUTES: usize = 10;
const SOFT_REFRESH_THRESHOLD_MINUTES: usize = 2;

pub struct MySubscriptionsManager {
    pub node: Weak<Mutex<Node>>,
    pub db: Weak<Mutex<ShinkaiDB>>,
    pub vector_fs: Weak<Mutex<VectorFS>>,
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub subscriptions_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
    pub subscription_processing_task: Option<tokio::task::JoinHandle<()>>, // Is it really needed?
    // TODO: add a new property to store the user's subscriptions
    pub subscribed_folders_trees: HashMap<String, Arc<FSItemTree>>, // We want it to be stored in the DB
    pub external_node_shared_folders: Arc<Mutex<LruCache<ShinkaiName, SharedFoldersExternalNodeSM>>>,

    // These values are already part of the node, but we want to minimize blocking the node mutex
    // The profile name of the node.
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
}

impl MySubscriptionsManager {
    pub async fn new(
        node: Weak<Mutex<Node>>,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        identity_manager: Weak<Mutex<IdentityManager>>,
    ) -> Self {
        let db_prefix = "my_subscriptions_prefix_"; // needs to be 24 characters
        let subscriptions_queue = JobQueueManager::<ShinkaiSubscription>::new(
            db.clone(),
            Topic::AnyQueuesPrefixed.as_str(),
            Some(db_prefix.to_string()),
        )
        .await
        .unwrap();
        let subscriptions_queue_manager = Arc::new(Mutex::new(subscriptions_queue));

        let thread_number = env::var("MYSUBSCRIPTION_MANAGER_NETWORK_CONCURRENCY")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS); // Start processing the job queue

        let subscription_queue_handler = MySubscriptionsManager::process_subscription_queue(
            subscriptions_queue_manager.clone(),
            db.clone(),
            vector_fs.clone(),
            thread_number,
            node.clone(),
            |job, db, vector_fs, node| MySubscriptionsManager::process_job_message_queued(job, db, vector_fs, node),
        )
        .await;

        let cache_capacity = env::var("MYSUBSCRIPTION_MANAGER_LRU_CAPACITY")
            .unwrap_or(LRU_CAPACITY.to_string())
            .parse::<usize>()
            .unwrap_or(LRU_CAPACITY); // Start processing the job queue

        let external_node_shared_folders = Arc::new(Mutex::new(LruCache::new(cache_capacity)));

        // Extracting values from the Node
        let node_name;
        let my_signature_secret_key;
        let my_encryption_secret_key;
        if let Some(node_lock) = node.upgrade() {
            let node = node_lock.lock().await;
            node_name = node.node_name.clone(); // Assuming Node has a field `node_name`
            my_signature_secret_key = node.identity_secret_key.clone(); // Assuming Node has a field `identity_secret_key`
            my_encryption_secret_key = node.encryption_secret_key.clone(); // Assuming Node has a field `encryption_secret_key`
        } else {
            // Handle the case where the node is no longer available
            // This might involve setting default values or returning an error
            panic!("MySubscriptionsManager> Node is no longer available!");
        }

        MySubscriptionsManager {
            node,
            db,
            vector_fs,
            identity_manager,
            subscriptions_queue_manager,
            subscription_processing_task: Some(subscription_queue_handler),
            subscribed_folders_trees: HashMap::new(),
            external_node_shared_folders,
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
        }
    }

    pub async fn insert_shared_folder(
        &mut self,
        name: ShinkaiName,
        folder: SharedFoldersExternalNodeSM,
    ) -> Result<(), SubscriberManagerError> {
        let mut external_node_shared_folders = self.external_node_shared_folders.lock().await;
        external_node_shared_folders.put(name, folder);
        Ok(())
    }

    pub async fn get_shared_folder(
        &mut self,
        name: &ShinkaiName,
    ) -> Result<SharedFoldersExternalNodeSM, SubscriberManagerError> {
        // Attempt to get the shared folder from the cache without holding onto the mutable borrow
        let (shareable_folder_ext_node, is_up_to_date, needs_refresh) = {
            let mut external_node_shared_folders = self.external_node_shared_folders.lock().await;
            if let Some(shareable_folder_ext_node) = external_node_shared_folders.get_mut(name) {
                let current_time = Utc::now();
                let duration_since_last_update =
                    current_time.signed_duration_since(shareable_folder_ext_node.last_updated);
                // Determine if the folder is up-to-date
                let is_up_to_date =
                    duration_since_last_update < chrono::Duration::minutes(REFRESH_THRESHOLD_MINUTES as i64);
                // Determine if the folder needs a refresh
                let needs_refresh =
                    duration_since_last_update > chrono::Duration::minutes(SOFT_REFRESH_THRESHOLD_MINUTES as i64);

                (Some(shareable_folder_ext_node.clone()), is_up_to_date, needs_refresh)
            } else {
                (None, false, false)
            }
        };

        // If the folder is up-to-date, return it directly
        if let Some(shareable_folder_ext_node) = shareable_folder_ext_node.clone() {
            if is_up_to_date {
                return Ok(shareable_folder_ext_node);
            }
        }

        // Note(Nico): this uses identity_manager, this could eventually be a bottleneck
        // if we have a lot of requests to a slow RPC endpoint (blocking).
        if let Some(identity_manager_lock) = self.identity_manager.upgrade() {
            let identity_manager = identity_manager_lock.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&name.get_node_name())
                .await?;
            let receiver_public_key = standard_identity.node_encryption_public_key;

            // If folder doesn't exist it should create a shinkai message and send it to the network queue
            // then it should create and update the LRU cache with the current status (waiting for the network to respond)

            let msg_request_shared_folders = ShinkaiMessageBuilder::vecfs_available_shared_items(
                clone_static_secret_key(&self.my_encryption_secret_key),
                clone_signature_secret_key(&self.my_signature_secret_key),
                receiver_public_key,
                self.node_name.get_node_name(),
                // Note: the other node doesn't care about the sender's profile in this context
                "".to_string(),
                name.get_node_name(),
                "".to_string(),
            ).map_err(|e| SubscriberManagerError::MessageProcessingError(e.to_string()))?;

            // Return the current cache value because it's not too old but we still needed to refresh it in the background
            if let Some(shareable_folder_ext_node) = shareable_folder_ext_node.clone() {
                // Note(Nico): needs_refresh is only valid when the data is available and not outdated
                // but we still want to refresh it in the background
                if needs_refresh {
                    // Create a new state indicating the data is available but a refresh is requested
                    let placeholder_shared_folder = shareable_folder_ext_node
                        .with_updated_state(ExternalNodeState::CachedAvailableButStillRequesting);
                    {
                        let mut external_node_shared_folders = self.external_node_shared_folders.lock().await;
                        external_node_shared_folders.put(name.clone(), placeholder_shared_folder.clone());
                    }
                    // Send the message to the network queue
                    // TODO: move this process_subscription_queue
                    self.send_message_to_peer(msg_request_shared_folders, standard_identity).await?;

                    // Return the placeholder to indicate the current state to the caller
                    return Ok(placeholder_shared_folder);
                }
            }

            let placeholder_shared_folder = SharedFoldersExternalNodeSM::new_placeholder(name.clone(), needs_refresh);
            {
                let mut external_node_shared_folders = self.external_node_shared_folders.lock().await;
                external_node_shared_folders.put(name.clone(), placeholder_shared_folder.clone());
            }
            // Send the message to the network queue
            // TODO: move this process_subscription_queue
            self.send_message_to_peer(msg_request_shared_folders, standard_identity).await?;

            return Ok(placeholder_shared_folder);
        } else {
            // Handle the case where the identity manager is no longer available
            return Err(SubscriberManagerError::IdentityManagerUnavailable);
        }
    }

    async fn send_message_to_peer(
        &self,
        message: ShinkaiMessage,
        receiver_identity: StandardIdentity,
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
        let receiver_profile_name = receiver_identity.full_identity_name.to_string();

        // Upgrade the weak reference to Node
        // Prepare the parameters for the send function
        let my_encryption_sk = Arc::new(self.my_encryption_secret_key.clone());
        let peer = (receiver_socket_addr, receiver_profile_name);
        let db = self.db.upgrade().ok_or(SubscriberManagerError::DatabaseError(
            "DB not available to be upgraded".to_string(),
        ))?;
        let maybe_identity_manager = self
            .identity_manager
            .upgrade()
            .ok_or(SubscriberManagerError::IdentityManagerUnavailable)?;

        // Call the send function
        Node::send(message, my_encryption_sk, peer, db, maybe_identity_manager, false, None);

        Ok(())
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
}
