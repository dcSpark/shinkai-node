use crate::llm_provider::queue::job_queue_manager::JobQueueManager;
use crate::db::db_errors::ShinkaiDBError;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::network::node::ProxyConnectionInfo;
use crate::network::subscription_manager::fs_entry_tree_generator::FSEntryTreeGenerator;
use crate::network::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use crate::network::Node;
use crate::schemas::identity::StandardIdentity;
use crate::vector_fs::vector_fs::VectorFS;
use chrono::Utc;
use ed25519_dalek::SigningKey;
use lru::LruCache;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_proxy_builder_info::ShinkaiProxyBuilderInfo;
use shinkai_message_primitives::schemas::shinkai_subscription::{
    ShinkaiSubscription, ShinkaiSubscriptionStatus, SubscriptionId,
};
use shinkai_message_primitives::schemas::shinkai_subscription_req::SubscriptionPayment;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    MessageSchemaType, SubscriptionGenericResponse, SubscriptionResponseStatus,
};
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, random_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_vector_resources::vector_resource::VRPath;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::Mutex;

use super::external_subscriber_manager::SharedFolderInfo;
use super::fs_entry_tree::FSEntryTree;
use super::http_manager::http_download_manager::HttpDownloadManager;
use super::shared_folder_sm::{ExternalNodeState, SharedFoldersExternalNodeSM};
use x25519_dalek::StaticSecret as EncryptionStaticKey;

const NUM_THREADS: usize = 2;
const LRU_CAPACITY: usize = 100;
const REFRESH_THRESHOLD_MINUTES: usize = 10;
const SOFT_REFRESH_THRESHOLD_MINUTES: usize = 2;

pub struct MySubscriptionsManager {
    pub db: Weak<ShinkaiDB>,
    pub vector_fs: Weak<VectorFS>,
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub subscriptions_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
    pub subscription_processing_task: Option<tokio::task::JoinHandle<()>>, // Is it really needed?
    pub http_download_manager: HttpDownloadManager,

    // Cache for shared folders including the ones that you are not subscribed to
    pub external_node_shared_folders: Arc<Mutex<LruCache<ShinkaiName, SharedFoldersExternalNodeSM>>>,
    // These values are already part of the node, but we want to minimize blocking the node mutex
    // The profile name of the node.
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
    // The address of the proxy server (if any)
    pub proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
}

impl MySubscriptionsManager {
    pub async fn new(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        identity_manager: Weak<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
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

        // Note(Nico): we can use this to update our subscription status
        let subscription_queue_handler = MySubscriptionsManager::process_subscription_queue(
            subscriptions_queue_manager.clone(),
            db.clone(),
            vector_fs.clone(),
            thread_number,
            |job, db, vector_fs| MySubscriptionsManager::process_subscription_job_message_queued(job, db, vector_fs),
        )
        .await;

        let cache_capacity = env::var("MYSUBSCRIPTION_MANAGER_LRU_CAPACITY")
            .unwrap_or(LRU_CAPACITY.to_string())
            .parse::<usize>()
            .unwrap_or(LRU_CAPACITY);

        let external_node_shared_folders = Arc::new(Mutex::new(LruCache::new(cache_capacity)));

        // Instantiate HttpDownloadManager
        let http_download_manager = HttpDownloadManager::new(db.clone(), vector_fs.clone(), node_name.clone()).await;

        MySubscriptionsManager {
            db,
            vector_fs,
            identity_manager,
            subscriptions_queue_manager,
            subscription_processing_task: Some(subscription_queue_handler),
            external_node_shared_folders,
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
            http_download_manager,
            proxy_connection_info,
        }
    }

