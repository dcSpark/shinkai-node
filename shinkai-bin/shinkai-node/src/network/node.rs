use super::agent_payments_manager::external_agent_offerings_manager::ExtAgentOfferingsManager;
use super::agent_payments_manager::my_agent_offerings_manager::MyAgentOfferingsManager;
use super::libp2p_manager::{LibP2PManager, NetworkEvent};
use super::libp2p_message_handler::ShinkaiMessageHandler;
use super::network_manager::network_job_manager::NetworkJobManager;
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
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::{future::FutureExt, pin_mut, prelude::*, select};
use libp2p::Multiaddr;
use rand::rngs::OsRng;
use rand::RngCore;
use reqwest::StatusCode;
use shinkai_embedding::embedding_generator::RemoteEmbeddingGenerator;
use shinkai_embedding::model_type::EmbeddingModelType;
use shinkai_http_api::node_api_router::APIError;
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::schemas::llm_providers::serialized_llm_provider::SerializedLLMProvider;
use shinkai_message_primitives::schemas::retry::RetryMessage;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::ws_types::WSUpdateHandler;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, encryption_public_key_to_string, encryption_secret_key_to_string,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_sqlite::errors::SqliteManagerError;
use shinkai_sqlite::SqliteManager;
use std::convert::TryInto;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::{io, net::SocketAddr, time::Duration};
use tokio::sync::Mutex;
use tokio::time::Instant;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

// A type alias for a string that represents a profile name.
type ProfileName = String;

