use chrono::Utc;
use ed25519_dalek::SigningKey;
use futures::Future;
use lru::LruCache;
use shinkai_db::db::db_errors::ShinkaiDBError;
use shinkai_db::db::{ShinkaiDB, Topic};
use shinkai_db::schemas::ws_types::WSUpdateHandler;
use shinkai_job_queue_manager::job_queue_manager::JobQueueManager;
use shinkai_message_primitives::schemas::identity::StandardIdentity;
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
use shinkai_subscription_manager::subscription_manager::fs_entry_tree::FSEntryTree;
use shinkai_subscription_manager::subscription_manager::fs_entry_tree_generator::FSEntryTreeGenerator;
use shinkai_subscription_manager::subscription_manager::http_manager::http_download_manager::{HttpDownloadJob, HttpDownloadManager};
use shinkai_subscription_manager::subscription_manager::shared_folder_info::SharedFolderInfo;
use shinkai_subscription_manager::subscription_manager::shared_folder_sm::{ExternalNodeState, SharedFoldersExternalNodeSM};
use shinkai_subscription_manager::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_fs::vector_fs::vector_fs_types::FSEntry;
use shinkai_vector_resources::vector_resource::VRPath;
use std::collections::HashMap;
use std::env;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::managers::identity_manager::IdentityManagerTrait;
use crate::managers::IdentityManager;
use crate::network::node::ProxyConnectionInfo;
use crate::network::Node;

use x25519_dalek::StaticSecret as EncryptionStaticKey;

const LRU_CAPACITY: usize = 100;
const REFRESH_THRESHOLD_MINUTES: usize = 10;
const SOFT_REFRESH_THRESHOLD_MINUTES: usize = 2;

pub struct MySubscriptionsManager {
    pub db: Weak<ShinkaiDB>,
    pub vector_fs: Weak<VectorFS>,
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub subscriptions_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
    pub subscription_processing_task: Option<tokio::task::JoinHandle<()>>, // Is it really needed?
    pub subscription_update_cache_task: Option<tokio::task::JoinHandle<()>>, // Is it really needed?
    pub http_download_manager: Arc<Mutex<HttpDownloadManager>>,

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
    // Websocket manager
    pub ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
}

impl MySubscriptionsManager {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        identity_manager: Weak<Mutex<IdentityManager>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
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

        let cache_capacity = env::var("MYSUBSCRIPTION_MANAGER_LRU_CAPACITY")
            .unwrap_or(LRU_CAPACITY.to_string())
            .parse::<usize>()
            .unwrap_or(LRU_CAPACITY);

        let external_node_shared_folders = Arc::new(Mutex::new(LruCache::new(cache_capacity)));

        // Instantiate HttpDownloadManager
        let http_download_manager = HttpDownloadManager::new(db.clone(), vector_fs.clone(), node_name.clone()).await;
        let http_download_manager = Arc::new(Mutex::new(http_download_manager));

        // Note(Nico): we can use this to update our subscription status
        let subscription_queue_handler = MySubscriptionsManager::process_subscription_queue(
            subscriptions_queue_manager.clone(),
            db.clone(),
            vector_fs.clone(),
            external_node_shared_folders.clone(),
            http_download_manager.clone(),
            |job_queue_manager, db, vector_fs, external_node_shared_folders, http_download_manager| {
                Box::pin(MySubscriptionsManager::process_subscription_job_message_queued(
                    job_queue_manager,
                    db,
                    vector_fs,
                    external_node_shared_folders,
                    http_download_manager,
                ))
            },
        )
        .await;

        let subscription_update_cache_task = MySubscriptionsManager::process_subscription_shared_folder_cache_updates(
            db.clone(),
            identity_manager.clone(),
            proxy_connection_info.clone(),
            node_name.clone(),
            my_signature_secret_key.clone(),
            my_encryption_secret_key.clone(),
        )
        .await;

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
            subscription_update_cache_task: Some(subscription_update_cache_task),
            ws_manager,
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
                        self.ws_manager.clone(),
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
                self.ws_manager.clone(),
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
                self.ws_manager.clone(),
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
        &mut self,
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
                Ok(subscription) => {
                    // Force
                    db_lock.remove_my_subscription(subscription_id.get_unique_id())?;

                    match subscription.get_streamer_with_profile() {
                        Ok(streamer_full_name) => {
                            self.get_shared_folder(&streamer_full_name).await?;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::MySubscriptions,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to get streamer full name: {:?}", e),
                            );
                        }
                    };
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
                streamer_node_name.clone(),
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
                db_lock.add_my_subscription(new_subscription.clone())?;

