use async_channel::{bounded, Receiver, Sender};
use shinkai_node::managers::IdentityManager;
use shinkai_node::managers::identity_manager::{Identity, IdentityType};
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::{Node};
use shinkai_node::shinkai_message::encryption::{
    encryption_public_key_to_string, hash_encryption_public_key,
    unsafe_deterministic_encryption_keypair, EncryptionMethod, decrypt_content_message, encryption_secret_key_to_string, decrypt_body_message, encrypt_body,
};
use shinkai_node::shinkai_message::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_node::shinkai_message::shinkai_message_handler::ShinkaiMessageHandler;
use shinkai_node::shinkai_message::signatures::{
    clone_signature_secret_key, signature_public_key_to_string,
    unsafe_deterministic_signature_keypair, sign_message, signature_secret_key_to_string,
};
use shinkai_node::shinkai_message::utils::hash_string;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;

#[test]
fn setup() {
    let path = Path::new("db_tests/");
    let _ = fs::remove_dir_all(&path);
}

#[test]
fn job_creation_test() {
    setup();
    let rt = Runtime::new().unwrap();

    rt.block_on(async {
        let node1_identity_name = "@@node1.shinkai";
        let node2_identity_name = "@@node2.shinkai";
        let node1_subidentity_name = "main_profile_node1";
        let node2_subidentity_name = "main_profile_node2";

    });
}