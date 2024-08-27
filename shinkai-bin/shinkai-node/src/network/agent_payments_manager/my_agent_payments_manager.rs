
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

pub struct MyAgentOfferingsManager {
    pub db: Weak<ShinkaiDB>,
    pub vector_fs: Weak<VectorFS>,
    pub identity_manager: Weak<Mutex<IdentityManager>>,
    pub node_name: ShinkaiName,
    // The secret key used for signing operations.
    pub my_signature_secret_key: SigningKey,
    // The secret key used for encryption and decryption.
    pub my_encryption_secret_key: EncryptionStaticKey,
    pub tool_router: Weak<Mutex<ToolRouter>>,
}

impl MyAgentOfferingsManager {
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
        tool_router: Weak<Mutex<ToolRouter>>,
    ) -> Self {
        Self {
            db,
            node_name,
            my_signature_secret_key,
            my_encryption_secret_key,
            identity_manager,
            offerings_queue_manager,
            offering_processing_task: Some(offering_queue_handler),
            tool_router,
        }
    }

    // Fn: Ask for the current offerings from the provider (directly)
    // Old fashion send message and wait network response

    // Fn: Ask for the current offerings of a provider through the indexer (indirectly)

    // Fn: (Temp) Hardcoded list of offerings of a provider (PoC)

    // Fn: Request an invoice <- can we triggered by the user through API

    // Fn: Verify an invoice
    // - Check if the invoice is valid (even if we asked for it)
    // - Check about any rules for auto-payment

    // Fn: Pay an invoice and wait for the blockchain update of it

    // Fn: Send the receipt and the data for the job to the provider

    // Fn: Receive the response from the provider and process it -> update the job

    // Note: Should be create a new type of ShinkaiTool "NetworkTool" that can be called by an LLM?
    // We could extend the schema with some rules for the payment and the usage of the tool depending on the network.

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
    // Should we add a way to scan previous invoices sent to the chain? if we reset the node but we wouldnt be able to know if they were already claimed.
}