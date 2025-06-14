use crate::llm_provider::error::LLMProviderError;
use crate::managers::identity_manager::IdentityManagerTrait;
use crate::managers::tool_router::ToolRouter;
use crate::network::libp2p_manager::NetworkEvent;
use crate::network::network_manager_utils::{get_proxy_builder_info_static, send_message_to_peer};
use crate::network::node::ProxyConnectionInfo;
use crate::wallet::wallet_error;
use crate::wallet::wallet_manager::WalletManager;
use chrono::{Duration, Utc};
use ed25519_dalek::SigningKey;
use futures::Future;
use shinkai_job_queue_manager::job_queue_manager::JobQueueManager;
use shinkai_message_primitives::schemas::invoices::{
    Invoice, InvoiceError, InvoiceRequest, InvoiceRequestNetworkError, InvoiceStatusEnum
};
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::schemas::shinkai_tool_offering::{
    ShinkaiToolOffering, ToolPrice, UsageType, UsageTypeInquiry
};
use shinkai_message_primitives::shinkai_message::shinkai_message::ExternalMetadata;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
use shinkai_message_primitives::shinkai_utils::encryption::clone_static_secret_key;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
use shinkai_non_rust_code::functions::x402;
use shinkai_non_rust_code::functions::x402::settle_payment::settle_payment;
use shinkai_non_rust_code::functions::x402::settle_payment::Input as SettleInput;
use shinkai_non_rust_code::functions::x402::verify_payment::verify_payment;
use shinkai_sqlite::SqliteManager;
use std::collections::HashSet;
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Arc;
use std::sync::Weak;
use std::{env, fmt};
use tokio::sync::{Mutex, Semaphore};

use shinkai_message_primitives::schemas::x402_types::{
    ERC20Asset, ERC20TokenAmount, FacilitatorConfig, Network, Price, EIP712
};
use x25519_dalek::StaticSecret as EncryptionStaticKey;

#[derive(Debug, Clone)]
pub enum AgentOfferingManagerError {
    OperationFailed(String),
    InvalidUsageType(String),
}

impl fmt::Display for AgentOfferingManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentOfferingManagerError::OperationFailed(msg) => write!(f, "Operation failed: {}", msg),
            AgentOfferingManagerError::InvalidUsageType(msg) => write!(f, "Invalid usage type: {}", msg),
        }
    }
}

impl From<wallet_error::WalletError> for AgentOfferingManagerError {
    fn from(error: wallet_error::WalletError) -> Self {
        AgentOfferingManagerError::OperationFailed(format!("Wallet error: {:?}", error))
    }
}

impl From<InvoiceError> for AgentOfferingManagerError {
    fn from(error: InvoiceError) -> Self {
        AgentOfferingManagerError::OperationFailed(format!("Invoice error: {:?}", error))
    }
}

// TODO: for the hash maybe we could use public_key + nonce
// and then that hash it is used to produce another hash that's shared
// this way we never share our public key + nonce
// what's this public key? is it a new one generated from the sk?
// should we use the name of the destination as part of the hash?

pub struct ExtAgentOfferingsManager {
    pub db: Weak<SqliteManager>,
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
    // The address of the proxy server (if any)
    pub proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
    // Identity manager like trait
    pub identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
    // pub shared_tools: Arc<DashMap<String, ShinkaiToolOffering>>, // (streamer_profile:::path, shared_folder)
    pub offerings_queue_manager: Arc<Mutex<JobQueueManager<Invoice>>>,
    pub offering_processing_task: Option<tokio::task::JoinHandle<()>>,
    pub tool_router: Weak<ToolRouter>,
    pub wallet_manager: Weak<Mutex<Option<WalletManager>>>,
    pub libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
}

const NUM_THREADS: usize = 4;

