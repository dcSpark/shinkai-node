use super::network_manager::network_job_manager::{NetworkJobManager, NetworkJobQueue};
use super::node_api::{APIError, APIUseRegistrationCodeSuccessResponse, SendResponseBody, SendResponseBodyData};
use super::node_error::NodeError;
use super::subscription_manager::external_subscriber_manager::ExternalSubscriberManager;
use super::subscription_manager::my_subscription_manager::{self, MySubscriptionsManager};
use crate::agent::job_manager::JobManager;
use crate::cron_tasks::cron_manager::CronManager;
use crate::db::db_retry::RetryMessage;
use crate::db::ShinkaiDB;
use crate::managers::IdentityManager;
use crate::network::network_limiter::ConnectionLimiter;
use crate::schemas::identity::{Identity, StandardIdentity};
use crate::schemas::smart_inbox::SmartInbox;
use crate::vector_fs::vector_fs::VectorFS;
use async_channel::{Receiver, Sender};
use chashmap::CHashMap;
use chrono::Utc;
use core::panic;
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::{future::FutureExt, pin_mut, prelude::*, select};
use lazy_static::lazy_static;
use shinkai_message_primitives::schemas::agents::serialized_agent::SerializedAgent;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    IdentityPermissions, JobToolCall, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, encryption_public_key_to_string, encryption_secret_key_to_string,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_vector_resources::model_type::{EmbeddingModelType, TextEmbeddingsInference};
use shinkai_vector_resources::unstructured::unstructured_api::UnstructuredAPI;
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
    APIRemoveInboxPermission {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    RemoveInboxPermission {
        inbox_name: String,
        perm_type: String,
        identity: String,
        res: Sender<String>,
    },
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
    JobMessage {
        shinkai_message: ShinkaiMessage,
        res: Sender<(String, String)>,
    },
    APIJobPreMessage {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    JobPreMessage {
        tool_calls: Vec<JobToolCall>,
        content: String,
        recipient: String,
        res: Sender<(String, String)>,
    },
    APIAddAgent {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    AddAgent {
        agent: SerializedAgent,
        profile: ShinkaiName,
        res: Sender<String>,
    },
    APIAvailableAgents {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<SerializedAgent>, APIError>>,
    },
    AvailableAgents {
        full_profile_name: String,
        res: Sender<Result<Vec<SerializedAgent>, String>>,
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
    LocalScanOllamaModels {
        res: Sender<Result<Vec<String>, String>>,
    },
    AddOllamaModels {
        models: Vec<String>,
        res: Sender<Result<(), String>>,
    },
    APIVecFSRetrievePathSimplifiedJson {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSRetrieveVectorResource {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>,
    },
    APIVecFSRetrieveVectorSearchSimplifiedJson {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<(String, Vec<String>, f32)>, APIError>>,
    },
    APIConvertFilesAndSaveToFolder {
        msg: ShinkaiMessage,
        res: Sender<Result<Vec<String>, APIError>>,
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
        res: Sender<Result<String, APIError>>,
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
    APIMySubscriptions {
        msg: ShinkaiMessage,
        res: Sender<Result<String, APIError>>, 
    }
}

/// Hard-coded embedding model that is set as the default when creating a new profile.
pub static NEW_PROFILE_DEFAULT_EMBEDDING_MODEL: EmbeddingModelType =
    EmbeddingModelType::TextEmbeddingsInference(TextEmbeddingsInference::AllMiniLML6v2);

lazy_static! {
    /// Hard-coded list of supported embedding models that is set when creating a new profile.
    /// These need to match the list that our Embedding server orchestration service supports.
    pub static ref NEW_PROFILE_SUPPORTED_EMBEDDING_MODELS: Vec<EmbeddingModelType> = vec![NEW_PROFILE_DEFAULT_EMBEDDING_MODEL.clone()];
}

// A type alias for a string that represents a profile name.
type ProfileName = String;

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
    // A map of known peer nodes.
    pub peers: CHashMap<(SocketAddr, ProfileName), chrono::DateTime<Utc>>,
    // The interval at which this node pings all known peers.
    pub ping_interval_secs: u64,
    // The channel from which this node receives commands.
    pub commands: Receiver<NodeCommand>,
    // The manager for subidentities.
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    // The database connection for this node.
    pub db: Arc<Mutex<ShinkaiDB>>,
    // First device needs registration code
    pub first_device_needs_registration_code: bool,
    // Initial Agent to auto-add on first registration
    pub initial_agents: Vec<SerializedAgent>,
    // The Job manager
    pub job_manager: Option<Arc<Mutex<JobManager>>>,
    // Cron Manager
    pub cron_manager: Option<Arc<Mutex<CronManager>>>,
    // JS Toolkit Executor Remote
    pub js_toolkit_executor_remote: Option<String>,
    // The Node's VectorFS
    pub vector_fs: Arc<Mutex<VectorFS>>,
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
}

impl Node {
    // Construct a new node. Returns a `Result` which is `Ok` if the node was successfully created,
    // and `Err` otherwise.
    pub async fn new(
        node_name: String,
        listen_address: SocketAddr,
        identity_secret_key: SigningKey,
        encryption_secret_key: EncryptionStaticKey,
        ping_interval_secs: u64,
        commands: Receiver<NodeCommand>,
        main_db_path: String,
        first_device_needs_registration_code: bool,
        initial_agents: Vec<SerializedAgent>,
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
        let db_arc = Arc::new(Mutex::new(db));
        let identity_public_key = identity_secret_key.verifying_key();
        let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);
        let node_name = ShinkaiName::new(node_name).unwrap();
        {
            let db_lock = db_arc.lock().await;
            match db_lock.update_local_node_keys(
                node_name.clone(),
                encryption_public_key.clone(),
                identity_public_key.clone(),
            ) {
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
            let db_lock = db_arc.lock().await;
            profile_list = match db_lock.get_all_profiles(node_name.clone()) {
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
        .unwrap_or_else(|e| {
            eprintln!("Error: {:?}", e);
            panic!("Failed to load VectorFS from database: {}", vector_fs_db_path)
        });
        let vector_fs_arc = Arc::new(Mutex::new(vector_fs));

        let conn_limiter = Arc::new(ConnectionLimiter::new(5, 10, 3)); // TODO: allow for ENV to set this

        let ext_subscriber_manager = Arc::new(Mutex::new(
            ExternalSubscriberManager::new(
                Arc::downgrade(&db_arc),
                Arc::downgrade(&vector_fs_arc),
                Arc::downgrade(&identity_manager),
                node_name.clone(),
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
        )
        .await;

        let node = Arc::new(Mutex::new(Node {
            node_name: node_name.clone(),
            identity_secret_key: clone_signature_secret_key(&identity_secret_key),
            identity_public_key,
            encryption_secret_key: clone_static_secret_key(&encryption_secret_key),
            encryption_public_key,
            peers: CHashMap::new(),
            listen_address,
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
            my_subscription_manager: my_subscription_manager,
            network_job_manager: Arc::new(Mutex::new(network_manager)),
        }));

        node
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
                vector_fs_weak,
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
                    clone_signature_secret_key(&self.identity_secret_key),
                    self.node_name.clone(),
                    Arc::clone(job_manager),
                )
                .await,
            ))),
            None => None,
        };

        let listen_future = self.listen_and_reconnect().fuse();
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

        loop {
            let ping_future = ping_interval.next().fuse();
            let commands_future = commands_clone.next().fuse();
            let retry_future = retry_interval.next().fuse();

            // TODO: update this to read onchain data and update db
            // let check_peers_future = check_peers_interval.next().fuse();
            pin_mut!(ping_future, commands_future, retry_future);

            select! {
                    _retry = retry_future => self.retry_messages().await?,
                    _listen = listen_future => unreachable!(),
                    _ping = ping_future => self.ping_all().await?,
                    // check_peers = check_peers_future => self.connect_new_peers().await?,
                    command = commands_future => {
                        match command {
                            Some(NodeCommand::Shutdown) => {
                                shinkai_log(ShinkaiLogOption::Node, ShinkaiLogLevel::Info, "Shutdown command received. Stopping the node.");
                            // self.db = Arc::new(Mutex::new(ShinkaiDB::new("PLACEHOLDER").expect("Failed to create a temporary database")));
                                break;
                            },
                            Some(NodeCommand::PingAll) => self.ping_all().await?,
                            Some(NodeCommand::GetPeers(sender)) => self.send_peer_addresses(sender).await?,
                            Some(NodeCommand::IdentityNameToExternalProfileData { name, res }) => self.handle_external_profile_data(name, res).await?,
                            Some(NodeCommand::SendOnionizedMessage { msg, res }) => self.api_handle_send_onionized_message(msg, res).await?,
                            Some(NodeCommand::GetPublicKeys(res)) => self.send_public_keys(res).await?,
                            Some(NodeCommand::FetchLastMessages { limit, res }) => self.fetch_and_send_last_messages(limit, res).await?,
                            Some(NodeCommand::GetAllSubidentitiesDevicesAndAgents(res)) => self.local_get_all_subidentities_devices_and_agents(res).await,
                            Some(NodeCommand::LocalCreateRegistrationCode { permissions, code_type, res }) => self.local_create_and_send_registration_code(permissions, code_type, res).await?,
                            Some(NodeCommand::GetLastMessagesFromInbox { inbox_name, limit, offset_key, res }) => self.local_get_last_messages_from_inbox(inbox_name, limit, offset_key, res).await,
                            Some(NodeCommand::MarkAsReadUpTo { inbox_name, up_to_time, res }) => self.local_mark_as_read_up_to(inbox_name, up_to_time, res).await,
                            Some(NodeCommand::GetLastUnreadMessagesFromInbox { inbox_name, limit, offset, res }) => self.local_get_last_unread_messages_from_inbox(inbox_name, limit, offset, res).await,
                            Some(NodeCommand::AddInboxPermission { inbox_name, perm_type, identity, res }) => self.local_add_inbox_permission(inbox_name, perm_type, identity, res).await,
                            Some(NodeCommand::RemoveInboxPermission { inbox_name, perm_type, identity, res }) => self.local_remove_inbox_permission(inbox_name, perm_type, identity, res).await,
                            Some(NodeCommand::HasInboxPermission { inbox_name, perm_type, identity, res }) => self.has_inbox_permission(inbox_name, perm_type, identity, res).await,
                            Some(NodeCommand::CreateJob { shinkai_message, res }) => self.local_create_new_job(shinkai_message, res).await,
                            Some(NodeCommand::JobMessage { shinkai_message, res: _ }) => self.internal_job_message(shinkai_message).await?,
                            Some(NodeCommand::AddAgent { agent, profile, res }) => self.local_add_agent(agent, &profile, res).await,
                            Some(NodeCommand::AvailableAgents { full_profile_name, res }) => self.local_available_agents(full_profile_name, res).await,
                            Some(NodeCommand::LocalScanOllamaModels { res }) => self.local_scan_ollama_models(res).await,
                            Some(NodeCommand::AddOllamaModels { models, res }) => self.local_add_ollama_models(models, res).await,
                            // Some(NodeCommand::JobPreMessage { tool_calls, content, recipient, res }) => self.job_pre_message(tool_calls, content, recipient, res).await?,
                            // API Endpoints
                            Some(NodeCommand::APICreateRegistrationCode { msg, res }) => self.api_create_and_send_registration_code(msg, res).await?,
                            Some(NodeCommand::APIUseRegistrationCode { msg, res }) => self.api_handle_registration_code_usage(msg, res).await?,
                            Some(NodeCommand::APIGetAllSubidentities { res }) => self.api_get_all_profiles(res).await?,
                            Some(NodeCommand::APIGetLastMessagesFromInbox { msg, res }) => self.api_get_last_messages_from_inbox(msg, res).await?,
                            Some(NodeCommand::APIGetLastUnreadMessagesFromInbox { msg, res }) => self.api_get_last_unread_messages_from_inbox(msg, res).await?,
                            Some(NodeCommand::APIMarkAsReadUpTo { msg, res }) => self.api_mark_as_read_up_to(msg, res).await?,
                            // Some(NodeCommand::APIAddInboxPermission { msg, res }) => self.api_add_inbox_permission(msg, res).await?,
                            // Some(NodeCommand::APIRemoveInboxPermission { msg, res }) => self.api_remove_inbox_permission(msg, res).await?,
                            Some(NodeCommand::APICreateJob { msg, res }) => self.api_create_new_job(msg, res).await?,
                            Some(NodeCommand::APIGetAllInboxesForProfile { msg, res }) => self.api_get_all_inboxes_for_profile(msg, res).await?,
                            Some(NodeCommand::APIAddAgent { msg, res }) => self.api_add_agent(msg, res).await?,
                            Some(NodeCommand::APIJobMessage { msg, res }) => self.api_job_message(msg, res).await?,
                            Some(NodeCommand::APIAvailableAgents { msg, res }) => self.api_available_agents(msg, res).await?,
                            Some(NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res }) => self.api_create_files_inbox_with_symmetric_key(msg, res).await?,
                            Some(NodeCommand::APIGetFilenamesInInbox { msg, res }) => self.api_get_filenames_in_inbox(msg, res).await?,
                            Some(NodeCommand::APIAddFileToInboxWithSymmetricKey { filename, file, public_key, encrypted_nonce, res }) => self.api_add_file_to_inbox_with_symmetric_key(filename, file, public_key, encrypted_nonce, res).await?,
                            Some(NodeCommand::APIGetAllSmartInboxesForProfile { msg, res }) => self.api_get_all_smart_inboxes_for_profile(msg, res).await?,
                            Some(NodeCommand::APIUpdateSmartInboxName { msg, res }) => self.api_update_smart_inbox_name(msg, res).await?,
                            Some(NodeCommand::APIUpdateJobToFinished { msg, res }) => self.api_update_job_to_finished(msg, res).await?,
                            Some(NodeCommand::APIPrivateDevopsCronList { res }) => self.api_private_devops_cron_list(res).await?,
                            Some(NodeCommand::APIAddToolkit { msg, res }) => self.api_add_toolkit(msg, res).await?,
                            Some(NodeCommand::APIListToolkits { msg, res }) => self.api_list_toolkits(msg, res).await?,
                            Some(NodeCommand::APIChangeNodesName { msg, res }) => self.api_change_nodes_name(msg, res).await?,
                            Some(NodeCommand::APIIsPristine { res }) => self.api_is_pristine(res).await?,
                            Some(NodeCommand::IsPristine { res }) => self.local_is_pristine(res).await,
                            Some(NodeCommand::APIGetLastMessagesFromInboxWithBranches { msg, res }) => self.api_get_last_messages_from_inbox_with_branches(msg, res).await?,
                            Some(NodeCommand::GetLastMessagesFromInboxWithBranches { inbox_name, limit, offset_key, res }) => self.local_get_last_messages_from_inbox_with_branches(inbox_name, limit, offset_key, res).await,
                            // Some(NodeCommand::APIRetryMessageWithInbox { inbox_name, message_hash, res }) => self.api_retry_message_with_inbox(inbox_name, message_hash, res).await,
                            // Some(NodeCommand::RetryMessageWithInbox { inbox_name, message_hash, res }) => self.local_retry_message_with_inbox(inbox_name, message_hash, res).await,
                            Some(NodeCommand::APIVecFSRetrievePathSimplifiedJson { msg, res }) => self.api_vec_fs_retrieve_path_simplified_json(msg, res).await?,
                            Some(NodeCommand::APIConvertFilesAndSaveToFolder { msg, res }) => self.api_convert_files_and_save_to_folder(msg, res).await?,
                            Some(NodeCommand::APIVecFSRetrieveVectorSearchSimplifiedJson { msg, res }) => self.api_vec_fs_retrieve_vector_search_simplified_json(msg, res).await?,
                            Some(NodeCommand::APIVecFSSearchItems { msg, res }) => self.api_vec_fs_search_items(msg, res).await?,
                            Some(NodeCommand::APIVecFSCreateFolder { msg, res }) => self.api_vec_fs_create_folder(msg, res).await?,
                            Some(NodeCommand::APIVecFSMoveItem { msg, res }) => self.api_vec_fs_move_item(msg, res).await?,
                            Some(NodeCommand::APIVecFSCopyItem { msg, res }) => self.api_vec_fs_copy_item(msg, res).await?,
                            Some(NodeCommand::APIVecFSMoveFolder { msg, res }) => self.api_vec_fs_move_folder(msg, res).await?,
                            Some(NodeCommand::APIVecFSCopyFolder { msg, res }) => self.api_vec_fs_copy_folder(msg, res).await?,
                            Some(NodeCommand::APIVecFSRetrieveVectorResource { msg, res }) => self.api_vec_fs_retrieve_vector_resource(msg, res).await?,
                            Some(NodeCommand::APIVecFSDeleteFolder { msg, res }) => self.api_vec_fs_delete_folder(msg, res).await?,
                            Some(NodeCommand::APIVecFSDeleteItem { msg, res }) => self.api_vec_fs_delete_item(msg, res).await?,
                            Some(NodeCommand::APIAvailableSharedItems { msg, res }) => self.api_subscription_available_shared_items(msg, res).await?,
                            Some(NodeCommand::APICreateShareableFolder { msg, res }) => self.api_subscription_create_shareable_folder(msg, res).await?,
                            Some(NodeCommand::APIUpdateShareableFolder { msg, res }) => self.api_subscription_update_shareable_folder(msg, res).await?,
                            Some(NodeCommand::APIUnshareFolder { msg, res }) => self.api_subscription_unshare_folder(msg, res).await?,
                            Some(NodeCommand::APISubscribeToSharedFolder { msg, res }) => self.api_subscription_subscribe_to_shared_folder(msg, res).await?,
                            Some(NodeCommand::APIMySubscriptions { msg, res }) => self.api_subscription_my_subscriptions(msg, res).await?,
                            _ => {},
                        }
                    }
            };
        }
        Ok(())
    }

    // A function that listens for incoming connections and tries to reconnect if a connection is lost.
    async fn listen_and_reconnect(&self) {
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("{} > TCP: Starting listen and reconnect loop.", self.listen_address),
        );
        loop {
            match self.listen().await {
                Ok(_) => unreachable!(),
                Err(_) => (),
            }
        }
    }

    // A function that listens for incoming connections.
    async fn listen(&self) -> io::Result<()> {
        let mut listener = TcpListener::bind(&self.listen_address).await?;
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("{} > TCP: Listening on {}", self.listen_address, self.listen_address),
        );

        // Initialize your connection limiter
        loop {
            let (mut socket, addr) = listener.accept().await?;
            // Too many requests by IP protection
            let ip = addr.ip().to_string();
            let conn_limiter_clone = self.conn_limiter.clone();

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

            let socket = Arc::new(Mutex::new(socket));
            let db = Arc::clone(&self.db);
            let identity_manager = Arc::clone(&self.identity_manager);
            let encryption_secret_key_clone = clone_static_secret_key(&self.encryption_secret_key);
            let identity_secret_key_clone = clone_signature_secret_key(&self.identity_secret_key);
            let node_profile_name_clone = self.node_name.clone();
            let network_job_manager = Arc::clone(&self.network_job_manager);

            tokio::spawn(async move {
                let mut buffer = Vec::new();
                let mut socket = socket.lock().await;
                socket.read_to_end(&mut buffer).await.unwrap();

                let destination_socket = socket.peer_addr().expect("Failed to get peer address");
                let network_job = NetworkJobQueue {
                    receiver_address: addr,
                    unsafe_sender_address: destination_socket.clone(),
                    content: buffer.clone(),
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
                if let Err(e) = socket.flush().await {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to flush the socket: {}", e),
                    );
                }
                conn_limiter_clone.decrement_connection(&ip).await;
            });
        }
    }

    async fn retry_messages(&self) -> Result<(), NodeError> {
        let db_lock = self.db.lock().await;
        let messages_to_retry = db_lock.get_messages_to_retry_before(None)?;
        drop(db_lock);

        for retry_message in messages_to_retry {
            let encrypted_secret_key = clone_static_secret_key(&self.encryption_secret_key);
            let save_to_db_flag = retry_message.save_to_db_flag;
            let retry = Some(retry_message.retry_count);

            // Remove the message from the retry queue
            let db_lock = self.db.lock().await;
            db_lock.remove_message_from_retry(&retry_message.message).unwrap();
            drop(db_lock);

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
                self.db.clone(),
                self.identity_manager.clone(),
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
        return self.peers.clone();
    }

    // Send a message to a peer.
    pub fn send(
        message: ShinkaiMessage,
        my_encryption_sk: Arc<EncryptionStaticKey>,
        peer: (SocketAddr, ProfileName),
        db: Arc<Mutex<ShinkaiDB>>,
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
            let stream = TcpStream::connect(address).await;
            match stream {
                Ok(mut stream) => {
                    let encoded_msg = message.encode_message().unwrap();
                    let _ = stream.write_all(encoded_msg.as_ref()).await;
                    let _ = stream.flush().await;
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Info,
                        &format!("Sent message to {}", stream.peer_addr().unwrap()),
                    );
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
                }
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", address, e);
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to connect to {}: {}", address, e),
                    );
                    // If retry is enabled, add the message to retry list on failure
                    let retry_count = retry.unwrap_or(0) + 1;
                    let db = db.lock().await;
                    let retry_message = RetryMessage {
                        retry_count: retry_count,
                        message: message.as_ref().clone(),
                        peer: peer.clone(),
                        save_to_db_flag,
                    };
                    // Calculate the delay for the next retry
                    let delay_seconds = (4 as u64).pow(retry_count - 1);
                    let retry_time = Utc::now() + chrono::Duration::seconds(delay_seconds as i64);
                    db.add_message_to_retry(&retry_message, retry_time).unwrap();
                }
            }
        });
    }

    pub fn subscriber_test_fn(&self) -> String {
        // return encryption key
        let encryption_secret_key = self.encryption_secret_key.clone();
        encryption_secret_key_to_string(encryption_secret_key)
    }

    pub fn send_file(
        message: ShinkaiMessage,
        my_encryption_sk: Arc<EncryptionStaticKey>,
        peer: (SocketAddr, ProfileName),
        db: Arc<Mutex<ShinkaiDB>>,
        maybe_identity_manager: Arc<Mutex<IdentityManager>>,
        save_to_db_flag: bool,
        retry: Option<u32>,
    ) {
        // TODO: redo all of this
        // Step 1: send symmetric key to peer
        // Step 2: convert file to chunks
        // Step 3: encrypt and send chunks to peer

        // TODO: the receiver should delete the inbox after the file is received

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("Sending {:?} to {:?}", message, peer),
        );
        let address = peer.0;
        let message = Arc::new(message);

        tokio::spawn(async move {
            let stream = TcpStream::connect(address).await;
            match stream {
                Ok(mut stream) => {
                    let encoded_msg = message.encode_message().unwrap();
                    let _ = stream.write_all(encoded_msg.as_ref()).await;
                    let _ = stream.flush().await;
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Info,
                        &format!("Sent message to {}", stream.peer_addr().unwrap()),
                    );
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
                }
                Err(e) => {
                    eprintln!("Failed to connect to {}: {}", address, e);
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to connect to {}: {}", address, e),
                    );
                    // If retry is enabled, add the message to retry list on failure
                    let retry_count = retry.unwrap_or(0) + 1;
                    let db = db.lock().await;
                    let retry_message = RetryMessage {
                        retry_count: retry_count,
                        message: message.as_ref().clone(),
                        peer: peer.clone(),
                        save_to_db_flag,
                    };
                    // Calculate the delay for the next retry
                    let delay_seconds = (4 as u64).pow(retry_count - 1);
                    let retry_time = Utc::now() + chrono::Duration::seconds(delay_seconds as i64);
                    db.add_message_to_retry(&retry_message, retry_time).unwrap();
                }
            }
        });
    }

    pub async fn save_to_db(
        am_i_sender: bool,
        message: &ShinkaiMessage,
        my_encryption_sk: EncryptionStaticKey,
        db: Arc<Mutex<ShinkaiDB>>,
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
        let mut db = db.lock().await;
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
}
