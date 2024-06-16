use super::network_manager::network_job_manager::{
    NetworkJobManager, NetworkJobQueue, NetworkVRKai, VRPackPlusChanges,
};
use super::node_api::{APIError, SendResponseBodyData};
use super::node_api_handlers::APIUseRegistrationCodeSuccessResponse;
use super::node_error::NodeError;
use super::subscription_manager::external_subscriber_manager::ExternalSubscriberManager;
use super::subscription_manager::my_subscription_manager::MySubscriptionsManager;
use crate::cron_tasks::cron_manager::CronManager;
use crate::db::db_retry::RetryMessage;
use crate::db::ShinkaiDB;
use crate::llm_provider::job_manager::JobManager;
use crate::managers::IdentityManager;
use crate::network::network_limiter::ConnectionLimiter;
use crate::schemas::identity::{Identity, StandardIdentity};
use crate::schemas::smart_inbox::SmartInbox;
use crate::vector_fs::vector_fs::VectorFS;
use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::Aead;
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use async_channel::{Receiver, Sender};
use chashmap::CHashMap;
use chrono::Utc;
use core::panic;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use futures::{future::FutureExt, pin_mut, prelude::*, select};
use lazy_static::lazy_static;
use rand::Rng;
use serde_json::Value;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use shinkai_message_primitives::schemas::shinkai_subscription::{ShinkaiSubscription, SubscriptionId};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    APIAvailableSharedItems, IdentityPermissions, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, encryption_public_key_to_string, encryption_secret_key_to_string,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_tcp_relayer::NetworkMessage;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::file_parser::unstructured_api::UnstructuredAPI;
use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub enum NodeCommand {
    Shutdown,
    // Command to make the node ping all the other nodes it knows about.
    PingAll,
    // Command to request the node's public keys for signing and encryption. The sender will receive the keys.
    GetPublicKeys(Sender<(VerifyingKey, EncryptionPublicKey)>),
    // Command to make the node send a `ShinkaiMessage` in an onionized (i.e., anonymous and encrypted) way.
    SendOnionizedMessage {
        msg: ShinkaiMessage,
        res: async_channel::Sender<Result<SendResponseBodyData, APIError>>,
    },
    GetNodeName {
        res: Sender<String>,
    },
    // Command to request the addresses of all nodes this node is aware of. The sender will receive the list of addresses.
    GetPeers(Sender<Vec<SocketAddr>>),
    // Command to make the node create a registration code through the API. The sender will receive the code.
    APICreateRegistrationCode {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    // Command to make the node create a registration code locally. The sender will receive the code.
    LocalCreateRegistrationCode {
        permissions: IdentityPermissions,
        code_type: RegistrationCodeType,
        res: Sender<String>,
    },
    // Command to make the node use a registration code encapsulated in a `ShinkaiMessage`. The sender will receive the result.
    APIUseRegistrationCode {
        msg: ShinkaiMessage,
        res: Sender<Result<APIUseRegistrationCodeSuccessResponse, APIError>>,
    },
    // Command to request the external profile data associated with a profile name. The sender will receive the data.
    IdentityNameToExternalProfileData {
        name: String,
        res: Sender<StandardIdentity>,
    },
    // Command to fetch the last 'n' messages, where 'n' is defined by `limit`. The sender will receive the messages.
    FetchLastMessages {
        limit: usize,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    // Command to request all subidentities that the node manages. The sender will receive the list of subidentities.
    APIGetAllSubidentities {
        res: Sender<Result<Vec<StandardIdentity>, APIError>>,
    },
    GetAllSubidentitiesDevicesAndAgents(Sender<Result<Vec<Identity>, APIError>>),
    APIGetAllInboxesForProfile {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    },
    APIGetAllSmartInboxesForProfile {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<SmartInbox>, APIError>>,
    },
    APIUpdateSmartInboxName {
        msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    },
    APIGetLastMessagesFromInbox {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    },
    APIUpdateJobToFinished {
        msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    },
    GetLastMessagesFromInbox {
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    APIMarkAsReadUpTo {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    MarkAsReadUpTo {
        inbox_name: String,
        up_to_time: String,
        res: Sender<String>,
    },
    APIGetLastUnreadMessagesFromInbox {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<ShinkaiMessage>, APIError>>,
    },
    GetLastUnreadMessagesFromInbox {
        inbox_name: String,
        limit: usize,
        offset: Option<String>,
        res: Sender<Vec<ShinkaiMessage>>,
    },
    APIGetLastMessagesFromInboxWithBranches {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<Vec<ShinkaiMessage>>, APIError>>,
    },
    GetLastMessagesFromInboxWithBranches {
        inbox_name: String,
        limit: usize,
        offset_key: Option<String>,
        res: Sender<Vec<Vec<ShinkaiMessage>>>,
    },
    APIRetryMessageWithInbox {
        inbox_name: String,
        message_hash: String,
        res: Sender<Result<(), APIError>>,
    },
    RetryMessageWithInbox {
        inbox_name: String,
        message_hash: String,
        res: Sender<Result<(), String>>,
    },
    APIAddInboxPermission {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    AddInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    },
    #[allow(dead_code)]
    APIRemoveInboxPermission {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    #[allow(dead_code)]
    RemoveInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    },
    #[allow(dead_code)]
    HasInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<bool>,
    },
    APICreateJob {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    #[allow(dead_code)]
    CreateJob {
        shinkai_message: ShinkaiMessage,
        res: Sender<(String, String)>,
    },
    APICreateFilesInboxWithSymmetricKey {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIGetFilenamesInInbox {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    },
    APIAddFileToInboxWithSymmetricKey {
        filename: String,
        file: Vec<u8>,
        public_key: String,
        encrypted_nonce: String,
        res: Sender<Result<String, APIError>>,
    },
    APIJobMessage {
        msg: ShinkaiMessage,
        res: Sender<Result<SendResponseBodyData, APIError>>,
    },
    #[allow(dead_code)]
    JobMessage {
        shinkai_message: ShinkaiMessage,
        res: Sender<(String, String)>,
    },
    APIAddAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    AddAgent {
        agent: SerializedLLMProvider,
        profile: ShinkaiName,
        res: Sender<String>,
    },
    APIChangeJobAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIAvailableAgents {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<SerializedLLMProvider>, APIError>>,
    },
    APIRemoveAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIModifyAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    AvailableAgents {
        full_profile_name: String,
        res: Sender<Result<Vec<SerializedLLMProvider>, String>>,
    },
    APIPrivateDevopsCronList {
        res: Sender<Result<String, APIError>>,
    },
    APIAddToolkit {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIListToolkits {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIChangeNodesName {
        msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    },
    APIIsPristine {
        res: Sender<Result<bool, APIError>>,
    },
    IsPristine {
        res: Sender<bool>,
    },
    APIScanOllamaModels {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<serde_json::Value>, APIError>>,
    },
    APIAddOllamaModels {
        msg: ShinkaiMessage,
        res: Sender<Result<(), APIError>>,
    },
    LocalScanOllamaModels {
        res: Sender<Result<Vec<serde_json::Value>, String>>,
    },
    AddOllamaModels {
        target_profile: ShinkaiName,
        models: Vec<String>,
        res: Sender<Result<(), String>>,
    },
    APIVecFSRetrievePathSimplifiedJson {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIVecFSRetrievePathMinimalJson {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIVecFSRetrieveVectorResource {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIVecFSRetrieveVectorSearchSimplifiedJson {
        msg: ShinkaiMessage,
        #[allow(clippy::complexity)]
        res: Sender<Result<Vec<(String, Vec<String>, f32)>, APIError>>,
    },
    APIConvertFilesAndSaveToFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<Value>, APIError>>,
    },
    APIVecFSCreateFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSMoveItem {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSCopyItem {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSMoveFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSCopyFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSDeleteFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSDeleteItem {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSSearchItems {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
    },
    APIAvailableSharedItems {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIAvailableSharedItemsOpen {
        msg: APIAvailableSharedItems,
        res: Sender<Result<Value, APIError>>,
    },
    APICreateShareableFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIUpdateShareableFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIUnshareFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APISubscribeToSharedFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIUnsubscribe {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIMySubscriptions {
        msg: ShinkaiMessage,
        res: Sender<Result<Value, APIError>>,
    },
    APIGetMySubscribers {
        msg: ShinkaiMessage,
        res: Sender<Result<HashMap<String, Vec<ShinkaiSubscription>>, APIError>>,
    },
    APIGetHttpFreeSubscriptionLinks {
        subscription_profile_path: String,
        res: Sender<Result<Value, APIError>>,
    },
    RetrieveVRKai {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    RetrieveVRPack {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    #[allow(dead_code)]
    LocalExtManagerProcessSubscriptionUpdates {
        res: Sender<Result<(), String>>,
    },
}

/// Hard-coded embedding model that is set as the default when creating a new profile.
pub static NEW_PROFILE_DEFAULT_EMBEDDING_MODEL: EmbeddingModelType =
    EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

lazy_static! {
    /// Hard-coded list of supported embedding models that is set when creating a new profile.
    /// These need to match the list that our Embedding server orchestration service supports.
    pub static ref NEW_PROFILE_SUPPORTED_EMBEDDING_MODELS: Vec<EmbeddingModelType> = vec![NEW_PROFILE_DEFAULT_EMBEDDING_MODEL.clone()];
}

// A type alias for a string that represents a profile name.
type ProfileName = String;
type TcpReadHalf = Arc<Mutex<ReadHalf<TcpStream>>>;
type TcpWriteHalf = Arc<Mutex<WriteHalf<TcpStream>>>;
type TcpConnection = Option<(TcpReadHalf, TcpWriteHalf)>;

// Define the ConnectionInfo struct
#[derive(Clone, Debug)]
pub struct ProxyConnectionInfo {
    pub proxy_identity: ShinkaiName,
    pub tcp_connection: TcpConnection,
}

// The `Node` struct represents a single node in the network.
pub struct Node {
    // The profile name of the node.
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub identity_secret_key: SigningKey,
    // The public key corresponding to `identity_secret_key`.
    pub identity_public_key: VerifyingKey,
    // The secret key used for encryption and decryption.
    pub encryption_secret_key: EncryptionStaticKey,
    // The public key corresponding to `encryption_secret_key`.
    pub encryption_public_key: EncryptionPublicKey,
    // The address this node is listening on.
    pub listen_address: SocketAddr,
    // Secrets file path
    pub secrets_file_path: String,
    // A map of known peer nodes.
    pub peers: CHashMap<(SocketAddr, ProfileName), chrono::DateTime<Utc>>,
    // The interval at which this node pings all known peers.
    pub ping_interval_secs: u64,
    // The channel from which this node receives commands.
    pub commands: Receiver<NodeCommand>,
    // The manager for subidentities.
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    // The database connection for this node.
    pub db: Arc<ShinkaiDB>,
    // First device needs registration code
    pub first_device_needs_registration_code: bool,
    // Initial Agent to auto-add on first registration
    pub initial_agents: Vec<SerializedLLMProvider>,
    // The Job manager
    pub job_manager: Option<Arc<Mutex<JobManager>>>,
    // Cron Manager
    pub cron_manager: Option<Arc<Mutex<CronManager>>>,
    // JS Toolkit Executor Remote
    pub js_toolkit_executor_remote: Option<String>,
    // The Node's VectorFS
    pub vector_fs: Arc<VectorFS>,
    // An EmbeddingGenerator initialized with the Node's default embedding model + server info
    pub embedding_generator: RemoteEmbeddingGenerator,
    /// Unstructured server connection
    pub unstructured_api: UnstructuredAPI,
    /// Rate Limiter
    pub conn_limiter: Arc<ConnectionLimiter>,
    /// External Subscription Manager (when others are subscribing to this node's data)
    pub ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
    /// My Subscription Manager
    pub my_subscription_manager: Arc<Mutex<MySubscriptionsManager>>,
    // Network Job Manager
    pub network_job_manager: Arc<Mutex<NetworkJobManager>>,
    // Proxy Address
    pub proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    // Handle for the listen_and_reconnect task
    pub listen_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Node {
    // Construct a new node. Returns a `Result` which is `Ok` if the node was successfully created,
    // and `Err` otherwise.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        node_name: String,
        listen_address: SocketAddr,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
        ping_interval_secs: u64,
        commands: Receiver<NodeCommand>,
        main_db_path: String,
        secrets_file_path: String,
        proxy_identity: Option<String>,
        first_device_needs_registration_code: bool,
        initial_agents: Vec<SerializedLLMProvider>,
        js_toolkit_executor_remote: Option<String>,
        vector_fs_db_path: String,
        embedding_generator: Option<RemoteEmbeddingGenerator>,
        unstructured_api: Option<UnstructuredAPI>,
    ) -> Arc<Mutex<Node>> {
        // if is_valid_node_identity_name_and_no_subidentities is false panic
        match ShinkaiName::new(node_name.to_string().clone()) {
            Ok(_) => (),
            Err(_) => panic!("Invalid node identity name: {}", node_name),
        }

        // Get public keys, and update the local node keys in the db
        let db = ShinkaiDB::new(&main_db_path).unwrap_or_else(|e| {
            eprintln!("Error: {:?}", e);
            panic!("Failed to open database: {}", main_db_path)
        });
        let db_arc = Arc::new(db);
        let identity_public_key = identity_secret_key.verifying_key();
        let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);
        let node_name = ShinkaiName::new(node_name).unwrap();
        {
            match db_arc.update_local_node_keys(node_name.clone(), encryption_public_key, identity_public_key) {
                Ok(_) => (),
                Err(e) => panic!("Failed to update local node keys: {}", e),
            }
            // TODO: maybe check if the keys in the Blockchain match and if not, then prints a warning message to update the keys
        }

        // Setup Identity Manager
        let db_weak = Arc::downgrade(&db_arc);
        let subidentity_manager = IdentityManager::new(db_weak, node_name.clone()).await.unwrap();
        let identity_manager = Arc::new(Mutex::new(subidentity_manager));

        // Initialize default UnstructuredAPI/RemoteEmbeddingGenerator if none provided
        let unstructured_api = unstructured_api.unwrap_or_else(UnstructuredAPI::new_default);
        let embedding_generator = embedding_generator.unwrap_or_else(RemoteEmbeddingGenerator::new_default);

        // Fetch list of existing profiles from the node to push into the VectorFS
        let mut profile_list = vec![];
        {
            profile_list = match db_arc.get_all_profiles(node_name.clone()) {
                Ok(profiles) => profiles.iter().map(|p| p.full_identity_name.clone()).collect(),
                Err(e) => panic!("Failed to fetch profiles: {}", e),
            };
        }

        // Initialize/setup the VectorFS.
        let vector_fs = VectorFS::new(
            embedding_generator.clone(),
            vec![embedding_generator.model_type.clone()],
            profile_list,
            &vector_fs_db_path,
            node_name.clone(),
        )
        .await
        .unwrap_or_else(|e| {
            eprintln!("Error: {:?}", e);
            panic!("Failed to load VectorFS from database: {}", vector_fs_db_path)
        });
        let vector_fs_arc = Arc::new(vector_fs);

        let max_connections: u32 = std::env::var("MAX_CONNECTIONS")
            .unwrap_or_else(|_| "5".to_string())
            .parse::<usize>()
            .expect("Failed to parse MAX_CONNECTIONS")
            .try_into()
            .expect("MAX_CONNECTIONS value out of range");

        let max_connections_per_ip: u32 = std::env::var("MAX_CONNECTIONS_PER_IP")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<usize>()
            .expect("Failed to parse MAX_CONNECTIONS_PER_IP")
            .try_into()
            .expect("MAX_CONNECTIONS_PER_IP value out of range");

        let burst_allowance: u32 = std::env::var("BURST_ALLOWANCE")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .expect("Failed to parse BURST_ALLOWANCE")
            .try_into()
            .expect("BURST_ALLOWANCE value out of range");

        let conn_limiter = Arc::new(ConnectionLimiter::new(
            max_connections,
            burst_allowance,
            max_connections_per_ip.try_into().unwrap(),
        ));

        // Initialize ProxyConnectionInfo if proxy_identity is provided
        let proxy_connection_info = Arc::new(Mutex::new(proxy_identity.map(|proxy_identity| {
            let proxy_identity = ShinkaiName::new(proxy_identity).expect("Invalid proxy identity name");
            ProxyConnectionInfo {
                proxy_identity,
                tcp_connection: None,
            }
        })));
        let proxy_connection_info_weak = Arc::downgrade(&proxy_connection_info);

        let ext_subscriber_manager = Arc::new(Mutex::new(
            ExternalSubscriberManager::new(
                Arc::downgrade(&db_arc),
                Arc::downgrade(&vector_fs_arc),
                Arc::downgrade(&identity_manager),
                node_name.clone(),
                clone_signature_secret_key(&identity_secret_key),
                clone_static_secret_key(&encryption_secret_key),
                proxy_connection_info_weak.clone(),
            )
            .await,
        ));

        let my_subscription_manager = Arc::new(Mutex::new(
            MySubscriptionsManager::new(
                Arc::downgrade(&db_arc),
                Arc::downgrade(&vector_fs_arc),
                Arc::downgrade(&identity_manager),
                node_name.clone(),
                clone_signature_secret_key(&identity_secret_key),
                clone_static_secret_key(&encryption_secret_key),
                proxy_connection_info_weak.clone(),
            )
            .await,
        ));

        // Create NetworkJobManager with a weak reference to this node
        let network_manager = NetworkJobManager::new(
            Arc::downgrade(&db_arc),
            Arc::downgrade(&vector_fs_arc),
            node_name.clone(),
            clone_static_secret_key(&encryption_secret_key),
            clone_signature_secret_key(&identity_secret_key),
            identity_manager.clone(),
            my_subscription_manager.clone(),
            ext_subscriber_manager.clone(),
            proxy_connection_info_weak.clone(),
        )
        .await;

        Arc::new(Mutex::new(Node {
            node_name: node_name.clone(),
            identity_secret_key: clone_signature_secret_key(&identity_secret_key),
            identity_public_key,
            encryption_secret_key: clone_static_secret_key(&encryption_secret_key),
            encryption_public_key,
            peers: CHashMap::new(),
            listen_address,
            secrets_file_path,
            ping_interval_secs,
            commands,
            identity_manager: identity_manager.clone(),
            db: db_arc.clone(),
            job_manager: None,
            cron_manager: None,
            first_device_needs_registration_code,
            initial_agents,
            js_toolkit_executor_remote,
            vector_fs: vector_fs_arc.clone(),
            embedding_generator,
            unstructured_api,
            conn_limiter,
            ext_subscription_manager: ext_subscriber_manager,
            my_subscription_manager,
            network_job_manager: Arc::new(Mutex::new(network_manager)),
            proxy_connection_info,
            listen_handle: None,
        }))
    }

    // Start the node's operations.
    pub async fn start(&mut self) -> Result<(), NodeError> {
        let db_weak = Arc::downgrade(&self.db);
        let vector_fs_weak = Arc::downgrade(&self.vector_fs);
        self.job_manager = Some(Arc::new(Mutex::new(
            JobManager::new(
                db_weak,
                Arc::clone(&self.identity_manager),
                clone_signature_secret_key(&self.identity_secret_key),
                self.node_name.clone(),
                vector_fs_weak.clone(),
                self.embedding_generator.clone(),
                self.unstructured_api.clone(),
            )
            .await,
        )));

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("Starting node with name: {}", self.node_name),
        );
        let db_weak = Arc::downgrade(&self.db);
        self.cron_manager = match &self.job_manager {
            Some(job_manager) => Some(Arc::new(Mutex::new(
                CronManager::new(
                    db_weak,
                    vector_fs_weak,
                    clone_signature_secret_key(&self.identity_secret_key),
                    self.node_name.clone(),
                    Arc::clone(job_manager),
                )
                .await,
            ))),
            None => None,
        };

        let listen_future = self.listen_and_reconnect(self.proxy_connection_info.clone()).fuse();
        pin_mut!(listen_future);

        let retry_interval_secs = 2;
        let mut retry_interval = async_std::stream::interval(Duration::from_secs(retry_interval_secs));

        let ping_interval_secs = if self.ping_interval_secs == 0 {
            315576000 * 10 // 10 years in seconds
        } else {
            self.ping_interval_secs
        };
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("Automatic Ping interval set to {} seconds", ping_interval_secs),
        );

        let mut ping_interval = async_std::stream::interval(Duration::from_secs(ping_interval_secs));
        let mut commands_clone = self.commands.clone();
        // TODO: here we can create a task to check the blockchain for new peers and update our list
        let check_peers_interval_secs = 5;
        let _check_peers_interval = async_std::stream::interval(Duration::from_secs(check_peers_interval_secs));

        // TODO: implement a TCP connection here with a proxy if it's set

        loop {
            let ping_future = ping_interval.next().fuse();
            let commands_future = commands_clone.next().fuse();
            let retry_future = retry_interval.next().fuse();

            // TODO: update this to read onchain data and update db
            // let check_peers_future = check_peers_interval.next().fuse();
            pin_mut!(ping_future, commands_future, retry_future);

            select! {
                    _retry = retry_future => {
                        // Clone the necessary variables for `retry_messages`
                        let db_clone = self.db.clone();
                        let encryption_secret_key_clone = self.encryption_secret_key.clone();
                        let identity_manager_clone = self.identity_manager.clone();
                        let proxy_connection_info = self.proxy_connection_info.clone();

                        // Spawn a new task to call `retry_messages` asynchronously
                        tokio::spawn(async move {
                            let _ = Self::retry_messages(
                                db_clone,
                                encryption_secret_key_clone,
                                identity_manager_clone,
                                proxy_connection_info,
                            ).await;
                        });
                    },
                    _listen = listen_future => unreachable!(),
                    _ping = ping_future => {
                        // Clone the necessary variables for `ping_all`
                        let node_name_clone = self.node_name.clone();
                        let encryption_secret_key_clone = self.encryption_secret_key.clone();
                        let identity_secret_key_clone = self.identity_secret_key.clone();
                        let peers_clone = self.peers.clone();
                        let db_clone = Arc::clone(&self.db);
                        let identity_manager_clone = Arc::clone(&self.identity_manager);
                        let listen_address_clone = self.listen_address;
                        let proxy_connection_info = self.proxy_connection_info.clone();

                        // Spawn a new task to call `ping_all` asynchronously
                        tokio::spawn(async move {
                            let _ = Self::ping_all(
                                node_name_clone,
                                encryption_secret_key_clone,
                                identity_secret_key_clone,
                                peers_clone,
                                db_clone,
                                identity_manager_clone,
                                listen_address_clone,
                                proxy_connection_info,
                            ).await;
                        });
                    },
                    // check_peers = check_peers_future => self.connect_new_peers().await,
                    command = commands_future => {
                        match command {
                            Some(command) => {
                                // Spawn a new task for each command to handle it concurrently
                                    match command {
                                        // NodeCommand::Shutdown => {
                                        //     shinkai_log(ShinkaiLogOption::Node, ShinkaiLogLevel::Info, "Shutdown command received. Stopping the node.");
                                        //     // self.db = Arc::new(Mutex::new(ShinkaiDB::new("PLACEHOLDER").expect("Failed to create a temporary database")));
                                        // },
                                        NodeCommand::PingAll => {
                                            let peers_clone = self.peers.clone();
                                            let identity_manager_clone = Arc::clone(&self.identity_manager);
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let identity_secret_key_clone = self.identity_secret_key.clone();
                                            let db_clone = Arc::clone(&self.db);
                                            let listen_address_clone = self.listen_address;
                                            let proxy_connection_info = self.proxy_connection_info.clone();
                                            tokio::spawn(async move {
                                                let _ = Self::ping_all(
                                                    node_name_clone,
                                                    encryption_secret_key_clone,
                                                    identity_secret_key_clone,
                                                    peers_clone,
                                                    db_clone,
                                                    identity_manager_clone,
                                                    listen_address_clone,
                                                    proxy_connection_info,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::GetPublicKeys(sender) => {
                                            let identity_public_key = self.identity_public_key;
                                            let encryption_public_key = self.encryption_public_key;
                                            tokio::spawn(async move {
                                                let _ = Node::send_public_keys(
                                                    identity_public_key,
                                                    encryption_public_key,
                                                    sender,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::IdentityNameToExternalProfileData { name, res } => {
                                            let identity_manager_clone = Arc::clone(&self.identity_manager);
                                            tokio::spawn(async move {
                                                let _ = Self::handle_external_profile_data(
                                                    identity_manager_clone,
                                                    name,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::SendOnionizedMessage { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = Arc::clone(&self.identity_manager);
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let identity_secret_key_clone = self.identity_secret_key.clone();
                                            let proxy_connection_info = self.proxy_connection_info.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_handle_send_onionized_message(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    identity_secret_key_clone,
                                                    msg,
                                                    proxy_connection_info,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::FetchLastMessages { limit, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            tokio::spawn(async move {
                                                let _ = Node::fetch_and_send_last_messages(
                                                    db_clone,
                                                    limit,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::GetAllSubidentitiesDevicesAndAgents(res) => {
                                            let identity_manager_clone = Arc::clone(&self.identity_manager);
                                            tokio::spawn(async move {
                                                let _ = Node::local_get_all_subidentities_devices_and_agents(
                                                    identity_manager_clone,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::LocalCreateRegistrationCode { permissions, code_type, res } => {
                                            let db = self.db.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::local_create_and_send_registration_code(
                                                    db,
                                                    permissions,
                                                    code_type,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::GetLastMessagesFromInbox { inbox_name, limit, offset_key, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            tokio::spawn(async move {
                                                let _ = Node::local_get_last_messages_from_inbox(
                                                    db_clone,
                                                    inbox_name,
                                                    limit,
                                                    offset_key,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::MarkAsReadUpTo { inbox_name, up_to_time, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            tokio::spawn(async move {
                                                let _ = Node::local_mark_as_read_up_to(
                                                    db_clone,
                                                    inbox_name,
                                                    up_to_time,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::GetLastUnreadMessagesFromInbox { inbox_name, limit, offset, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            tokio::spawn(async move {
                                                let _ = Node::local_get_last_unread_messages_from_inbox(
                                                    db_clone,
                                                    inbox_name,
                                                    limit,
                                                    offset,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::AddInboxPermission { inbox_name, perm_type, identity, res } => {
                                            let identity_manager_clone = Arc::clone(&self.identity_manager);
                                            let db_clone = Arc::clone(&self.db);
                                            tokio::spawn(async move {
                                                let _ = Node::local_add_inbox_permission(
                                                    identity_manager_clone,
                                                    db_clone,
                                                    inbox_name,
                                                    perm_type,
                                                    identity,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::RemoveInboxPermission { inbox_name, perm_type, identity, res } => {
                                            let identity_manager_clone = Arc::clone(&self.identity_manager);
                                            let db_clone = Arc::clone(&self.db);
                                            tokio::spawn(async move {
                                                let _ = Node::local_remove_inbox_permission(
                                                    db_clone,
                                                    identity_manager_clone,
                                                    inbox_name,
                                                    perm_type,
                                                    identity,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::HasInboxPermission { inbox_name, perm_type, identity, res } => {
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let db_clone = self.db.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::has_inbox_permission(
                                                    identity_manager_clone,
                                                    db_clone,
                                                    inbox_name,
                                                    perm_type,
                                                    identity,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::CreateJob { shinkai_message, res } => {
                                            let job_manager_clone = self.job_manager.clone().unwrap();
                                            let db_clone = self.db.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::local_create_new_job(
                                                    db_clone,
                                                    identity_manager_clone,
                                                    job_manager_clone,
                                                    shinkai_message,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::JobMessage { shinkai_message, res } => {
                                            let job_manager_clone = self.job_manager.clone().unwrap();
                                            tokio::spawn(async move {
                                                let _ = Node::local_job_message(
                                                    job_manager_clone,
                                                    shinkai_message,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::AddAgent { agent, profile, res } => {
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let job_manager_clone = self.job_manager.clone().unwrap();
                                            let db_clone = self.db.clone();
                                            let identity_secret_key_clone = self.identity_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::local_add_agent(
                                                    db_clone,
                                                    identity_manager_clone,
                                                    job_manager_clone,
                                                    identity_secret_key_clone,
                                                    agent,
                                                    &profile,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::AvailableAgents { full_profile_name, res } => {
                                            let db_clone = self.db.clone();
                                            let node_name_clone = self.node_name.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::local_available_agents(
                                                    db_clone,
                                                    &node_name_clone,
                                                    full_profile_name,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::LocalScanOllamaModels { res } => {
                                            tokio::spawn(async move {
                                                let _ = Node::local_scan_ollama_models(
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::AddOllamaModels { target_profile, models, res } => {
                                            let db_clone = self.db.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let job_manager_clone = self.job_manager.clone().unwrap();
                                            let identity_secret_key_clone = self.identity_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::local_add_ollama_models(
                                                    db_clone,
                                                    identity_manager_clone,
                                                    job_manager_clone,
                                                    identity_secret_key_clone,
                                                    models,
                                                    target_profile,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::APICreateRegistrationCode { msg, res } => {
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_create_and_send_registration_code(
                                                    encryption_secret_key_clone,
                                                    db_clone,
                                                    identity_manager_clone,
                                                    node_name_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::APIUseRegistrationCode { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vec_fs_clone = self.vector_fs.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let first_device_needs_registration_code = self.first_device_needs_registration_code;
                                            let embedding_generator_clone = Arc::new(self.embedding_generator.clone());
                                            let encryption_public_key_clone = self.encryption_public_key;
                                            let identity_public_key_clone = self.identity_public_key;
                                            let identity_secret_key_clone = self.identity_secret_key.clone();
                                            let initial_agents_clone = self.initial_agents.clone();
                                            let job_manager = self.job_manager.clone().unwrap();
                                            tokio::spawn(async move {
                                                let _ = Node::api_handle_registration_code_usage(
                                                    db_clone,
                                                    vec_fs_clone,
                                                    node_name_clone,
                                                    encryption_secret_key_clone,
                                                    first_device_needs_registration_code,
                                                    embedding_generator_clone,
                                                    identity_manager_clone,
                                                    job_manager,
                                                    encryption_public_key_clone,
                                                    identity_public_key_clone,
                                                    identity_secret_key_clone,
                                                    initial_agents_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::APIGetAllSubidentities { res } => {
                                            let identity_manager_clone = self.identity_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_get_all_profiles(
                                                    identity_manager_clone,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::APIGetLastMessagesFromInbox { msg, res } => {
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_get_last_messages_from_inbox(
                                                    encryption_secret_key_clone,
                                                    db_clone,
                                                    identity_manager_clone,
                                                    node_name_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::APIGetLastUnreadMessagesFromInbox { msg, res } => {
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_get_last_unread_messages_from_inbox(
                                                    encryption_secret_key_clone,
                                                    db_clone,
                                                    identity_manager_clone,
                                                    node_name_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::APIMarkAsReadUpTo { msg, res } => {
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_mark_as_read_up_to(
                                                    encryption_secret_key_clone,
                                                    db_clone,
                                                    identity_manager_clone,
                                                    node_name_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::APICreateJob { msg, res } => {
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let job_manager_clone = self.job_manager.clone().unwrap();
                                            let node_name_clone = self.node_name.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_create_new_job(
                                                    encryption_secret_key_clone,
                                                    db_clone,
                                                    identity_manager_clone,
                                                    node_name_clone,
                                                    job_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIGetAllInboxesForProfile { msg, res } => self.api_get_all_inboxes_for_profile(msg, res).await,
                                        NodeCommand::APIGetAllInboxesForProfile { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_get_all_inboxes_for_profile(
                                                    db_clone,
                                                    identity_manager_clone,
                                                    node_name_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIAddAgent { msg, res } => self.api_add_agent(msg, res).await,
                                        NodeCommand::APIAddAgent { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let job_manager_clone = self.job_manager.clone().unwrap();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let identity_secret_key_clone = self.identity_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_add_agent(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    job_manager_clone,
                                                    identity_secret_key_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIRemoveAgent { msg, res } => self.api_remove_agent(msg, res).await,
                                        NodeCommand::APIRemoveAgent { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_remove_agent(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIModifyAgent { msg, res } => self.api_modify_agent(msg, res).await,
                                        NodeCommand::APIModifyAgent { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_modify_agent(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        NodeCommand::APIJobMessage { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let job_manager_clone = self.job_manager.clone().unwrap();
                                            tokio::spawn(async move {
                                                let _ = Node::api_job_message(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    job_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIChangeJobAgent { msg, res } => self.api_change_job_agent(msg, res).await,
                                        NodeCommand::APIChangeJobAgent { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_change_job_agent(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIAvailableAgents { msg, res } => self.api_available_agents(msg, res).await,
                                        NodeCommand::APIAvailableAgents { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_available_agents(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res } => self.api_create_files_inbox_with_symmetric_key(msg, res).await,
                                        NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let encryption_public_key_clone = self.encryption_public_key;
                                            tokio::spawn(async move {
                                                let _ = Node::api_create_files_inbox_with_symmetric_key(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    encryption_public_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIGetFilenamesInInbox { msg, res } => self.api_get_filenames_in_inbox(msg, res).await,
                                        NodeCommand::APIGetFilenamesInInbox { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let encryption_public_key_clone = self.encryption_public_key;
                                            tokio::spawn(async move {
                                                let _ = Node::api_get_filenames_in_inbox(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    encryption_public_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIAddFileToInboxWithSymmetricKey { filename, file, public_key, encrypted_nonce, res } => self.api_add_file_to_inbox_with_symmetric_key(filename, file, public_key, encrypted_nonce, res).await,
                                        NodeCommand::APIAddFileToInboxWithSymmetricKey { filename, file, public_key, encrypted_nonce, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_add_file_to_inbox_with_symmetric_key(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    filename,
                                                    file,
                                                    public_key,
                                                    encrypted_nonce,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIGetAllSmartInboxesForProfile { msg, res } => self.api_get_all_smart_inboxes_for_profile(msg, res).await,
                                        NodeCommand::APIGetAllSmartInboxesForProfile { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_get_all_smart_inboxes_for_profile(
                                                    db_clone,
                                                    identity_manager_clone,
                                                    node_name_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIUpdateSmartInboxName { msg, res } => self.api_update_smart_inbox_name(msg, res).await,
                                        NodeCommand::APIUpdateSmartInboxName { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_update_smart_inbox_name(
                                                    encryption_secret_key_clone,
                                                    db_clone,
                                                    identity_manager_clone,
                                                    node_name_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIUpdateJobToFinished { msg, res } => self.api_update_job_to_finished(msg, res).await,
                                        NodeCommand::APIUpdateJobToFinished { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_update_job_to_finished(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIPrivateDevopsCronList { res } => self.api_private_devops_cron_list(res).await,
                                        NodeCommand::APIPrivateDevopsCronList { res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let node_name_clone = self.node_name.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_private_devops_cron_list(
                                                    db_clone,
                                                    node_name_clone,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIAddToolkit { msg, res } => self.api_add_toolkit(msg, res).await,
                                        NodeCommand::APIAddToolkit { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let js_toolkit_executor_remote = self.js_toolkit_executor_remote.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_add_toolkit(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    js_toolkit_executor_remote,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIListToolkits { msg, res } => self.api_list_toolkits(msg, res).await,
                                        NodeCommand::APIListToolkits { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_list_toolkits(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIScanOllamaModels { msg, res } => self.api_scan_ollama_models(msg, res).await,
                                        NodeCommand::APIScanOllamaModels { msg, res } => {
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_scan_ollama_models(
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIAddOllamaModels { msg, res } => self.api_add_ollama_models(msg, res).await,
                                        NodeCommand::APIAddOllamaModels { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let job_manager_clone = self.job_manager.clone().unwrap();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let identity_secret_key_clone = self.identity_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_add_ollama_models(
                                                    db_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    job_manager_clone,
                                                    identity_secret_key_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIChangeNodesName { msg, res } => self.api_change_nodes_name(msg, res).await,
                                        NodeCommand::APIChangeNodesName { msg, res } => {
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let encryption_public_key_clone = self.encryption_public_key;
                                            let identity_public_key_clone = self.identity_public_key;
                                            let secret_file_path = self.secrets_file_path.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_change_nodes_name(
                                                    secret_file_path.as_str(),
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    encryption_public_key_clone,
                                                    identity_public_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIIsPristine { res } => self.api_is_pristine(res).await,
                                        NodeCommand::APIIsPristine { res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            tokio::spawn(async move {
                                                let _ = Self::api_is_pristine(db_clone, res).await;
                                            });
                                        },
                                        // NodeCommand::IsPristine { res } => self.local_is_pristine(res).await,
                                        NodeCommand::IsPristine { res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            tokio::spawn(async move {
                                                let _ = Self::local_is_pristine(db_clone, res).await;
                                            });
                                        },
                                        // NodeCommand::GetNodeName { res: Sender<String> },
                                        NodeCommand::GetNodeName { res } => {
                                            let node_name = self.node_name.clone();
                                            tokio::spawn(async move {
                                                let _ = res.send(node_name.node_name).await;
                                            });
                                        },
                                        // NodeCommand::APIGetLastMessagesFromInboxWithBranches { msg, res } => self.api_get_last_messages_from_inbox_with_branches(msg, res).await,
                                        NodeCommand::APIGetLastMessagesFromInboxWithBranches { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_get_last_messages_from_inbox_with_branches(
                                                    encryption_secret_key_clone,
                                                    db_clone,
                                                    identity_manager_clone,
                                                    node_name_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::GetLastMessagesFromInboxWithBranches { inbox_name, limit, offset_key, res } => self.local_get_last_messages_from_inbox_with_branches(inbox_name, limit, offset_key, res).await,
                                        NodeCommand::GetLastMessagesFromInboxWithBranches { inbox_name, limit, offset_key, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            tokio::spawn(async move {
                                                let _ = Node::local_get_last_messages_from_inbox_with_branches(
                                                    db_clone,
                                                    inbox_name,
                                                    limit,
                                                    offset_key,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res } => self.api_vec_fs_retrieve_path_simplified_json(msg, res).await,
                                        NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_retrieve_path_simplified_json(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    ext_subscription_manager_clone,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res } => self.api_vec_fs_retrieve_path_minimal_json(msg, res).await,
                                        NodeCommand::APIVecFSRetrievePathMinimalJson { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_retrieve_path_minimal_json(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    ext_subscription_manager_clone,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIConvertFilesAndSaveToFolder { msg, res } => self.api_convert_files_and_save_to_folder(msg, res).await,
                                        NodeCommand::APIConvertFilesAndSaveToFolder { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let embedding_generator_clone = self.embedding_generator.clone();
                                            let unstructured_api_clone = self.unstructured_api.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_convert_files_and_save_to_folder(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    Arc::new(embedding_generator_clone),
                                                    Arc::new(unstructured_api_clone),
                                                    ext_subscription_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSRetrieveVectorSearchSimplifiedJson { msg, res } => self.api_vec_fs_retrieve_vector_search_simplified_json(msg, res).await,
                                        NodeCommand::APIVecFSRetrieveVectorSearchSimplifiedJson { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_retrieve_vector_search_simplified_json(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSSearchItems { msg, res } => self.api_vec_fs_search_items(msg, res).await,
                                        NodeCommand::APIVecFSSearchItems { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_search_items(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSCreateFolder { msg, res } => self.api_vec_fs_create_folder(msg, res).await,
                                        NodeCommand::APIVecFSCreateFolder { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_create_folder(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSMoveItem { msg, res } => self.api_vec_fs_move_item(msg, res).await,
                                        NodeCommand::APIVecFSMoveItem { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_move_item(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSCopyItem { msg, res } => self.api_vec_fs_copy_item(msg, res).await,
                                        NodeCommand::APIVecFSCopyItem { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_copy_item(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSMoveFolder { msg, res } => self.api_vec_fs_move_folder(msg, res).await,
                                        NodeCommand::APIVecFSMoveFolder { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_move_folder(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSCopyFolder { msg, res } => self.api_vec_fs_copy_folder(msg, res).await,
                                        NodeCommand::APIVecFSCopyFolder { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_copy_folder(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSRetrieveVectorResource { msg, res } => self.api_vec_fs_retrieve_vector_resource(msg, res).await,
                                        NodeCommand::APIVecFSRetrieveVectorResource { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_retrieve_vector_resource(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSDeleteFolder { msg, res } => self.api_vec_fs_delete_folder(msg, res).await,
                                        NodeCommand::APIVecFSDeleteFolder { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_delete_folder(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIVecFSDeleteItem { msg, res } => self.api_vec_fs_delete_item(msg, res).await,
                                        NodeCommand::APIVecFSDeleteItem { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_vec_fs_delete_item(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIAvailableSharedItems { msg, res } => self.api_subscription_available_shared_items(msg, res).await,
                                        NodeCommand::APIAvailableSharedItems { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            let my_subscription_manager_clone = self.my_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_subscription_available_shared_items(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    ext_subscription_manager_clone,
                                                    my_subscription_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIAvailableSharedItemsOpen { msg, res } => self.api_subscription_available_shared_items_open(msg, res).await,
                                        NodeCommand::APIAvailableSharedItemsOpen { msg, res } => {
                                            let node_name_clone = self.node_name.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_subscription_available_shared_items_open(
                                                    node_name_clone,
                                                    ext_subscription_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APICreateShareableFolder { msg, res } => self.api_subscription_create_shareable_folder(msg, res).await,
                                        NodeCommand::APICreateShareableFolder { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_subscription_create_shareable_folder(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    ext_subscription_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIUpdateShareableFolder { msg, res } => self.api_subscription_update_shareable_folder(msg, res).await,
                                        NodeCommand::APIUpdateShareableFolder { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_subscription_update_shareable_folder(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    ext_subscription_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIUnshareFolder { msg, res } => self.api_subscription_unshare_folder(msg, res).await,
                                        NodeCommand::APIUnshareFolder { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_subscription_unshare_folder(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    ext_subscription_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APISubscribeToSharedFolder { msg, res } => self.api_subscription_subscribe_to_shared_folder(msg, res).await,
                                        NodeCommand::APISubscribeToSharedFolder { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let my_subscription_manager_clone = self.my_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_subscription_subscribe_to_shared_folder(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    my_subscription_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIMySubscriptions { msg, res } => self.api_subscription_my_subscriptions(msg, res).await,
                                        NodeCommand::APIMySubscriptions { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_subscription_my_subscriptions(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIUnsubscribe { msg, res } => self.api_unsubscribe_my_subscriptions(msg, res).await,
                                        NodeCommand::APIUnsubscribe { msg, res } => {
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let my_subscription_manager_clone = self.my_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_unsubscribe_my_subscriptions(
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    my_subscription_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIGetMySubscribers { msg, res } => self.api_get_my_subscribers(msg, res).await,
                                        NodeCommand::APIGetMySubscribers { msg, res } => {
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_get_my_subscribers(
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    ext_subscription_manager_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::APIGetHttpFreeSubscriptionLinks { subscription_id: ShinkaiMessage, res: Sender<Result<Value, APIError>>, },
                                        NodeCommand::APIGetHttpFreeSubscriptionLinks { subscription_profile_path, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let node_name_clone = self.node_name.clone();
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::api_get_http_free_subscription_links(
                                                    db_clone,
                                                    node_name_clone,
                                                    ext_subscription_manager_clone,
                                                    subscription_profile_path,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::RetrieveVRKai { msg, res } => self.retrieve_vr_kai(msg, res).await,
                                        NodeCommand::RetrieveVRKai { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::retrieve_vr_kai(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::RetrieveVRPack { msg, res } => self.retrieve_vr_pack(msg, res).await,
                                        NodeCommand::RetrieveVRPack { msg, res } => {
                                            let db_clone = Arc::clone(&self.db);
                                            let vector_fs_clone = self.vector_fs.clone();
                                            let node_name_clone = self.node_name.clone();
                                            let identity_manager_clone = self.identity_manager.clone();
                                            let encryption_secret_key_clone = self.encryption_secret_key.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::retrieve_vr_pack(
                                                    db_clone,
                                                    vector_fs_clone,
                                                    node_name_clone,
                                                    identity_manager_clone,
                                                    encryption_secret_key_clone,
                                                    msg,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        // NodeCommand::LocalExtManagerProcessSubscriptionUpdates { res } => self.local_ext_manager_process_subscription_updates(res).await,
                                        NodeCommand::LocalExtManagerProcessSubscriptionUpdates { res } => {
                                            let ext_subscription_manager_clone = self.ext_subscription_manager.clone();
                                            tokio::spawn(async move {
                                                let _ = Node::local_ext_manager_process_subscription_updates(
                                                    ext_subscription_manager_clone,
                                                    res,
                                                ).await;
                                            });
                                        },
                                        _ => (),
                                    }
                            },
                            None => {
                                // do nothing
                            }
                        }
                    }
            };
        }
    }

    // A function that listens for incoming connections and tries to reconnect if a connection is lost.
    async fn listen_and_reconnect(&self, proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>) {
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("{} > TCP: Starting listen and reconnect loop.", self.listen_address),
        );

        let mut retry_count = 0;

        loop {
            // let listen_address = self.listen_address;
            let identity_manager = self.identity_manager.clone();
            let network_job_manager = self.network_job_manager.clone();
            // let conn_limiter = self.conn_limiter.clone();
            let node_name = self.node_name.clone();
            let identity_secret_key = self.identity_secret_key.clone();

            let proxy_info = {
                let proxy_info_lock = proxy_connection_info.lock().await;
                proxy_info_lock.clone()
            };

            if let Some(proxy_info) = proxy_info {
                let connection_result = Node::establish_proxy_connection(
                    identity_manager.clone(),
                    &proxy_info,
                    node_name,
                    identity_secret_key,
                )
                .await;

                match connection_result {
                    Ok(Some((reader, writer))) => {
                        let _ = Self::handle_proxy_listen_connection(
                            reader,
                            writer,
                            proxy_info.proxy_identity.clone(),
                            proxy_connection_info.clone(),
                            network_job_manager.clone(),
                            identity_manager.clone(),
                        )
                        .await;
                    }
                    Ok(None) | Err(_) => {
                        // Increment retry count and determine sleep duration
                        retry_count += 1;
                        let sleep_duration = match retry_count {
                            1 => Duration::from_secs(5),
                            2 => Duration::from_secs(10),
                            3 => Duration::from_secs(30),
                            _ => Duration::from_secs(300), // 5 minutes
                        };

                        tokio::time::sleep(sleep_duration).await;
                    }
                };
            } else {
                break;
            }
        }

        // Execute direct listening if no proxy was ever connected
        let result = Self::handle_listen_connection(
            self.listen_address,
            self.network_job_manager.clone(),
            self.conn_limiter.clone(),
            self.node_name.clone(),
        )
        .await;
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Error,
            &format!("{} > TCP: Listening error {:?}", self.listen_address, result),
        );
    }

    async fn establish_proxy_connection(
        identity_manager: Arc<Mutex<IdentityManager>>,
        proxy_info: &ProxyConnectionInfo,
        node_name: ShinkaiName,
        identity_secret_key: SigningKey,
    ) -> io::Result<
        Option<(
            Arc<Mutex<ReadHalf<tokio::net::TcpStream>>>,
            Arc<Mutex<WriteHalf<tokio::net::TcpStream>>>,
        )>,
    > {
        // If proxy connection info is provided, connect to the proxy
        let proxy_addr = Node::get_address_from_identity(
            identity_manager.clone(),
            &proxy_info.proxy_identity.get_node_name_string(),
        )
        .await;

        let proxy_addr = match proxy_addr {
            Ok(addr) => addr,
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to get proxy address: {}", e),
                );
                return Err(io::Error::new(io::ErrorKind::Other, e));
            }
        };

        let node_name = node_name.clone();
        let signing_sk = identity_secret_key.clone();

        match TcpStream::connect(proxy_addr).await {
            Ok(proxy_stream) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Info,
                    &format!("Connected to proxy at {}", proxy_addr),
                );

                // Split the socket into reader and writer
                let (reader, writer) = tokio::io::split(proxy_stream);
                let reader = Arc::new(Mutex::new(reader));
                let writer = Arc::new(Mutex::new(writer));

                // Send the initial connection message
                let identity_msg = NetworkMessage {
                    identity: node_name.to_string(),
                    message_type: NetworkMessageType::ProxyMessage,
                    payload: Vec::new(),
                };
                Self::send_network_message(writer.clone(), &identity_msg).await;

                // Authenticate identity or localhost
                Self::authenticate_identity_or_localhost(reader.clone(), writer.clone(), &signing_sk).await;

                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Info,
                    &format!("Authenticated identity or localhost at {}", proxy_addr),
                );

                // Return the reader and writer so they can be handled
                Ok(Some((reader, writer)))
            }
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to connect to proxy {}: {}", proxy_addr, e),
                );
                Err(e)
            }
        }
    }

    async fn handle_proxy_listen_connection(
        reader: Arc<Mutex<ReadHalf<tokio::net::TcpStream>>>,
        writer: Arc<Mutex<WriteHalf<tokio::net::TcpStream>>>,
        proxy_identity: ShinkaiName,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        network_job_manager: Arc<Mutex<NetworkJobManager>>,
        identity_manager: Arc<Mutex<IdentityManager>>,
    ) -> io::Result<()> {
        // Store the tcp_connection in proxy_connection_info
        {
            let mut proxy_info_lock = proxy_connection_info.lock().await;
            if let Some(ref mut proxy_info) = *proxy_info_lock {
                proxy_info.tcp_connection = Some((reader.clone(), writer.clone()));
            }
        }

        // Handle the connection
        loop {
            let reader_clone = Arc::clone(&reader);
            let network_job_manager_clone = Arc::clone(&network_job_manager);
            let identity_manager = identity_manager.clone();
            let proxy_identity = proxy_identity.clone();

            let handle = tokio::spawn(async move {
                // If proxy connection info is provided, connect to the proxy
                let proxy_addr = Node::get_address_from_identity(
                    identity_manager.clone(),
                    &proxy_identity.clone().get_node_name_string(),
                )
                .await;

                let proxy_addr = match proxy_addr {
                    Ok(addr) => addr,
                    Err(e) => {
                        eprintln!("Failed to get proxy address: {}", e);
                        return Err(io::Error::new(io::ErrorKind::Other, e));
                    }
                };
                Self::handle_connection(reader_clone, proxy_addr, network_job_manager_clone).await;
                Ok::<(), std::io::Error>(())
            });

            // Await the task's completion
            if let Err(e) = handle.await {
                eprintln!("Task failed: {:?}", e);
                // Sleep for 50ms before retrying
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }

    async fn handle_listen_connection(
        listen_address: SocketAddr,
        network_job_manager: Arc<Mutex<NetworkJobManager>>,
        conn_limiter: Arc<ConnectionLimiter>,
        node_name: ShinkaiName,
    ) -> io::Result<()> {
        let listener = TcpListener::bind(&listen_address).await?;

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("{} > TCP: Listening on {} (Direct)", listen_address, listen_address),
        );

        // Initialize your connection limiter
        loop {
            let (socket, addr) = listener.accept().await?;

            // Too many requests by IP protection
            let ip = addr.ip().to_string();
            let conn_limiter_clone = conn_limiter.clone();

            if !conn_limiter_clone.check_rate_limit(&ip).await {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Info,
                    &format!("Rate limit exceeded for IP: {}", ip),
                );
                continue;
            }

            if !conn_limiter_clone.increment_connection(&ip).await {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Info,
                    &format!("Too many connections from IP: {}", ip),
                );
                continue;
            }

            let network_job_manager = Arc::clone(&network_job_manager);
            let conn_limiter_clone = conn_limiter.clone();

            eprintln!("loop before spawn for normal socket");
            tokio::spawn(async move {
                let (reader, _writer) = tokio::io::split(socket);
                let reader = Arc::new(Mutex::new(reader));
                Self::handle_connection(reader, addr, network_job_manager).await;
                conn_limiter_clone.decrement_connection(&ip).await;
            });
        }
    }

    // Static function to get the address from a ShinkaiName identity
    async fn get_address_from_identity(
        identity_manager: Arc<Mutex<IdentityManager>>,
        proxy_identity: &str,
    ) -> Result<SocketAddr, String> {
        let identity_manager = identity_manager.lock().await;
        match identity_manager
            .external_profile_to_global_identity(proxy_identity)
            .await
        {
            Ok(identity) => {
                if let Some(proxy_addr) = identity.addr {
                    Ok(proxy_addr)
                } else {
                    Err(format!("No address found for proxy identity: {}", proxy_identity))
                }
            }
            Err(e) => Err(format!("Failed to resolve proxy identity {}: {}", proxy_identity, e)),
        }
    }

    async fn handle_connection(
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        addr: SocketAddr,
        network_job_manager: Arc<Mutex<NetworkJobManager>>,
    ) {
        let mut length_bytes = [0u8; 4];
        {
            let mut reader = reader.lock().await;
            if reader.read_exact(&mut length_bytes).await.is_ok() {
                let total_length = u32::from_be_bytes(length_bytes) as usize;

                // Read the identity length
                let mut identity_length_bytes = [0u8; 4];
                if reader.read_exact(&mut identity_length_bytes).await.is_err() {
                    return; // Exit if we fail to read identity length
                }
                let identity_length = u32::from_be_bytes(identity_length_bytes) as usize;

                // Read the identity bytes
                let mut identity_bytes = vec![0u8; identity_length];
                if reader.read_exact(&mut identity_bytes).await.is_err() {
                    return; // Exit if we fail to read identity
                }

                // Calculate the message length excluding the identity length and the identity itself
                let msg_length = total_length - 1 - 4 - identity_length; // Subtract 1 for the header and 4 for the identity length bytes

                // Initialize buffer to fit the message
                let mut buffer = vec![0u8; msg_length];

                // Read the header byte to determine the message type
                let mut header_byte = [0u8; 1];
                if reader.read_exact(&mut header_byte).await.is_ok() {
                    let message_type = match header_byte[0] {
                        0x01 => NetworkMessageType::ShinkaiMessage,
                        0x02 => NetworkMessageType::VRKaiPathPair,
                        0x03 => NetworkMessageType::ProxyMessage,
                        _ => {
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                "Received message with unknown type identifier",
                            );
                            return; // Exit the task if the message type is unknown
                        }
                    };

                    // Read the rest of the message into the buffer
                    if reader.read_exact(&mut buffer).await.is_ok() {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Info,
                            &format!("Received message of type {:?} from: {:?}", message_type, addr),
                        );

                        let network_job = NetworkJobQueue {
                            receiver_address: addr, // TODO: this should be my socketaddr!
                            unsafe_sender_address: addr,
                            message_type,
                            content: buffer.clone(), // Now buffer does not include the header
                            date_created: Utc::now(),
                        };

                        let mut network_job_manager = network_job_manager.lock().await;
                        if let Err(e) = network_job_manager.add_network_job_to_queue(&network_job).await {
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to add network job to queue: {}", e),
                            );
                        }
                    } else {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to read message from: {:?}", addr),
                        );
                    }
                }
            } else {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to read message length from: {:?}", addr),
                );
            }
        }
    }

    async fn retry_messages(
        db: Arc<ShinkaiDB>,
        encryption_secret_key: EncryptionStaticKey,
        identity_manager: Arc<Mutex<IdentityManager>>,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ) -> Result<(), NodeError> {
        let messages_to_retry = db.get_messages_to_retry_before(None)?;

        for retry_message in messages_to_retry {
            let encrypted_secret_key = clone_static_secret_key(&encryption_secret_key);
            let save_to_db_flag = retry_message.save_to_db_flag;
            let retry = Some(retry_message.retry_count);

            // Remove the message from the retry queue
            db.remove_message_from_retry(&retry_message.message).unwrap();

            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Info,
                &format!("Retrying message: {:?}", retry_message.message),
            );

            // Retry the message
            Node::send(
                retry_message.message,
                Arc::new(encrypted_secret_key),
                retry_message.peer,
                proxy_connection_info.clone(),
                db.clone(),
                identity_manager.clone(),
                save_to_db_flag,
                retry,
            );
        }

        Ok(())
    }

    // indicates if the node is ready or not
    pub async fn is_node_ready(&self) -> bool {
        let identity_manager_guard = self.identity_manager.lock().await;
        identity_manager_guard.is_ready
    }

    // Get a list of peers this node knows about.
    pub fn get_peers(&self) -> CHashMap<(SocketAddr, ProfileName), chrono::DateTime<Utc>> {
        self.peers.clone()
    }

    // TODO: Add a new send that schedules messages to be sent at a later time.
    // It may be more complex than what it sounds because there could be a big backlog of messages to send which were already generated
    // and the time associated with the message may be too old to be recognized by the other node.
    // so most likely we need a way to update the messages (they are signed by this node after all) so it can update the time to the current time

    // Send a message to a peer.
    #[allow(clippy::too_many_arguments)]
    pub fn send(
        message: ShinkaiMessage,
        my_encryption_sk: Arc<EncryptionStaticKey>,
        peer: (SocketAddr, ProfileName),
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        db: Arc<ShinkaiDB>,
        maybe_identity_manager: Arc<Mutex<IdentityManager>>,
        save_to_db_flag: bool,
        retry: Option<u32>,
    ) {
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("Sending {:?} to {:?}", message, peer),
        );
        let address = peer.0;
        let message = Arc::new(message);

        tokio::spawn(async move {
            let writer = Node::get_writer(address, proxy_connection_info, maybe_identity_manager.clone()).await;

            if let Some(writer) = writer {
                let encoded_msg = message.encode_message().unwrap();
                let identity = &message.external_metadata.recipient;
                let identity_bytes = identity.as_bytes();
                let identity_length = (identity_bytes.len() as u32).to_be_bytes();

                // Prepare the message with a length prefix and identity length
                let total_length = (encoded_msg.len() as u32 + 1 + identity_bytes.len() as u32 + 4).to_be_bytes(); // Convert the total length to bytes, adding 1 for the header and 4 for the identity length

                let mut data_to_send = Vec::new();
                let header_data_to_send = vec![0x01]; // Message type identifier for ShinkaiMessage
                data_to_send.extend_from_slice(&total_length);
                data_to_send.extend_from_slice(&identity_length);
                data_to_send.extend(identity_bytes);
                data_to_send.extend(header_data_to_send);
                data_to_send.extend_from_slice(&encoded_msg);

                {
                    let mut writer = writer.lock().await;
                    let _ = writer.write_all(&data_to_send).await;
                    let _ = writer.flush().await;
                }

                if save_to_db_flag {
                    let _ = Node::save_to_db(
                        true,
                        &message,
                        Arc::clone(&my_encryption_sk).as_ref().clone(),
                        db.clone(),
                        maybe_identity_manager.clone(),
                    )
                    .await;
                }
            } else {
                // If retry is enabled, add the message to retry list on failure
                let retry_count = retry.unwrap_or(0) + 1;
                let retry_message = RetryMessage {
                    retry_count,
                    message: message.as_ref().clone(),
                    peer: peer.clone(),
                    save_to_db_flag,
                };
                // Calculate the delay for the next retry
                let delay_seconds = 4_u64.pow(retry_count - 1);
                let retry_time = Utc::now() + chrono::Duration::seconds(delay_seconds as i64);
                db.add_message_to_retry(&retry_message, retry_time).unwrap();
            }
        });
    }

    /// Function to get the writer, either directly or through a proxy
    async fn get_writer(
        address: SocketAddr,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        _identity_manager: Arc<Mutex<IdentityManager>>,
        // node_name: ShinkaiName,
        // identity_secret_key: SigningKey,
    ) -> Option<Arc<Mutex<WriteHalf<TcpStream>>>> {
        let proxy_connection = proxy_connection_info.lock().await;
        if let Some(proxy_info) = proxy_connection.as_ref() {
            if let Some((_, writer)) = &proxy_info.tcp_connection {
                Some(writer.clone())
            } else {
                None
            }
        } else {
            match TcpStream::connect(address).await {
                Ok(stream) => {
                    let (_, writer) = tokio::io::split(stream);
                    Some(Arc::new(Mutex::new(writer)))
                }
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to connect to {}: {}", address, e),
                    );
                    None
                }
            }
        }
    }

    pub async fn send_encrypted_vrpack(
        vr_pack_plus_changes: VRPackPlusChanges,
        subscription_id: SubscriptionId,
        encryption_key_hex: String,
        peer: SocketAddr,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        maybe_identity_manager: Arc<Mutex<IdentityManager>>,
        recipient: ShinkaiName,
    ) {
        tokio::spawn(async move {
            // Serialize only the VRKaiPath pairs
            let serialized_data = bincode::serialize(&vr_pack_plus_changes).unwrap();
            let encryption_key = hex::decode(encryption_key_hex.clone()).unwrap();
            let key = GenericArray::from_slice(&encryption_key);
            let cipher = Aes256Gcm::new(key);

            // Generate a random nonce
            let mut nonce = [0u8; 12];
            rand::thread_rng().fill(&mut nonce);
            let nonce_generic = GenericArray::from_slice(&nonce);

            // Encrypt the data
            let encrypted_data = cipher
                .encrypt(nonce_generic, serialized_data.as_ref())
                .expect("encryption failure!");

            // Calculate the hash of the symmetric key
            let mut hasher = blake3::Hasher::new();
            hasher.update(encryption_key_hex.as_bytes());
            let result = hasher.finalize();
            let symmetric_key_hash = hex::encode(result.as_bytes());

            // Create the NetworkVRKai struct with the encrypted pairs, subscription ID, nonce, and symmetric key hash
            let vr_kai = NetworkVRKai {
                enc_pairs: encrypted_data,
                subscription_id,
                nonce: hex::encode(nonce),
                symmetric_key_hash,
            };
            let vr_kai_serialized = bincode::serialize(&vr_kai).unwrap();

            let identity = recipient.get_node_name_string();
            let identity_bytes = identity.as_bytes();
            let identity_length = (identity_bytes.len() as u32).to_be_bytes();

            // Prepare the message with a length prefix, identity length, and identity
            let total_length = (vr_kai_serialized.len() as u32 + 1 + identity_bytes.len() as u32 + 4).to_be_bytes(); // Convert the total length to bytes, adding 1 for the header and 4 for the identity length

            let mut data_to_send = Vec::new();
            let header_data_to_send = vec![0x02]; // Network Message type identifier for VRKaiPathPair
            data_to_send.extend_from_slice(&total_length);
            data_to_send.extend_from_slice(&identity_length);
            data_to_send.extend(identity_bytes);
            data_to_send.extend(header_data_to_send);
            data_to_send.extend_from_slice(&vr_kai_serialized);

            // Get the stream using the get_stream function
            let writer = Node::get_writer(peer, proxy_connection_info, maybe_identity_manager).await;

            if let Some(writer) = writer {
                let mut writer = writer.lock().await;
                let _ = writer.write_all(&data_to_send).await;
                let _ = writer.flush().await;
            } else {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to connect to {}", peer),
                );
            }
        });
    }

    pub async fn save_to_db(
        am_i_sender: bool,
        message: &ShinkaiMessage,
        my_encryption_sk: EncryptionStaticKey,
        db: Arc<ShinkaiDB>,
        maybe_identity_manager: Arc<Mutex<IdentityManager>>,
    ) -> io::Result<()> {
        // We want to save it decrypted if possible
        // We are just going to check for the body encryption

        let is_body_encrypted = message.is_body_currently_encrypted();

        // Clone the message to get a fully owned version
        let mut message_to_save = message.clone();

        // The body should only be decrypted if it's currently encrypted.
        if is_body_encrypted {
            let mut counterpart_identity: String = "".to_string();
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Debug,
                &format!("save_to_db> message: {:?}", message.clone()),
            );
            if am_i_sender {
                counterpart_identity = ShinkaiName::from_shinkai_message_only_using_recipient_node_name(message)
                    .unwrap()
                    .to_string();
            } else {
                counterpart_identity = ShinkaiName::from_shinkai_message_only_using_sender_node_name(message)
                    .unwrap()
                    .to_string();
            }
            // find the sender's encryption public key in external
            let sender_encryption_pk = maybe_identity_manager
                .lock()
                .await
                .external_profile_to_global_identity(&counterpart_identity.clone())
                .await
                .unwrap()
                .node_encryption_public_key;

            // Decrypt the message body
            let decrypted_result = message.decrypt_outer_layer(&my_encryption_sk, &sender_encryption_pk);
            match decrypted_result {
                Ok(decrypted_content) => {
                    message_to_save = decrypted_content;
                }
                Err(e) => {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!(
                            "save_to_db> my_encrypt_sk: {:?}",
                            encryption_secret_key_to_string(my_encryption_sk)
                        ),
                    );
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!(
                            "save_to_db> sender_encrypt_pk: {:?}",
                            encryption_public_key_to_string(sender_encryption_pk)
                        ),
                    );
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("save_to_db> Failed to decrypt message body: {}", e),
                    );
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("save_to_db> For message: {:?}", message),
                    );
                    return Err(io::Error::new(io::ErrorKind::Other, "Failed to decrypt message body"));
                }
            }
        }

        // TODO: add identity to this fn so we can check for permissions
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("save_to_db> message_to_save: {:?}", message_to_save.clone()),
        );
        let db_result = db.unsafe_insert_inbox_message(&message_to_save, None).await;
        match db_result {
            Ok(_) => (),
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to insert message into inbox: {}", e),
                );
                // we will panic for now because that way we can be aware that something is off
                // NOTE: we shouldn't panic on production!
                panic!("Failed to insert message into inbox: {}", e);
            }
        }
        Ok(())
    }

    async fn send_network_message(writer: Arc<Mutex<WriteHalf<TcpStream>>>, msg: &NetworkMessage) {
        eprintln!("send_network_message> Sending message: {:?}", msg);
        let encoded_msg = msg.payload.clone();
        let identity = &msg.identity;
        let identity_bytes = identity.as_bytes();
        let identity_length = (identity_bytes.len() as u32).to_be_bytes();

        // Prepare the message with a length prefix and identity length
        let total_length = (encoded_msg.len() as u32 + 1 + identity_bytes.len() as u32 + 4).to_be_bytes();

        let mut data_to_send = Vec::new();
        let header_data_to_send = vec![match msg.message_type {
            NetworkMessageType::ShinkaiMessage => 0x01,
            NetworkMessageType::VRKaiPathPair => 0x02,
            NetworkMessageType::ProxyMessage => 0x03,
        }];
        data_to_send.extend_from_slice(&total_length);
        data_to_send.extend_from_slice(&identity_length);
        data_to_send.extend(identity_bytes);
        data_to_send.extend(header_data_to_send);
        data_to_send.extend_from_slice(&encoded_msg);

        // Print the name and length of each component
        let mut writer = writer.lock().await;
        writer.write_all(&data_to_send).await.unwrap();
        writer.flush().await.unwrap();
    }

    async fn authenticate_identity_or_localhost(
        reader: Arc<Mutex<ReadHalf<TcpStream>>>,
        writer: Arc<Mutex<WriteHalf<TcpStream>>>,
        signing_key: &SigningKey,
    ) {
        // Handle validation
        let mut len_buffer = [0u8; 4];
        {
            let mut reader = reader.lock().await;
            reader.read_exact(&mut len_buffer).await.unwrap();
        }
        let validation_data_len = u32::from_be_bytes(len_buffer) as usize;

        let mut buffer = vec![0u8; validation_data_len];
        let res = {
            let mut reader = reader.lock().await;
            reader.read_exact(&mut buffer).await
        };
        match res {
            Ok(_) => {
                let validation_data = String::from_utf8(buffer).unwrap().trim().to_string();

                // Sign the validation data
                let signature = signing_key.sign(validation_data.as_bytes());
                let signature_hex = hex::encode(signature.to_bytes());

                // Get the public key
                let public_key = signing_key.verifying_key();
                let public_key_bytes = public_key.to_bytes();
                let public_key_hex = hex::encode(public_key_bytes);

                // Send the length of the public key and signed validation data back to the server
                let public_key_len = public_key_hex.len() as u32;
                let signature_len = signature_hex.len() as u32;
                let total_len = public_key_len + signature_len + 8; // 8 bytes for the lengths

                let total_len_bytes = (total_len as u32).to_be_bytes();
                {
                    let mut writer = writer.lock().await;
                    writer.write_all(&total_len_bytes).await.unwrap();

                    // Send the length of the public key
                    let public_key_len_bytes = public_key_len.to_be_bytes();
                    writer.write_all(&public_key_len_bytes).await.unwrap();

                    // Send the public key
                    writer.write_all(public_key_hex.as_bytes()).await.unwrap();

                    // Send the length of the signed validation data
                    let signature_len_bytes = signature_len.to_be_bytes();
                    writer.write_all(&signature_len_bytes).await.unwrap();

                    // Send the signed validation data
                    match writer.write_all(signature_hex.as_bytes()).await {
                        Ok(_) => eprintln!("Sent signed validation data and public key back to server"),
                        Err(e) => eprintln!("Failed to send signed validation data: {}", e),
                    }
                }

                // Wait for the server to validate the signature
                let mut len_buffer = [0u8; 4];
                {
                    let mut reader = reader.lock().await;
                    reader.read_exact(&mut len_buffer).await.unwrap();
                }
                let response_len = u32::from_be_bytes(len_buffer) as usize;

                let mut response_buffer = vec![0u8; response_len];
                {
                    let mut reader = reader.lock().await;
                    reader.read_exact(&mut response_buffer).await.unwrap();
                }
                let response = String::from_utf8(response_buffer).unwrap();

                // Assert the validation response
                if response != "Validation successful" {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to validate the identity: {}", response),
                    );
                }
            }
            Err(e) => eprintln!("Failed to read validation data: {}", e),
        }
    }
}
