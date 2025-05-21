use std::sync::{Arc, Weak};

use ed25519_dalek::SigningKey;
use serde_json::Value;

use shinkai_message_primitives::{
    schemas::{
        invoices::{InternalInvoiceRequest, Invoice, InvoiceStatusEnum, Payment}, shinkai_name::ShinkaiName, shinkai_proxy_builder_info::ShinkaiProxyBuilderInfo, shinkai_tool_offering::{ToolPrice, UsageTypeInquiry}, wallet_mixed::AddressBalanceList
    }, shinkai_message::shinkai_message_schemas::MessageSchemaType, shinkai_utils::{
        encryption::clone_static_secret_key, shinkai_message_builder::ShinkaiMessageBuilder, signatures::clone_signature_secret_key
    }
};
use shinkai_sqlite::SqliteManager;
use shinkai_tools_primitives::tools::{
    network_tool::NetworkTool, parameters::Parameters, shinkai_tool::ShinkaiToolHeader, tool_output_arg::ToolOutputArg
};
use tokio::sync::{Mutex, RwLock};
use x25519_dalek::StaticSecret as EncryptionStaticKey;

use crate::{
    managers::{identity_manager::IdentityManagerTrait, tool_router::ToolRouter}, network::{
        network_manager_utils::{get_proxy_builder_info_static, send_message_to_peer}, node::ProxyConnectionInfo
    }, wallet::wallet_manager::WalletManager
};

use super::external_agent_offerings_manager::AgentOfferingManagerError;

pub struct MyAgentOfferingsManager {
    pub db: Weak<SqliteManager>,
    pub identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
    // The address of the proxy server (if any)
    pub proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
    // Tool router
    pub tool_router: Weak<ToolRouter>,
    // Wallet manager
    pub wallet_manager: Weak<Mutex<Option<WalletManager>>>,
    // pub crypto_invoice_manager: Arc<Option<Box<dyn CryptoInvoiceManagerTrait + Send + Sync>>>,
}

