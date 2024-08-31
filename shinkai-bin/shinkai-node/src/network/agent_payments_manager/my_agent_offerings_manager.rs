/*
- My Agent Payments Manager


Flow:
- request invoice
- pay invoice
- send receipt and data to provider
- receive response from provider and process it
(for this we need to know the provenance from where it came from)


Notes:
- what's the flow between requesting an invoice and paying it?
can it be done in one step? maybe we have rules per: tool, provider or overall spending.

*/

use std::sync::{Arc, Weak};

use ed25519_dalek::SigningKey;
use shinkai_message_primitives::{
    schemas::{shinkai_name::ShinkaiName, shinkai_proxy_builder_info::ShinkaiProxyBuilderInfo},
    shinkai_message::shinkai_message_schemas::MessageSchemaType,
    shinkai_utils::{
        encryption::clone_static_secret_key, shinkai_message_builder::ShinkaiMessageBuilder,
        signatures::clone_signature_secret_key,
    },
};
use tokio::sync::Mutex;
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::{
    db::ShinkaiDB,
    managers::identity_manager::IdentityManagerTrait,
    network::{
        agent_payments_manager::shinkai_tool_offering::ToolPrice,
        network_manager_utils::{get_proxy_builder_info_static, send_message_to_peer},
        node::ProxyConnectionInfo,
    },
    tools::{network_tool::NetworkTool, shinkai_tool::ShinkaiToolHeader, tool_router::ToolRouter},
    vector_fs::vector_fs::VectorFS,
    wallet::wallet_manager::WalletManager,
};

use super::{
    external_agent_offerings_manager::AgentOfferingManagerError,
    invoices::{InternalInvoiceRequest, Invoice, InvoiceStatusEnum, Payment},
    shinkai_tool_offering::UsageTypeInquiry,
};

pub struct MyAgentOfferingsManager {
    pub db: Weak<ShinkaiDB>,
    pub vector_fs: Weak<VectorFS>,
    pub identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
    // The address of the proxy server (if any)
    pub proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
    // Tool router
    pub tool_router: Weak<Mutex<ToolRouter>>,
    // Wallet manager
    pub wallet_manager: Weak<Mutex<Option<WalletManager>>>,
    // pub crypto_invoice_manager: Arc<Option<Box<dyn CryptoInvoiceManagerTrait + Send + Sync>>>,
}

