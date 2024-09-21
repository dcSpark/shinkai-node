use super::utils::test_boilerplate::run_test_one_node_network;
use aes_gcm::aead::{generic_array::GenericArray, Aead};
use aes_gcm::Aes256Gcm;
use aes_gcm::KeyInit;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{JobMessage, MessageSchemaType};
use shinkai_message_primitives::shinkai_utils::encryption::{
    clone_static_secret_key, encryption_public_key_to_string, encryption_secret_key_to_string,
    ephemeral_encryption_keys, unsafe_deterministic_encryption_keypair, EncryptionMethod,
};
use shinkai_message_primitives::shinkai_utils::file_encryption::{
    aes_encryption_key_to_string, aes_nonce_to_hex_string, hash_of_aes_encryption_key_hex,
    unsafe_deterministic_aes_encryption_key,
};
use shinkai_message_primitives::shinkai_utils::shinkai_logging::init_default_tracing;
use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
use shinkai_message_primitives::shinkai_utils::signatures::{
    clone_signature_secret_key, unsafe_deterministic_signature_keypair,
};
use shinkai_http_api::node_commands::NodeCommand;
use shinkai_vector_resources::resource_errors::VRError;
use std::path::Path;

use super::utils::node_test_api::{
    api_agent_registration, api_create_job, api_get_all_inboxes_from_profile, api_get_all_smart_inboxes_from_profile,
    api_initial_registration_with_no_code_for_device, api_message_job, api_registration_device_node_profile_main,
};
use mockito::Server;

// #[test]
fn node_toolkit_api() {
    
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
            let node1_abort_handler = env.node1_abort_handler;

            // For this test
            let symmetrical_sk = unsafe_deterministic_aes_encryption_key(0);

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

                let message_content = aes_encryption_key_to_string(symmetrical_sk.clone());
                let msg = ShinkaiMessageBuilder::create_files_inbox_with_sym_key(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                    "job::t123est::false".to_string(),
                    message_content.clone(),
                    node1_profile_name.to_string(),
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string(),
                )
                .unwrap();

                let (res_sender, res_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APICreateFilesInboxWithSymmetricKey { msg, res: res_sender })
                    .await
                    .unwrap();
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive messages");
            }
            {
                eprintln!("\n\n### Uploading Header\n\n");

                // Prepare the file to be read
                let filename_header = "../../files/example-toolkit-setup.json";
                let file_path_header = Path::new(filename_header.clone());

                // Read the file into a buffer
                let file_data = std::fs::read(&file_path_header)
                    .map_err(|_| VRError::FailedPDFParsing)
                    .unwrap();

                // Encrypt the file using Aes256Gcm
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));
                let nonce = GenericArray::from_slice(&[0u8; 12]);
                let nonce_slice = nonce.as_slice();
                let nonce_str = aes_nonce_to_hex_string(nonce_slice);
                let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
                        filename: filename_header.to_string(),
                        file: ciphertext,
                        public_key: hash_of_aes_encryption_key_hex(symmetrical_sk),
                        encrypted_nonce: nonce_str,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                // Receive the response
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("response: {}", response);
            }
            {
                eprintln!("\n\n### Uploading content\n\n");

                // Prepare the file to be read
                let filename_header = "../../files/example-packaged-shinkai-toolkit.js";
                let file_path_header = Path::new(filename_header.clone());

                // Read the file into a buffer
                let file_data = std::fs::read(&file_path_header)
                    .map_err(|_| VRError::FailedPDFParsing)
                    .unwrap();

                // Encrypt the file using Aes256Gcm
                let cipher = Aes256Gcm::new(GenericArray::from_slice(&symmetrical_sk));
                let nonce = GenericArray::from_slice(&[0u8; 12]);
                let nonce_slice = nonce.as_slice();
                let nonce_str = aes_nonce_to_hex_string(nonce_slice);
                let ciphertext = cipher.encrypt(nonce, file_data.as_ref()).expect("encryption failure!");

                // Prepare the response channel
                let (res_sender, res_receiver) = async_channel::bounded(1);

                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
                        filename: filename_header.to_string(),
                        file: ciphertext,
                        public_key: hash_of_aes_encryption_key_hex(symmetrical_sk),
                        encrypted_nonce: nonce_str,
                        res: res_sender,
                    })
                    .await
                    .unwrap();

                // Receive the response
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                eprintln!("response: {}", response);
            }
            {
                // Get filenames in inbox
                let message_content = hash_of_aes_encryption_key_hex(symmetrical_sk);
                let msg = ShinkaiMessageBuilder::new(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                )
                .message_raw_content(message_content.clone())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node1_profile_name.to_string().clone(),
                    "".to_string(),
                    EncryptionMethod::None,
                    None,
                )
                .external_metadata_with_intra_sender(
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string().clone(),
                    node1_profile_name.to_string().clone(),
                )
                .build()
                .unwrap();

                let (res_sender, res_receiver) = async_channel::bounded(1);
                // Send the command
                node1_commands_sender
                    .send(NodeCommand::APIGetFilenamesInInbox { msg, res: res_sender })
                    .await
                    .unwrap();

                // Receive the response
                let response = res_receiver.recv().await.unwrap().expect("Failed to receive response");
                assert_eq!(
                    response,
                    vec![
                        "../../files/example-packaged-shinkai-toolkit.js",
                        "../../files/example-toolkit-setup.json"
                    ]
                );
            }
            {
                // Send a Message to the Job for processing
                eprintln!("\n\nAdd Toolkit");
                let message_content = hash_of_aes_encryption_key_hex(symmetrical_sk);
                let msg = ShinkaiMessageBuilder::new(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                )
                .message_raw_content(message_content.clone())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node1_profile_name.to_string().clone(),
                    "".to_string(),
                    EncryptionMethod::None,
                    None,
                )
                .external_metadata_with_intra_sender(
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string().clone(),
                    node1_profile_name.to_string().clone(),
                )
                .build()
                .unwrap();

                let (res_registration_sender, res_registraton_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIAddToolkit {
                        msg,
                        res: res_registration_sender,
                    })
                    .await
                    .unwrap();
                let resp = res_registraton_receiver.recv().await.unwrap();
                eprintln!("resp: {:?}", resp);
            }
            {
                // Send a Message to the Job for processing
                eprintln!("\n\nList Toolkits");
                let message_content = hash_of_aes_encryption_key_hex(symmetrical_sk);
                let msg = ShinkaiMessageBuilder::new(
                    node1_profile_encryption_sk.clone(),
                    clone_signature_secret_key(&node1_profile_identity_sk),
                    node1_encryption_pk,
                )
                .message_raw_content(message_content.clone())
                .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
                .message_schema_type(MessageSchemaType::TextContent)
                .internal_metadata(
                    node1_profile_name.to_string().clone(),
                    "".to_string(),
                    EncryptionMethod::None,
                    None,
                )
                .external_metadata_with_intra_sender(
                    node1_identity_name.to_string(),
                    node1_identity_name.to_string().clone(),
                    node1_profile_name.to_string().clone(),
                )
                .build()
                .unwrap();

                let (res_list_sender, res_list_receiver) = async_channel::bounded(1);
                node1_commands_sender
                    .send(NodeCommand::APIListToolkits {
                        msg,
                        res: res_list_sender,
                    })
                    .await
                    .unwrap();
                let _ = res_list_receiver.recv().await.unwrap();
                node1_abort_handler.abort();
            }
        })
    });
}