impl MyAgentOfferingsManager {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        db: Weak<SqliteManager>,
        identity_manager: Weak<Mutex<dyn IdentityManagerTrait + Send>>,
        node_name: ShinkaiName,
        my_signature_secret_key: SigningKey,
        my_encryption_secret_key: EncryptionStaticKey,
        proxy_connection_info: Weak<Mutex<Option<ProxyConnectionInfo>>>,
        tool_router: Weak<ToolRouter>,
        wallet_manager: Weak<Mutex<Option<WalletManager>>>,
    ) -> Self {
        Self {
            db,
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
            proxy_connection_info,
            identity_manager,
            tool_router,
            wallet_manager,
        }
    }

    // Notes:
    // Fn: Ask for the current offerings from the provider (directly)
    // Old fashion send message and wait network response

    // Fn: Ask for the current offerings of a provider through the indexer (indirectly)

    // Fn: (Temp) Hardcoded list of offerings of a provider (PoC)

    // Fn: Request an invoice <- can we triggered by the user through API
    // Note: currently works only for added network tools

    /// Request an invoice for a network tool
    ///
    /// # Arguments
    ///
    /// * `network_tool` - The network tool for which the invoice is requested.
    /// * `usage_type_inquiry` - The type of usage inquiry for the tool.
    ///
    /// # Returns
    ///
    /// * `Result<InternalInvoiceRequest, AgentOfferingManagerError>` - The internal invoice request or an error.
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

        Ok(internal_invoice_request)
    }

    /// Request an invoice from the network
    ///
    /// # Arguments
    ///
    /// * `network_tool` - The network tool for which the invoice is requested.
    ///
    /// # Returns
    ///
    /// * `Result<String, AgentOfferingManagerError>` - The payment response (e.g., a token) or an error.
    pub async fn network_request_invoice(
        &self,
        network_tool: NetworkTool,
    ) -> Result<String, AgentOfferingManagerError> {
        // TODO: Implement x402 payment flow
        // 1. Check if network_tool.payment_url is Some. If not, return error or handle as appropriate.
        let payment_url = network_tool.payment_url.ok_or_else(|| {
            AgentOfferingManagerError::OperationFailed("Payment URL not provided for the tool".to_string())
        })?;

        // 2. Make an HTTP GET request to payment_url.
        //    - This would involve an HTTP client like `reqwest`.
        //    - For now, we'll assume a placeholder for the response.
        //    Example:
        //    let client = reqwest::Client::new();
        //    let response = client.get(&payment_url).send().await.map_err(|e| AgentOfferingManagerError::NetworkError(e.to_string()))?;

        // 3. If the response status is 402 Payment Required:
        //    - Parse payment requirements from the response headers (e.g., `WWW-Authenticate` for L402).
        //    - This involves parsing the header string to extract macaroon, invoice, etc.
        //    Placeholder: let payment_requirements = parse_402_response_headers(response.headers());

        // 4. Check if network_tool.facilitator_url is Some. If not, return error or handle.
        let _facilitator_url = network_tool.facilitator_url.ok_or_else(|| {
            AgentOfferingManagerError::OperationFailed("Facilitator URL not provided for the tool".to_string())
        })?;

        // 5. Use the facilitator_url and parsed payment requirements to complete the payment.
        //    - This would involve using an x402 library (e.g., a Rust crate for L402 or other x402 protocols).
        //    - The library would handle interactions with the facilitator.
        //    Placeholder:
        //    let payment_result = x402_library::pay(facilitator_url, payment_requirements).await;
        //    match payment_result {
        //        Ok(token) => Ok(token), // This token is then used for the actual tool request
        //        Err(e) => Err(AgentOfferingManagerError::PaymentError(e.to_string())),
        //    }

        // For now, returning a placeholder.
        // This function will need to be filled with actual HTTP client and x402 library logic.
        eprintln!(
            "Placeholder: Initiating x402 payment for tool: {} using payment_url: {}",
            network_tool.name, payment_url
        );
        // Simulate a successful payment token for now
        Ok("dummy_payment_token_or_confirmation".to_string())
    }

    /// Store the quote invoice (this invoice doesn't contain the result -- it's just the quote)
    ///
    /// # Arguments
    ///
    /// * `invoice` - The invoice to be stored.
    ///
    /// # Returns
    ///
    /// * `Result<(), AgentOfferingManagerError>` - Ok if successful, otherwise an error.
    pub async fn store_invoice(&self, invoice: &Invoice) -> Result<(), AgentOfferingManagerError> {
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;
        let db_write = db;

        db_write
            .set_invoice(invoice)
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to store invoice: {:?}", e)))
    }

    /// Store the invoice result (from the external agent's work)
    ///
    /// # Arguments
    ///
    /// * `invoice` - The invoice result to be stored.
    ///
    /// # Returns
    ///
    /// * `Result<(), AgentOfferingManagerError>` - Ok if successful, otherwise an error.
    pub async fn store_invoice_result(&self, invoice: &Invoice) -> Result<(), AgentOfferingManagerError> {
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;
        let db_write = db;

        db_write
            .set_invoice(invoice)
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to store invoice: {:?}", e)))
    }

    /// Pay an invoice and send receipt and data to provider
    ///
    /// # Arguments
    ///
    /// * `invoice_id` - The ID of the invoice to be paid.
    /// * `tool_data` - The data related to the tool.
    ///
    /// # Returns
    ///
    /// * `Result<Invoice, AgentOfferingManagerError>` - The updated invoice or an error.
    pub async fn pay_invoice_and_send_receipt(
        &self,
        invoice_id: String,
        tool_data: Value,
        node_name: ShinkaiName,
    ) -> Result<Invoice, AgentOfferingManagerError> {
        // TODO: check that the invoice is valid (exists) and still valid (not expired)
        // TODO: This function will need significant rework with x402.
        // The concept of "paying an invoice" in the old sense and then "sending a receipt"
        // changes with x402, where the payment is a prerequisite to getting the service token.

        // Step 0: Get the invoice from the database (this might change, as x402 might not create an "invoice" in this way)
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;

        let invoice = db
            .get_invoice(&invoice_id)
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to get invoice: {:?}", e)))?;

        // Step 1: Verify the invoice (this concept might change or be removed)
        // The `verify_invoice` function was removed. If some verification is needed,
        // it needs to be re-thought in the context of x402.
        // For now, let's assume if we have an "invoice", it's been through some initial request.
        // However, its state (Paid, Pending) is critical.

        // Step 2: "Pay the invoice" - this is the part that drastically changes.
        // We would have already obtained a payment token/confirmation from `network_request_invoice` (x402 flow).
        // So, this function might now be more about associating that payment with the tool usage
        // and sending a *confirmation* or the *actual paid request* to the provider.

        // For now, let's assume `network_request_invoice` was called and was successful.
        // The `payment` object here would be different; it would be the result of the x402 flow.
        // This part needs a placeholder for the new x402 logic.
        // let payment_confirmation = self.network_request_invoice(tool_associated_with_invoice).await?;
        // This is problematic because we don't have the network_tool directly here.
        // This function might need to be called *after* network_request_invoice,
        // or network_request_invoice needs to handle the "receipt" part.

        // Create a new updated invoice with the payment information
        let mut updated_invoice = invoice.clone();
        // updated_invoice.payment = Some(payment); // This `payment` is from the old flow.
        updated_invoice.update_status(InvoiceStatusEnum::Paid); // Or some other status relevant to x402
        updated_invoice.tool_data = Some(tool_data);

        // Store the paid invoice in the database
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;
        db.set_invoice(&updated_invoice).map_err(|e| {
            AgentOfferingManagerError::OperationFailed(format!("Failed to store paid invoice: {:?}", e))
        })?;

        // Step 3: Send receipt and data to provider
        // This step also changes. With x402, the "receipt" might be the token itself,
        // and this token is used in the actual request to the provider's tool endpoint.
        // So, `send_receipt_and_data_to_provider` might be more like "execute tool request with payment token".
        self.send_receipt_and_data_to_provider(&updated_invoice).await?;

        Ok(updated_invoice)
    }

    // Note: Only For Testing (!!!)
    // TODO: it could be re-purposed for auto-payment if we have a preset of rules and whitelisted tools
    // We want to have a way to confirm payment from the user perspective
    // Fn: Automatically verify and pay an invoice, then send receipt and data to the provider
    pub async fn auto_pay_invoice(
        &self,
        invoice: Invoice,
        node_name: ShinkaiName,
    ) -> Result<(), AgentOfferingManagerError> {
        // TODO: This function is heavily reliant on the old invoice and payment model.
        // It will need to be completely re-thought for x402 or removed.
        // The `verify_invoice` and `pay_invoice` calls are based on the old system.

        // Step 1: Verify the invoice (removed, needs x402 context)
        // let is_valid = self.verify_invoice(&invoice).await?;
        // if !is_valid {
        //     return Err(AgentOfferingManagerError::OperationFailed(
        //         "Invoice verification failed".to_string(),
        //     ));
        // }
        println!("Warning: auto_pay_invoice is using outdated logic and needs to be updated for x402.");


        // Step 2: Pay the invoice (removed, needs x402 context)
        // let payment = self.pay_invoice(&invoice, node_name.clone()).await?;

        // Create a new updated invoice with the payment information
        let mut updated_invoice = invoice.clone();
        // updated_invoice.payment = Some(payment); // Old payment model
        updated_invoice.update_status(InvoiceStatusEnum::Paid); // Or new x402 status

        // Store the paid invoice in the database
        let db = self
            .db
            .upgrade()
            .ok_or_else(|| AgentOfferingManagerError::OperationFailed("Failed to upgrade db reference".to_string()))?;
        db.set_invoice(&updated_invoice).map_err(|e| {
            AgentOfferingManagerError::OperationFailed(format!("Failed to store paid invoice: {:?}", e))
        })?;

        // Step 3: Send receipt and data to provider (needs x402 context)
        self.send_receipt_and_data_to_provider(&updated_invoice).await?;

        Ok(())
    }

    /// Send the receipt and the data for the job to the provider
    ///
    /// # Arguments
    ///
    /// * `invoice` - The invoice for which the receipt and data are to be sent.
    ///
    /// # Returns
    ///
    /// * `Result<(), AgentOfferingManagerError>` - Ok if successful, otherwise an error.
    pub async fn send_receipt_and_data_to_provider(&self, invoice: &Invoice) -> Result<(), AgentOfferingManagerError> {
        println!(
            "Sending receipt for invoice ID: {} to provider: {}",
            invoice.invoice_id, invoice.provider_name
        );

        if let Some(identity_manager_arc) = self.identity_manager.upgrade() {
            let identity_manager = identity_manager_arc.lock().await;
            let standard_identity = identity_manager
                .external_profile_to_global_identity(&invoice.provider_name.to_string(), None)
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
    /// Add a network tool
    ///
    /// # Arguments
    ///
    /// * `network_tool` - The network tool to be added.
    ///
    /// # Returns
    ///
    /// * `Result<(), AgentOfferingManagerError>` - Ok if successful, otherwise an error.
    pub async fn add_network_tool(&self, network_tool: NetworkTool) -> Result<(), AgentOfferingManagerError> {
        let tool_router = self.tool_router.upgrade().ok_or_else(|| {
            AgentOfferingManagerError::OperationFailed("Failed to upgrade tool_router reference".to_string())
        })?;

        let result = tool_router
            .add_network_tool(network_tool)
            .await
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to add network tool: {:?}", e)));

        result
    }

    /// Create and add a NetworkTool
    ///
    /// # Arguments
    ///
    /// * `tool_header` - The header information for the tool.
    /// * `provider` - The provider of the tool.
    ///
    /// # Returns
    ///
    /// * `Result<(), AgentOfferingManagerError>` - Ok if successful, otherwise an error.
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
            tool_header.description,
            tool_header.version,
            provider.node_name.clone(),
            provider,
            true, // Assuming the tool is activated by default
            tool_header.config.expect("Config is required"),
            Parameters::new(), // TODO: Fix input_args
            ToolOutputArg { json: "".to_string() },
            None,
            None,
            None, // payment_url
            None, // facilitator_url
        );

        tool_router
            .add_network_tool(network_tool)
            .await
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to add network tool: {:?}", e)))
    }

    /// Get balances from the wallet manager
    ///
    /// # Returns
    ///
    /// * `Result<AddressBalanceList, AgentOfferingManagerError>` - The list of address balances or an error.
    pub async fn get_balances(&self, node_name: ShinkaiName) -> Result<AddressBalanceList, AgentOfferingManagerError> {
        let wallet_manager = self.wallet_manager.upgrade().ok_or_else(|| {
            AgentOfferingManagerError::OperationFailed("Failed to upgrade wallet_manager reference".to_string())
        })?;

        let wallet_manager_lock = wallet_manager.lock().await;

        // Check that wallet_manager is not None
        if wallet_manager_lock.is_none() {
            return Err(AgentOfferingManagerError::OperationFailed(
                "Wallet manager is None".to_string(),
            ));
        }

        let wallet = wallet_manager_lock.as_ref().ok_or_else(|| {
            AgentOfferingManagerError::OperationFailed("Failed to get wallet manager lock".to_string())
        })?;

        wallet
            .payment_wallet
            .check_balances(node_name.clone())
            .await
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to get balances: {:?}", e)))
    }

    /// Get proxy builder info
    ///
    /// # Arguments
    ///
    /// * `identity_manager_lock` - The identity manager lock.
    ///
    /// # Returns
    ///
    /// * `Option<ShinkaiProxyBuilderInfo>` - The proxy builder info or None.
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
    use crate::managers::identity_manager::IdentityManagerTrait;
    use async_trait::async_trait;

    use shinkai_message_primitives::{
        schemas::identity::{Identity, StandardIdentity, StandardIdentityType}, shinkai_message::shinkai_message_schemas::IdentityPermissions, shinkai_utils::{
            encryption::unsafe_deterministic_encryption_keypair, signatures::unsafe_deterministic_signature_keypair
        }
    };

    use std::{fs, path::Path};

    #[derive(Clone, Debug)]
    struct MockIdentityManager {
        dummy_standard_identity: Identity,
    }

    impl MockIdentityManager {
        pub fn new() -> Self {
            let (_, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
            let (_, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

            let dummy_standard_identity = Identity::Standard(StandardIdentity {
                full_identity_name: ShinkaiName::new("@@localhost.sep-shinkai/main".to_string()).unwrap(),
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
            if _full_profile_name.to_string() == "@@localhost.sep-shinkai/main" {
                Some(&self.dummy_standard_identity)
            } else {
                None
            }
        }

        async fn search_identity(&self, full_identity_name: &str) -> Option<Identity> {
            if full_identity_name == "@@localhost.sep-shinkai/main" {
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
            _: Option<bool>,
        ) -> Result<StandardIdentity, String> {
            if full_profile_name == "@@localhost.sep-shinkai" {
                if let Identity::Standard(identity) = &self.dummy_standard_identity {
                    Ok(identity.clone())
                } else {
                    Err("Identity is not of type Standard".to_string())
                }
            } else {
                Err("Profile not found".to_string())
            }
        }
    }

    fn setup() {
        let path = Path::new("lance_db_tests/");
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

    // async fn setup_default_vector_fs() -> VectorFS {
    //     let generator = RemoteEmbeddingGenerator::new_default();
    //     let fs_db_path = format!("db_tests/{}", "vector_fs");
    //     let profile_list = vec![default_test_profile()];
    //     let supported_embedding_models = vec![EmbeddingModelType::OllamaTextEmbeddingsInference(
    //         OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M,
    //     )];

    //     VectorFS::new(
    //         generator,
    //         supported_embedding_models,
    //         profile_list,
    //         &fs_db_path,
    //         node_name(),
    //     )
    //     .await
    //     .unwrap()
    // }

    #[tokio::test]
    async fn test_parse_available_amount() {
        struct MockBalance {
            amount: String,
        }

        let balance = MockBalance {
            amount: "4999000.000".to_string(),
        };

        let available_amount = balance
            .amount
            .split('.')
            .next()
            .unwrap()
            .parse::<u128>()
            .map_err(|e| AgentOfferingManagerError::OperationFailed(format!("Failed to parse available amount: {}", e)))
            .unwrap();

        assert_eq!(available_amount, 4999000);
    }
}
