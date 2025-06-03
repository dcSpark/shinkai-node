use super::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use super::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use super::network_manager::network_job_manager::{NetworkJobManager, NetworkJobQueue};
use super::node_error::NodeError;
use super::ws_manager::WebSocketManager;
use crate::cron_tasks::cron_manager::CronManager;
use crate::llm_provider::job_callback_manager::JobCallbackManager;
use crate::llm_provider::job_manager::JobManager;
use crate::llm_provider::llm_stopper::LLMStopper;
use crate::managers::identity_manager::IdentityManagerTrait;
use crate::managers::tool_router::ToolRouter;
use crate::managers::IdentityManager;
use crate::network::network_limiter::ConnectionLimiter;
use crate::network::ws_routes::run_ws_api;
use crate::wallet::coinbase_mpc_wallet::CoinbaseMPCWallet;
use crate::wallet::wallet_manager::WalletManager;
use async_channel::Receiver;
use base64::Engine;
use chrono::Utc;
use core::panic;
use dashmap::DashMap;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use futures::{future::FutureExt, pin_mut, prelude::*, select};
use rand::rngs::OsRng;
use rand::RngCore;
use reqwest::StatusCode;
use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_embedding::model_type::EmbeddingModelType;
use shinkai_http_api::node_api_router::APIError;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::{
    LLMProviderInterface, SerializedLLMProvider, ShinkaiBackend
};
use shinkai_message_primitives::schemas::retry::RetryMessage;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_network::NetworkMessageType;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, encryption_public_key_to_string, encryption_secret_key_to_string
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;
use shinkai_tcp_relayer::NetworkMessage;
use std::convert::TryInto;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::Instant;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