impl ExtAgentOfferingsManager {
    #[allow(clippy::too_many_arguments)]
    ///
    /// Creates a new instance of `ExtAgentOfferingsManager`.
    ///
    /// # Arguments
    ///
    /// * `db` - Weak reference to the ShinkaiDB.
    /// * `vector_fs` - Weak reference to the VectorFS.
    /// * `identity_manager` - Weak reference to the identity manager.
    /// * `node_name` - The name of the node.
    /// * `my_signature_secret_key` - The secret key used for signing operations.
    /// * `my_encryption_secret_key` - The secret key used for encryption and decryption.
    /// * `proxy_connection_info` - Weak reference to the proxy connection info.
    /// * `tool_router` - Weak reference to the tool router.
    /// * `wallet_manager` - Weak reference to the wallet manager.
    ///
    /// # Returns
    ///
    /// * `Self` - A new instance of `ExtAgentOfferingsManager`.
    pub async fn new(
        db: Weak<SqliteManager>,
        identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        tool_router: Weak<ToolRouter>,
        wallet_manager: Weak<Mutex<Option<WalletManager>>>,
        libp2p_event_sender: Option<tokio::sync::mpsc::UnboundedSender<NetworkEvent>>,
        // need tool_router
    ) -> Self {
        let db_prefix = "shinkai__tool__offering_"; // dont change it
        let offerings_queue = JobQueueManager::<Invoice>::new(db.clone(), Some(db_prefix.to_string()))
            .await
            .unwrap();

        let thread_number = env::var("AGENTS_OFFERING_NETWORK_CONCURRENCY")
            .unwrap_or(NUM_THREADS.to_string())
            .parse::<usize>()
            .unwrap_or(NUM_THREADS); // Start processing the job queue

        let offerings_queue_manager = Arc::new(Mutex::new(offerings_queue));

        let offering_queue_handler = ExtAgentOfferingsManager::process_offerings_queue(
            offerings_queue_manager.clone(),
            db.clone(),
            node_name.clone(),
            my_signature_secret_key.clone(),
            my_encryption_secret_key.clone(),
            identity_manager.clone(),
            thread_number,
            proxy_connection_info.clone(),
            tool_router.clone(),
            |invoice_payment,
             db,
             node_name,
             my_signature_secret_key,
             my_encryption_secret_key,
             identity_manager,
             proxy_connection_info,
             tool_router| {
                ExtAgentOfferingsManager::process_invoice_payment(
                    invoice_payment,
                    db,
                    node_name,
                    my_signature_secret_key,
                    my_encryption_secret_key,
                    identity_manager,
                    proxy_connection_info,
                    tool_router,
                )
            },
        )
        .await;

        Self {
            db,
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
            proxy_connection_info,
            identity_manager,
            offerings_queue_manager,
            offering_processing_task: Some(offering_queue_handler),
            tool_router,
            wallet_manager,
            libp2p_event_sender,
        }
    }

    /// Update the libp2p event sender after initialization
    pub fn update_libp2p_event_sender(&mut self, sender: tokio::sync::mpsc::UnboundedSender<NetworkEvent>) {
        self.libp2p_event_sender = Some(sender);
    }