                // Write notification to the DB
                let notification_message = if let Some(http) = http_preferred {
                    if http {
                        format!(
                            "Requested subscription to folder '{}' from user '{}'. HTTP preferred.",
                            folder_name,
                            streamer_node_name.get_node_name_string()
                        )
                    } else {
                        format!(
                            "Requested subscription to folder '{}' from user '{}'.",
                            folder_name,
                            streamer_node_name.get_node_name_string()
                        )
                    }
                } else {
                    format!(
                        "Requested subscription to folder '{}' from user '{}'.",
                        folder_name,
                        streamer_node_name.get_node_name_string()
                    )
                };

                let user_profile = new_subscription.get_subscriber_with_profile()?;
                db_lock.write_notification(user_profile, notification_message)?;
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
                self.ws_manager.clone(),
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
        &mut self,
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
                db.update_my_subscription(new_subscription.clone())?;

                // Trigger get_shared_folder so we can indirectly trigger a download sync
                self.get_shared_folder(&streamer_node_name).await?;

                // Add a nice message after subscription has been confirmed
                let notification_message = format!(
                    "Subscription to folder '{}' from user '{}' has been confirmed. It may take a few minutes to fully sync, depending on the streaming node's payload.",
                    payload.shared_folder,
                    streamer_node_name.get_node_name_string()
                );
                let user_profile = new_subscription.get_subscriber_with_profile()?;
                db.write_notification(user_profile, notification_message)?;
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

