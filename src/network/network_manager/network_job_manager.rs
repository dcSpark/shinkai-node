use crate::agent::queue::job_queue_manager::JobQueueManager;
use crate::db::{ShinkaiDB, Topic};
use crate::managers::IdentityManager;
use crate::network::subscription_manager::external_subscriber_manager::ExternalSubscriberManager;
use crate::network::subscription_manager::my_subscription_manager::MySubscriptionsManager;
use crate::vector_fs::vector_fs::VectorFS;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::Aead;
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use futures::Future;
use serde::{Deserialize, Serialize};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_subscription::SubscriptionId;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_vector_resources::vector_resource::{VRKai, VRPack, VRPath};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Weak;
use std::{collections::HashMap, sync::Arc};
use std::{env, mem};
use tokio::sync::{Mutex, Semaphore};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use super::network_handlers::{
    extract_message, handle_based_on_message_content_and_encryption, verify_message_signature,
};
use super::network_job_manager_error::NetworkJobQueueError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkVRKai {
    pub enc_pairs: Vec<u8>, // encrypted VRPack
    pub subscription_id: SubscriptionId,
    pub nonce: String,
    pub symmetric_key_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum NetworkMessageType {
    ShinkaiMessage,
    VRKaiPathPair,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct NetworkJobQueue {
    pub receiver_address: SocketAddr,
    pub unsafe_sender_address: SocketAddr,
    pub message_type: NetworkMessageType,
    pub content: Vec<u8>,
    pub date_created: DateTime<Utc>,
}

impl PartialOrd for NetworkJobQueue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NetworkJobQueue {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date_created.cmp(&other.date_created)
    }
}

/// The idea behind NetworkJobManager is that we can queue the work that needs to be done
/// so we don't overload the node with too many jobs at once. This is especially important
/// for jobs that require a lot of resources or block some Mutexes because then the
/// connections wouldn't close.
const NUM_THREADS: usize = 2;

pub struct NetworkJobManager {
    pub network_job_queue_manager: Arc<Mutex<JobQueueManager<NetworkJobQueue>>>,
    pub network_job_processing_task: Option<tokio::task::JoinHandle<()>>,
}

impl NetworkJobManager {
    pub async fn new(
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        my_node_name: ShinkaiName,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        identity_manager: Arc<Mutex<IdentityManager>>,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        external_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
    ) -> Self {
        let jobs_map = Arc::new(Mutex::new(HashMap::new()));
        {
            let db_arc = db.upgrade().ok_or("Failed to upgrade shinkai_db").unwrap();
            let shinkai_db = db_arc.lock().await;
            let all_jobs = shinkai_db.get_all_jobs().unwrap();
            let mut jobs = jobs_map.lock().await;
            for job in all_jobs {
                jobs.insert(job.job_id().to_string(), job);
            }
        }

        let db_prefix = "network_queue_abcprefix_";
        let network_job_queue = JobQueueManager::<NetworkJobQueue>::new(
            db.clone(),
            Topic::AnyQueuesPrefixed.as_str(),
            Some(db_prefix.to_string()),
        )
        .await
        .unwrap();
        let network_job_queue_manager = Arc::new(Mutex::new(network_job_queue));

        let thread_number = env::var("NETWORK_JOB_MANAGER_THREADS")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS);

        // Start processing the job queue
        let job_queue_handler = NetworkJobManager::process_job_queue(
            db.clone(),
            vector_fs.clone(),
            my_node_name.clone(),
            clone_static_secret_key(&my_encryption_secret_key),
            clone_signature_secret_key(&my_signature_secret_key),
            thread_number,
            identity_manager.clone(),
            my_subscription_manager,
            external_subscription_manager,
            network_job_queue_manager.clone(),
            |job,
             db,
             vector_fs,
             my_node_profile_name,
             my_encryption_secret_key,
             my_signature_secret_key,
             identity_manager,
             my_subscription_manager,
             external_subscription_manager| {
                Box::pin(NetworkJobManager::process_network_request_queued(
                    job,
                    db,
                    vector_fs,
                    my_node_profile_name,
                    my_encryption_secret_key,
                    my_signature_secret_key,
                    identity_manager,
                    my_subscription_manager,
                    external_subscription_manager,
                ))
            },
        )
        .await;

        let network_job_manager = Self {
            network_job_queue_manager,
            network_job_processing_task: Some(job_queue_handler),
        };

        network_job_manager
    }

    pub async fn process_job_queue(
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        my_node_profile_name: ShinkaiName,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        max_parallel_jobs: usize,
        identity_manager: Arc<Mutex<IdentityManager>>,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        external_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
        job_queue_manager: Arc<Mutex<JobQueueManager<NetworkJobQueue>>>,
        job_processing_fn: impl Fn(
                NetworkJobQueue,                       // job to process
                Weak<Mutex<ShinkaiDB>>,                // db
                Weak<Mutex<VectorFS>>,                 // vector_fs
                ShinkaiName,                           // my_profile_name
                EncryptionStaticKey,                   // my_encryption_secret_key
                SigningKey,                            // my_signature_secret_key
                Arc<Mutex<IdentityManager>>,           // identity_manager
                Arc<Mutex<MySubscriptionsManager>>,    // my_subscription_manager
                Arc<Mutex<ExternalSubscriberManager>>, // external_subscription_manager
            ) -> Pin<Box<dyn Future<Output = Result<String, NetworkJobQueueError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let job_queue_manager = Arc::clone(&job_queue_manager);
        let mut receiver = job_queue_manager.lock().await.subscribe_to_all().await;
        let db_clone = db.clone();
        let vector_fs_clone = vector_fs.clone();
        let my_node_profile_name_clone = my_node_profile_name.clone();
        let my_encryption_sk_clone = clone_static_secret_key(&my_encryption_secret_key);
        let my_signature_sk_clone = clone_signature_secret_key(&my_signature_secret_key);
        let identity_manager_clone = identity_manager.clone();
        let my_subscription_manager_clone = my_subscription_manager.clone();
        let external_subscription_manager_clone = external_subscription_manager.clone();

        let job_processing_fn = Arc::new(job_processing_fn);

        let processing_jobs = Arc::new(Mutex::new(HashSet::new()));
        let semaphore = Arc::new(Semaphore::new(max_parallel_jobs));

        return tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::JobExecution,
                ShinkaiLogLevel::Info,
                "Starting job queue processing loop",
            );

            let mut handles = Vec::new();
            loop {
                let mut continue_immediately = false;

                // Scope for acquiring and releasing the lock quickly
                let job_ids_to_process: Vec<String> = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_lock = job_queue_manager.lock().await;
                    let all_jobs = job_queue_manager_lock
                        .get_all_elements_interleave()
                        .await
                        .unwrap_or(Vec::new());
                    std::mem::drop(job_queue_manager_lock);

                    let filtered_jobs = all_jobs
                        .into_iter()
                        .filter_map(|job| {
                            let job_id = job.clone().receiver_address.to_string();
                            if !processing_jobs_lock.contains(&job_id) {
                                processing_jobs_lock.insert(job_id.clone());
                                Some(job_id)
                            } else {
                                None
                            }
                        })
                        .take(max_parallel_jobs)
                        .collect::<Vec<_>>();

                    // Check if the number of jobs to process is equal to max_parallel_jobs
                    continue_immediately = filtered_jobs.len() == max_parallel_jobs;

                    std::mem::drop(processing_jobs_lock);
                    filtered_jobs
                };

                // Spawn tasks based on filtered job IDs
                for job_id in job_ids_to_process {
                    let job_queue_manager = Arc::clone(&job_queue_manager);
                    let processing_jobs = Arc::clone(&processing_jobs);
                    let semaphore = Arc::clone(&semaphore);
                    let db_clone_2 = db_clone.clone();
                    let vector_fs_clone_2 = vector_fs_clone.clone();
                    let my_node_profile_name_clone_2 = my_node_profile_name_clone.clone();
                    let my_encryption_sk_clone_2 = clone_static_secret_key(&my_encryption_sk_clone);
                    let my_signature_sk_clone_2 = clone_signature_secret_key(&my_signature_sk_clone);
                    let identity_manager_clone_2 = identity_manager_clone.clone();
                    let my_subscription_manager_clone_2 = my_subscription_manager_clone.clone();
                    let external_subscription_manager_clone_2 = external_subscription_manager_clone.clone();

                    let job_processing_fn = Arc::clone(&job_processing_fn);

                    let handle = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.unwrap();

                        // Acquire the lock, dequeue the job, and immediately release the lock
                        let job = {
                            let job_queue_manager = job_queue_manager.lock().await;
                            let job = job_queue_manager.peek(&job_id).await;
                            job
                        };

                        match job {
                            Ok(Some(job)) => {
                                // Acquire the lock, process the job, and immediately release the lock
                                let result = {
                                    let result = job_processing_fn(
                                        job,
                                        db_clone_2,
                                        vector_fs_clone_2,
                                        my_node_profile_name_clone_2,
                                        my_encryption_sk_clone_2,
                                        my_signature_sk_clone_2,
                                        identity_manager_clone_2,
                                        my_subscription_manager_clone_2,
                                        external_subscription_manager_clone_2,
                                    )
                                    .await;
                                    if let Ok(Some(_)) = job_queue_manager.lock().await.dequeue(&job_id.clone()).await {
                                        result
                                    } else {
                                        Err(NetworkJobQueueError::JobDequeueFailed(job_id.clone()))
                                    }
                                };

                                match result {
                                    Ok(_) => {
                                        shinkai_log(
                                            ShinkaiLogOption::JobExecution,
                                            ShinkaiLogLevel::Debug,
                                            "Job processed successfully",
                                        );
                                    } // handle success case
                                    Err(_) => {} // handle error case
                                }
                            }
                            Ok(None) => {}
                            Err(_) => {
                                // Log the error
                            }
                        }
                        drop(_permit);
                        processing_jobs.lock().await.remove(&job_id);
                    });
                    handles.push(handle);
                }

                let handles_to_join = mem::replace(&mut handles, Vec::new());
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
                        ShinkaiLogOption::JobExecution,
                        ShinkaiLogLevel::Info,
                        format!("Received new network job {:?}", new_job.receiver_address.to_string()).as_str(),
                    );
                }
            }
        });
    }

    pub async fn process_network_request_queued(
        job: NetworkJobQueue,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        my_node_profile_name: ShinkaiName,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        identity_manager: Arc<Mutex<IdentityManager>>,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        external_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
    ) -> Result<String, NetworkJobQueueError> {
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            format!(
                "Processing job {:?} and type {:?}",
                job.receiver_address.to_string(),
                job.message_type
            )
            .as_str(),
        );

        match job.message_type {
            NetworkMessageType::ShinkaiMessage => {
                let _ = Self::handle_message_internode(
                    job.receiver_address,
                    job.unsafe_sender_address,
                    &job.content,
                    my_node_profile_name.get_node_name_string(),
                    my_encryption_secret_key,
                    my_signature_secret_key,
                    db.clone(),
                    identity_manager.clone(),
                    my_subscription_manager.clone(),
                    external_subscription_manager.clone(),
                )
                .await;
            }
            NetworkMessageType::VRKaiPathPair => {
                shinkai_log(
                    ShinkaiLogOption::Network,
                    ShinkaiLogLevel::Debug,
                    "Processing VRKaiPathPair message type",
                );

                // Deserialize job.content into NetworkVRKai using bincode
                let network_vr_kai: Result<NetworkVRKai, _> = bincode::deserialize(&job.content);
                let network_vr_kai = network_vr_kai.map_err(|_| NetworkJobQueueError::ContentParseFailed)?;
                eprintln!("NetworkVRKai subscription_id: {:?}", network_vr_kai.subscription_id);

                let _ = Self::handle_receiving_vr_pack_from_subscription(
                    network_vr_kai,
                    db.clone(),
                    vector_fs.clone(),
                    my_node_profile_name.clone(),
                    my_encryption_secret_key,
                    my_signature_secret_key,
                    identity_manager.clone(),
                    my_subscription_manager.clone(),
                    external_subscription_manager.clone(),
                )
                .await;
            }
        }

        Ok("OK".to_string())
    }

    pub async fn add_network_job_to_queue(
        &mut self,
        network_job: &NetworkJobQueue,
    ) -> Result<String, NetworkJobQueueError> {
        let mut job_queue_manager = self.network_job_queue_manager.lock().await;
        let _ = job_queue_manager
            .push(&network_job.receiver_address.to_string(), network_job.clone())
            .await;

        Ok(network_job.receiver_address.to_string())
    }

    pub async fn handle_receiving_vr_pack_from_subscription(
        network_vr_pack: NetworkVRKai,
        db: Weak<Mutex<ShinkaiDB>>,
        vector_fs: Weak<Mutex<VectorFS>>,
        my_node_profile_name: ShinkaiName,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        identity_manager: Arc<Mutex<IdentityManager>>,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        external_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
    ) -> Result<(), NetworkJobQueueError> {
        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Debug,
            &format!("Handling VRPack from {:?}", my_node_profile_name),
        );
        // check that the subscription exists
        let subscription = {
            let maybe_db = db.upgrade().ok_or(NetworkJobQueueError::ShinkaDBUpgradeFailed)?;
            let db_lock = maybe_db.lock().await;

            match db_lock.get_my_subscription(network_vr_pack.subscription_id.get_unique_id()) {
                Ok(sub) => sub,
                Err(_) => return Err(NetworkJobQueueError::Other("Subscription not found".to_string())),
            }
        };

        // get the symmetric key from the database
        let symmetric_sk_bytes = {
            let maybe_db = db.upgrade().ok_or(NetworkJobQueueError::ShinkaDBUpgradeFailed)?;
            let db_lock = maybe_db.lock().await;

            // Retrieve the symmetric key using the symmetric_key_hash from the database
            match db_lock.read_symmetric_key(&network_vr_pack.symmetric_key_hash) {
                Ok(key) => key,
                Err(_) => {
                    return Err(NetworkJobQueueError::SymmetricKeyNotFound(
                        network_vr_pack.symmetric_key_hash.clone(),
                    ))
                }
            }
        };

        let key = GenericArray::from_slice(&symmetric_sk_bytes);
        let cipher = Aes256Gcm::new(key);

        // Decode the nonce from hex string to bytes
        let nonce_bytes = hex::decode(&network_vr_pack.nonce).map_err(|_| NetworkJobQueueError::NonceParseFailed)?;
        let nonce = GenericArray::from_slice(&nonce_bytes);

        // Decrypt the enc_pairs
        let decrypted_data = cipher
            .decrypt(nonce, network_vr_pack.enc_pairs.as_ref())
            .map_err(|_| NetworkJobQueueError::DecryptionFailed)?;

        // Deserialize the decrypted data back into Vec<(VRKai, VRPath)>
        let vr_pack: VRPack = bincode::deserialize(&decrypted_data)
            .map_err(|_| NetworkJobQueueError::DeserializationFailed("Failed to deserialize VRPack".to_string()))?;

        // Find destination path from my_subscripton
        let destination_path = {
            if subscription.subscriber_destination_path.is_none() {
                subscription.shared_folder.clone()
            } else {
                subscription.subscriber_destination_path.clone().unwrap()
            }
        };

        let local_subscriber = ShinkaiName::from_node_and_profile_names(
            subscription.subscriber_node.node_name,
            subscription.subscriber_profile,
        )?;

        // Check if the folder already exists. For now, we delete the folder and recreate it
        {
            let maybe_vector_fs = vector_fs.upgrade().ok_or(NetworkJobQueueError::VectorFSUpgradeFailed)?;
            let mut vector_fs_lock = maybe_vector_fs.lock().await;

            let vr_path = VRPath::from_string(&destination_path)
                .map_err(|e| NetworkJobQueueError::InvalidVRPath(e.to_string()))?;

            let path_already_exists = vector_fs_lock
                .validate_path_points_to_folder(vr_path.clone(), &local_subscriber.clone())
                .is_ok();

            let writer = vector_fs_lock
                .new_writer(local_subscriber.clone(), vr_path.clone(), local_subscriber.clone())
                .unwrap();

            if path_already_exists {
                let deletion_writer = writer.new_writer_copied_data(vr_path, &mut vector_fs_lock).unwrap();
                vector_fs_lock.delete_folder(&deletion_writer)?;
            }

            // TODO: extend it to support multiple levels of folders
            let result = vector_fs_lock
                .create_new_folder(&writer, &destination_path.clone());

            // Unpack the VRKaiPath pairs
            let result = vector_fs_lock.extract_vrpack_in_folder(&writer, vr_pack);
            // eprintln!("VR Unpack Result: {:?}", result);

            {
                // debug. print current files
                // eprintln!("debug current files");
                // // let root_path = VRPath::root();
                // let root_path = VRPath::from_string("/").unwrap();
                // let reader = vector_fs_lock.new_reader(
                //     local_subscriber.clone(),
                //     root_path.clone(),
                //     local_subscriber.clone(),
                // );
                // let reader = reader.unwrap();
                // let result = vector_fs_lock.retrieve_fs_path_simplified_json(&reader);
                // eprintln!("Current files: {:?}", result);
            }
        }
     
        Ok(())
    }

    pub async fn handle_message_internode(
        receiver_address: SocketAddr,
        unsafe_sender_address: SocketAddr,
        bytes: &[u8],
        my_node_profile_name: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        shinkai_db: Weak<Mutex<ShinkaiDB>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
        my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
        external_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
    ) -> Result<(), NetworkJobQueueError> {
        let maybe_db = shinkai_db
            .upgrade()
            .ok_or(NetworkJobQueueError::ShinkaDBUpgradeFailed)?;

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("{} > Got message from {:?}", receiver_address, unsafe_sender_address),
        );

        // Extract and validate the message
        let message = extract_message(bytes, receiver_address)?;
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Decoded Message: {:?}", receiver_address, message),
        );

        // Extract sender's public keys and verify the signature
        let sender_profile_name_string = ShinkaiName::from_shinkai_message_only_using_sender_node_name(&message)
            .unwrap()
            .get_node_name_string();
        let sender_identity = identity_manager
            .lock()
            .await
            .external_profile_to_global_identity(&sender_profile_name_string)
            .await
            .unwrap();

        verify_message_signature(sender_identity.node_signature_public_key, &message)?;

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!(
                "{} > Sender Profile Name: {:?}",
                receiver_address, sender_profile_name_string
            ),
        );
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Node Sender Identity: {}", receiver_address, sender_identity),
        );
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Verified message signature", receiver_address),
        );

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Debug,
            &format!("{} > Sender Identity: {}", receiver_address, sender_identity),
        );

        handle_based_on_message_content_and_encryption(
            message.clone(),
            sender_identity.node_encryption_public_key,
            sender_identity.addr.unwrap(),
            sender_profile_name_string,
            &my_encryption_secret_key,
            &my_signature_secret_key,
            &my_node_profile_name,
            maybe_db,
            identity_manager,
            receiver_address,
            unsafe_sender_address,
            my_subscription_manager,
            external_subscription_manager,
        )
        .await
    }
}