    pub async fn insert_shared_folder(
        &mut self,
        name: ShinkaiName,
        folders: Vec<SharedFolderInfo>,
    ) -> Result<(), SubscriberManagerError> {
        shinkai_log(
            ShinkaiLogOption::MySubscriptions,
            ShinkaiLogLevel::Debug,
            format!("Inserting shared folder: {}", name.get_node_name_string()).as_str(),
        );
        let shared_folder_sm = SharedFoldersExternalNodeSM::new_with_folders_info(name.clone(), folders);
        let mut external_node_shared_folders = self.external_node_shared_folders.lock().await;
        external_node_shared_folders.put(name, shared_folder_sm);
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn insert_shared_folder_sm(
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
        streamer_full_name: &ShinkaiName,
    ) -> Result<SharedFoldersExternalNodeSM, SubscriberManagerError> {
        // Attempt to get the shared folder from the cache without holding onto the mutable borrow
        let (shareable_folder_ext_node, is_up_to_date, needs_refresh) = {
            let mut external_node_shared_folders = self.external_node_shared_folders.lock().await;
            if let Some(shareable_folder_ext_node) = external_node_shared_folders.get_mut(streamer_full_name) {
                let current_time = Utc::now();
                // Use response_last_updated for determining the time since the last update
                let duration_since_last_update = shareable_folder_ext_node
                    .response_last_updated
                    .map(|last_updated| current_time.signed_duration_since(last_updated))
                    // If response_last_updated is None, consider the duration since last update to be maximum to force a refresh
                    .unwrap_or_else(chrono::Duration::max_value);
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
        if let Some(identity_manager_arc) = self.identity_manager.upgrade() {
            let identity_manager = identity_manager_arc.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&streamer_full_name.get_node_name_string())
                .await?;
            drop(identity_manager);
            let receiver_public_key = standard_identity.node_encryption_public_key;
            let proxy_builder_info = self.get_proxy_builder_info(identity_manager_arc).await;

            // If folder doesn't exist it should create a shinkai message and send it to the network queue
            // then it should create and update the LRU cache with the current status (waiting for the network to respond)

            let msg_request_shared_folders = ShinkaiMessageBuilder::vecfs_available_shared_items(
                None,
                streamer_full_name.get_node_name_string(),
                streamer_full_name.get_profile_name_string().unwrap_or("".to_string()),
                clone_static_secret_key(&self.my_encryption_secret_key),
                clone_signature_secret_key(&self.my_signature_secret_key),
                receiver_public_key,
                self.node_name.get_node_name_string(),
                // Note: the other node doesn't care about the sender's profile in this context
                "".to_string(),
                streamer_full_name.get_node_name_string(),
                streamer_full_name.get_profile_name_string().unwrap_or("".to_string()),
                proxy_builder_info,
            )
            .map_err(|e| SubscriberManagerError::MessageProcessingError(e.to_string()))?;

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
                        external_node_shared_folders.put(streamer_full_name.clone(), placeholder_shared_folder.clone());
                    }
                    // Send the message to the network queue
                    Self::send_message_to_peer(
                        msg_request_shared_folders,
                        self.db.clone(),
                        standard_identity,
                        self.my_encryption_secret_key.clone(),
                        self.identity_manager.clone(),
                        self.proxy_connection_info.clone(),
                    )
                    .await?;

                    // Return the placeholder to indicate the current state to the caller
                    return Ok(placeholder_shared_folder);
                }
            }

            let placeholder_shared_folder =
                SharedFoldersExternalNodeSM::new_placeholder(streamer_full_name.clone(), needs_refresh);
            {
                let mut external_node_shared_folders = self.external_node_shared_folders.lock().await;
                external_node_shared_folders.put(streamer_full_name.clone(), placeholder_shared_folder.clone());
            }
            // Send the message to the network queue
            Self::send_message_to_peer(
                msg_request_shared_folders,
                self.db.clone(),
                standard_identity,
                self.my_encryption_secret_key.clone(),
                self.identity_manager.clone(),
                self.proxy_connection_info.clone(),
            )
            .await?;

            Ok(placeholder_shared_folder)
        } else {
            // Handle the case where the identity manager is no longer available
            Err(SubscriberManagerError::IdentityManagerUnavailable)
        }
    }

    pub async fn unsubscribe_to_shared_folder(
        &self,
        streamer_node_name: ShinkaiName,
        streamer_profile: String,
        my_profile: String,
        folder_name: String,
    ) -> Result<(), SubscriberManagerError> {
        shinkai_log(
            ShinkaiLogOption::MySubscriptions,
            ShinkaiLogLevel::Debug,
            format!(
                "Unsubscribing from shared folder: {} from {} {}",
                folder_name, streamer_node_name.node_name, streamer_profile
            )
            .as_str(),
        );
        // Check locally if I'm already subscribed to the folder using the DB
        let subscription_id = {
            let db_lock = self
                .db
                .upgrade()
                .ok_or(SubscriberManagerError::DatabaseError("Unable to access DB".to_string()))?;
            let my_node_name = ShinkaiName::new(self.node_name.get_node_name_string())?;
            let subscription_id = SubscriptionId::new(
                streamer_node_name.clone(),
                streamer_profile.clone(),
                folder_name.clone(),
                my_node_name,
                my_profile.clone(),
            );
            // Check if the subscription exists in the DB
            match db_lock.get_my_subscription(subscription_id.get_unique_id()) {
                Ok(_) => subscription_id, // Subscription exists, proceed with unsubscribe
                Err(ShinkaiDBError::DataNotFound) => {
                    // Subscription does not exist, cannot unsubscribe
                    shinkai_log(
                        ShinkaiLogOption::MySubscriptions,
                        ShinkaiLogLevel::Error,
                        format!("Subscription does not exist: {}", subscription_id.get_unique_id()).as_str(),
                    );
                    return Err(SubscriberManagerError::SubscriptionNotFound(
                        "Subscription does not exist.".to_string(),
                    ));
                }
                Err(e) => {
                    // Other database errors
                    return Err(SubscriberManagerError::DatabaseError(e.to_string()));
                }
            }
        };

        // Continue
        if let Some(identity_manager_arc) = self.identity_manager.upgrade() {
            let identity_manager = identity_manager_arc.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&streamer_node_name.get_node_name_string())
                .await?;
            drop(identity_manager);
            let receiver_public_key = standard_identity.node_encryption_public_key;
            let proxy_builder_info = self.get_proxy_builder_info(identity_manager_arc).await;

            // If folder doesn't exist it should create a shinkai message and send it to the network queue
            // then it should create and update a local cache with the current status (waiting for the network to respond)

            let msg_request_subscription = ShinkaiMessageBuilder::vecfs_unsubscribe_to_shared_folder(
                folder_name.clone(),
                streamer_node_name.clone().get_node_name_string(),
                streamer_profile.clone(),
                clone_static_secret_key(&self.my_encryption_secret_key),
                clone_signature_secret_key(&self.my_signature_secret_key),
                receiver_public_key,
                self.node_name.get_node_name_string(),
                my_profile.clone(),
                streamer_node_name.get_node_name_string(),
                streamer_profile.clone(),
                proxy_builder_info,
            )
            .map_err(|e| SubscriberManagerError::MessageProcessingError(e.to_string()))?;

            if let Some(db_lock) = self.db.upgrade() {
                db_lock.remove_my_subscription(subscription_id.get_unique_id())?;
            } else {
                return Err(SubscriberManagerError::DatabaseError(
                    "Unable to access DB for updating".to_string(),
                ));
            }

            Self::send_message_to_peer(
                msg_request_subscription,
                self.db.clone(),
                standard_identity,
                self.my_encryption_secret_key.clone(),
                self.identity_manager.clone(),
                self.proxy_connection_info.clone(),
            )
            .await?;

            shinkai_log(
                ShinkaiLogOption::MySubscriptions,
                ShinkaiLogLevel::Debug,
                format!("Unsubscribed from shared folder: {}", folder_name).as_str(),
            );
            Ok(())
        } else {
            // Handle the case where the identity manager is no longer available
            Err(SubscriberManagerError::IdentityManagerUnavailable)
        }
    }

    // TODO: add new fn to create a scheduler for an HTTP API
    // it needs to be able to ping the API and check if the folder has been updated
    // probably we can expand the api endpoint to return some versioning (timestamp / merkle tree root hash)
    // here or in download_manager we should be checking every X time

    #[allow(clippy::too_many_arguments)]
    pub async fn subscribe_to_shared_folder(
        &self,
        streamer_node_name: ShinkaiName,
        streamer_profile: String,
        my_profile: String,
        folder_name: String,
        payment: SubscriptionPayment,
        base_folder: Option<String>,
        http_preferred: Option<bool>,
    ) -> Result<(), SubscriberManagerError> {
        shinkai_log(
            ShinkaiLogOption::MySubscriptions,
            ShinkaiLogLevel::Debug,
            format!(
                "Subscribing to shared folder: {} from {} {}",
                folder_name, streamer_node_name.node_name, streamer_profile
            )
            .as_str(),
        );
        // Check locally if I'm already subscribed to the folder using the DB
        if let Some(db_lock) = self.db.upgrade() {
            let my_node_name = ShinkaiName::new(self.node_name.get_node_name_string())?;
            let subscription_id = SubscriptionId::new(
                streamer_node_name.clone(),
                streamer_profile.clone(),
                folder_name.clone(),
                my_node_name,
                my_profile.clone(),
            );
            match db_lock.get_my_subscription(subscription_id.get_unique_id()) {
                Ok(_) => {
                    // Already subscribed, no need to proceed further
                    return Err(SubscriberManagerError::AlreadySubscribed(
                        "Already subscribed to the folder".to_string(),
                    ));
                }
                Err(ShinkaiDBError::DataNotFound) => {
                    // Subscription doesn't exist. Continue with the subscription process
                }
                Err(e) => {
                    return Err(SubscriberManagerError::DatabaseError(e.to_string()));
                }
            }
        } else {
            return Err(SubscriberManagerError::DatabaseError("Unable to access DB".to_string()));
        }

        // TODO: Check if the payment is valid so we don't waste sending a message for a rejection

        // Continue
        if let Some(identity_manager_arc) = self.identity_manager.upgrade() {
            let identity_manager = identity_manager_arc.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&streamer_node_name.get_node_name_string())
                .await?;
            drop(identity_manager);
            let receiver_public_key = standard_identity.node_encryption_public_key;
            let proxy_builder_info = self.get_proxy_builder_info(identity_manager_arc).await;

            // If folder doesn't exist it should create a shinkai message and send it to the network queue
            // then it should create and update a local cache with the current status (waiting for the network to respond)

            let msg_request_subscription = ShinkaiMessageBuilder::vecfs_subscribe_to_shared_folder(
                folder_name.clone(),
                payment.clone(),
                http_preferred,
                None,
                streamer_node_name.clone().get_node_name_string(),
                streamer_profile.clone(),
                clone_static_secret_key(&self.my_encryption_secret_key),
                clone_signature_secret_key(&self.my_signature_secret_key),
                receiver_public_key,
                self.node_name.get_node_name_string(),
                my_profile.clone(),
                streamer_node_name.get_node_name_string(),
                "".to_string(),
                proxy_builder_info,
            )
            .map_err(|e| SubscriberManagerError::MessageProcessingError(e.to_string()))?;

            // Update local status
            let mut new_subscription = ShinkaiSubscription::new(
                folder_name.clone(),
                streamer_node_name,
                streamer_profile,
                self.node_name.clone(),
                my_profile.clone(),
                ShinkaiSubscriptionStatus::SubscriptionRequested,
                Some(payment),
                base_folder,
                None,
            );

            new_subscription.update_http_preferred(http_preferred);

            if let Some(db_lock) = self.db.upgrade() {
                db_lock.add_my_subscription(new_subscription)?;
            } else {
                return Err(SubscriberManagerError::DatabaseError(
                    "Unable to access DB for updating".to_string(),
                ));
            }

            Self::send_message_to_peer(
                msg_request_subscription,
                self.db.clone(),
                standard_identity,
                self.my_encryption_secret_key.clone(),
                self.identity_manager.clone(),
                self.proxy_connection_info.clone(),
            )
            .await?;

            shinkai_log(
                ShinkaiLogOption::MySubscriptions,
                ShinkaiLogLevel::Debug,
                format!("Subscribed to shared folder: {}", folder_name).as_str(),
            );

            Ok(())
        } else {
            // Handle the case where the identity manager is no longer available
            Err(SubscriberManagerError::IdentityManagerUnavailable)
        }
    }

    pub async fn update_subscription_status(
        &self,
        streamer_node_name: ShinkaiName,
        streamer_profile: String,
        my_profile: String,
        action: MessageSchemaType,
        payload: SubscriptionGenericResponse,
    ) -> Result<(), SubscriberManagerError> {
        shinkai_log(
            ShinkaiLogOption::MySubscriptions,
            ShinkaiLogLevel::Debug,
            format!(
                "Updating subscription status for {} {} with action {:?}",
                streamer_node_name.node_name, streamer_profile, action
            )
            .as_str(),
        );
        let my_node_name = ShinkaiName::new(self.node_name.get_node_name_string())?;
        let subscription_id = SubscriptionId::new(
            streamer_node_name.clone(),
            streamer_profile.clone(),
            payload.shared_folder.clone(),
            my_node_name,
            my_profile,
        );

        match action {
            MessageSchemaType::SubscribeToSharedFolderResponse => {
                // Validate that we requested the subscription
                let db = self
                    .db
                    .upgrade()
                    .ok_or(SubscriberManagerError::DatabaseError("DB not available".to_string()))?;
                let subscription_result = db.get_my_subscription(subscription_id.get_unique_id())?;
                if subscription_result.state != ShinkaiSubscriptionStatus::SubscriptionRequested {
                    // return error
                    return Err(SubscriberManagerError::SubscriptionFailed(
                        "Subscription was not requested".to_string(),
                    ));
                }
                // Update the subscription status in the db
                let new_subscription = subscription_result.with_state(ShinkaiSubscriptionStatus::SubscriptionConfirmed);
                db.update_my_subscription(new_subscription)?;
            }
            _ => {
                // For other actions, do nothing
            }
        }
        shinkai_log(
            ShinkaiLogOption::MySubscriptions,
            ShinkaiLogLevel::Debug,
            format!(
                "Subscription status updated for {} {} with action {:?}",
                streamer_node_name.node_name, streamer_profile, action
            )
            .as_str(),
        );
        Ok(())
    }

    pub async fn share_local_shared_folder_copy_state(
        &self,
        streamer_node: ShinkaiName,
        streamer_profile: String,
        subscriber_node: ShinkaiName,
        subscriber_profile: String,
        subscription_id: String,
    ) -> Result<(), SubscriberManagerError> {
        shinkai_log(
            ShinkaiLogOption::MySubscriptions,
            ShinkaiLogLevel::Debug,
            format!(
                "Sharing local shared folder copy state with {} {}",
                subscriber_node.node_name, subscriber_profile
            )
            .as_str(),
        );
        let mut subscription_folder_path: Option<String> = None;
        let subscription_shared_path: String;
        {
            // Attempt to upgrade the weak pointer to the DB and lock it
            let db = self
                .db
                .upgrade()
                .ok_or(SubscriberManagerError::DatabaseError("DB not available".to_string()))?;

            // Attempt to get the subscription from the DB
            let subscription = db.get_my_subscription(&subscription_id).map_err(|e| match e {
                ShinkaiDBError::DataNotFound => {
                    SubscriberManagerError::SubscriptionNotFound(subscription_id.to_string())
                }
                _ => SubscriberManagerError::DatabaseError(e.to_string()),
            })?;

            // Check that the subscription is not incorrect (for the same node)
            if subscription.subscriber_node.get_node_name_string() != subscriber_node.get_node_name_string() {
                return Err(SubscriberManagerError::InvalidSubscriber(
                    "Subscription doesn't belong to the subscriber".to_string(),
                ));
            }

            subscription_folder_path = Some(
                subscription
                    .subscriber_destination_path
                    .clone()
                    .unwrap_or_else(|| subscription.shared_folder.clone()),
            );
            subscription_shared_path = subscription.shared_folder.clone();
        }

        let folder_path = subscription_folder_path.ok_or_else(|| {
            SubscriberManagerError::SubscriptionNotFound("Subscription folder path not found".to_string())
        })?;

        let full_subscriber =
            ShinkaiName::from_node_and_profile_names(subscriber_node.clone().node_name, subscriber_profile.clone())?;

        // Acquire VectorFS
        let vector_fs = self
            .vector_fs
            .upgrade()
            .ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;

        let vr_path =
            VRPath::from_string(&folder_path).map_err(|e| SubscriberManagerError::InvalidRequest(e.to_string()))?;

        let reader_result = vector_fs
            .new_reader(full_subscriber.clone(), vr_path, full_subscriber.clone())
            .await;

        let result = match reader_result {
            Ok(reader) => match vector_fs.retrieve_fs_entry(&reader).await {
                Ok(entry) => match FSEntryTreeGenerator::fs_entry_to_tree(entry) {
                    Ok(tree) => tree,
                    Err(_) => FSEntryTree {
                        name: "/".to_string(),
                        path: folder_path.clone(),
                        last_modified: Utc::now(),
                        web_link: None,
                        children: HashMap::new(),
                    },
                },
                Err(_) => FSEntryTree {
                    name: "/".to_string(),
                    path: folder_path.clone(),
                    last_modified: Utc::now(),
                    web_link: None,
                    children: HashMap::new(),
                },
            },
            Err(_) => FSEntryTree {
                name: "/".to_string(),
                path: folder_path.clone(),
                web_link: None,
                last_modified: Utc::now(),
                children: HashMap::new(),
            },
        };

        let result_json =
            serde_json::to_string(&result).map_err(|e| SubscriberManagerError::OperationFailed(e.to_string()))?;
        if let Some(identity_manager_lock) = self.identity_manager.upgrade() {
            let identity_manager = identity_manager_lock.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&streamer_node.get_node_name_string())
                .await?;
            drop(identity_manager);

            let receiver_public_key = standard_identity.node_encryption_public_key;
            let symmetric_sk = aes_encryption_key_to_string(random_aes_encryption_key());
            let db = self
                .db
                .upgrade()
                .ok_or(SubscriberManagerError::DatabaseError("DB not available".to_string()))?;

            match Node::process_symmetric_key(symmetric_sk.clone(), db).await {
                Ok(_hash_hex) => {
                    // Prepare metadata hashmap
                    let mut metadata = std::collections::HashMap::new();
                    metadata.insert("folder_state".to_string(), result_json);
                    metadata.insert("symmetric_key".to_string(), symmetric_sk);

                    // Update to use SubscriptionRequiresTreeUpdateResponse instead
                    let response = SubscriptionGenericResponse {
                        subscription_details: "Subscriber shared folder tree state shared".to_string(),
                        status: SubscriptionResponseStatus::Success,
                        shared_folder: subscription_shared_path,
                        error: None,
                        metadata: Some(metadata.clone()),
                    };

                    shinkai_log(
                        ShinkaiLogOption::MySubscriptions,
                        ShinkaiLogLevel::Debug,
                        format!(
                            "Shared local shared folder copy state to {} with metadata: {:?}",
                            streamer_node.node_name, metadata
                        )
                        .as_str(),
                    );

                    let msg_request_subscription = ShinkaiMessageBuilder::vecfs_share_current_shared_folder_state(
                        response,
                        clone_static_secret_key(&self.my_encryption_secret_key),
                        clone_signature_secret_key(&self.my_signature_secret_key),
                        receiver_public_key,
                        subscriber_node.get_node_name_string(),
                        subscriber_profile.clone(),
                        streamer_node.get_node_name_string(),
                        streamer_profile,
                    )
                    .map_err(|e| SubscriberManagerError::MessageProcessingError(e.to_string()))?;

                    Self::send_message_to_peer(
                        msg_request_subscription,
                        self.db.clone(),
                        standard_identity,
                        self.my_encryption_secret_key.clone(),
                        self.identity_manager.clone(),
                        self.proxy_connection_info.clone(),
                    )
                    .await?;
                }
                Err(e) => {
                    return Err(SubscriberManagerError::OperationFailed(format!(
                        "Failed to create temp inbox: {}",
                        e.message
                    )));
                }
            }
        } else {
            return Err(SubscriberManagerError::IdentityManagerUnavailable);
        }
        shinkai_log(
            ShinkaiLogOption::MySubscriptions,
            ShinkaiLogLevel::Debug,
            format!(
                "Shared local shared folder copy state with {} {}",
                subscriber_node.node_name, subscriber_profile
            )
            .as_str(),
        );
        Ok(())
    }

    pub async fn send_message_to_peer(
        message: ShinkaiMessage,
        db: Weak<ShinkaiDB>,
        receiver_identity: StandardIdentity,
        my_encryption_secret_key: EncryptionStaticKey,
        maybe_identity_manager: Weak<Mutex<IdentityManager>>,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
    ) -> Result<(), SubscriberManagerError> {
        shinkai_log(
            ShinkaiLogOption::MySubscriptions,
            ShinkaiLogLevel::Debug,
            format!(
                "Sending message to peer: {}",
                receiver_identity.full_identity_name.extract_node()
            )
            .as_str(),
        );
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
        let my_encryption_sk = Arc::new(my_encryption_secret_key.clone());
        let peer = (receiver_socket_addr, receiver_profile_name);
        let db = db.upgrade().ok_or(SubscriberManagerError::DatabaseError(
            "DB not available to be upgraded".to_string(),
        ))?;
        let maybe_identity_manager = maybe_identity_manager
            .upgrade()
            .ok_or(SubscriberManagerError::IdentityManagerUnavailable)?;

        let proxy_connection_info = proxy_connection_info
            .upgrade()
            .ok_or(SubscriberManagerError::ProxyConnectionInfoUnavailable)?;

        // Call the send function
        Node::send(
            message,
            my_encryption_sk,
            peer,
            proxy_connection_info,
            db,
            maybe_identity_manager,
            false,
            None,
        );

        Ok(())
    }

    pub async fn process_subscription_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        thread_number: usize,
        _process_job: impl Fn(
            ShinkaiSubscription,
            Weak<ShinkaiDB>,
            Weak<VectorFS>,
        ) -> Box<dyn std::future::Future<Output = ()> + Send + 'static>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut handles = Vec::new();
            for _ in 0..thread_number {
                let job_queue_manager = job_queue_manager.clone();
                let _db = db.clone();
                let _vector_fs = vector_fs.clone();
                let handle = tokio::spawn(async move {
                    loop {
                        match job_queue_manager.lock().await.dequeue("some_key").await {
                            Ok(Some(_job)) => {
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

    // Placeholder for process_subscription_job_message_queued
    // Correct the return type of the function to match the expected type
    fn process_subscription_job_message_queued(
        job: ShinkaiSubscription,
        _db: Weak<ShinkaiDB>,
        _vector_fs: Weak<VectorFS>,
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

    async fn get_proxy_builder_info(
        &self,
        identity_manager_lock: Arc<Mutex<IdentityManager>>,
    ) -> Option<ShinkaiProxyBuilderInfo> {
        let identity_manager = identity_manager_lock.lock().await;
        let proxy_connection_info = match self.proxy_connection_info.upgrade() {
            Some(proxy_info) => proxy_info,
            None => return None,
        };
    
        let proxy_connection_info = proxy_connection_info.lock().await;
        if let Some(proxy_connection) = proxy_connection_info.as_ref() {
            let proxy_name = proxy_connection.proxy_identity.clone().get_node_name_string();
            match identity_manager.external_profile_to_global_identity(&proxy_name).await {
                Ok(proxy_identity) => Some(ShinkaiProxyBuilderInfo {
                    proxy_enc_public_key: proxy_identity.node_encryption_public_key,
                }),
                Err(_) => None,
            }
        } else {
            None
        }
    }
}