// Define the ConnectionInfo struct for LibP2P proxy
#[derive(Clone, Debug)]
pub struct ProxyConnectionInfo {
    pub proxy_identity: ShinkaiName,
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
    // LibP2P Manager for peer-to-peer networking
    pub libp2p_manager: Option<Arc<Mutex<LibP2PManager>>>,
    // LibP2P event sender for sending network events
    pub libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
    // LibP2P task handle
    pub libp2p_task: Option<tokio::task::JoinHandle<()>>,
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
                .map_or_else(|_| None, |proxy_identity| Some(ProxyConnectionInfo { proxy_identity }))
        });
        let proxy_connection_info = Arc::new(Mutex::new(proxy_connection_info));

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
                Arc::downgrade(&proxy_connection_info),
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
                Arc::downgrade(&proxy_connection_info),
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
            Arc::downgrade(&proxy_connection_info),
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
            libp2p_manager: None,
            libp2p_event_sender: None,
            libp2p_task: None,
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

        // Initialize LibP2P networking
        eprintln!(">> DEBUG: About to enter LibP2P initialization block");
        {
            eprintln!(">> DEBUG: Creating ShinkaiMessageHandler");
            let message_handler = ShinkaiMessageHandler::new(self.network_job_manager.clone(), self.listen_address);

            eprintln!(">> DEBUG: Setting listen_port");
            // Extract port from listen_address for libp2p
            let listen_port = Some(self.listen_address.port());

            eprintln!(">> DEBUG: About to acquire proxy_connection_info lock");
            // Get relay address from proxy connection if available
            let relay_address = {
                let proxy_info = self.proxy_connection_info.lock().await;
                eprintln!(">> DEBUG: Acquired proxy_connection_info lock, checking proxy configuration");
                if let Some(proxy) = proxy_info.as_ref() {
                    eprintln!(">> DEBUG: Proxy found: {}", proxy.proxy_identity);
                    
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        &format!("Setting up LibP2P with relay: {}", proxy.proxy_identity),
                    );
                    
                    eprintln!(">> DEBUG: About to resolve proxy identity to address");
                    // Try to resolve proxy identity to address
                    // Add a small random delay to avoid simultaneous requests from multiple test nodes
                    let delay_ms = rand::RngCore::next_u32(&mut rand::rngs::OsRng) % 1000; // 0-1000ms
                    tokio::time::sleep(Duration::from_millis(delay_ms as u64)).await;
                    eprintln!(">> DEBUG: Applied random delay of {}ms before resolution", delay_ms);
                    
                    // Add timeout to prevent hanging on identity resolution
                    let resolution_timeout = Duration::from_secs(30);
                    match tokio::time::timeout(
                        resolution_timeout,
                        Node::get_address_from_identity(
                            self.identity_manager.clone(),
                            &proxy.proxy_identity.get_node_name_string(),
                        )
                    )
                    .await
                    {
                        Ok(Ok(addr)) => {
                            let multiaddr_str = format!("/ip4/{}/tcp/{}", addr.ip(), addr.port());
                            eprintln!(">> DEBUG: Successfully resolved proxy address: {}", multiaddr_str);
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Info,
                                &format!("Connecting to LibP2P relay at: {}", multiaddr_str),
                            );
                            multiaddr_str.parse::<Multiaddr>().ok()
                        }
                        Ok(Err(e)) => {
                            eprintln!(">> DEBUG: Failed to resolve proxy address: {}", e);
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Failed to resolve relay address: {}", e),
                            );
                            None
                        }
                        Err(_) => {
                            eprintln!(">> DEBUG: Timeout while resolving proxy address after {}s", resolution_timeout.as_secs());
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("Timeout while resolving relay address after {}s", resolution_timeout.as_secs()),
                            );
                            None
                        }
                    }
                } else {
                    eprintln!(">> DEBUG: No proxy configured");
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        "No relay configured for LibP2P",
                    );
                    None
                }
            };

            eprintln!(">> DEBUG: About to call LibP2PManager::new");
            shinkai_log(
                ShinkaiLogOption::Network,
                ShinkaiLogLevel::Info,
                &format!("Initializing LibP2P manager with node: {}, port: {:?}, relay: {:?}", 
                    self.node_name, listen_port, relay_address),
            );

            eprintln!(">> DEBUG: Calling LibP2PManager::new with args: node={}, port={:?}, relay={:?}", 
                self.node_name, listen_port, relay_address);
            match LibP2PManager::new(self.node_name.to_string(), listen_port, message_handler, relay_address).await {
                Ok(libp2p_manager) => {
                    eprintln!(">> DEBUG: LibP2PManager::new succeeded!");
                    let event_sender = libp2p_manager.event_sender();
                    let libp2p_manager_arc = Arc::new(Mutex::new(libp2p_manager));

                    eprintln!(">> DEBUG: About to spawn libp2p task");
                    // Spawn the libp2p task
                    let manager_clone = libp2p_manager_arc.clone();
                    let libp2p_task = tokio::spawn(async move {
                        let mut manager = manager_clone.lock().await;
                        shinkai_log(
                            ShinkaiLogOption::Network,
                            ShinkaiLogLevel::Info,
                            "Starting LibP2P manager event loop",
                        );
                        if let Err(e) = manager.run().await {
                            shinkai_log(
                                ShinkaiLogOption::Network,
                                ShinkaiLogLevel::Error,
                                &format!("LibP2P manager error: {}", e),
                            );
                        }
                    });

                    eprintln!(">> DEBUG: Setting LibP2P manager fields");
                    self.libp2p_manager = Some(libp2p_manager_arc);
                    self.libp2p_event_sender = Some(event_sender);
                    self.libp2p_task = Some(libp2p_task);

                    eprintln!(">> DEBUG: LibP2P initialization completed successfully");
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Info,
                        "LibP2P networking initialized successfully",
                    );
                }
                Err(e) => {
                    eprintln!(">> DEBUG: LibP2PManager::new failed with error: {}", e);
                    shinkai_log(
                        ShinkaiLogOption::Network,
                        ShinkaiLogLevel::Error,
                        &format!("Failed to initialize LibP2P: {}", e),
                    );
                    // Continue without libp2p - fallback to TCP networking
                }
            }
        }
        eprintln!(">> DEBUG: Exited LibP2P initialization block");

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
                        let libp2p_event_sender = self.libp2p_event_sender.clone();

                        // Spawn a new task to call `retry_messages` asynchronously
                        tokio::spawn(async move {
                            let _ = Self::retry_messages(
                                db_clone,
                                encryption_secret_key_clone,
                                identity_manager_clone,
                                proxy_connection_info,
                                ws_manager_trait,
                                libp2p_event_sender,
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

    // A function that handles LibP2P networking when a proxy is configured
    async fn listen_and_reconnect(&self, proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>) {
        shinkai_log(
            ShinkaiLogOption::Node,
            ShinkaiLogLevel::Info,
            &format!("{} > Starting networking with LibP2P relay.", self.listen_address),
        );

        let proxy_info = {
            let proxy_info_lock = proxy_connection_info.lock().await;
            proxy_info_lock.clone()
        };

        if let Some(proxy_info) = proxy_info {
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Info,
                &format!("Proxy configured: {} - using LibP2P relay", proxy_info.proxy_identity),
            );
        } else {
            shinkai_log(
                ShinkaiLogOption::Node,
                ShinkaiLogLevel::Info,
                "No proxy configured - running without LibP2P relay networking",
            );
        }

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
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

    async fn retry_messages(
        db: Arc<SqliteManager>,
        encryption_secret_key: EncryptionStaticKey,
        identity_manager: Arc<Mutex<IdentityManager>>,
        proxy_connection_info: Arc<Mutex<Option<ProxyConnectionInfo>>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
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
                libp2p_event_sender.clone(),
            );
        }

        Ok(())
    }

    // Send a message to a peer using libp2p or TCP as fallback
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
        libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
    ) {
        tokio::spawn(async move {
            eprintln!(">> DEBUG: Node::send called");
            
            // Check if we have LibP2P available (with or without proxy)
            let has_proxy = {
                let proxy_info = proxy_connection_info.lock().await;
                proxy_info.is_some()
            };
            eprintln!(">> DEBUG: has_proxy = {}", has_proxy);

            // Try LibP2P first if available (either with proxy or direct networking)
            if libp2p_event_sender.is_some() {
                eprintln!(">> DEBUG: libp2p_event_sender is available");
                let use_libp2p_reason = if has_proxy {
                    "proxy configured"
                } else {
                    "direct networking"
                };
                
                eprintln!(">> DEBUG: Using LibP2P for message sending ({})", use_libp2p_reason);
                shinkai_log(
                    ShinkaiLogOption::Node,
                    ShinkaiLogLevel::Info,
                    &format!("Using LibP2P for message sending ({})", use_libp2p_reason),
                );

                let profile_name = &peer.1;
                eprintln!(">> DEBUG: profile_name = '{}'", profile_name);
                if let Some(sender) = libp2p_event_sender {
                    eprintln!(">> DEBUG: About to call send_via_libp2p");
                    match Node::send_via_libp2p(
                        message.clone(),
                        &sender,
                        profile_name,
                        save_to_db_flag,
                        my_encryption_sk.clone(),
                        db.clone(),
                        maybe_identity_manager.clone(),
                        ws_manager.clone(),
                    )
                    .await
                    {
                        Ok(_) => {
                            eprintln!(">> DEBUG: send_via_libp2p succeeded");
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Info,
                                "Message sent successfully via LibP2P",
                            );
                            return; // Success, no need to fallback
                        }
                        Err(e) => {
                            eprintln!(">> DEBUG: send_via_libp2p failed: {}", e);
                            shinkai_log(
                                ShinkaiLogOption::Node,
                                ShinkaiLogLevel::Error,
                                &format!("LibP2P sending failed, falling back to TCP: {}", e),
                            );
                        }
                    }
                }
            } else {
                eprintln!(">> DEBUG: libp2p_event_sender is None, skipping LibP2P");
            }
        });
    }

    // Send a message via libp2p
    #[allow(clippy::too_many_arguments)]
    pub async fn send_via_libp2p(
        message: ShinkaiMessage,
        libp2p_event_sender: &tokio::sync::mpsc::UnboundedSender<NetworkEvent>,
        _profile_name: &str, // Keep parameter for compatibility but don't use it
        save_to_db_flag: bool,
        my_encryption_sk: Arc<EncryptionStaticKey>,
        db: Arc<SqliteManager>,
        maybe_identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        eprintln!(">> DEBUG: send_via_libp2p called");
        
        // Extract the recipient node name from the message itself for topic creation
        let recipient_node_name = message.external_metadata.recipient.clone();
        eprintln!(">> DEBUG: Using recipient from message: '{}'", recipient_node_name);
        
        let topic = format!("shinkai-{}", recipient_node_name);
        eprintln!(">> DEBUG: Broadcasting to topic: '{}'", topic);

        let network_event = NetworkEvent::BroadcastMessage {
            topic: topic.clone(),
            message: message.clone(),
        };

        eprintln!(">> DEBUG: About to send network event via libp2p_event_sender");
        if let Err(e) = libp2p_event_sender.send(network_event) {
            eprintln!(">> DEBUG: Failed to send network event: {}", e);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to send via libp2p: {}", e),
            )));
        }
        eprintln!(">> DEBUG: Network event sent successfully");

        // Save to database if requested
        if save_to_db_flag {
            eprintln!(">> DEBUG: About to save message to database");
            Node::save_to_db(
                true,
                &message,
                my_encryption_sk.as_ref().clone(),
                db,
                maybe_identity_manager,
                ws_manager,
            )
            .await?;
            eprintln!(">> DEBUG: Message saved to database successfully");
        } else {
            eprintln!(">> DEBUG: Skipping database save (save_to_db_flag=false)");
        }

        shinkai_log(
            ShinkaiLogOption::Network,
            ShinkaiLogLevel::Info,
            &format!("Message sent via LibP2P to topic: {}", topic),
        );

        eprintln!(">> DEBUG: send_via_libp2p completed successfully");
        Ok(())
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
        if let Some(handle) = self.libp2p_task.take() {
            handle.abort();
        }
    }
}
