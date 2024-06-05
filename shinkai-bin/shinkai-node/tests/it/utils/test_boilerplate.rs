use super::db_handlers::setup;
use async_channel::{bounded, Receiver, Sender};
use shinkai_node::db::ShinkaiDB;
use shinkai_node::network::subscription_manager::external_subscriber_manager::ExternalSubscriberManager;
use shinkai_node::network::subscription_manager::my_subscription_manager::MySubscriptionsManager;
use shinkai_node::vector_fs::vector_fs::VectorFS;
use tokio::sync::Mutex;

use core::panic;
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::Future;
use std::env;
use std::sync::Arc;

use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, hash_signature_public_key, unsafe_deterministic_signature_keypair,
};
use shinkai_node::network::node::NodeCommand;
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
    pub node1_agent: String,
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
    pub node1_db: Arc<ShinkaiDB>,
    pub node1_ext_subscription_manager: Arc<Mutex<ExternalSubscriberManager>>,
    pub node1_my_subscriptions_manager: Arc<Mutex<MySubscriptionsManager>>,
    pub node1_abort_handler: AbortHandle,
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
        let node1_agent = "node1_gpt_agent";

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

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk.clone(),
            0,
            node1_commands_receiver.clone(),
            node1_db_path,
            "".to_string(),
            proxy_identity,
            false,
            vec![],
            None,
            node1_fs_db_path,
            None,
            None,
        )
        .await;

        let node1_locked = node1.lock().await;
        let node1_vecfs = node1_locked.vector_fs.clone();
        let node1_db = node1_locked.db.clone();
        let node1_ext_subscription_manager = node1_locked.ext_subscription_manager.clone();
        let node1_my_subscriptions_manager = node1_locked.my_subscription_manager.clone();
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
            node1_agent: node1_agent.to_string(),
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
            node1_ext_subscription_manager,
            node1_my_subscriptions_manager,
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
