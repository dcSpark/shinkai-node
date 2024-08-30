use crate::db::{ShinkaiDB, Topic};
use crate::llm_provider::queue::job_queue_manager::JobQueueManager;
use crate::managers::identity_manager::IdentityManagerTrait;
use crate::network::node::ProxyConnectionInfo;
use crate::network::subscription_manager::subscriber_manager_error::SubscriberManagerError;
use crate::tools::tool_router::ToolRouter;
use crate::vector_fs::vector_fs::VectorFS;
use crate::wallet::wallet_manager::WalletManager;
use chrono::Utc;
use ed25519_dalek::SigningKey;
use futures::Future;
use serde_json::Value;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption};
use std::collections::HashSet;
use std::env;
use std::pin::Pin;
use std::result::Result::Ok;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::{Mutex, Semaphore};

use x25519_dalek::StaticSecret as EncryptionStaticKey;

use super::invoices::{InternalInvoiceRequest, Invoice, InvoicePayment, InvoiceStatusEnum};
use super::shinkai_tool_offering::{ShinkaiToolOffering, UsageType, UsageTypeInquiry};

#[derive(Debug, Clone)]
pub enum AgentOfferingManagerError {
    OperationFailed(String),
    InvalidUsageType(String),
}

// TODO: for the hash maybe we could use public_key + nonce
// and then that hash it is used to produce another hash that's shared
// this way we never share our public key + nonce
// what's this public key? is it a new one generated from the sk?
// should we use the name of the destination as part of the hash?

pub struct AgentOfferingsManager {
    pub db: Weak<ShinkaiDB>,
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
    pub identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
    // pub shared_tools: Arc<DashMap<String, ShinkaiToolOffering>>, // (streamer_profile:::path, shared_folder)
    pub offerings_queue_manager: Arc<Mutex<JobQueueManager<InvoicePayment>>>,
    pub offering_processing_task: Option<tokio::task::JoinHandle<()>>,
    pub tool_router: Weak<Mutex<ToolRouter>>,
    pub wallet_manager: Weak<Mutex<Option<WalletManager>>>,
}

const NUM_THREADS: usize = 4;

