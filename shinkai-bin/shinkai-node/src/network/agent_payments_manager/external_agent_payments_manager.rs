use crate::db::db_errors::ShinkaiDBError;
use crate::db::{ShinkaiDB, Topic};
use crate::llm_provider::queue::job_queue_manager::JobQueueManager;
use crate::managers::IdentityManager;
use crate::network::network_manager::network_job_manager::VRPackPlusChanges;
use crate::network::node::ProxyConnectionInfo;
use crate::network::subscription_manager::fs_entry_tree_generator::FSEntryTreeGenerator;
use crate::network::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use crate::network::ws_manager::WSUpdateHandler;
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
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::FileDestinationCredentials;
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

use x25519_dalek::StaticSecret as EncryptionStaticKey;

use super::shinkai_tool_offering::{ShinkaiToolOffering, UsageTypeInquiry};

#[derive(Debug)]
pub enum AgentOfferingManagerError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq)]
pub struct InvoiceRequest {
    pub requester_name: ShinkaiName,
    pub tool_key_name: String,
    pub usage_type_inquiry: UsageTypeInquiry,
    pub date_time: DateTime<Utc>,
}

impl InvoiceRequest {
    pub fn unique_id(&self) -> String {
        format!("{}{}", self.requester_name.to_string(), self.date_time)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Invoice {
    pub invoice_id: String,
    pub requester_name: ShinkaiName,
    pub shinkai_offering: ShinkaiToolOffering,
    pub expiration_time: DateTime<Utc>,
    // Maybe add something related to current estimated response times
    // average response time / congestion level or something like that
}

impl Invoice {
    pub fn unique_id(&self) -> String {
        self.invoice_id.clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct InvoicePayment {
    pub invoice_id: String,
    pub date_time: DateTime<Utc>,
    pub signed_invoice: String, // necessary? it acts like a written contract
    pub payment_id: String,
    pub payment_amount: String,
    pub payment_time: DateTime<Utc>,
}

// TODO: NEtworkOffering not required
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum NetworkOffering {
    InvoiceRequest(InvoiceRequest),
    CashoutRequest(InvoicePayment),
}

impl Ord for NetworkOffering {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date_time().cmp(&other.date_time())
    }
}

impl PartialOrd for NetworkOffering {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl NetworkOffering {
    fn date_time(&self) -> &DateTime<Utc> {
        match self {
            NetworkOffering::InvoiceRequest(req) => &req.date_time,
            NetworkOffering::CashoutRequest(payment) => &payment.date_time,
        }
    }
}

pub struct AgentOfferingsManager {
    pub db: Weak<ShinkaiDB>,
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    // pub shared_tools: Arc<DashMap<String, ShinkaiToolOffering>>, // (streamer_profile:::path, shared_folder)
    pub offerings_queue_manager: Arc<Mutex<JobQueueManager<NetworkOffering>>>,
}

const NUM_THREADS: usize = 4;

impl AgentOfferingsManager {
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
        // need tool_router
    ) -> Self {
        let db_prefix = "shinkai__tool__offering_"; // dont change it
        let offerings_queue = JobQueueManager::<NetworkOffering>::new(
            db.clone(),
            Topic::AnyQueuesPrefixed.as_str(),
            Some(db_prefix.to_string()),
        )
        .await
        .unwrap();

        let thread_number = env::var("AGENTS_OFFERING_NETWORK_CONCURRENCY")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS); // Start processing the job queue

        let offerings_queue_manager = Arc::new(Mutex::new(offerings_queue));

        let subscription_queue_handler = AgentOfferingsManager::process_offerings_queue(
            offerings_queue_manager.clone(),
            db.clone(),
            vector_fs.clone(),
            node_name.clone(),
            my_signature_secret_key.clone(),
            my_encryption_secret_key.clone(),
            identity_manager.clone(),
            thread_number,
            proxy_connection_info.clone(),
            |network_offering,
             db,
             vector_fs,
             node_name,
             my_signature_secret_key,
             my_encryption_secret_key,
             identity_manager,
             proxy_connection_info| {
                AgentOfferingsManager::process_subscription_job_message_queued(
                    network_offering,
                    db,
                    vector_fs,
                    node_name,
                    my_signature_secret_key,
                    my_encryption_secret_key,
                    identity_manager,
                    proxy_connection_info,
                )
            },
        )
        .await;

        Self {
            db,
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
            identity_manager,
            offerings_queue_manager,
        }
    }

    // TODO: Should be split this into two? one for invoices and one for actual tool jobs?
    #[allow(clippy::too_many_arguments)]
    pub async fn process_offerings_queue(
        offering_queue_manager: Arc<Mutex<JobQueueManager<NetworkOffering>>>,
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        identity_manager: Weak<Mutex<IdentityManager>>,
        // shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
        thread_number: usize,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        process_job: impl Fn(
                NetworkOffering,
                Weak<ShinkaiDB>,
                Weak<VectorFS>,
                ShinkaiName,
                SigningKey,
                EncryptionStaticKey,
                Weak<Mutex<IdentityManager>>,
                Weak<Mutex<Option<ProxyConnectionInfo>>>,
            ) -> Pin<Box<dyn Future<Output = Result<String, SubscriberManagerError>> + Send>>
            + Send
            + Sync
            + 'static,
        process_invoice: impl Fn(
                NetworkOffering,
                Weak<ShinkaiDB>,
                Weak<VectorFS>,
                ShinkaiName,
                SigningKey,
                EncryptionStaticKey,
                Weak<Mutex<IdentityManager>>,
                Weak<Mutex<Option<ProxyConnectionInfo>>>,
            ) -> Pin<Box<dyn Future<Output = Result<String, SubscriberManagerError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let offering_queue_manager = Arc::clone(&offering_queue_manager);
        let mut receiver = offering_queue_manager.lock().await.subscribe_to_all().await;
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

                // Sort jobs by paid and then by inquiry
                let jobs_sorted = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_lock = offering_queue_manager.lock().await;
                    let all_jobs = job_queue_manager_lock.get_all_elements_interleave().await;
                    drop(job_queue_manager_lock);

                    let filtered_jobs = all_jobs
                        .unwrap_or(Vec::new())
                        .into_iter()
                        .filter_map(|job| {
                            let job_id = match &job {
                                NetworkOffering::InvoiceRequest(req) => req.tool_key_name.clone(),
                                NetworkOffering::CashoutRequest(payment) => payment.invoice_id.clone(),
                            };
                            if !processing_jobs_lock.contains(&job_id) {
                                processing_jobs_lock.insert(job_id.clone());
                                Some(job)
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
                // TODO: Sort jobs by paid and then by inquiry

                for job_offering in jobs_sorted {
                    eprintln!(
                        ">> (process_offerings_queue) Processing job_offering: {:?}",
                        job_offering
                    );
                    let offering_queue_manager = Arc::clone(&offering_queue_manager);
                    let processing_jobs = Arc::clone(&processing_jobs);
                    let semaphore = semaphore.clone();
                    let db = db.clone();
                    let vector_fs = vector_fs.clone();
                    let node_name = node_name.clone();
                    let my_signature_secret_key = my_signature_secret_key.clone();
                    let my_encryption_secret_key = my_encryption_secret_key.clone();
                    let identity_manager = identity_manager.clone();
                    let process_job = process_job.clone();
                    let proxy_connection_info = proxy_connection_info.clone();

                    let handle = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.expect("Failed to acquire semaphore permit");

                        // Acquire the lock, dequeue the job, and immediately release the lock
                        let subscription_with_tree = {
                            let job_queue_manager = offering_queue_manager.lock().await;
                            job_queue_manager.peek(&job_offering).await
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
                                        proxy_connection_info.clone(),
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
                        processing_jobs.lock().await.remove(&job_offering);
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
}