// A type alias for a string that represents a profile name.
type ProfileName = String;
pub type TcpReadHalf = Arc<Mutex<ReadHalf<TcpStream>>>;
pub type TcpWriteHalf = Arc<Mutex<WriteHalf<TcpStream>>>;
pub type TcpConnection = Option<(TcpReadHalf, TcpWriteHalf)>;

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
    // The HTTPS certificate in PEM format
    pub private_https_certificate: Option<String>,
    // The HTTPS private key in PEM format
    pub public_https_certificate: Option<String>,
    // Secrets file path
    pub secrets_file_path: String,
    // A map of known peer nodes.
    pub peers: DashMap<(SocketAddr, ProfileName), chrono::DateTime<Utc>>,
    // The interval at which this node pings all known peers.
    pub ping_interval_secs: u64,
    // The channel from which this node receives commands.
    pub commands: Receiver<NodeCommand>,
    // The manager for subidentities.
    pub identity_manager: Arc<Mutex<IdentityManager>>,
    // The database connection for this node.
    pub db: Arc<SqliteManager>,
    // First device needs registration code
    pub first_device_needs_registration_code: bool,
    // Initial Agent to auto-add on first registration
    pub initial_llm_providers: Vec<SerializedLLMProvider>,
    // The Job manager
    pub job_manager: Option<Arc<Mutex<JobManager>>>,
    // Cron Manager
    pub cron_manager: Option<Arc<Mutex<CronManager>>>,
    // An EmbeddingGenerator initialized with the Node's default embedding model + server info
    pub embedding_generator: RemoteEmbeddingGenerator,
    /// Rate Limiter
    pub conn_limiter: Arc<ConnectionLimiter>,
    // Network Job Manager
    pub network_job_manager: Arc<Mutex<NetworkJobManager>>,
    // Proxy Address
    pub proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    // Websocket Manager
    pub ws_manager: Option<Arc<Mutex<WebSocketManager>>>,
    // Websocket Manager Trait
    pub ws_manager_trait: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    // Websocket Address
    pub ws_address: Option<SocketAddr>,
    // Websocket Server
    pub ws_server: Option<tokio::task::JoinHandle<()>>,
    // Tool Router. Option so it is less painful to test
    pub tool_router: Option<Arc<ToolRouter>>,
    // Callback Manager. Option so it is compatible with the Option (timing wise) inputs.
    pub callback_manager: Arc<Mutex<JobCallbackManager>>,
    // Default embedding model for new profiles
    pub default_embedding_model: Arc<Mutex<EmbeddingModelType>>,
    // Supported embedding models for profiles
    pub supported_embedding_models: Arc<Mutex<Vec<EmbeddingModelType>>>,
    // API V2 Key
    #[allow(dead_code)]
    pub api_v2_key: String,
    // Wallet Manager
    pub wallet_manager: Arc<Mutex<Option<WalletManager>>>,
    /// My Agent Payments Manager
    pub my_agent_payments_manager: Arc<Mutex<MyAgentOfferingsManager>>,
    /// Ext Agent Payments Manager
    pub ext_agent_payments_manager: Arc<Mutex<ExtAgentOfferingsManager>>,
    // LLM Stopper
    pub llm_stopper: Arc<LLMStopper>,
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
        private_https_certificate: Option<String>,
        public_https_certificate: Option<String>,
        ping_interval_secs: u64,
        commands: Receiver<NodeCommand>,
        main_db_path: String,
        secrets_file_path: String,
        proxy_identity: Option<String>,
        first_device_needs_registration_code: bool,
        initial_llm_providers: Vec<SerializedLLMProvider>,
        embedding_generator: Option<RemoteEmbeddingGenerator>,
        ws_address: Option<SocketAddr>,
        default_embedding_model: EmbeddingModelType,
        supported_embedding_models: Vec<EmbeddingModelType>,
        api_v2_key: Option<String>,
    ) -> Arc<Mutex<Node>> {
        // if is_valid_node_identity_name_and_no_subidentities is false panic
        match ShinkaiName::new(node_name.to_string().clone()) {
            Ok(_) => (),
            Err(_) => panic!("Invalid node identity name: {}", node_name),
        }

        // Initialize default RemoteEmbeddingGenerator if none provided
        let embedding_generator = embedding_generator.unwrap_or_else(RemoteEmbeddingGenerator::new_default);

        // Initialize SqliteManager
        let embedding_api_url = embedding_generator.api_url.clone();
        let db_arc = Arc::new(
            SqliteManager::new(main_db_path.clone(), embedding_api_url, default_embedding_model.clone())
                .unwrap_or_else(|e| {
                    eprintln!("Error: {:?}", e);
                    panic!("Failed to open database: {}", main_db_path)
                }),
        );

        // Get public keys, and update the local node keys in the db
        let identity_public_key = identity_secret_key.verifying_key();
        let encryption_public_key = EncryptionPublicKey::from(&encryption_secret_key);
        let node_name = ShinkaiName::new(node_name).unwrap();
        {
            match db_arc.update_local_node_keys(node_name.clone(), encryption_public_key, identity_public_key) {
                Ok(_) => (),
                Err(e) => panic!("Failed to update local node keys: {}", e),
            }
            // TODO: maybe check if the keys in the Blockchain match and if not, then prints a warning message to update
            // the keys
        }

        // Setup Identity Manager
        let db_weak = Arc::downgrade(&db_arc);
        let subidentity_manager = IdentityManager::new(Arc::downgrade(&db_arc), node_name.clone())
            .await
            .unwrap();
        let identity_manager = Arc::new(Mutex::new(subidentity_manager));

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
        let proxy_connection_info: Option<ProxyConnectionInfo> = proxy_identity.and_then(|proxy_identity| {
            ShinkaiName::new(proxy_identity.clone())
                .inspect_err(|_| {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        format!("Invalid proxy identity name: {}", proxy_identity).as_str(),
                    );
                })
                .map_or_else(
                    |_| None,
                    |proxy_identity| {
                        Some(ProxyConnectionInfo {
                            proxy_identity,
                            tcp_connection: None,
                        })
                    },
                )
        });
        let proxy_connection_info = Arc::new(Mutex::new(proxy_connection_info));

        let proxy_connection_info_weak = Arc::downgrade(&proxy_connection_info);

        let identity_manager_trait: Arc<Mutex<dyn IdentityManagerTrait + Send + 'static>> = {
            // Convert the Arc<Mutex<IdentityManager>> to Arc<Mutex<dyn IdentityManagerTrait + Send + 'static>>
            identity_manager.clone()
        };

        let ws_manager = if ws_address.is_some() {
            let manager = WebSocketManager::new(
                db_weak,
                node_name.clone(),
                identity_manager_trait.clone(),
                clone_static_secret_key(&encryption_secret_key),
            )
            .await;
            Some(manager)
        } else {
            None
        };

        let ws_manager_trait = ws_manager.clone().map(|manager| {
            let manager_trait: Arc<Mutex<dyn WSUpdateHandler + Send>> = manager.clone();
            manager_trait
        });

        // Initialize ToolRouter
        let tool_router = ToolRouter::new(
            db_arc.clone(),
            identity_manager.clone(),
            clone_static_secret_key(&encryption_secret_key),
            encryption_public_key,
            clone_signature_secret_key(&identity_secret_key),
            None,
        );

        // Read wallet_manager from db if it exists, if not, None
        let mut wallet_manager: Option<WalletManager> = match db_arc.read_wallet_manager() {
            Ok(manager_value) => match serde_json::from_value::<WalletManager>(manager_value) {
                Ok(manager) => Some(manager),
                Err(e) => {
                    eprintln!("Failed to deserialize WalletManager: {}", e);
                    None
                }
            },
            Err(SqliteManagerError::WalletManagerNotFound) => None,
            Err(e) => panic!("Failed to read wallet manager from database: {}", e),
        };

        // Update LanceDB in CoinbaseMPCWallet if it exists
        if let Some(ref mut manager) = wallet_manager {
            if let Some(coinbase_wallet) = manager.payment_wallet.as_any_mut().downcast_mut::<CoinbaseMPCWallet>() {
                coinbase_wallet.update_sqlite_manager(db_arc.clone());
            }
            if let Some(coinbase_wallet) = manager
                .receiving_wallet
                .as_any_mut()
                .downcast_mut::<CoinbaseMPCWallet>()
            {
                coinbase_wallet.update_sqlite_manager(db_arc.clone());
            }
        }

        let wallet_manager = Arc::new(Mutex::new(wallet_manager));

        let tool_router = Arc::new(tool_router);

        let my_agent_payments_manager = Arc::new(Mutex::new(
            MyAgentOfferingsManager::new(
                Arc::downgrade(&db_arc),
                Arc::downgrade(&identity_manager_trait),
                node_name.clone(),
                clone_signature_secret_key(&identity_secret_key),
                clone_static_secret_key(&encryption_secret_key),
                proxy_connection_info_weak.clone(),
                Arc::downgrade(&tool_router),
                Arc::downgrade(&wallet_manager),
            )
            .await,
        ));

        let ext_agent_payments_manager = Arc::new(Mutex::new(
            ExtAgentOfferingsManager::new(
                Arc::downgrade(&db_arc),
                Arc::downgrade(&identity_manager_trait),
                node_name.clone(),
                clone_signature_secret_key(&identity_secret_key),
                clone_static_secret_key(&encryption_secret_key),
                proxy_connection_info_weak.clone(),
                Arc::downgrade(&tool_router),
                Arc::downgrade(&wallet_manager),
            )
            .await,
        ));

        // Create NetworkJobManager with a weak reference to this node
        let network_manager = NetworkJobManager::new(
            Arc::downgrade(&db_arc),
            node_name.clone(),
            clone_static_secret_key(&encryption_secret_key),
            clone_signature_secret_key(&identity_secret_key),
            identity_manager.clone(),
            Arc::downgrade(&my_agent_payments_manager),
            Arc::downgrade(&ext_agent_payments_manager),
            proxy_connection_info_weak.clone(),
            ws_manager_trait.clone(),
        )
        .await;

        let default_embedding_model = Arc::new(Mutex::new(default_embedding_model));
        let supported_embedding_models = Arc::new(Mutex::new(supported_embedding_models));

        // It reads the api_v2_key from env, if not from db and if not, then it generates a new one that gets saved in
        // the db
        let api_v2_key = if let Some(key) = api_v2_key {
            db_arc
                .set_api_v2_key(&key)
                .expect("Failed to set api_v2_key in the database");
            key
        } else {
            match db_arc.read_api_v2_key() {
                Ok(Some(key)) => key,
                Ok(None) | Err(_) => {
                    let new_key = Node::generate_api_v2_key();
                    db_arc
                        .set_api_v2_key(&new_key)
                        .expect("Failed to set api_v2_key in the database");
                    new_key
                }
            }
        };

        let llm_stopper = Arc::new(LLMStopper::new());

        Arc::new(Mutex::new(Node {
            node_name: node_name.clone(),
            identity_secret_key: clone_signature_secret_key(&identity_secret_key),
            identity_public_key,
            encryption_secret_key: clone_static_secret_key(&encryption_secret_key),
            encryption_public_key,
            private_https_certificate,
            public_https_certificate,
            peers: DashMap::new(),
            listen_address,
            secrets_file_path,
            ping_interval_secs,
            commands,
            identity_manager: identity_manager.clone(),
            db: db_arc.clone(),
            job_manager: None,
            cron_manager: None,
            first_device_needs_registration_code,
            initial_llm_providers,
            embedding_generator,
            conn_limiter,
            network_job_manager: Arc::new(Mutex::new(network_manager)),
            proxy_connection_info,
            ws_manager,
            ws_address,
            ws_manager_trait,
            ws_server: None,
            callback_manager: Arc::new(Mutex::new(JobCallbackManager::new())),
            tool_router: Some(tool_router),
            default_embedding_model,
            supported_embedding_models,
            api_v2_key,
            wallet_manager,
            my_agent_payments_manager,
            ext_agent_payments_manager,
            llm_stopper,
        }))
    }

    // Start the node's operations.
    pub async fn start(&mut self) -> Result<(), NodeError> {
        let db_weak = Arc::downgrade(&self.db);

        {
            let vr_path = ShinkaiPath::from_base_path();

            // Check if the directory exists, and create it if it doesn't
            if !Path::new(&vr_path.as_path()).exists() {
                fs::create_dir_all(&vr_path.as_path()).map_err(|e| {
                    NodeError::from(format!(
                        "Failed to create directory {}: {}",
                        vr_path.as_path().display(),
                        e
                    ))
                })?;
            }
        }

        let job_manager = Arc::new(Mutex::new(
            JobManager::new(
                db_weak,
                Arc::clone(&self.identity_manager),
                clone_signature_secret_key(&self.identity_secret_key),
                self.node_name.clone(),
                self.embedding_generator.clone(),
                self.ws_manager_trait.clone(),
                self.tool_router.clone(),
                self.callback_manager.clone(),
                self.my_agent_payments_manager.clone(),
                self.ext_agent_payments_manager.clone(),
                self.llm_stopper.clone(),
            )
            .await,
        ));
        self.job_manager = Some(job_manager.clone());

        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("Starting node with name: {}", self.node_name),
        );
        let db_weak = Arc::downgrade(&self.db);

        let cron_manager_result = CronManager::new(
            db_weak.clone(),
            clone_signature_secret_key(&self.identity_secret_key),
            self.node_name.clone(),
            job_manager.clone(),
            self.identity_manager.clone(),
            self.encryption_secret_key.clone(),
            self.encryption_public_key.clone(),
            self.ws_manager_trait.clone(),
        )
        .await;

        let cron_manager = Arc::new(Mutex::new(cron_manager_result));
        self.cron_manager = Some(cron_manager.clone());

        {
            let mut callback_manager = self.callback_manager.lock().await;
            callback_manager.update_job_manager(job_manager.clone());
            callback_manager.update_cron_manager(cron_manager.clone());
        }

        self.initialize_embedding_models().await?;
        {
            // Starting the WebSocket server
            if let (Some(ws_manager), Some(ws_address)) = (&self.ws_manager, self.ws_address) {
                let ws_manager = Arc::clone(ws_manager);
                let ws_server = tokio::spawn(async move {
                    run_ws_api(ws_address, ws_manager).await;
                });
                self.ws_server = Some(ws_server);
            }
        }

        {
            // Update the version in the database
            // Retrieve the current version
            let version = env!("CARGO_PKG_VERSION");

            // Update the version in the database
            self.db.set_version(version).expect("Failed to set version");
        }

        // Call ToolRouter initialization in a new task
        if let Some(tool_router) = &self.tool_router {
            let tool_router = tool_router.clone();
            let generator = self.embedding_generator.clone();
            let reinstall_tools = std::env::var("REINSTALL_TOOLS").unwrap_or_else(|_| "false".to_string()) == "true";

            tokio::spawn(async move {
                if reinstall_tools {
                    if let Err(e) = tool_router.force_reinstall_all(Arc::new(generator.clone())).await {
                        eprintln!("ToolRouter force reinstall failed: {:?}", e);
                    }
                } else {
                    if let Err(e) = tool_router.initialization(Arc::new(generator.clone())).await {
                        eprintln!("ToolRouter initialization failed: {:?}", e);
                    }
                }
            });
        }
        eprintln!(">> Node start set variables successfully");

        let listen_future = self.listen_and_reconnect(self.proxy_connection_info.clone()).fuse();
        pin_mut!(listen_future);

        let retry_interval_secs = 2;
        let mut retry_interval = tokio::time::interval(Duration::from_secs(retry_interval_secs));

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

        let mut ping_interval = tokio::time::interval(Duration::from_secs(ping_interval_secs));
        let mut commands_clone = self.commands.clone();
        // TODO: here we can create a task to check the blockchain for new peers and update our list
        let check_peers_interval_secs = 5;
        let _check_peers_interval = tokio::time::interval(Duration::from_secs(check_peers_interval_secs));

        // Add 6-hour interval for periodic tasks
        let six_hours_in_secs = 6 * 60 * 60; // 6 hours in seconds
        let start = Instant::now() + Duration::from_secs(six_hours_in_secs);
        let mut six_hour_interval = tokio::time::interval_at(start, Duration::from_secs(six_hours_in_secs));

        // TODO: implement a TCP connection here with a proxy if it's set

        loop {
            let ping_future = ping_interval.tick().fuse();
            let commands_future = commands_clone.next().fuse();
            let retry_future = retry_interval.tick().fuse();
            let six_hour_future = six_hour_interval.tick().fuse();

            // TODO: update this to read onchain data and update db
            // let check_peers_future = check_peers_interval.next().fuse();
            pin_mut!(ping_future, commands_future, retry_future, six_hour_future);

            select! {
                    _retry = retry_future => {
                        // Clone the necessary variables for `retry_messages`
                        let db_clone = self.db.clone();
                        let encryption_secret_key_clone = self.encryption_secret_key.clone();
                        let identity_manager_clone = self.identity_manager.clone();
                        let proxy_connection_info = self.proxy_connection_info.clone();
                        let ws_manager_trait = self.ws_manager_trait.clone();

                        // Spawn a new task to call `retry_messages` asynchronously
                        tokio::spawn(async move {
                            let _ = Self::retry_messages(
                                db_clone,
                                encryption_secret_key_clone,
                                identity_manager_clone,
                                proxy_connection_info,
                                ws_manager_trait,
                            ).await;
                        });
                    },
                    _six_hour = six_hour_future => {
                        // Clone necessary variables for periodic tasks
                        let db_clone = self.db.clone();
                        let node_name_clone = self.node_name.clone();
                        let identity_manager_clone = self.identity_manager.clone();
                        let tool_router_clone = self.tool_router.clone();
                        let embedding_generator_clone = self.embedding_generator.clone();
                        // Spawn a new task to handle periodic maintenance
                        tokio::spawn(async move {
                            let _ = Self::handle_periodic_maintenance(
                                db_clone,
                                node_name_clone,
                                identity_manager_clone,
                                tool_router_clone,
                                Arc::new(embedding_generator_clone),
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
                        let ws_manager_trait = self.ws_manager_trait.clone();

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
                                ws_manager_trait,
                            ).await;
                        });
                    },
                    // check_peers = check_peers_future => self.connect_new_peers().await,
                    command = commands_future => {
                        if let Some(command) = command {
                            self.handle_command(command).await;
                        }
                    }
            };
        }
    }

    // A function that initializes the embedding models from the database
    async fn initialize_embedding_models(&self) -> Result<(), Box<dyn std::error::Error + Send>> {
        // Read the default embedding model from the database
        {
            match self.db.get_default_embedding_model() {
                Ok(model) => {
                    let mut default_model_guard = self.default_embedding_model.lock().await;
                    *default_model_guard = model;
                }
                Err(SqliteManagerError::DataNotFound) => {
                    // If not found, update the database with the current value
                    let default_model_guard = self.default_embedding_model.lock().await;
                    self.db
                        .update_default_embedding_model(default_model_guard.clone())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;
                }
                Err(e) => return Err(Box::new(NodeError::from(e.to_string())) as Box<dyn std::error::Error + Send>),
            }
        }

        // Read the supported embedding models from the database
        {
            match self.db.get_supported_embedding_models() {
                Ok(models) => {
                    // If empty, update the database with the current value
                    if models.is_empty() {
                        let supported_models_guard = self.supported_embedding_models.lock().await;
                        self.db
                            .update_supported_embedding_models(supported_models_guard.clone())
                            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;
                    } else {
                        let mut supported_models_guard = self.supported_embedding_models.lock().await;
                        *supported_models_guard = models;
                    }
                }
                Err(SqliteManagerError::DataNotFound) => {
                    // If not found, update the database with the current value
                    let supported_models_guard = self.supported_embedding_models.lock().await;
                    self.db
                        .update_supported_embedding_models(supported_models_guard.clone())
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;
                }
                Err(e) => return Err(Box::new(NodeError::from(e.to_string())) as Box<dyn std::error::Error + Send>),
            }
        }

        Ok(())
    }

    // A function that listens for incoming connections and tries to reconnect if a connection is lost.
    async fn listen_and_reconnect(&self, proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>) {
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("{} > TCP: Starting listen and reconnect loop.", self.listen_address),
        );

        let identity_manager = self.identity_manager.clone();
        let network_job_manager = self.network_job_manager.clone();
        let node_name = self.node_name.clone();
        let identity_secret_key = self.identity_secret_key.clone();
        let proxy_connection_info_clone = proxy_connection_info.clone();

        // Spawn proxy connection attempts so startup can continue in parallel
        tokio::spawn(async move {
            let mut retry_count = 0;
            loop {
                let proxy_info = {
                    let proxy_info_lock = proxy_connection_info_clone.lock().await;
                    proxy_info_lock.clone()
                };

                if let Some(proxy_info) = proxy_info {
                    let connection_result = Node::establish_proxy_connection(
                        identity_manager.clone(),
                        &proxy_info,
                        node_name.clone(),
                        identity_secret_key.clone(),
                    )
                    .await;

                    match connection_result {
                        Ok(Some((reader, writer))) => {
                            let _ = Self::handle_proxy_listen_connection(
                                reader,
                                writer,
                                proxy_info.proxy_identity.clone(),
                                proxy_connection_info_clone.clone(),
                                network_job_manager.clone(),
                                identity_manager.clone(),
                            )
                            .await;
                        }
                        Ok(None) => {
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                "Failed to establish proxy connection: returned None",
                            );
                            {
                                let mut info = proxy_connection_info_clone.lock().await;
                                *info = None;
                            }
                            break;
                        }
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to establish proxy connection: {}", e),
                            );
                            {
                                let mut info = proxy_connection_info_clone.lock().await;
                                *info = None;
                            }
                            break;
                        }
                    }
                } else {
                    break;
                }

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
        });

        // Execute direct listening in a separate task while the proxy task runs
        // in the background so that startup isn't blocked if either fails.
        let listen_address = self.listen_address;
        let network_job_manager = self.network_job_manager.clone();
        let conn_limiter = self.conn_limiter.clone();
        let node_name = self.node_name.clone();

        tokio::spawn(async move {
            let result = Self::handle_listen_connection(
                listen_address,
                network_job_manager,
                conn_limiter,
                node_name,
            )
            .await;

            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Error,
                &format!("{} > TCP: Listening error {:?}", listen_address, result),
            );
        });

        // Keep this future alive indefinitely. The listening task above will
        // run until cancelled, ensuring the select! loop in `start` does not
        // terminate early.
        futures::future::pending::<()>().await;
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
        eprintln!("handle_proxy_listen_connection");
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
                Self::handle_connection(reader_clone, proxy_addr, network_job_manager_clone)
                    .await
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{:?}", e)))?;
                Ok::<(), std::io::Error>(())
            });

            // Await the task's completion
            match handle.await {
                Ok(Ok(())) => {
                    // Connection handled successfully, continue the loop
                }
                Ok(Err(e)) => {
                    eprintln!("Task failed: {:?}", e);
                    return Err(e); // Return error to trigger reconnection
                }
                Err(e) => {
                    eprintln!("Task panicked: {:?}", e);
                    return Err(io::Error::new(io::ErrorKind::Other, format!("{:?}", e)));
                    // Return error to trigger reconnection
                }
            }
        }
    }

    async fn handle_listen_connection(
        listen_address: SocketAddr,
        network_job_manager: Arc<Mutex<NetworkJobManager>>,
        conn_limiter: Arc<ConnectionLimiter>,
        _node_name: ShinkaiName,
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

            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Info,
                &format!("Spawning task to handle connection from {}", ip),
            );

            tokio::spawn(async move {
                let (reader, _writer) = tokio::io::split(socket);
                let reader = Arc::new(Mutex::new(reader));
                let _ = Self::handle_connection(reader, addr, network_job_manager).await;
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
            .external_profile_to_global_identity(proxy_identity, None)
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
    ) -> Result<(), Box<dyn std::error::Error>> {
        let start_time = Utc::now();
        let mut length_bytes = [0u8; 4];
        {
            let mut reader = reader.lock().await;
            reader.read_exact(&mut length_bytes).await?;
            let total_length = u32::from_be_bytes(length_bytes) as usize;

            // Read the identity length
            let mut identity_length_bytes = [0u8; 4];
            reader.read_exact(&mut identity_length_bytes).await?;
            let identity_length = u32::from_be_bytes(identity_length_bytes) as usize;

            // Read the identity bytes
            let mut identity_bytes = vec![0u8; identity_length];
            reader.read_exact(&mut identity_bytes).await?;

            // Calculate the message length excluding the identity length and the identity itself
            let msg_length = total_length - 1 - 4 - identity_length; // Subtract 1 for the header and 4 for the identity length bytes

            // Read the header byte to determine the message type
            let mut header_byte = [0u8; 1];
            reader.read_exact(&mut header_byte).await?;
            let message_type = match header_byte[0] {
                0x01 => NetworkMessageType::ShinkaiMessage,
                0x03 => NetworkMessageType::ProxyMessage,
                _ => {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        "Received message with unknown type identifier",
                    );
                    return Err("Unknown message type".into());
                }
            };

            if msg_length == 0 {
                return Ok(()); // Exit, unless there is a message_type without body
            }

            // Initialize buffer to fit the message
            let mut buffer = vec![0u8; msg_length];

            // Read the rest of the message into the buffer
            reader.read_exact(&mut buffer).await?;
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
            network_job_manager.add_network_job_to_queue(&network_job).await?;
        }

        let end_time = Utc::now();
        let duration = end_time - start_time;
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("Finished handling connection from {:?} in {:?}", addr, duration),
        );

        Ok(())
    }

    async fn retry_messages(
        db: Arc<SqliteManager>,
        encryption_secret_key: EncryptionStaticKey,
        identity_manager: Arc<Mutex<IdentityManager>>,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), NodeError> {
        let messages_to_retry = db
            .get_messages_to_retry_before(None)
            .map_err(|e| NodeError::from(e.to_string()))?;

        for retry_message in messages_to_retry {
            let encrypted_secret_key = clone_static_secret_key(&encryption_secret_key);
            let save_to_db_flag = retry_message.save_to_db_flag;
            let retry = Some(retry_message.retry_count);

            // Remove the message from the retry queue
            db.remove_message_from_retry(&retry_message.message).unwrap();

            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Info,
                &format!(
                    "Retrying Message with External Metadata: {:?}",
                    retry_message.message.external_metadata
                ),
            );

            // Retry the message
            Node::send(
                retry_message.message,
                Arc::new(encrypted_secret_key),
                retry_message.peer,
                proxy_connection_info.clone(),
                db.clone(),
                identity_manager.clone(),
                ws_manager.clone(),
                save_to_db_flag,
                retry,
            );
        }

        Ok(())
    }

    // TODO: Add a new send that schedules messages to be sent at a later time.
    // It may be more complex than what it sounds because there could be a big backlog of messages to send which were
    // already generated and the time associated with the message may be too old to be recognized by the other node.
    // so most likely we need a way to update the messages (they are signed by this node after all) so it can update the
    // time to the current time

    // Send a message to a peer.
    #[allow(clippy::too_many_arguments)]
    pub fn send(
        message: ShinkaiMessage,
        my_encryption_sk: Arc<EncryptionStaticKey>,
        peer: (SocketAddr, ProfileName),
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        db: Arc<SqliteManager>,
        maybe_identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        save_to_db_flag: bool,
        retry: Option<u32>,
    ) {
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!(
                "Sending Msg with External Metadata {:?} to {:?}",
                message.external_metadata, peer
            ),
        );
        let address = peer.0;
        let message = Arc::new(message);

        tokio::spawn(async move {
            let start_time = Utc::now();
            let writer_start_time = Utc::now();
            let writer = Node::get_writer(address, proxy_connection_info).await;
            let writer_end_time = Utc::now(); // End time for get_writer
            let writer_duration = writer_end_time - writer_start_time;
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Info,
                &format!("Time taken to get_writer: {:?}", writer_duration),
            );

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
                        ws_manager,
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
            let end_time = Utc::now();
            let duration = end_time - start_time;
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Info,
                &format!("Finished sending message to {:?} in {:?}", address, duration),
            );
        });
    }

    /// Function to get the writer, either directly or through a proxy
    async fn get_writer(
        address: SocketAddr,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
    ) -> Option<Arc<Mutex<WriteHalf<TcpStream>>>> {
        let proxy_connection = proxy_connection_info.lock().await;
        if let Some(proxy_info) = proxy_connection.as_ref() {
            if let Some((_, writer)) = &proxy_info.tcp_connection {
                Some(writer.clone())
            } else {
                None
            }
        } else {
            match tokio::time::timeout(Duration::from_secs(4), TcpStream::connect(address)).await {
                Ok(Ok(stream)) => {
                    let (_, writer) = tokio::io::split(stream);
                    Some(Arc::new(Mutex::new(writer)))
                }
                Ok(Err(e)) => {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to connect to {}: {}", address, e),
                    );
                    None
                }
                Err(_) => {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Connection to {} timed out", address),
                    );
                    None
                }
            }
        }
    }

    pub async fn save_to_db(
        am_i_sender: bool,
        message: &ShinkaiMessage,
        my_encryption_sk: EncryptionStaticKey,
        db: Arc<SqliteManager>,
        maybe_identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> io::Result<()> {
        // We want to save it decrypted if possible
        // We are just going to check for the body encryption

        let is_body_encrypted = message.is_body_currently_encrypted();

        // Clone the message to get a fully owned version
        let mut message_to_save = message.clone();

        // The body should only be decrypted if it's currently encrypted.
        if is_body_encrypted {
            #[allow(unused_assignments)]
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
                .external_profile_to_global_identity(&counterpart_identity.clone(), None)
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
        let db_result = db.unsafe_insert_inbox_message(&message_to_save, None, ws_manager).await;
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
        let read_result = {
            let mut reader = reader.lock().await;
            reader.read_exact(&mut len_buffer).await
        };

        if let Err(e) = read_result {
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Error,
                &format!("Failed to read validation data length: {}", e),
            );
            return;
        }

        let validation_data_len = u32::from_be_bytes(len_buffer) as usize;

        let mut buffer = vec![0u8; validation_data_len];
        let res = {
            let mut reader = reader.lock().await;
            reader.read_exact(&mut buffer).await
        };
        match res {
            Ok(_) => {
                let validation_data = match String::from_utf8(buffer) {
                    Ok(s) => s.trim().to_string(),
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to convert validation data to string: {}", e),
                        );
                        return;
                    }
                };

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
                    if let Err(e) = writer.write_all(&total_len_bytes).await {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to write total length: {}", e),
                        );
                        return;
                    }

                    // Send the length of the public key
                    let public_key_len_bytes = public_key_len.to_be_bytes();
                    if let Err(e) = writer.write_all(&public_key_len_bytes).await {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to write public key length: {}", e),
                        );
                        return;
                    }

                    // Send the public key
                    if let Err(e) = writer.write_all(public_key_hex.as_bytes()).await {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to write public key: {}", e),
                        );
                        return;
                    }

                    // Send the length of the signed validation data
                    let signature_len_bytes = signature_len.to_be_bytes();
                    if let Err(e) = writer.write_all(&signature_len_bytes).await {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to write signature length: {}", e),
                        );
                        return;
                    }

                    // Send the signed validation data
                    match writer.write_all(signature_hex.as_bytes()).await {
                        Ok(_) => shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Info,
                            "Sent signed validation data and public key back to server",
                        ),
                        Err(e) => {
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to send signed validation data: {}", e),
                            );
                            return;
                        }
                    }
                }

                // Wait for the server to validate the signature
                let mut len_buffer = [0u8; 4];
                let read_result = {
                    let mut reader = reader.lock().await;
                    reader.read_exact(&mut len_buffer).await
                };

                if let Err(e) = read_result {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to read response length: {}", e),
                    );
                    return;
                }

                let response_len = u32::from_be_bytes(len_buffer) as usize;

                let mut response_buffer = vec![0u8; response_len];
                let read_result = {
                    let mut reader = reader.lock().await;
                    reader.read_exact(&mut response_buffer).await
                };

                if let Err(e) = read_result {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to read response: {}", e),
                    );
                    return;
                }

                let response = match String::from_utf8(response_buffer) {
                    Ok(s) => s,
                    Err(e) => {
                        shinkai_log(
                            ShinkaiLogOption::Node,
                            ShinkaiLogLevel::Error,
                            &format!("Failed to convert response to string: {}", e),
                        );
                        return;
                    }
                };

                // Assert the validation response
                if response != "Validation successful" {
                    shinkai_log(
                        ShinkaiLogOption::Node,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to validate the identity: {}", response),
                    );
                }
            }
            Err(e) => {
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to read validation data: {}", e),
                );
            }
        }
    }

    fn generate_api_v2_key() -> String {
        let mut key = [0u8; 32]; // 256-bit key
        OsRng.fill_bytes(&mut key);
        base64::engine::general_purpose::STANDARD.encode(&key)
    }

    pub fn generic_api_error(e: &str) -> APIError {
        APIError {
            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            error: "Internal Server Error".to_string(),
            message: format!("Error receiving result: {}", e),
        }
    }

    pub fn shinkai_free_provider_id() -> String {
        "shinkai_free_trial".to_string()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        if let Some(handle) = self.ws_server.take() {
            handle.abort();
        }
    }
}
