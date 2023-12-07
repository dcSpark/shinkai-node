use super::db_handlers::setup;
use async_channel::{bounded, Receiver, Sender};
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::Future;
use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use std::net::{IpAddr, Ipv4Addr};
use std::pin::Pin;
use std::net::SocketAddr;
use tokio::runtime::Runtime;
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
}

pub fn run_test_one_node_network<F>(interactions_handler_logic: F)
where
    F: FnOnce(TestEnvironment) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
{
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.shinkai";
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

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let node1 = Node::new(
            node1_identity_name.clone().to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk.clone(),
            0,
            node1_commands_receiver.clone(),
            node1_db_path,
            false,
            vec![],
        );

        eprintln!("Starting Node");
        let node1_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 1");
            let _ = node1.await.start().await;
        });

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
        };

        let interactions_handler = tokio::spawn(interactions_handler_logic(env));

        let _ = tokio::try_join!(node1_handler, interactions_handler).unwrap();
    });
}