    // TODO: Should be split this into two? one for invoices and one for actual tool jobs?
    #[allow(clippy::too_many_arguments)]
    ///
    /// Processes the offerings queue.
    ///
    /// # Arguments
    ///
    /// * `offering_queue_manager` - The job queue manager for invoices.
    /// * `db` - Weak reference to the ShinkaiDB.
    /// * `vector_fs` - Weak reference to the VectorFS.
    /// * `node_name` - The name of the node.
    /// * `my_signature_secret_key` - The secret key used for signing operations.
    /// * `my_encryption_secret_key` - The secret key used for encryption and decryption.
    /// * `identity_manager` - Weak reference to the identity manager.
    /// * `thread_number` - The number of threads to use for processing.
    /// * `proxy_connection_info` - Weak reference to the proxy connection info.
    /// * `tool_router` - Weak reference to the tool router.
    /// * `process_job` - The function to process each job.
    ///
    /// # Returns
    ///
    /// * `tokio::task::JoinHandle<()>` - A handle to the spawned task.
    pub async fn process_offerings_queue(
        offering_queue_manager: Arc<Mutex<JobQueueManager<Invoice>>>,
        db: Weak<SqliteManager>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
        // shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
        thread_number: usize,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        tool_router: Weak<ToolRouter>,
        process_job: impl Fn(
                Invoice,
                Weak<SqliteManager>,
                ShinkaiName,
                SigningKey,
                EncryptionStaticKey,
                Weak<Mutex<dyn IdentityManagerTrait + Send>>,
                Weak<Mutex<Option<ProxyConnectionInfo>>>,
                Weak<ToolRouter>,
            ) -> Pin<Box<dyn Future<Output = Result<String, AgentOfferingManagerError>> + Send>>
            + Send
            + Sync
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let offering_queue_manager = Arc::clone(&offering_queue_manager);
        let mut receiver = offering_queue_manager.lock().await.subscribe_to_all().await;
        let processing_jobs = Arc::new(Mutex::new(HashSet::new()));
        let semaphore = Arc::new(Semaphore::new(thread_number));
        let process_job = Arc::new(process_job);

        let is_testing = env::var("IS_TESTING").ok().map(|v| v == "1").unwrap_or(false);

        if is_testing {
            return tokio::spawn(async {});
        }

        tokio::spawn(async move {
            shinkai_log(
                ShinkaiLogOption::ExtSubscriptions,
                ShinkaiLogLevel::Info,
                "process_subscription_queue> Starting subscribers processing loop",
            );

            let mut handles = Vec::new();
            loop {
                let continue_immediately;

                // Get the jobs to process
                let jobs_sorted = {
                    let mut processing_jobs_lock = processing_jobs.lock().await;
                    let job_queue_manager_lock = offering_queue_manager.lock().await;
                    let all_jobs = job_queue_manager_lock.get_all_elements_interleave().await;
                    drop(job_queue_manager_lock);

                    let filtered_jobs = all_jobs
                        .unwrap_or(Vec::new())
                        .into_iter()
                        .filter_map(|invoice_payment| {
                            let invoice_id = invoice_payment.invoice_id.clone(); // All jobs are now of the form of payment
                            if !processing_jobs_lock.contains(&invoice_id) {
                                processing_jobs_lock.insert(invoice_id.clone());
                                Some(invoice_payment)
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

                for invoice in jobs_sorted {
                    eprintln!(">> (process_offerings_queue) Processing job_offering: {:?}", invoice);
                    let offering_queue_manager = Arc::clone(&offering_queue_manager);
                    let processing_jobs = Arc::clone(&processing_jobs);
                    let semaphore = semaphore.clone();
                    let db = db.clone();
                    let node_name = node_name.clone();
                    let my_signature_secret_key = my_signature_secret_key.clone();
                    let my_encryption_secret_key = my_encryption_secret_key.clone();
                    let identity_manager = identity_manager.clone();
                    let process_job = process_job.clone();
                    let proxy_connection_info = proxy_connection_info.clone();
                    let invoice = invoice.clone();
                    let tool_router = tool_router.clone();

                    let handle = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.expect("Failed to acquire semaphore permit");

                        // Acquire the lock, process the job, and immediately release the lock
                        let result = {
                            let result = process_job(
                                invoice.clone(),
                                db.clone(),
                                node_name.clone(),
                                my_signature_secret_key.clone(),
                                my_encryption_secret_key.clone(),
                                identity_manager.clone(),
                                proxy_connection_info.clone(),
                                tool_router.clone(),
                            )
                            .await;
                            if let Ok(Some(_)) = offering_queue_manager
                                .lock()
                                .await
                                .dequeue(invoice.invoice_id.as_str())
                                .await
                            {
                                result
                            } else {
                                Err(AgentOfferingManagerError::OperationFailed(format!(
                                    "Failed to dequeue job: {}",
                                    invoice.invoice_id.as_str()
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

                        drop(_permit);
                        processing_jobs.lock().await.remove(&invoice.invoice_id);
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
                        format!("Received new paid invoice job {:?}", new_job.invoice_id.as_str()).as_str(),
                    );
                }
            }
        })
    }

    /// Note: The idea of this function is to be able to process the invoice payment and then
    /// call the tool router to process the tool job
    /// but in a way that we can have control of how many jobs are processed at the same time

    /// Processes the invoice payment.
    ///
    /// # Arguments
    ///
    /// * `_invoice` - The invoice to be processed.
    /// * `_db` - Weak reference to the ShinkaiDB.
    /// * `_vector_fs` - Weak reference to the VectorFS.
    /// * `_node_name` - The name of the node.
    /// * `_my_signature_secret_key` - The secret key used for signing operations.
    /// * `_my_encryption_secret_key` - The secret key used for encryption and decryption.
    /// * `_maybe_identity_manager` - Weak reference to the identity manager.
    /// * `_proxy_connection_info` - Weak reference to the proxy connection info.
    /// * `_tool_router` - Weak reference to the tool router.
    ///
    /// # Returns
    ///
    /// * `Pin<Box<dyn Future<Output = Result<String, AgentOfferingManagerError>> + Send + 'static>>` - A future that
    ///   resolves to the result of the processing.
    #[allow(clippy::too_many_arguments)]
    fn process_invoice_payment(
        _invoice: Invoice,
        _db: Weak<SqliteManager>,
        _node_name: ShinkaiName,
        _my_signature_secret_key: SigningKey,
        _my_encryption_secret_key: EncryptionStaticKey,
        _maybe_identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
        _proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        _tool_router: Weak<ToolRouter>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, AgentOfferingManagerError>> + Send + 'static>> {
        Box::pin(async move {
            // Actually do the work by calling tool_router
            // Then craft the message with the response and send it back to the requester
            unimplemented!()
        })
    }

    ///
    /// Retrieves the available tools.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<String>, AgentOfferingManagerError>` - A list of available tools or an error.
    pub async fn available_tools(&mut self) -> Result<Vec<String>, AgentOfferingManagerError> {
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        let tools = db.get_all_tool_offerings().map_err(|e| {
            AgentOfferingManagerError::OperationFailed(format!("Failed to get all tool offerings: {:?}", e))
        })?;

        let tool_names = tools.into_iter().map(|tool| tool.tool_key).collect();

        Ok(tool_names)
    }

    ///
    /// Updates the shareable tool requirements.
    ///
    /// # Arguments
    ///
    /// * `updated_offering` - The updated tool offering.
    ///
    /// # Returns
    ///
    /// * `Result<bool, AgentOfferingManagerError>` - True if successful, otherwise an error.
    pub async fn update_shareable_tool_requirements(
        &self,
        updated_offering: ShinkaiToolOffering,
    ) -> Result<bool, AgentOfferingManagerError> {
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        db.set_tool_offering(updated_offering).map_err(|e| {
            AgentOfferingManagerError::OperationFailed(format!("Failed to update tool offering: {:?}", e))
        })?;

        Ok(true)
    }

    ///
    /// Makes a tool shareable.
    ///
    /// # Arguments
    ///
    /// * `offering` - The tool offering to be made shareable.
    ///
    /// # Returns
    ///
    /// * `Result<bool, AgentOfferingManagerError>` - True if successful, otherwise an error.
    pub async fn make_tool_shareable(
        &mut self,
        offering: ShinkaiToolOffering,
    ) -> Result<bool, AgentOfferingManagerError> {
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        db.set_tool_offering(offering)
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to share tool: {:?}", e)))?;

        Ok(true)
    }

    ///
    /// Unshares a tool.
    ///
    /// # Arguments
    ///
    /// * `tool_key_name` - The key name of the tool to be unshared.
    ///
    /// # Returns
    ///
    /// * `Result<bool, AgentOfferingManagerError>` - True if successful, otherwise an error.
    pub async fn unshare_tool(&mut self, tool_key_name: String) -> Result<bool, AgentOfferingManagerError> {
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        db.remove_tool_offering(&tool_key_name)
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to unshare tool: {:?}", e)))?;

        Ok(true)
    }

    ///
    /// Requests an invoice.
    ///
    /// # Arguments
    ///
    /// * `_requester_node_name` - The name of the requester node.
    /// * `invoice_request` - The invoice request.
    ///
    /// # Returns
    ///
    /// * `Result<Invoice, AgentOfferingManagerError>` - The generated invoice or an error.
    pub async fn invoice_requested(
        &mut self,
        _requester_node_name: ShinkaiName,
        invoice_request: InvoiceRequest,
    ) -> Result<Invoice, AgentOfferingManagerError> {
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        // Validate and convert the tool_key_name
        let actual_tool_key_name = invoice_request.validate_and_convert_tool_key(&self.node_name)?;

        let shinkai_offering = db
            .get_tool_offering(&actual_tool_key_name)
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to get tool offering: {:?}", e)))?;

        let usage_type = match invoice_request.usage_type_inquiry {
            UsageTypeInquiry::PerUse => match shinkai_offering.usage_type {
                UsageType::PerUse(price) => UsageType::PerUse(price),
                _ => {
                    return Err(AgentOfferingManagerError::InvalidUsageType(
                        "Invalid usage type for PerUse inquiry".to_string(),
                    ))
                }
            },
        };

        // Check if an invoice with the same ID already exists
        if db.get_invoice(&invoice_request.unique_id).is_ok() {
            return Err(AgentOfferingManagerError::OperationFailed(
                "Invoice with the same ID already exists".to_string(),
            ));
        }

        // Scoped block to get address and network
        let public_address = {
            let wallet_manager = self.wallet_manager.upgrade().ok_or_else(|| {
                AgentOfferingManagerError::OperationFailed("Failed to upgrade wallet_manager reference".to_string())
            })?;
            let wallet_manager_lock = wallet_manager.lock().await;
            let wallet = wallet_manager_lock.as_ref().ok_or_else(|| {
                AgentOfferingManagerError::OperationFailed("Failed to get wallet manager lock".to_string())
            })?;
            wallet.receiving_wallet.get_payment_address()
        };

        let invoice = Invoice {
            invoice_id: invoice_request.unique_id.clone(),
            provider_name: self.node_name.clone(),
            requester_name: invoice_request.requester_name.clone(),
            shinkai_offering: ShinkaiToolOffering {
                tool_key: invoice_request.tool_key_name,
                usage_type,
                meta_description: None,
            },
            expiration_time: Utc::now() + Duration::hours(12),
            status: InvoiceStatusEnum::Pending,
            payment: None, // Payment will be set when the buyer pays
            address: public_address,
            usage_type_inquiry: invoice_request.usage_type_inquiry,
            request_date_time: invoice_request.request_date_time,
            invoice_date_time: Utc::now(),
            tool_data: None,
            result_str: None,
            response_date_time: None,
        };

        // Store the new invoice in the database
        db.set_invoice(&invoice)
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to store invoice: {:?}", e)))?;

        Ok(invoice)
    }

    ///
    /// Requests an invoice from the network.
    ///
    /// # Arguments
    ///
    /// * `requester_node_name` - The name of the requester node.
    /// * `invoice_request` - The invoice request.
    ///
    /// # Returns
    ///
    /// * `Result<Invoice, AgentOfferingManagerError>` - The generated invoice or an error.
    pub async fn network_invoice_requested(
        &mut self,
        requester_node_name: ShinkaiName,
        invoice_request: InvoiceRequest,
        external_metadata: Option<ExternalMetadata>,
    ) -> Result<Invoice, AgentOfferingManagerError> {
        // Call request_invoice to generate an invoice
        let invoice = self
            .invoice_requested(requester_node_name.clone(), invoice_request.clone())
            .await;

        let invoice = match invoice {
            Ok(inv) => inv,
            Err(e) => {
                // Handle the error manually
                shinkai_log(
                    ShinkaiLogOption::ExtSubscriptions,
                    ShinkaiLogLevel::Error,
                    &format!("Failed to request invoice: {:?}", e),
                );
                eprintln!("Failed to request invoice: {:?}", e);

                // Create an InvoiceNetworkError
                let network_error = InvoiceRequestNetworkError {
                    invoice_id: invoice_request.unique_id.clone(),
                    provider_name: self.node_name.clone(),
                    requester_name: invoice_request.requester_name.clone(),
                    request_date_time: invoice_request.request_date_time,
                    response_date_time: Utc::now(),
                    user_error_message: Some(format!("{:?}", e)),
                    error_message: format!("{:?}", e),
                };

                // Send the InvoiceRequestNetworkError back to the requester
                if let Some(identity_manager_arc) = self.identity_manager.upgrade() {
                    let identity_manager = identity_manager_arc.lock().await;
                    let standard_identity = identity_manager
                        .external_profile_to_global_identity(&invoice_request.requester_name.to_string(), None)
                        .await
                        .map_err(|e| AgentOfferingManagerError::OperationFailed(e))?;
                    drop(identity_manager);
                    let receiver_public_key = standard_identity.node_encryption_public_key;

                    let error_message = ShinkaiMessageBuilder::create_generic_invoice_message(
                        network_error.clone(),
                        MessageSchemaType::InvoiceRequestNetworkError,
                        clone_static_secret_key(&self.my_encryption_secret_key),
                        clone_signature_secret_key(&self.my_signature_secret_key),
                        receiver_public_key,
                        self.node_name.to_string(),
                        "".to_string(),
                        invoice_request.requester_name.to_string(),
                        "main".to_string(),
                        external_metadata,
                    )
                    .map_err(|e| AgentOfferingManagerError::OperationFailed(e.to_string()))?;

                    send_message_to_peer(
                        error_message,
                        self.db.clone(),
                        standard_identity,
                        self.my_encryption_secret_key.clone(),
                        self.identity_manager.clone(),
                        self.proxy_connection_info.clone(),
                        self.libp2p_event_sender.clone(),
                    )
                    .await?;
                }

                return Err(e);
            }
        };

        // Continue
        if let Some(identity_manager_arc) = self.identity_manager.upgrade() {
            eprintln!("🔑 Creating invoice message, requester_node_name: {:?}, invoice_request: {:?}", requester_node_name, invoice_request);
            let identity_manager = identity_manager_arc.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&requester_node_name.to_string(), None)
                .await
                .map_err(|e| AgentOfferingManagerError::OperationFailed(e))?;
            drop(identity_manager);
            let receiver_public_key = if invoice_request.requester_name.get_node_name_string().starts_with("@@localhost.") {
                // For localhost nodes, we need to use the public key from the external metadata
                let public_key_bytes = hex::decode(external_metadata.clone().unwrap().other).map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to decode public key hex: {}", e)))?;
                if public_key_bytes.len() != 32 {
                    return Err(AgentOfferingManagerError::OperationFailed("Public key must be 32 bytes".to_string()));
                }
                let mut array = [0u8; 32];
                array.copy_from_slice(&public_key_bytes);
                x25519_dalek::PublicKey::from(array)
            } else {
                standard_identity.node_encryption_public_key
            };

            // Generate the message to request the invoice
            let message = ShinkaiMessageBuilder::create_generic_invoice_message(
                invoice.clone(),
                MessageSchemaType::Invoice,
                clone_static_secret_key(&self.my_encryption_secret_key),
                clone_signature_secret_key(&self.my_signature_secret_key),
                receiver_public_key,
                self.node_name.to_string(),
                "".to_string(),
                invoice_request.requester_name.to_string(),
                "main".to_string(),
                external_metadata,
            )
            .map_err(|e| AgentOfferingManagerError::OperationFailed(e.to_string()))?;

            eprintln!(
                "sending message to peer {:?}",
                invoice_request.requester_name.to_string()
            );
            send_message_to_peer(
                message,
                self.db.clone(),
                standard_identity,
                self.my_encryption_secret_key.clone(),
                self.identity_manager.clone(),
                self.proxy_connection_info.clone(),
                self.libp2p_event_sender.clone(),
            )
            .await?;
        }

        // Return the generated invoice
        Ok(invoice)
    }

    ///
    /// Confirms the payment of an invoice and processes it.
    ///
    /// # Arguments
    ///
    /// * `requester_node_name` - The name of the requester node.
    /// * `invoice` - The invoice to be confirmed and processed.
    ///
    /// # Returns
    ///
    /// * `Result<Invoice, AgentOfferingManagerError>` - The processed invoice or an error.
    pub async fn confirm_invoice_payment_and_process(
        &mut self,
        _requester_node_name: ShinkaiName,
        invoice: Invoice,
        // prehash_validation: String, // TODO: connect later on
    ) -> Result<Invoice, AgentOfferingManagerError> {
        // Step 1: verify that the invoice is actually real
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        let mut local_invoice = db
            .get_invoice(&invoice.invoice_id)
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to get invoice: {:?}", e)))?;

        println!("local_invoice: {:?}", local_invoice);
        println!("received invoice: {:?}", invoice);

        // Step 2: verify that the invoice is actually paid
        let payment_payload = invoice
            .payment
            .as_ref()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("No payment found in invoice".to_string()))?;
        let transaction_signed = Some(payment_payload.transaction_signed.clone());

        // Extract payment requirements from local_invoice
        let payment_requirements = match &local_invoice.shinkai_offering.usage_type {
            // Note: we are only supporting one payment requirement for now
            UsageType::PerUse(ToolPrice::Payment(reqs)) => reqs.get(0).ok_or_else(|| {
                AgentOfferingManagerError::OperationFailed("No payment requirements found".to_string())
            })?,
            _ => {
                return Err(AgentOfferingManagerError::OperationFailed(
                    "Unsupported usage type".to_string(),
                ))
            }
        };

        // TODO: needs refactor
        let input = {
            // If the asset is USDC, use ERC20TokenAmount, otherwise use Money
            if payment_requirements.asset == "USDC"
                || payment_requirements.asset.to_lowercase() == "usdc"
                || payment_requirements.asset == "0x036CbD53842c5426634e7929541eC2318f3dCF7e"
                || payment_requirements.asset == "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
            {
                // Determine address and decimals based on network
                let (address, decimals) = match payment_requirements.network {
                    Network::BaseSepolia => ("0x036CbD53842c5426634e7929541eC2318f3dCF7e", 6),
                    Network::Base => ("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", 6),
                    _ => (payment_requirements.asset.as_str(), 6), // fallback
                };
                let erc20_asset = ERC20Asset {
                    address: address.to_string(),
                    decimals,
                    eip712: EIP712 {
                        name: "USDC".to_string(),
                        version: "2".to_string(),
                    },
                };
                x402::verify_payment::Input {
                    price: Price::ERC20TokenAmount(ERC20TokenAmount {
                        amount: payment_requirements.max_amount_required.clone(),
                        asset: erc20_asset,
                    }),
                    network: payment_requirements.network.clone(),
                    pay_to: payment_requirements.pay_to.clone(),
                    payment: transaction_signed,
                    x402_version: 1, // or your version
                    facilitator: FacilitatorConfig::default(),
                }
            } else {
                x402::verify_payment::Input {
                    price: Price::Money(payment_requirements.max_amount_required.parse::<f64>().unwrap_or(0.0)),
                    network: payment_requirements.network.clone(),
                    pay_to: payment_requirements.pay_to.clone(),
                    payment: transaction_signed,
                    x402_version: 1, // or your version
                    facilitator: FacilitatorConfig::default(),
                }
            }
        };

        println!("\n\ninput for payment verification: {:?}", input);

        let output = verify_payment(input)
            .await
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Payment verification failed: {:?}", e)))?;

        println!("\noutput of payment verification: {:?}", output);

        if output.valid.is_none() {
            return Err(AgentOfferingManagerError::OperationFailed(
                "Payment verification failed".to_string(),
            ));
        }

        // Step 3: we extract the data_payload and then we call the tool with it
        let data_payload = invoice
            .tool_data
            .and_then(|args_value: serde_json::Value| args_value.as_object().cloned())
            .unwrap_or_else(|| serde_json::Map::new());
        {
            let tool_router = self.tool_router.upgrade().ok_or_else(|| {
                AgentOfferingManagerError::OperationFailed("Failed to upgrade tool_router reference".to_string())
            })?;

            // js tool name
            let local_tool_key = local_invoice.shinkai_offering.convert_tool_to_local().map_err(|e| {
                AgentOfferingManagerError::OperationFailed(format!(
                    "Failed to convert tool_key to local tool_key: {:?}",
                    e
                ))
            })?;

            let result = tool_router
                .call_js_function(data_payload, _requester_node_name, &local_tool_key)
                .await
                .map_err(|e: LLMProviderError| {
                    AgentOfferingManagerError::OperationFailed(format!("LLMProviderError: {:?}", e))
                })?;

            println!("result: {:?}", result);

            local_invoice.result_str = Some(result);
            local_invoice.status = InvoiceStatusEnum::Processed;
            local_invoice.response_date_time = Some(Utc::now());

            db.set_invoice(&local_invoice)
                .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to set invoice: {:?}", e)))?;
        }

        // Step 4: if we got a successful result, we settle the payment
        // For testing maybe we can add a flag to avoid this step
        let is_testing = std::env::var("IS_TESTING").ok().map(|v| v == "1").unwrap_or(false);
        if !is_testing {
            // Extract decoded_payment for settlement
            let decoded_payment = output.valid.as_ref().unwrap().decoded_payment.clone();

            let payment_requirements = match &local_invoice.shinkai_offering.usage_type {
                UsageType::PerUse(ToolPrice::Payment(reqs)) => reqs.clone(),
                _ => {
                    return Err(AgentOfferingManagerError::OperationFailed(
                        "Unsupported usage type for settlement".to_string(),
                    ))
                }
            };
            let settle_input = SettleInput {
                payment: decoded_payment,
                accepts: payment_requirements,
                facilitator: FacilitatorConfig::default(),
            };
            let settle_result = settle_payment(settle_input).await.map_err(|e| {
                AgentOfferingManagerError::OperationFailed(format!("Payment settlement failed: {:?}", e))
            })?;
            if settle_result.valid.is_none() {
                local_invoice.status = InvoiceStatusEnum::Failed;
                db.set_invoice(&local_invoice).map_err(|e| {
                    AgentOfferingManagerError::OperationFailed(format!(
                        "Failed to set invoice after failed settlement: {:?}",
                        e
                    ))
                })?;
                return Err(AgentOfferingManagerError::OperationFailed(
                    "Payment settlement failed".to_string(),
                ));
            }
        }

        // Old stuff below

        // TODO: we need the transaction_id and then call the crypto service to verify the payment
        // Note: how do we know that this identity actually was the one that paid for it? -> prehash validation

        // TODO: update the db and mark the invoice as paid (maybe after the job is done)
        // Note: what happens if the job fails? should we retry and then good-luck with the payment?
        // Should we actually receive the job input before the payment so we can confirm that we are "comfortable" with
        // the job? What happens if you want to crawl a website, but the website is down? should we refund the
        // payment? What happens if the job is done, but the requester is not happy with the result? should we
        // refund the payment?

        Ok(local_invoice)
    }

    ///
    /// Confirms the payment of an invoice from the network and processes it.
    ///
    /// # Arguments
    ///
    /// * `requester_node_name` - The name of the requester node.
    /// * `invoice` - The invoice to be confirmed and processed.
    ///
    /// # Returns
    ///
    /// * `Result<(), AgentOfferingManagerError>` - Ok if successful, otherwise an error.
    pub async fn network_confirm_invoice_payment_and_process(
        &mut self,
        requester_node_name: ShinkaiName,
        invoice: Invoice,
    ) -> Result<(), AgentOfferingManagerError> {
        // Call confirm_invoice_payment_and_process to process the invoice
        let local_invoice = self
            .confirm_invoice_payment_and_process(requester_node_name.clone(), invoice.clone())
            .await?;

        // Continue
        if let Some(identity_manager_arc) = self.identity_manager.upgrade() {
            let identity_manager = identity_manager_arc.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&requester_node_name.to_string(), None)
                .await
                .map_err(|e| AgentOfferingManagerError::OperationFailed(e))?;
            drop(identity_manager);
            let receiver_public_key = standard_identity.node_encryption_public_key;

            // Send result back to requester
            let message = ShinkaiMessageBuilder::create_generic_invoice_message(
                local_invoice.clone(),
                MessageSchemaType::InvoiceResult,
                clone_static_secret_key(&self.my_encryption_secret_key),
                clone_signature_secret_key(&self.my_signature_secret_key),
                receiver_public_key,
                self.node_name.to_string(),
                "".to_string(),
                requester_node_name.to_string(),
                "main".to_string(),
                None,
            )
            .map_err(|e| AgentOfferingManagerError::OperationFailed(e.to_string()))?;

            send_message_to_peer(
                message,
                self.db.clone(),
                standard_identity,
                self.my_encryption_secret_key.clone(),
                self.identity_manager.clone(),
                self.proxy_connection_info.clone(),
                self.libp2p_event_sender.clone(),
            )
            .await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;
    use async_trait::async_trait;
    use shinkai_message_primitives::{
        schemas::identity::{Identity, StandardIdentity, StandardIdentityType}, shinkai_message::shinkai_message_schemas::IdentityPermissions, shinkai_utils::{
            encryption::unsafe_deterministic_encryption_keypair, signatures::unsafe_deterministic_signature_keypair
        }
    };

    #[derive(Clone, Debug)]
    struct MockIdentityManager {
        dummy_standard_identity: Identity,
        // Add any fields you need for your mock
    }

    impl MockIdentityManager {
        pub fn new() -> Self {
            let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
            let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

            let dummy_standard_identity = Identity::Standard(StandardIdentity {
                full_identity_name: ShinkaiName::new("@@node1.shinkai/main_profile_node1".to_string()).unwrap(),
                addr: None,
                node_encryption_public_key: node1_encryption_pk,
                node_signature_public_key: node1_identity_pk,
                profile_encryption_public_key: Some(node1_encryption_pk),
                profile_signature_public_key: Some(node1_identity_pk),
                identity_type: StandardIdentityType::Global,
                permission_type: IdentityPermissions::Admin,
            });

            Self {
                dummy_standard_identity,
                // initialize other fields...
            }
        }
    }

    #[async_trait]
    impl IdentityManagerTrait for MockIdentityManager {
        fn find_by_identity_name(&self, _full_profile_name: ShinkaiName) -> Option<&Identity> {
            if _full_profile_name.to_string() == "@@node1.shinkai/main" {
                Some(&self.dummy_standard_identity)
            } else {
                None
            }
        }

        async fn search_identity(&self, full_identity_name: &str) -> Option<Identity> {
            if full_identity_name == "@@node1.shinkai/main" {
                Some(self.dummy_standard_identity.clone())
            } else {
                None
            }
        }

        fn clone_box(&self) -> Box<dyn IdentityManagerTrait + Send> {
            Box::new(self.clone())
        }

        async fn external_profile_to_global_identity(
            &self,
            _full_profile_name: &str,
            _: Option<bool>,
        ) -> Result<StandardIdentity, String> {
            unimplemented!()
        }

        async fn get_routing_info(
            &self,
            _full_profile_name: &str,
            _: Option<bool>,
        ) -> Result<(bool, Vec<String>), String> {
            if _full_profile_name.to_string() == "@@node1.shinkai/main" {
                Ok((false, vec!["127.0.0.1:9552".to_string()]))
            } else {
                Err("Identity not found".to_string())
            }
        }
    }

    fn setup() {
        let path = Path::new("sqlite_tests/");
        let _ = fs::remove_dir_all(path);

        let path = Path::new("shinkai_db_tests/");
        let _ = fs::remove_dir_all(path);
    }

    fn default_test_profile() -> ShinkaiName {
        ShinkaiName::new("@@localhost.sep-shinkai/main".to_string()).unwrap()
    }

    fn node_name() -> ShinkaiName {
        ShinkaiName::new("@@localhost.sep-shinkai".to_string()).unwrap()
    }
}