impl MyAgentOfferingsManager {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        db: Weak<ShinkaiDB>,
        vector_fs: Weak<VectorFS>,
        identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        // ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
        tool_router: Weak<Mutex<ToolRouter>>,
        wallet_manager: Weak<Mutex<Option<WalletManager>>>,
        // crypto_invoice_manager: Arc<Option<Box<dyn CryptoInvoiceManagerTrait + Send + Sync>>>,
        // Do I need the payment manager for the node?
    ) -> Self {
        Self {
            db,
            vector_fs,
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
            proxy_connection_info,
            identity_manager,
            tool_router,
            wallet_manager,
            // crypto_invoice_manager,
        }
    }

    // Fn: Ask for the current offerings from the provider (directly)
    // Old fashion send message and wait network response

    // Fn: Ask for the current offerings of a provider through the indexer (indirectly)

    // Fn: (Temp) Hardcoded list of offerings of a provider (PoC)

    // Fn: Request an invoice <- can we triggered by the user through API
    // Note: currently works only for added network tools
    pub async fn request_invoice(
        &self,
        network_tool: NetworkTool,
        usage_type_inquiry: UsageTypeInquiry,
    ) -> Result<InternalInvoiceRequest, AgentOfferingManagerError> {
        // Upgrade the database reference to a strong reference
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        // Create a new InternalInvoiceRequest
        let internal_invoice_request = InternalInvoiceRequest::new(
            network_tool.provider.clone(),
            self.node_name.clone(),
            network_tool.tool_router_key(),
            usage_type_inquiry,
        );

        // Store the InternalInvoiceRequest in the database
        db.set_internal_invoice_request(&internal_invoice_request)
            .map_err(|e| {
                AgentOfferingManagerError::OperationFailed(format!("Failed to store internal invoice request: {:?}", e))
            })?;

        // Create the payload for the invoice request
        let payload = internal_invoice_request.to_invoice_request();

        // Continue
        if let Some(identity_manager_arc) = self.identity_manager.upgrade() {
            let identity_manager = identity_manager_arc.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&network_tool.provider.get_node_name_string())
                .await
                .map_err(|e| AgentOfferingManagerError::OperationFailed(e))?;
            drop(identity_manager);
            let receiver_public_key = standard_identity.node_encryption_public_key;
            let proxy_builder_info = self.get_proxy_builder_info(identity_manager_arc).await;

            // Generate the message to request the invoice
            let message = ShinkaiMessageBuilder::create_generic_invoice_message(
                payload,
                MessageSchemaType::InvoiceRequest,
                clone_static_secret_key(&self.my_encryption_secret_key),
                clone_signature_secret_key(&self.my_signature_secret_key),
                receiver_public_key,
                self.node_name.to_string(),
                "".to_string(),
                network_tool.provider.get_node_name_string(),
                "main".to_string(),
                proxy_builder_info,
            )
            .map_err(|e| AgentOfferingManagerError::OperationFailed(e.to_string()))?;

            send_message_to_peer(
                message,
                self.db.clone(),
                standard_identity,
                self.my_encryption_secret_key.clone(),
                self.identity_manager.clone(),
                self.proxy_connection_info.clone(),
            )
            .await?;
        }

        // Return the generated invoice request
        Ok(internal_invoice_request)
    }

    // Fn: Verify an invoice
    // - Check if the invoice is valid (even if we asked for it)
    // - Check about any rules for auto-payment
    pub async fn verify_invoice(&self, invoice: &Invoice) -> Result<bool, AgentOfferingManagerError> {
        // Upgrade the database reference to a strong reference
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        // Try to retrieve the corresponding InternalInvoiceRequest from the database
        let internal_invoice_request = match db.get_internal_invoice_request(&invoice.invoice_id) {
            Ok(request) => request,
            Err(_) => {
                // If no corresponding InternalInvoiceRequest is found, the invoice is invalid
                return Ok(false);
            }
        };

        eprintln!("internal_invoice_request: {:?}", internal_invoice_request);

        // Additional logic could be added here to check any rules for auto-payment.
        // For example, checking if the invoice matches certain criteria or thresholds
        // for automatic approval/payment.

        // If we found the corresponding InternalInvoiceRequest, the invoice is valid
        Ok(true)
    }

    // Fn: Pay an invoice and wait for the blockchain update of it
    pub async fn pay_invoice(&self, invoice: &Invoice) -> Result<Payment, AgentOfferingManagerError> {
        // Mocking the payment process
        println!("Initiating payment for invoice ID: {}", invoice.invoice_id);

        let wallet_manager = self.wallet_manager.upgrade().ok_or_else(|| {
            AgentOfferingManagerError::OperationFailed("Failed to upgrade wallet_manager reference".to_string())
        })?;
        let wallet_manager_lock = wallet_manager.lock().await;
        let wallet = wallet_manager_lock.as_ref().ok_or_else(|| {
            AgentOfferingManagerError::OperationFailed("Failed to get wallet manager lock".to_string())
        })?;

        // Get the price for the usage type
        let usage_type_inquiry = UsageTypeInquiry::PerUse; // or UsageTypeInquiry::Downloadable based on your context
        let price = invoice
            .shinkai_offering
            .get_price_for_usage(&usage_type_inquiry)
            .ok_or_else(|| {
                AgentOfferingManagerError::OperationFailed("Failed to get price for usage type".to_string())
            })?;

        // Assuming the price is of type ToolPrice::Payment
        let asset_payment = match price {
            ToolPrice::Payment(payments) => payments.first().ok_or_else(|| {
                AgentOfferingManagerError::OperationFailed("No payments found in ToolPrice".to_string())
            })?,
            _ => {
                return Err(AgentOfferingManagerError::OperationFailed(
                    "Unsupported ToolPrice type".to_string(),
                ))
            }
        };

        let my_address = wallet.payment_wallet.get_address();

        // Check the balance before attempting to pay
        let balance = match wallet
            .check_balance_payment_wallet(my_address.clone().into(), asset_payment.asset.clone())
            .await
        {
            Ok(balance) => balance,
            Err(e) => {
                eprintln!("Error checking balance: {:?}", e);
                return Err(AgentOfferingManagerError::OperationFailed(format!(
                    "Error checking balance: {:?}",
                    e
                )));
            }
        };
        println!("wallet {} balance: {:?}", my_address.address_id.clone(), balance);

        let required_amount = asset_payment.amount.parse::<u128>().map_err(|e| {
            AgentOfferingManagerError::OperationFailed(format!("Failed to parse required amount: {}", e))
        })?;
        println!("required_amount: {:?}", required_amount);

        let available_amount = balance.amount.parse::<u128>().map_err(|e| {
            AgentOfferingManagerError::OperationFailed(format!("Failed to parse available amount: {}", e))
        })?;

        if available_amount < required_amount {
            return Err(AgentOfferingManagerError::OperationFailed(
                "Insufficient balance to pay the invoice".to_string(),
            ));
        }

        let payment = match wallet.pay_invoice(invoice.clone()).await {
            Ok(payment) => {
                println!("Payment successful: {:?}", payment);
                payment
            }
            Err(e) => {
                eprintln!("Error paying invoice: {:?}", e);
                return Err(AgentOfferingManagerError::OperationFailed(format!(
                    "Error paying invoice: {:?}",
                    e
                )));
            }
        };

        println!("Payment: {:?}", payment);

        Ok(payment)
    }

    // Note: For Testing
    // Fn: Automatically verify and pay an invoice, then send receipt and data to the provider
    pub async fn auto_pay_invoice(&self, invoice: Invoice) -> Result<(), AgentOfferingManagerError> {
        // Step 1: Verify the invoice
        let is_valid = self.verify_invoice(&invoice).await?;
        if !is_valid {
            return Err(AgentOfferingManagerError::OperationFailed(
                "Invoice verification failed".to_string(),
            ));
        }

        // Step 2: Pay the invoice
        let payment = self.pay_invoice(&invoice).await?;

        // Create a new updated invoice with the payment information
        let mut updated_invoice = invoice.clone();
        updated_invoice.payment = Some(payment);
        updated_invoice.update_status(InvoiceStatusEnum::Paid);

        // Store the paid invoice in the database
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;
        db.set_invoice(&updated_invoice).map_err(|e| {
            AgentOfferingManagerError::OperationFailed(format!("Failed to store paid invoice: {:?}", e))
        })?;

        // Step 3: Send receipt and data to provider
        self.send_receipt_and_data_to_provider(&updated_invoice).await?;

        Ok(())
    }

    // Fn: Send the receipt and the data for the job to the provider
    pub async fn send_receipt_and_data_to_provider(&self, invoice: &Invoice) -> Result<(), AgentOfferingManagerError> {
        println!(
            "Sending receipt for invoice ID: {} to provider: {}",
            invoice.invoice_id, invoice.provider_name
        );

        if let Some(identity_manager_arc) = self.identity_manager.upgrade() {
            let identity_manager = identity_manager_arc.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&invoice.provider_name.to_string())
                .await
                .map_err(|e| AgentOfferingManagerError::OperationFailed(e))?;
            drop(identity_manager);
            let receiver_public_key = standard_identity.node_encryption_public_key;
            let proxy_builder_info =
                get_proxy_builder_info_static(identity_manager_arc, self.proxy_connection_info.clone()).await;

            // Generate the message to send the receipt and data
            let message = ShinkaiMessageBuilder::create_generic_invoice_message(
                invoice.clone(),
                MessageSchemaType::PaidInvoice,
                clone_static_secret_key(&self.my_encryption_secret_key),
                clone_signature_secret_key(&self.my_signature_secret_key),
                receiver_public_key,
                self.node_name.to_string(),
                "".to_string(),
                invoice.provider_name.to_string(),
                "main".to_string(),
                proxy_builder_info,
            )
            .map_err(|e| AgentOfferingManagerError::OperationFailed(e.to_string()))?;

            send_message_to_peer(
                message,
                self.db.clone(),
                standard_identity,
                self.my_encryption_secret_key.clone(),
                self.identity_manager.clone(),
                self.proxy_connection_info.clone(),
            )
            .await?;
        }

        println!(
            "Receipt and data successfully sent for invoice ID: {}",
            invoice.invoice_id
        );

        Ok(())
    }

    pub async fn add_network_tool(&self, network_tool: NetworkTool) -> Result<(), AgentOfferingManagerError> {
        let tool_router = self.tool_router.upgrade().ok_or_else(|| {
            AgentOfferingManagerError::OperationFailed("Failed to upgrade tool_router reference".to_string())
        })?;

        let result = tool_router
            .lock()
            .await
            .add_network_tool(network_tool)
            .await
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to add network tool: {:?}", e)));

        result
    }

    // New function to create and add a NetworkTool
    pub async fn create_and_add_network_tool(
        &self,
        tool_header: ShinkaiToolHeader,
        provider: ShinkaiName,
    ) -> Result<(), AgentOfferingManagerError> {
        let tool_router = self.tool_router.upgrade().ok_or_else(|| {
            AgentOfferingManagerError::OperationFailed("Failed to upgrade tool_router reference".to_string())
        })?;

        // TODO: avoid the expects
        let network_tool = NetworkTool::new(
            tool_header.name,
            tool_header.toolkit_name,
            tool_header.description,
            tool_header.version,
            provider,
            tool_header.usage_type.expect("Usage type is required"),
            true, // Assuming the tool is activated by default
            tool_header.config.expect("Config is required"),
            vec![], // TODO: Fix input_args
            None,
            None,
        );

        let tool_router_lock = tool_router.lock().await;
        tool_router_lock
            .add_network_tool(network_tool)
            .await
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to add network tool: {:?}", e)))
    }

    // Fn: Receive the response from the provider and process it -> update the job

    // Note: Should be create a new type of ShinkaiTool "NetworkTool" that can be called by an LLM?
    // We could extend the schema with some rules for the payment and the usage of the tool depending on the network.
    // When we try to execute it it would perform the entire flow (and even wait for user confirmation if required).

    // For now we could do an extra search for available tools on the network and show the user
    // if any of the options is interesting for them and the price.

    // Note: do we need a job to check the status (offer) of tools on the network?
    // For now only official tools are allowed to be used.
    // We could hardcode the official tools and their prices for a beta version -> indexer (whitelisted) -> very open.

    // Where do we store these available tools? rocksdb? lancedb? memory?
    // Should we add all of the tools to lancedb with their price and then filter them based on the current network?
    // We could even do two searches: one of the locals and one for the locals + network tools (so we know which ones are the best to use).
    // ONLY: if the user has a wallet set up.

    // Thoughts
    // Should we add a way to scan previous invoices sent to the chain? if we reset the node but we wouldn't be able to know if they were already claimed.

    async fn get_proxy_builder_info(
        &self,
        identity_manager_lock: Arc<Mutex<dyn IdentityManagerTrait + Send>>,
    ) -> Option<ShinkaiProxyBuilderInfo> {
        get_proxy_builder_info_static(identity_manager_lock, self.proxy_connection_info.clone()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::ShinkaiDB,
        lance_db::{shinkai_lance_db::LanceShinkaiDb, shinkai_lancedb_error::ShinkaiLanceDBError},
        managers::identity_manager::IdentityManagerTrait,
        network::agent_payments_manager::{
            invoices::InvoiceStatusEnum,
            shinkai_tool_offering::{ShinkaiToolOffering, ToolPrice, UsageType},
        },
        schemas::identity::{Identity, StandardIdentity, StandardIdentityType},
        tools::tool_router::ToolRouter,
        vector_fs::vector_fs::VectorFS,
        wallet::mixed::{Address, NetworkIdentifier, PublicAddress},
    };
    use async_trait::async_trait;
    use chrono::Utc;
    use shinkai_message_primitives::{
        shinkai_message::shinkai_message_schemas::IdentityPermissions,
        shinkai_utils::{
            encryption::unsafe_deterministic_encryption_keypair, signatures::unsafe_deterministic_signature_keypair,
        },
    };
    use shinkai_vector_resources::{
        embedding_generator::{EmbeddingGenerator, RemoteEmbeddingGenerator},
        model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference},
    };
    use std::{fs, path::Path, sync::Arc};
    use tokio::sync::Mutex;

    #[derive(Clone, Debug)]
    struct MockIdentityManager {
        dummy_standard_identity: Identity,
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
            full_profile_name: &str,
        ) -> Result<StandardIdentity, String> {
            unimplemented!()
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

    #[tokio::test]
    async fn test_verify_invoice() -> Result<(), ShinkaiLanceDBError> {
        setup();

        // Setup the necessary components for MyAgentOfferingsManager
        let db = Arc::new(ShinkaiDB::new("shinkai_db_tests/shinkaidb").unwrap());
        let vector_fs = Arc::new(setup_default_vector_fs().await);
        let identity_manager: Arc<Mutex<dyn IdentityManagerTrait + Send>> =
            Arc::new(Mutex::new(MockIdentityManager::new()));
        let generator = RemoteEmbeddingGenerator::new_default();
        let embedding_model = generator.model_type().clone();
        let lance_db = Arc::new(Mutex::new(
            LanceShinkaiDb::new("lance_db_tests/lancedb", embedding_model.clone(), generator.clone()).await?,
        ));

        let tool_router = Arc::new(Mutex::new(ToolRouter::new(lance_db.clone())));
        let node_name = ShinkaiName::new("@@localhost.arb-sep-shinkai/main".to_string()).unwrap();

        let (my_signature_secret_key, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_secret_key, _) = unsafe_deterministic_encryption_keypair(0);

        // Remove?
        // Create a real CryptoInvoiceManager with a provider using Base Sepolia
        // let provider_url = "https://sepolia.base.org";
        // let crypto_invoice_manager = Arc::new(CryptoInvoiceManager::new(provider_url).unwrap());

        let wallet_manager: Arc<Mutex<Option<WalletManager>>> = Arc::new(Mutex::new(None));

        let manager = MyAgentOfferingsManager::new(
            Arc::downgrade(&db),
            Arc::downgrade(&vector_fs),
            Arc::downgrade(&identity_manager),
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
            Arc::downgrade(&Arc::new(Mutex::new(None))),
            Arc::downgrade(&tool_router),
            Arc::downgrade(&wallet_manager),
        )
        .await;

        // Create a mock network tool
        let network_tool = NetworkTool::new(
            "Test Tool".to_string(),
            "shinkai_toolkit".to_string(),
            "A tool for testing".to_string(),
            "1.0".to_string(),
            ShinkaiName::new("@@test_provider.shinkai".to_string()).unwrap(),
            UsageType::PerUse(ToolPrice::DirectDelegation("0.01".to_string())),
            true,
            vec![],
            vec![],
            None,
            None,
        );

        // Create a usage type inquiry
        let usage_type_inquiry = UsageTypeInquiry::PerUse;

        // Call request_invoice to generate an invoice request
        let internal_invoice_request = manager.request_invoice(network_tool, usage_type_inquiry).await.unwrap();

        // Simulate receiving an invoice from the server
        let invoice = Invoice {
            invoice_id: internal_invoice_request.unique_id.clone(),
            requester_name: internal_invoice_request.provider_name.clone(),
            provider_name: internal_invoice_request.provider_name.clone(),
            usage_type_inquiry: UsageTypeInquiry::PerUse,
            shinkai_offering: ShinkaiToolOffering {
                tool_key: internal_invoice_request.tool_key_name.clone(),
                usage_type: UsageType::PerUse(ToolPrice::DirectDelegation("0.01".to_string())),
                meta_description: Some("A tool for testing".to_string()),
            },
            expiration_time: Utc::now() + chrono::Duration::hours(1), // Example expiration time
            status: InvoiceStatusEnum::Pending,
            payment: None,
            address: PublicAddress {
                network_id: NetworkIdentifier::BaseSepolia,
                address_id: "0x1234567890123456789012345678901234567890".to_string(),
            },
            request_date_time: Utc::now(),
            invoice_date_time: Utc::now(),
        };

        // Call verify_invoice
        let result = manager.verify_invoice(&invoice).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        Ok(())
    }
}
