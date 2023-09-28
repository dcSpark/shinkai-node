use async_channel::{bounded, Receiver, Sender};
use shinkai_message_primitives::schemas::agents::serialized_agent::{AgentLLMInterface, OpenAI, SerializedAgent};
use shinkai_message_primitives::schemas::inbox_name::InboxName;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{JobMessage, MessageSchemaType};
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, encrypt_with_chacha20poly1305, encryption_public_key_to_string,
    encryption_secret_key_to_string, ephemeral_encryption_keys, unsafe_deterministic_encryption_keypair,
    EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_message_primitives::shinkai_utils::utils::hash_string;
use shinkai_node::agent::agent;
use shinkai_node::network::node::NodeCommand;
use shinkai_node::network::node_api::APIError;
use shinkai_node::network::Node;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::{net::SocketAddr, time::Duration};
use tokio::runtime::Runtime;
use utils::test_boilerplate::run_test_one_node_network;

mod utils;
use crate::utils::node_test_api::{
    api_agent_registration, api_create_job, api_initial_registration_with_no_code_for_device, api_message_job,
    api_registration_device_node_profile_main,
};
use crate::utils::node_test_local::local_registration_profile_node;

#[test]
fn sandwich_messages_with_files_test() {
    run_test_one_node_network(|env| {
        Box::pin(async move {
            let node1_commands_sender = env.node1_commands_sender.clone();
            let node1_identity_name = env.node1_identity_name.clone();
            let node1_profile_name = env.node1_profile_name.clone();
            let node1_device_name = env.node1_device_name.clone();
            let node1_encryption_pk = env.node1_encryption_pk.clone();
            let node1_device_encryption_sk = env.node1_device_encryption_sk.clone();
            let node1_profile_encryption_sk = env.node1_profile_encryption_sk.clone();
            let node1_device_identity_sk = clone_signature_secret_key(&env.node1_device_identity_sk);
            let node1_profile_identity_sk = clone_signature_secret_key(&env.node1_profile_identity_sk);

            // For this test
            let (symmetrical_sk, symmetrical_pk) = ephemeral_encryption_keys();

            {
                // Register a Profile in Node1 and verifies it
                eprintln!("\n\nRegister a Device with main Profile in Node1 and verify it");
                api_initial_registration_with_no_code_for_device(
                    node1_commands_sender.clone(),
                    env.node1_profile_name.as_str(),
                    env.node1_identity_name.as_str(),
                    node1_encryption_pk,
                    node1_device_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_device_identity_sk),
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_device_name.as_str(),
                )
                .await;
            }

            // Send message (APICreateFilesInboxWithSymmetricKey) from Device subidentity to Node 1
            {
                eprintln!("\n\n### Sending message (APICreateFilesInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

                let sender = format!("{}/{}", node1_identity_name, node1_profile_name);
                let message_content = encryption_secret_key_to_string(symmetrical_sk.clone());
                let msg = ShinkaiMessageBuilder::create_files_inbox_with_sym_key(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    "job::test::false".to_string(),
                    message_content.clone(),
                    "".to_string(),
                    sender,
                    node1_identity_name.to_string(),
                )
                .unwrap();

                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res: res_sender })
                    .await
                    .unwrap();
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive messages");
                eprintln!("response: {}", response);
            }
            {
                eprintln!("\n\n### Sending message (APIAddFileToInboxWithSymmetricKey) from profile subidentity to node 1\n\n");

                // Prepare the file to be sent
                let file_path = Path::new("tmp_tests/test_file.txt");
                // Create the directory if it does not exist
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).expect("Failed to create directory");
                }
                let file_content = "This is a test file";
                fs::write(&file_path, file_content).expect("Unable to write file");

                // Read the entire file into a Vec<u8>
                let file_data = tokio::fs::read(&file_path).await.expect("Failed to read file");

                // Encrypt the file using ChaCha20Poly1305
                let (ciphertext, nonce) =
                    encrypt_with_chacha20poly1305(&symmetrical_sk, &file_data).expect("encryption failure!");

                // Prepare the other parameters
                let filename = "test_file.txt";

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
                        filename: filename.to_string(),
                        file: ciphertext,
                        public_key: encryption_public_key_to_string(symmetrical_pk),
                        encrypted_nonce: nonce.to_string(),
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                // Receive the response
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("response: {}", response);
            }
        })
    });
}