impl AgentOfferingsManager {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        tool_router: Weak<Mutex<ToolRouter>>,
        wallet_manager: Weak<Mutex<Option<WalletManager>>>,
        // need tool_router
    ) -> Self {
        let db_prefix = "shinkai__tool__offering_"; // dont change it
        let offerings_queue = JobQueueManager::<InvoicePayment>::new(
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

        let offering_queue_handler = AgentOfferingsManager::process_offerings_queue(
            offerings_queue_manager.clone(),
            db.clone(),
            vector_fs.clone(),
            node_name.clone(),
            my_signature_secret_key.clone(),
            my_encryption_secret_key.clone(),
            identity_manager.clone(),
            thread_number,
            proxy_connection_info.clone(),
            tool_router.clone(),
            |invoice_payment,
             db,
             vector_fs,
             node_name,
             my_signature_secret_key,
             my_encryption_secret_key,
             identity_manager,
             proxy_connection_info,
             tool_router| {
                AgentOfferingsManager::process_invoice_payment(
                    invoice_payment,
                    db,
                    vector_fs,
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
            identity_manager,
            offerings_queue_manager,
            offering_processing_task: Some(offering_queue_handler),
            tool_router,
            wallet_manager,
        }
    }

    // TODO: Should be split this into two? one for invoices and one for actual tool jobs?
    #[allow(clippy::too_many_arguments)]
    pub async fn process_offerings_queue(
        offering_queue_manager: Arc<Mutex<JobQueueManager<InvoicePayment>>>,
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
        // shared_folders_trees: Arc<DashMap<String, SharedFolderInfo>>,
        thread_number: usize,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        tool_router: Weak<Mutex<ToolRouter>>,
        process_job: impl Fn(
                InvoicePayment,
                Weak<ShinkaiDB>,
                Weak<VectorFS>,
                ShinkaiName,
                SigningKey,
                EncryptionStaticKey,
                Weak<Mutex<dyn IdentityManagerTrait + Send>>,
                Weak<Mutex<Option<ProxyConnectionInfo>>>,
                Weak<Mutex<ToolRouter>>,
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
                let mut continue_immediately = false;

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

                for invoice_payment in jobs_sorted {
                    eprintln!(
                        ">> (process_offerings_queue) Processing job_offering: {:?}",
                        invoice_payment
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
                    let invoice_payment = invoice_payment.clone();
                    let tool_router = tool_router.clone();

                    let handle = tokio::spawn(async move {
                        let _permit = semaphore.acquire().await.expect("Failed to acquire semaphore permit");

                        // Acquire the lock, process the job, and immediately release the lock
                        let result = {
                            let result = process_job(
                                invoice_payment.clone(),
                                db.clone(),
                                vector_fs.clone(),
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
                                .dequeue(invoice_payment.invoice_id.as_str())
                                .await
                            {
                                result
                            } else {
                                Err(AgentOfferingManagerError::OperationFailed(format!(
                                    "Failed to dequeue job: {}",
                                    invoice_payment.invoice_id.as_str()
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
                        processing_jobs.lock().await.remove(&invoice_payment.invoice_id);
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

    #[allow(clippy::too_many_arguments)]
    fn process_invoice_payment(
        invoice_payment: InvoicePayment,
        db: Weak<ShinkaiDB>,
        _vector_fs: Weak<VectorFS>,
        _node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        maybe_identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        tool_router: Weak<Mutex<ToolRouter>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, AgentOfferingManagerError>> + Send + 'static>> {
        Box::pin(async move {
            // Actually do the work by calling tool_router
            // Then craft the message with the response and send it back to the requester
            Ok(format!("Done. thx"))
        })
    }

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

    pub async fn unshare_tool(&mut self, tool_key_name: String) -> Result<bool, SubscriberManagerError> {
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| SubscriberManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        db.remove_tool_offering(&tool_key_name)
            .map_err(|e| SubscriberManagerError::OperationFailed(format!("Failed to unshare tool: {:?}", e)))?;

        Ok(true)
    }

    pub async fn request_invoice(
        &mut self,
        requester_node_name: ShinkaiName,
        tool_key_name: String,
        usage_type_inquiry: UsageTypeInquiry,
    ) -> Result<Invoice, AgentOfferingManagerError> {
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        let shinkai_offering = db
            .get_tool_offering(&tool_key_name)
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to get tool offering: {:?}", e)))?;

        let usage_type = match usage_type_inquiry {
            UsageTypeInquiry::PerUse => match shinkai_offering.usage_type {
                UsageType::PerUse(price) => UsageType::PerUse(price),
                UsageType::Both { per_use_price, .. } => UsageType::PerUse(per_use_price),
                _ => {
                    return Err(AgentOfferingManagerError::InvalidUsageType(
                        "Invalid usage type for PerUse inquiry".to_string(),
                    ))
                }
            },
            UsageTypeInquiry::Downloadable => match shinkai_offering.usage_type {
                UsageType::Downloadable(price) => UsageType::Downloadable(price),
                UsageType::Both { download_price, .. } => UsageType::Downloadable(download_price),
                _ => {
                    return Err(AgentOfferingManagerError::InvalidUsageType(
                        "Invalid usage type for Downloadable inquiry".to_string(),
                    ))
                }
            },
        };

        // TODO: invoice_id needs to be smarter
        // we need to make it based on the content of the request + some time + random number

        let invoice_request =
            InternalInvoiceRequest::new(requester_node_name.clone(), tool_key_name, usage_type_inquiry);

        let invoice = Invoice {
            invoice_id: invoice_request.unique_id.clone(),
            requester_name: requester_node_name,
            shinkai_offering: ShinkaiToolOffering {
                tool_key: invoice_request.tool_key_name.clone(),
                usage_type,
                meta_description: None,
            },
            expiration_time: Utc::now(),
            status: InvoiceStatusEnum::Pending,
            payment: None,
        };

        Ok(invoice)
    }

    pub async fn confirm_invoice_payment_and_process(
        &mut self,
        requester_node_name: ShinkaiName,
        invoice_id: String,
        signed_invoice: String, // TODO: maybe not required? we could just look into the db
        prehash_validation: String,
        process_data: Value,
        payment_id: String,
        payment_amount: String,
    ) -> Result<InvoicePayment, AgentOfferingManagerError> {
        let invoice_payment = InvoicePayment {
            invoice_id,
            date_time: Utc::now(),
            signed_invoice,
            payment_id,
            payment_amount,
            payment_time: Utc::now(),
            requester_node_name,
        };

        // TODO: we need the transaction_id and then call the crypto service to verify the payment
        // Note: how do we know that this identity actually was the one that paid for it? -> prehash validation

        // TODO: update the db and mark the invoice as paid (maybe after the job is done)
        // Note: what happens if the job fails? should we retry and then good-luck with the payment?
        // Should we actually receive the job input before the payment so we can confirm that we are "comfortable" with the job?
        // What happens if you want to crawl a website, but the website is down? should we refund the payment?
        // What happens if the job is done, but the requester is not happy with the result? should we refund the payment?

        Ok(invoice_payment)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use crate::{
        lance_db::{shinkai_lance_db::LanceShinkaiDb, shinkai_lancedb_error::ShinkaiLanceDBError},
        network::agent_payments_manager::shinkai_tool_offering::{Asset, AssetPayment, ToolPrice},
        schemas::identity::{Identity, StandardIdentity, StandardIdentityType},
        tools::{js_toolkit::JSToolkit, shinkai_tool::ShinkaiTool},
    };

    use super::*;
    use async_trait::async_trait;
    use shinkai_message_primitives::{
        shinkai_message::shinkai_message_schemas::IdentityPermissions,
        shinkai_utils::{
            encryption::unsafe_deterministic_encryption_keypair, shinkai_logging::init_default_tracing,
            signatures::unsafe_deterministic_signature_keypair,
        },
    };
    use shinkai_tools_runner::built_in_tools;
    use shinkai_vector_resources::{
        embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator},
        model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference},
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
    }

    fn setup() {
        let path = Path::new("lance_db_tests/");
        let _ = fs::remove_dir_all(path);

        let path = Path::new("shinkai_db_tests/");
        let _ = fs::remove_dir_all(path);
    }

    fn default_test_profile() -> ShinkaiName {
        ShinkaiName::new("@@localhost.arb-sep-shinkai/main".to_string()).unwrap()
    }

    fn node_name() -> ShinkaiName {
        ShinkaiName::new("@@localhost.arb-sep-shinkai".to_string()).unwrap()
    }

    async fn setup_default_vector_fs() -> VectorFS {
        let generator = RemoteEmbeddingGenerator::new_default();
        let fs_db_path = format!("db_tests/{}", "vector_fs");
        let profile_list = vec![default_test_profile()];
        let supported_embedding_models = vec![EmbeddingModelType::OllamaTextEmbeddingsInference(
            OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M,
        )];

        VectorFS::new(
            generator,
            supported_embedding_models,
            profile_list,
            &fs_db_path,
            node_name(),
        )
        .await
        .unwrap()
    }

    #[test]
    fn test_unique_id() {
        let invoice_request = InternalInvoiceRequest::new(
            ShinkaiName::new("@@nico.shinkai".to_string()).unwrap(),
            "test_tool".to_string(),
            UsageTypeInquiry::PerUse,
        );

        println!("Generated unique_id: {}", invoice_request.unique_id);

        // Assert that the unique_id is not empty
        assert!(!invoice_request.unique_id.is_empty());
    }

    #[tokio::test]
    async fn test_agent_offerings_manager() -> Result<(), ShinkaiLanceDBError> {
        
        setup();

        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        // Initialize ShinkaiDB
        let shinkai_db =
            Arc::new(ShinkaiDB::new("shinkai_db_tests/shinkaidb").map_err(|e| ShinkaiLanceDBError::from(e))?);

        let lance_db = Arc::new(Mutex::new(
            LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?,
        ));

        let tools = built_in_tools::get_tools();

        // Generate crypto keys
        let (my_signature_secret_key, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_secret_key, _) = unsafe_deterministic_encryption_keypair(0);

        // Create ToolRouter
        let tool_router = Arc::new(Mutex::new(ToolRouter::new(lance_db.clone())));

        // Create AgentOfferingsManager
        let node_name = node_name();
        let identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>> =
            Arc::new(Mutex::new(MockIdentityManager::new()));
        let proxy_connection_info = Arc::new(Mutex::new(None));
        let vector_fs = Arc::new(setup_default_vector_fs().await);

        // Wallet Manager
        let wallet_manager = Arc::new(Mutex::new(None));

        let mut agent_offerings_manager = AgentOfferingsManager::new(
            Arc::downgrade(&shinkai_db),
            Arc::downgrade(&vector_fs),
            Arc::downgrade(&identity_manager),
            node_name.clone(),
            my_signature_secret_key.clone(),
            my_encryption_secret_key.clone(),
            Arc::downgrade(&proxy_connection_info),
            Arc::downgrade(&tool_router),
            Arc::downgrade(&wallet_manager),
        )
        .await;

        // Add tools to the database
        for (name, definition) in tools {
            let toolkit = JSToolkit::new(&name, vec![definition.clone()]);
            for tool in toolkit.tools {
                let mut shinkai_tool = ShinkaiTool::JS(tool.clone(), true);
                eprintln!("shinkai_tool name: {:?}", shinkai_tool.name());
                let embedding = generator
                    .generate_embedding_default(&shinkai_tool.format_embedding_string())
                    .await
                    .unwrap();
                shinkai_tool.set_embedding(embedding);

                lance_db
                    .lock()
                    .await
                    .set_tool(&shinkai_tool)
                    .await
                    .map_err(|e| ShinkaiLanceDBError::ToolError(e.to_string()))?;

                // Check if the tool is "shinkai__weather_by_city" and make it shareable
                if shinkai_tool.name() == "shinkai__weather_by_city" {
                    let shinkai_offering = ShinkaiToolOffering {
                        tool_key: shinkai_tool.tool_router_key(),
                        usage_type: UsageType::PerUse(ToolPrice::Payment(vec![AssetPayment {
                            asset: Asset {
                                network_id: "1".to_string(),
                                asset_id: "ETH".to_string(),
                                decimals: Some(18),
                                contract_address: None,
                            },
                            amount: "0.01".to_string(),
                        }])),
                        meta_description: None,
                    };

                    agent_offerings_manager
                        .make_tool_shareable(shinkai_offering)
                        .await
                        .unwrap();
                }
            }
        }

        // Check available tools
        let available_tools = agent_offerings_manager.available_tools().await.unwrap();
        assert!(available_tools.contains(&"Weather by City".to_string()));

        Ok(())
    }
}
