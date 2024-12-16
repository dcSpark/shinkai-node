use super::db_handlers::setup;
use async_channel::{bounded, Receiver, Sender};

use shinkai_node::llm_provider::job_callback_manager::JobCallbackManager;
use shinkai_node::managers::sheet_manager::SheetManager;
use shinkai_node::managers::tool_router::ToolRouter;
use shinkai_sqlite::SqliteManager;
use shinkai_vector_fs::vector_fs::vector_fs::VectorFS;
use shinkai_vector_resources::embedding_generator::RemoteEmbeddingGenerator;
{EmbeddingModelType, OllamaTextEmbeddingsInference};
use tokio::sync::{Mutex, RwLock};

use core::panic;
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::Future;
use std::env;
use std::sync::Arc;

use shinkai_http_api::node_commands::NodeCommand;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, hash_signature_public_key, unsafe_deterministic_signature_keypair,
};
use shinkai_node::network::Node;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr};
use std::pin::Pin;
use tokio::runtime::Runtime;
use tokio::task::AbortHandle;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub struct TestEnvironment {
    pub node1_identity_name: String,
    pub node1_profile_name: String,
    pub node1_device_name: String,
    pub node1_llm_provider: String,
    pub node1_commands_sender: Sender<NodeCommand>,
    pub node1_commands_receiver: Receiver<NodeCommand>,
    pub node1_identity_sk: SigningKey,
    pub node1_identity_pk: VerifyingKey,
    pub node1_encryption_sk: EncryptionStaticKey,
    pub node1_encryption_pk: EncryptionPublicKey,
    pub node1_profile_identity_sk: SigningKey,
    pub node1_profile_identity_pk: VerifyingKey,
    pub node1_profile_encryption_sk: EncryptionStaticKey,
    pub node1_profile_encryption_pk: EncryptionPublicKey,
    pub node1_device_identity_sk: SigningKey,
    pub node1_device_identity_pk: VerifyingKey,
    pub node1_device_encryption_sk: EncryptionStaticKey,
    pub node1_device_encryption_pk: EncryptionPublicKey,
    pub node1_vecfs: Arc<VectorFS>,
    pub node1_db: Arc<RwLock<SqliteManager>>,
    pub node1_sheet_manager: Arc<Mutex<SheetManager>>,
    pub node1_callback_manager: Arc<Mutex<JobCallbackManager>>,
    pub node1_tool_router: Option<Arc<ToolRouter>>,
    pub node1_abort_handler: AbortHandle,
}

pub fn default_embedding_model() -> EmbeddingModelType {
    env::var("DEFAULT_EMBEDDING_MODEL")
        .map(|s| EmbeddingModelType::from_string(&s).expect("Failed to parse DEFAULT_EMBEDDING_MODEL"))
        .unwrap_or_else(|_| {
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M)
        })
}

pub fn supported_embedding_models() -> Vec<EmbeddingModelType> {
    env::var("SUPPORTED_EMBEDDING_MODELS")
        .map(|s| {
            s.split(',')
                .map(|s| EmbeddingModelType::from_string(s).expect("Failed to parse SUPPORTED_EMBEDDING_MODELS"))
                .collect()
        })
        .unwrap_or_else(|_| {
            vec![EmbeddingModelType::OllamaTextEmbeddingsInference(
                OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M,
            )]
        })
}

pub fn run_test_one_node_network<F>(interactions_handler_logic: F)
where
    F: FnOnce(TestEnvironment) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
{
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.arb-sep-shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";
        let node1_llm_provider = "node1_gpt_agent";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node1_device_identity_sk, node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let node1_db_path = format!("db_tests/{}", hash_signature_public_key(&node1_identity_pk));
        let node1_fs_db_path = format!("db_tests/vector_fs{}", hash_signature_public_key(&node1_identity_pk));

        // Fetch the PROXY_ADDRESS environment variable
        let proxy_identity: Option<String> = env::var("PROXY_IDENTITY").ok().and_then(|addr| addr.parse().ok());

        let api_v2_key = env::var("API_V2_KEY").unwrap_or_else(|_| "SUPER_SECRET".to_string());

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk.clone(),
            None,
            None,
            0,
            node1_commands_receiver.clone(),
            node1_db_path,
            "".to_string(),
            proxy_identity,
            false,
            vec![],
            node1_fs_db_path,
            Some(RemoteEmbeddingGenerator::new_default()),
            None,
            default_embedding_model(),
            supported_embedding_models(),
            Some(api_v2_key),
        )
        .await;

        let node1_locked = node1.lock().await;
        let node1_vecfs = node1_locked.vector_fs.clone();
        let node1_db = node1_locked.db.clone();
        let node1_sheet_manager = node1_locked.sheet_manager.clone();
        let node1_callback_manager = node1_locked.callback_manager.clone();
        let node1_tool_router = node1_locked.tool_router.clone();
        drop(node1_locked);

        eprintln!("Starting Node");
        let node1_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 1");
            let _ = node1.lock().await.start().await;
        });

        let node1_abort_handler = node1_handler.abort_handle();

        let env = TestEnvironment {
            node1_identity_name: node1_identity_name.to_string(),
            node1_profile_name: node1_profile_name.to_string(),
            node1_device_name: node1_device_name.to_string(),
            node1_llm_provider: node1_llm_provider.to_string(),
            node1_commands_sender,
            node1_commands_receiver,
            node1_identity_sk,
            node1_identity_pk,
            node1_encryption_sk,
            node1_encryption_pk,
            node1_profile_identity_sk,
            node1_profile_identity_pk,
            node1_profile_encryption_sk,
            node1_profile_encryption_pk,
            node1_device_identity_sk,
            node1_device_identity_pk,
            node1_device_encryption_sk,
            node1_device_encryption_pk,
            node1_vecfs,
            node1_db,
            node1_sheet_manager,
            node1_callback_manager,
            node1_tool_router,
            node1_abort_handler,
        };

        let interactions_handler = tokio::spawn(interactions_handler_logic(env));

        let result = tokio::try_join!(node1_handler, interactions_handler);
        match result {
            Ok(_) => {}
            Err(e) => {
                // Check if the error is because one of the tasks was aborted
                if e.is_cancelled() {
                    eprintln!("One of the tasks was aborted, but this is expected.");
                } else {
                    // If the error is not due to an abort, then it's unexpected
                    panic!("An unexpected error occurred: {:?}", e);
                }
            }
        }
    });
    rt.shutdown_background();
}