    pub async fn handle_shared_folder_response_update(
        &mut self,
        shared_folder_name: ShinkaiName,
        shared_folder_infos: Vec<SharedFolderInfo>,
    ) -> Result<(), SubscriberManagerError> {
        let mut external_node_shared_folders = self.external_node_shared_folders.lock().await;
        let was_none = external_node_shared_folders.get(&shared_folder_name.clone()).is_none();
        drop(external_node_shared_folders);

        self.insert_shared_folder(shared_folder_name.clone(), shared_folder_infos.clone())
            .await?;

        if was_none {
            // Check if we have a subscription for this folder
            // If we do, trigger a get_shared_folder to indirectly trigger a download sync
            if let Some(db_lock) = self.db.upgrade() {
                let subscriptions = db_lock.list_all_my_subscriptions()?;
                for subscription in subscriptions {
                    let streamer_full_name = match subscription.get_streamer_with_profile() {
                        Ok(streamer_full_name) => streamer_full_name,
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::MySubscriptions,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to get streamer full name: {:?}", e),
                            );
                            continue;
                        }
                    };
                    if streamer_full_name == shared_folder_name {
                        // Count the number of files for the specific subscription folder
                        let file_count = shared_folder_infos.iter()
                            .filter(|info| info.path == subscription.shared_folder)
                            .map(|info| info.tree.count_files())
                            .sum::<usize>();

                        // Trigger a sync so the user doesnt need to wait X minutes for the next sync
                        self.call_process_subscription_job_message_queued().await?;

                        // Add notification message
                        let notification_message = format!(
                            "Received downloading links for {} files from {} for folder subscription '{}'.",
                            file_count,
                            shared_folder_name.get_node_name_string(),
                            subscription.shared_folder
                        );
                        let user_profile = subscription.get_subscriber_with_profile()?;
                        db_lock.write_notification(user_profile, notification_message)?;

                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Shares the current shared folder state with the subscriber
    /// It will return empty if the subscription is http-preferred
    /// That way it doesn't trigger a TCP send from the streamer
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

            // Check if the subscription is http-preferred
            if subscription.http_preferred.unwrap_or(false) {
                return Ok(());
            }

            let mut path = subscription
                .subscriber_destination_path
                .clone()
                .unwrap_or_else(|| subscription.shared_folder.clone());

            // Ensure the path starts with "/My Subscriptions"
            if !path.starts_with("/My Subscriptions") {
                path = format!("/My Subscriptions{}", path);
            }

            subscription_folder_path = Some(path);
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

        let result = FSEntryTreeGenerator::remove_prefix_from_paths(&result, "/My Subscriptions");

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

            match Node::process_symmetric_key(symmetric_sk.clone(), db.clone()).await {
                Ok(_hash_hex) => {
                    // Prepare metadata hashmap
                    let mut metadata = std::collections::HashMap::new();
                    metadata.insert("folder_state".to_string(), result_json);
                    metadata.insert("symmetric_key".to_string(), symmetric_sk);

                    // Update to use SubscriptionRequiresTreeUpdateResponse instead
                    let response = SubscriptionGenericResponse {
                        subscription_details: "Subscriber shared folder tree state shared".to_string(),
                        status: SubscriptionResponseStatus::Success,
                        shared_folder: subscription_shared_path.clone(),
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
                        self.ws_manager.clone(),
                    )
                    .await?;

                    let notification_message = format!(
                        "Shared local status of shared folder '{}' with the streamer user '{}' (necessary to receive the update).",
                        subscription_shared_path,
                        subscriber_node.get_node_name_string()
                    );
                    let user_profile = ShinkaiName::from_node_and_profile_names(
                        subscriber_node.get_node_name_string(),
                        subscriber_profile.clone(),
                    )?;
                    db.write_notification(user_profile, notification_message)?;
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
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
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
            ws_manager,
            false,
            None,
        );

        Ok(())
    }

    /// Scheduler that sends a message to the network queue to update the shared folder cache
    #[allow(clippy::too_many_arguments)]
    pub async fn process_subscription_shared_folder_cache_updates(
        db: Weak<ShinkaiDB>,
        identity_manager: Weak<Mutex<IdentityManager>>,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
    ) -> tokio::task::JoinHandle<()> {
        let interval_minutes = env::var("SUBSCRIPTION_UPDATE_CACHE_INTERVAL_MINUTES")
            .unwrap_or("5".to_string()) // Default to 5 minutes if not set
            .parse::<u64>()
            .unwrap_or(5);

        let is_testing = env::var("IS_TESTING").ok().map(|v| v == "1").unwrap_or(false);

        if is_testing {
            return tokio::spawn(async {});
        }

        tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::MySubscriptions,
                ShinkaiLogLevel::Info,
                "process_subscription_shared_folder_cache_updates> Starting subscribers processing loop",
            );

            loop {
                let subscriptions = {
                    let db = match db.upgrade() {
                        Some(db) => db,
                        None => {
                            shinkai_log(
                                ShinkaiLogOption::MySubscriptions,
                                ShinkaiLogLevel::Error,
                                "Database instance is not available",
                            );
                            return;
                        }
                    };
                    match db.list_all_my_subscriptions() {
                        Ok(subscriptions) => subscriptions,
                        Err(_e) => {
                            vec![] // Return an empty list of subscriptions
                        }
                    }
                };

                for subscription in subscriptions {
                    let streamer_full_name = match subscription.get_streamer_with_profile() {
                        Ok(streamer_full_name) => streamer_full_name,
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::MySubscriptions,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to get streamer full name: {:?}", e),
                            );
                            continue;
                        }
                    };

                    if let Some(identity_manager_arc) = identity_manager.upgrade() {
                        let res = {
                            let identity_manager = identity_manager_arc.lock().await;
                            identity_manager
                                .external_profile_to_global_identity(&streamer_full_name.get_node_name_string())
                                .await
                        };
                        let standard_identity = match res {
                            Ok(identity) => identity,
                            Err(e) => {
                                shinkai_log(
                                    ShinkaiLogOption::MySubscriptions,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to get global identity: {:?}", e),
                                );
                                continue;
                            }
                        };
                        let receiver_public_key = standard_identity.node_encryption_public_key;
                        let proxy_builder_info =
                            Self::get_proxy_builder_info_static(identity_manager_arc, proxy_connection_info.clone())
                                .await;

                        let msg_request_shared_folders = match ShinkaiMessageBuilder::vecfs_available_shared_items(
                            None,
                            streamer_full_name.get_node_name_string(),
                            streamer_full_name.get_profile_name_string().unwrap_or("".to_string()),
                            clone_static_secret_key(&my_encryption_secret_key),
                            clone_signature_secret_key(&my_signature_secret_key),
                            receiver_public_key,
                            node_name.get_node_name_string(),
                            "".to_string(),
                            streamer_full_name.get_node_name_string(),
                            streamer_full_name.get_profile_name_string().unwrap_or("".to_string()),
                            proxy_builder_info,
                        ) {
                            Ok(msg) => msg,
                            Err(e) => {
                                shinkai_log(
                                    ShinkaiLogOption::MySubscriptions,
                                    ShinkaiLogLevel::Error,
                                    &format!("Failed to build message: {:?}", e),
                                );
                                continue;
                            }
                        };

                        Self::send_message_to_peer(
                            msg_request_shared_folders,
                            db.clone(),
                            standard_identity,
                            my_encryption_secret_key.clone(),
                            identity_manager.clone(),
                            proxy_connection_info.clone(),
                            None,
                        )
                        .await
                        .map_err(|e| {
                            shinkai_log(
                                ShinkaiLogOption::MySubscriptions,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to send message to peer: {:?}", e),
                            );
                            e
                        })
                        .ok();
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(interval_minutes * 60)).await;
            }
        })
    }

    pub async fn process_subscription_queue(
        job_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        external_node_shared_folders: Arc<Mutex<LruCache<ShinkaiName, SharedFoldersExternalNodeSM>>>,
        http_download_manager: Arc<Mutex<HttpDownloadManager>>,
        process_job: impl Fn(
                Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
                Weak<ShinkaiDB>,
                Weak<VectorFS>,
                Arc<Mutex<LruCache<ShinkaiName, SharedFoldersExternalNodeSM>>>,
                Arc<Mutex<HttpDownloadManager>>,
            ) -> Pin<Box<dyn Future<Output = Result<(), SubscriberManagerError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            // Read the wait duration from the environment variable
            let wait_duration = env::var("MYSUBSCRIPTION_MANAGER_INTERVAL_CHECK")
                .unwrap_or_else(|_| "120".to_string()) // Default to 10 seconds if the variable is not set
                .parse::<u64>()
                .unwrap_or(5); // Default to 10 seconds if parsing fails
            let wait_duration = Duration::from_secs(wait_duration);

            loop {
                // Process the job
                if let Err(e) = process_job(
                    job_queue_manager.clone(),
                    db.clone(),
                    vector_fs.clone(),
                    external_node_shared_folders.clone(),
                    http_download_manager.clone(),
                )
                .await
                {
                    eprintln!("Error processing subscription job: {:?}", e);
                }

                // Wait for the specified duration before processing the next job
                tokio::time::sleep(wait_duration).await;
            }
        })
    }

    pub async fn process_subscription_job_message_queued(
        job_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        external_node_shared_folders: Arc<Mutex<LruCache<ShinkaiName, SharedFoldersExternalNodeSM>>>,
        http_download_manager: Arc<Mutex<HttpDownloadManager>>,
    ) -> Result<(), SubscriberManagerError> {
        // Read all subscriptions from the database
        if let Some(db_lock) = db.upgrade() {
            let all_subscriptions = db_lock.list_all_my_subscriptions()?;
            let http_preferred_subscriptions: Vec<ShinkaiSubscription> = all_subscriptions
                .into_iter()
                .filter(|sub| sub.http_preferred.unwrap_or(false))
                .collect();

            // 1- read the current cache and check if the local files are up to date
            // 2- for the ones that are not up to date, create a download job
            // 3- also create a download job for the files that dont exist locally

            // Check if the local files are up to date
            let vector_fs_arc = vector_fs.upgrade().ok_or(SubscriberManagerError::VectorFSNotAvailable(
                "VectorFS instance is not available".to_string(),
            ))?;

            // Process the filtered subscriptions
            for subscription in http_preferred_subscriptions {
                let mut external_node_shared_folders = external_node_shared_folders.lock().await;
                let streamer = match subscription.get_streamer_with_profile() {
                    Ok(name) => name,
                    Err(_) => continue, // If it fails, continue to the next subscription
                };

                if let Some(shared_folder_sm) = external_node_shared_folders.get(&streamer) {
                    // Extract the information of subscription.shared_folder
                    if let Some(shared_folder_info) = shared_folder_sm.response.get(&subscription.shared_folder) {
                        // Recursively check files in the shared folder tree
                        let files_added = Self::check_and_enqueue_files(
                            vector_fs_arc.clone(),
                            Arc::new(shared_folder_info.tree.clone()),
                            job_queue_manager.clone(),
                            subscription.clone(),
                            http_download_manager.clone(),
                        )
                        .await?;

                        if files_added > 0 {
                            let notification_message = format!(
                                "Added {} file{} to download queue for subscription to folder '{}' from user '{}' (HTTP)",
                                files_added,
                                if files_added > 1 { "s" } else { "" },
                                subscription.shared_folder,
                                subscription.streaming_node.get_node_name_string(),
                            );
                            let user_profile = subscription.get_subscriber_with_profile()?;
                            db_lock.write_notification(user_profile, notification_message)?;
                        }
                    }
                } else {
                    shinkai_log(
                        ShinkaiLogOption::MySubscriptions,
                        ShinkaiLogLevel::Debug,
                        format!(
                            "process_subscription_job_message_queued> No cached information for: {:?}",
                            subscription.streaming_node
                        )
                        .as_str(),
                    );
                }
            }
        } else {
            return Err(SubscriberManagerError::DatabaseError("Unable to access DB".to_string()));
        }

        Ok(())
    }

    // Note: for now it doesn't work with updated files. Update the code to check if the files are up to date
    fn check_and_enqueue_files(
        vector_fs: Arc<VectorFS>,
        tree: Arc<FSEntryTree>,
        job_queue_manager: Arc<Mutex<JobQueueManager<ShinkaiSubscription>>>,
        subscription: ShinkaiSubscription,
        http_download_manager: Arc<Mutex<HttpDownloadManager>>,
    ) -> Pin<Box<dyn Future<Output = Result<usize, SubscriberManagerError>> + Send>> {
        // If the file is a folder, we continue with the children
        // If the file is a file, we check if it exists locally
        // If it doesn't we add it to the job queue
        // If it exists, we check if it's a new version
        // If it's a new version, we add it to the job queue

        Box::pin(async move {
            // Used for creating a notification for the user
            let mut files_added = 0;

            // Skip folders and move directly to checking the children
            if tree.is_folder() {
                // Recursively check children
                let mut futures = vec![];
                for child in tree.children.values() {
                    futures.push(Self::check_and_enqueue_files(
                        vector_fs.clone(),
                        child.clone(),
                        job_queue_manager.clone(),
                        subscription.clone(),
                        http_download_manager.clone(),
                    ));
                }

                for future in futures {
                    files_added += future.await?;
                }

                return Ok(files_added);
            }

            // Check if the file exists in the vector_fs
            let my_subscription_path = if !tree.path.contains("/My Subscriptions") {
                format!("/My Subscriptions{}", tree.path)
            } else {
                tree.path.clone()
            };

            let vr_path = VRPath::from_string(&my_subscription_path)
                .map_err(|_| SubscriberManagerError::InvalidRequest("Invalid VRPath".to_string()))?;

            let subscriber_wprofile = match subscription.get_subscriber_with_profile() {
                Ok(name) => name,
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::MySubscriptions,
                        ShinkaiLogLevel::Error,
                        format!("Failed to create subscriber_wprofile: {}", e).as_str(),
                    );
                    return Ok(files_added); // If it fails, continue to the next subscription
                }
            };

            if vector_fs
                .validate_path_points_to_entry(vr_path.clone(), &subscriber_wprofile)
                .await
                .is_err()
            {
                // Create HttpDownloadJob using the new method
                let job = match HttpDownloadJob::from_subscription_and_tree(subscription.clone(), &tree) {
                    Ok(job) => job,
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::MySubscriptions,
                            ShinkaiLogLevel::Error,
                            format!("Failed to create download job: {}", e).as_str(),
                        );
                        return Ok(files_added);
                    }
                };

                // Use HttpDownloadManager's method to add job to download queue
                let http_download_manager = http_download_manager.lock().await;
                http_download_manager
                    .add_job_to_download_queue(job)
                    .await
                    .map_err(|e| {
                        SubscriberManagerError::JobEnqueueFailed(format!("Failed to enqueue download job: {}", e))
                    })?;
                files_added += 1;
            } else {
                // File exists, create a VFSReader and retrieve the fs_entry
                let reader = vector_fs
                    .new_reader(
                        subscriber_wprofile.clone(),
                        vr_path.clone(),
                        subscriber_wprofile.clone(),
                    )
                    .await
                    .map_err(|e| SubscriberManagerError::OperationFailed(e.to_string()))?;

                let fs_entry = vector_fs
                    .retrieve_fs_entry(&reader)
                    .await
                    .map_err(|e| SubscriberManagerError::OperationFailed(e.to_string()))?;

                if !Self::is_file_up_to_date(fs_entry.clone(), &tree) {
                    // Create HttpDownloadJob using the new method
                    let job = match HttpDownloadJob::from_subscription_and_tree(subscription.clone(), &tree) {
                        Ok(job) => job,
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::MySubscriptions,
                                ShinkaiLogLevel::Error,
                                format!("Failed to create download job: {}", e).as_str(),
                            );
                            return Ok(files_added);
                        }
                    };

                    // Use HttpDownloadManager's method to add job to download queue
                    let http_download_manager = http_download_manager.lock().await;
                    http_download_manager
                        .add_job_to_download_queue(job)
                        .await
                        .map_err(|e| {
                            SubscriberManagerError::JobEnqueueFailed(format!("Failed to enqueue download job: {}", e))
                        })?;
                    files_added += 1;
                }
            }

            // Recursively check children
            let mut futures = vec![];
            for child in tree.children.values() {
                futures.push(Self::check_and_enqueue_files(
                    vector_fs.clone(),
                    child.clone(),
                    job_queue_manager.clone(),
                    subscription.clone(),
                    http_download_manager.clone(),
                ));
            }

            for future in futures {
                files_added += future.await?;
            }

            Ok(files_added)
        })
    }

    fn is_file_up_to_date(fs_entry: FSEntry, tree: &FSEntryTree) -> bool {
        if let Ok(item) = fs_entry.as_item() {
            let merkle_hash = item.merkle_hash.clone();
            if let Some(last_8_bytes) = merkle_hash.get(merkle_hash.len().saturating_sub(8)..) {
                // Extracted the last 8 bytes of the merkle hash
                if let Some(web_link) = &tree.web_link {
                    let last8_in_streamer = &web_link.file.last_8_hash;
                    let last8_in_streamer = last8_in_streamer.get(last8_in_streamer.len().saturating_sub(8)..).unwrap_or("");
                    return last_8_bytes == last8_in_streamer;
                }
            }
        }
        false
    }

    pub async fn call_process_subscription_job_message_queued(&self) -> Result<(), SubscriberManagerError> {
        MySubscriptionsManager::process_subscription_job_message_queued(
            self.subscriptions_queue_manager.clone(),
            self.db.clone(),
            self.vector_fs.clone(),
            self.external_node_shared_folders.clone(),
            self.http_download_manager.clone(),
        )
        .await
    }

    async fn get_proxy_builder_info(
        &self,
        identity_manager_lock: Arc<Mutex<IdentityManager>>,
    ) -> Option<ShinkaiProxyBuilderInfo> {
        MySubscriptionsManager::get_proxy_builder_info_static(identity_manager_lock, self.proxy_connection_info.clone())
            .await
    }

    async fn get_proxy_builder_info_static(
        identity_manager_lock: Arc<Mutex<IdentityManager>>,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
    ) -> Option<ShinkaiProxyBuilderInfo> {
        let identity_manager = identity_manager_lock.lock().await;
        let proxy_connection_info = match proxy_connection_info.upgrade() {
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
