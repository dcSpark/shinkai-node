// We can import a ShinkaiNode and start it from scratch
// Then we can register a new identity and profile (ready for testing)
// Then we copy a starting folder and
// We sync the filesystem from a specific folder (files + subfolders with more items)
// We check that the files are correctly synced
// Then we add a new file and remove another file
// These changes should be reflected in the Node system

use async_channel::{bounded, Receiver, Sender};
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::{fs, path::Path};
use tokio::runtime::Runtime;

use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{
    IdentityPermissions, MessageSchemaType, RegistrationCodeType,
};
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
};

use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, signature_public_key_to_string, signature_secret_key_to_string,
    unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::Node;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn sync_tests() {
    eprintln!("Starting sync tests");
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1_test.sepolia-shinkai";
        let node2_identity_name = "@@node2_test.sepolia-shinkai";
        let node1_profile_name = "main";
        let node1_device_name = "node1_device";
        let node2_profile_name = "main_profile_node2";

        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let node1_encryption_sk_clone = node1_encryption_sk.clone();
        let node1_encryption_sk_clone2 = node1_encryption_sk.clone();

        let (node2_identity_sk, node2_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (node2_encryption_sk, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let node2_encryption_sk_clone = node2_encryption_sk.clone();

        let node1_identity_sk_clone = clone_signature_secret_key(&node1_identity_sk);
        let node2_identity_sk_clone = clone_signature_secret_key(&node2_identity_sk);

        let (node1_profile_identity_sk, node1_profile_identity_pk) = unsafe_deterministic_signature_keypair(100);
        let (node1_profile_encryption_sk, node1_profile_encryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let (node2_subidentity_sk, node2_subidentity_pk) = unsafe_deterministic_signature_keypair(101);
        let (node2_subencryption_sk, node2_subencryption_pk) = unsafe_deterministic_encryption_keypair(101);

        let node1_subencryption_sk_clone = node1_profile_encryption_sk.clone();
        let node2_subencryption_sk_clone = node2_subencryption_sk.clone();

        let node1_subidentity_sk_clone = clone_signature_secret_key(&node1_profile_identity_sk);
        let node2_subidentity_sk_clone = clone_signature_secret_key(&node2_subidentity_sk);

        let (node1_device_identity_sk, node1_device_identity_pk) = unsafe_deterministic_signature_keypair(200);
        let (node1_device_encryption_sk, node1_device_encryption_pk) = unsafe_deterministic_encryption_keypair(200);

        let (node1_commands_sender, node1_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);
        let (node2_commands_sender, node2_commands_receiver): (Sender<NodeCommand>, Receiver<NodeCommand>) =
            bounded(100);

        let node1_db_path = format!("db_tests/{}", hash_string(node1_identity_name.clone()));
        let node1_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node1_identity_name.clone()));
        let node2_db_path = format!("db_tests/{}", hash_string(node2_identity_name.clone()));
        let node2_fs_db_path = format!("db_tests/vector_fs{}", hash_string(node2_identity_name.clone()));

        // Create node1 and node2
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut node1 = Node::new(
            node1_identity_name.to_string(),
            addr1,
            clone_signature_secret_key(&node1_identity_sk),
            node1_encryption_sk,
            0,
            node1_commands_receiver,
            node1_db_path,
            true,
            vec![],
            None,
            node1_fs_db_path,
            None,
            None,
        )
        .await;

        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let mut node2 = Node::new(
            node2_identity_name.to_string(),
            addr2,
            clone_signature_secret_key(&node2_identity_sk),
            node2_encryption_sk,
            0,
            node2_commands_receiver,
            node2_db_path,
            true,
            vec![],
            None,
            node2_fs_db_path,
            None,
            None,
        )
        .await;

        // Printing
        eprintln!(
            "Node 1 encryption sk: {:?}",
            encryption_secret_key_to_string(node1_encryption_sk_clone2.clone())
        );
        eprintln!(
            "Node 1 encryption pk: {:?}",
            encryption_public_key_to_string(node1_encryption_pk)
        );

        eprintln!(
            "Node 2 encryption sk: {:?}",
            encryption_secret_key_to_string(node2_encryption_sk_clone)
        );
        eprintln!(
            "Node 2 encryption pk: {:?}",
            encryption_public_key_to_string(node2_encryption_pk)
        );

        eprintln!(
            "Node 1 identity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node1_identity_sk))
        );
        eprintln!(
            "Node 1 identity pk: {:?}",
            signature_public_key_to_string(node1_identity_pk)
        );

        eprintln!(
            "Node 2 identity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node2_identity_sk))
        );
        eprintln!(
            "Node 2 identity pk: {:?}",
            signature_public_key_to_string(node2_identity_pk)
        );

        eprintln!(
            "Node 1 subidentity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node1_profile_identity_sk))
        );
        eprintln!(
            "Node 1 subidentity pk: {:?}",
            signature_public_key_to_string(node1_profile_identity_pk)
        );

        eprintln!(
            "Node 2 subidentity sk: {:?}",
            signature_secret_key_to_string(clone_signature_secret_key(&node2_subidentity_sk))
        );
        eprintln!(
            "Node 2 subidentity pk: {:?}",
            signature_public_key_to_string(node2_subidentity_pk)
        );

        eprintln!(
            "Node 1 subencryption sk: {:?}",
            encryption_secret_key_to_string(node1_subencryption_sk_clone.clone())
        );
        eprintln!(
            "Node 1 subencryption pk: {:?}",
            encryption_public_key_to_string(node1_profile_encryption_pk)
        );

        eprintln!(
            "Node 2 subencryption sk: {:?}",
            encryption_secret_key_to_string(node2_subencryption_sk_clone.clone())
        );
        eprintln!(
            "Node 2 subencryption pk: {:?}",
            encryption_public_key_to_string(node2_subencryption_pk)
        );

        eprintln!("Starting nodes");
        // Start node1 and node2
        let node1_clone = Arc::clone(&node1);
        let node1_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 1");
            let _ = node1_clone.lock().await.start().await;
        });

        let node1_abort_handler = node1_handler.abort_handle();

        let node2_clone = Arc::clone(&node2);
        let node2_handler = tokio::spawn(async move {
            eprintln!("\n\n");
            eprintln!("Starting node 2");
            let _ = node2_clone.lock().await.start().await;
        });
        let node2_abort_handler = node2_handler.abort_handle();

        let interactions_handler = tokio::spawn(async move {
            eprintln!("Starting interactions");
            eprintln!("Registration of Subidentities");
        });
    });
}
