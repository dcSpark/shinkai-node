use super::db_handlers::setup;
use async_channel::{bounded, Receiver, Sender};
use async_std::println;
use tokio::task::AbortHandle;
use core::panic;
use ed25519_dalek::{SigningKey, VerifyingKey};
use futures::Future;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    IdentityPermissions, MessageSchemaType, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::pin::Pin;
use std::{net::SocketAddr, time::Duration};
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
    pub node1_abort_handler: AbortHandle,
}

pub fn run_test_one_node_network<F>(interactions_handler_logic: F)
where
    F: FnOnce(TestEnvironment) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
{
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.sepolia-shinkai";
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
        let node1_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node1_identity_name.clone()));

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
            None,
            node1_fs_db_path,
            None,
            None,
        );

        eprintln!("Starting Node");
        let node1_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 1");
            let _ = node1.await.lock().await.start().await;
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
            node1_abort_handler
        };

        let interactions_handler = tokio::spawn(interactions_handler_logic(env));

        let result = tokio::try_join!(node1_handler, interactions_handler);
        match result {
            Ok(_) => {},
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
