#[cfg(test)]
mod tests {
    use shinkai_message_primitives::{
        schemas::registration_code::RegistrationCode,
        shinkai_message::{
            shinkai_message::{MessageBody, MessageData},
            shinkai_message_schemas::{IdentityPermissions, JobMessage, MessageSchemaType},
        },
        shinkai_utils::{
            encryption::{
                string_to_encryption_public_key, string_to_encryption_static_key,
                unsafe_deterministic_encryption_keypair, EncryptionMethod,
            },
            shinkai_message_builder::ShinkaiMessageBuilder,
            signatures::{string_to_signature_secret_key, unsafe_deterministic_signature_keypair},
        },
    };

    #[test]
    fn test_job_message() {
        let my_encryption_sk =
            string_to_encryption_static_key("b01a8082dba0a866fa82d8d3e2dea25b053387e2ac06f35a5e237104de2c5374")
                .unwrap();
        let my_signature_sk =
            string_to_signature_secret_key("03b1cd8cdce9a8a54ce73a262f11c3ff17eedf9f696f14e9ab55a89476b22306").unwrap();
        let receiver_public_key =
            string_to_encryption_public_key("798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679")
                .unwrap();

        let node_sender = "@@localhost.shinkai".to_string();
        let node_receiver = "@@localhost.shinkai".to_string();
        let sender_subidentity = "main".to_string();
        let inbox = "jobid_399c5571-3504-4aa7-a291-b1e086c1440c".to_string();
        let message_raw_content = "hello hello, are u there?".to_string();

        let message_result = ShinkaiMessageBuilder::job_message(
            inbox.clone(),
            message_raw_content.clone(),
            "".to_string(),
            "".to_string(),
            None,
            my_encryption_sk.clone(),
            my_signature_sk.clone(),
            receiver_public_key,
            node_sender.clone(),
            sender_subidentity.clone(),
            node_receiver.clone(),
            "".to_string(),
        );

        assert!(message_result.is_ok());
        let message = message_result.unwrap();

        // Check if the message content is as expected
        if let MessageBody::Unencrypted(shinkai_body) = &message.body {
            if let MessageData::Unencrypted(shinkai_data) = &shinkai_body.message_data {
                let job_message: JobMessage = serde_json::from_str(&shinkai_data.message_raw_content).unwrap();
                assert_eq!(job_message.job_id, inbox);
                assert_eq!(job_message.content, "hello hello, are u there?");
                assert_eq!(job_message.files_inbox, "");
            }
        }

        // Check if the external metadata is as expected
        assert_eq!(message.external_metadata.sender, node_sender);
        assert_eq!(message.external_metadata.recipient, node_receiver);

        // Check if the encryption method is as expected
        assert_eq!(message.encryption, EncryptionMethod::DiffieHellmanChaChaPoly1305);
    }

    #[test]
    fn test_builder_with_all_fields_no_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "2023-07-02T20:53:34Z".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk)
            .message_raw_content("body content".to_string())
            .body_encryption(EncryptionMethod::None)
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata("".to_string(), "".to_string(), EncryptionMethod::None, None)
            .external_metadata_with_schedule(recipient.clone(), sender.clone(), scheduled_time.clone())
            .build();

        println!("message_result = {:?}", message_result);
        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();

        if let MessageBody::Unencrypted(shinkai_body) = message.body {
            if let MessageData::Unencrypted(shinkai_data) = shinkai_body.message_data {
                assert_eq!(shinkai_data.message_raw_content, "body content");
            }
            assert_eq!(shinkai_body.internal_metadata.sender_subidentity, "");
            assert_eq!(shinkai_body.internal_metadata.recipient_subidentity, "");
            assert_eq!(
                shinkai_body.internal_metadata.inbox,
                "inbox::@@my_node.shinkai::@@other_node.shinkai::false"
            );
        }

        assert_eq!(message.encryption, EncryptionMethod::None);
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.scheduled_time, scheduled_time);
        assert_eq!(external_metadata.recipient, recipient);
        assert!(message_clone.verify_outer_layer_signature(&my_identity_pk).unwrap())
    }

    #[test]
    fn test_builder_with_all_fields_body_encryption() {
        println!("\n\n\ntest_builder_with_all_fields_body_encryption");
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk.clone(), my_identity_sk, node2_encryption_pk)
            .message_raw_content("body content".to_string())
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata("".to_string(), "".to_string(), EncryptionMethod::None, None)
            .external_metadata(recipient, sender.clone())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();
        assert_eq!(message.encryption, EncryptionMethod::DiffieHellmanChaChaPoly1305);

        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &node2_encryption_pk)
            .expect("Failed to decrypt body content");

        let binding = decrypted_message.body.clone();
        if let MessageBody::Unencrypted(shinkai_body) = binding {
            let message_data = shinkai_body.message_data;
            if let MessageData::Unencrypted(shinkai_data) = message_data {
                let decrypted_content = shinkai_data.message_raw_content;
                assert_eq!(decrypted_content, "body content");
            }
        }

        let binding = decrypted_message.body.clone();
        if let MessageBody::Unencrypted(shinkai_body) = binding {
            let internal_metadata = &shinkai_body.internal_metadata;
            assert_eq!(internal_metadata.sender_subidentity, "");
            assert_eq!(internal_metadata.recipient_subidentity, "");
            assert_eq!(
                internal_metadata.inbox,
                "inbox::@@my_node.shinkai::@@other_node.shinkai::false"
            );
        }
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender);
        assert!(message_clone.verify_outer_layer_signature(&my_identity_pk).unwrap())
    }

    #[test]
    fn test_builder_with_all_fields_content_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk.clone(), my_identity_sk, node2_encryption_pk)
            .message_raw_content("body content".to_string())
            .no_body_encryption()
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata(
                "".to_string(),
                "".to_string(),
                EncryptionMethod::DiffieHellmanChaChaPoly1305,
                None,
            )
            .external_metadata(recipient, sender.clone())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();

        if let MessageBody::Unencrypted(shinkai_body) = message.clone().body {
            assert_eq!(
                shinkai_body.internal_metadata.encryption,
                EncryptionMethod::DiffieHellmanChaChaPoly1305
            );
        }

        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &node2_encryption_pk)
            .expect("Failed to decrypt body content");

        if let MessageBody::Unencrypted(shinkai_body) = decrypted_message.body {
            if let MessageData::Unencrypted(shinkai_data) = shinkai_body.message_data {
                assert_eq!(shinkai_data.message_raw_content, "body content");
            }
        }

        assert_eq!(message.encryption, EncryptionMethod::None);
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender);
        assert!(message_clone.verify_outer_layer_signature(&my_identity_pk).unwrap())
    }

    #[test]
    fn test_builder_with_all_fields_onion_encryption() {}

    #[test]
    fn test_builder_use_code_registration() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (profile_identity_sk, _profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (profile_encryption_sk, _profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = recipient.clone();
        let sender_subidentity = "main".to_string();

        let code = "registration_code".to_string();
        let identity_type = IdentityPermissions::Admin.to_string();
        let permission_type = "profile".to_string();
        let registration_name = "registration_name".to_string();

        let message_result = ShinkaiMessageBuilder::use_code_registration_for_device(
            my_encryption_sk.clone(),
            my_identity_sk,
            profile_encryption_sk,
            profile_identity_sk,
            node2_encryption_pk,
            code,
            identity_type,
            permission_type,
            registration_name,
            sender_subidentity.clone(),
            sender.clone(),
            recipient.clone(),
        );
        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();

        assert_eq!(message.encryption, EncryptionMethod::DiffieHellmanChaChaPoly1305);

        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &node2_encryption_pk)
            .expect("Failed to decrypt body content");

        if let MessageBody::Unencrypted(shinkai_body) = decrypted_message.body {
            if let MessageData::Unencrypted(shinkai_data) = shinkai_body.message_data {
                // Parse the decrypted content from a JSON string to a RegistrationCode struct
                let parsed_content: Result<RegistrationCode, _> =
                    serde_json::from_str(&shinkai_data.message_raw_content);
                match &parsed_content {
                    Ok(registration_code) => {
                        println!("Parsed content: {:?}", registration_code);
                    }
                    Err(e) => {
                        eprintln!("Failed to parse content: {:?}", e);
                    }
                }

                let registration_code = parsed_content.unwrap();
                assert_eq!(registration_code.code, "registration_code");
                assert_eq!(registration_code.registration_name, "registration_name");
                assert_eq!(registration_code.permission_type, "profile");
                assert_eq!(registration_code.identity_type, "admin");
            }
            assert_eq!(shinkai_body.internal_metadata.sender_subidentity, sender_subidentity);
        }

        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, recipient);
        assert!(message_clone.verify_outer_layer_signature(&my_identity_pk).unwrap())
    }

    #[test]
    fn test_initial_registration_with_no_code_for_device() {
        let (my_device_identity_sk, _my_device_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_device_encryption_sk, _my_device_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (profile_identity_sk, _profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (profile_encryption_sk, _profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = recipient.clone();
        let sender_subidentity = "main".to_string();

        let registration_name = "registration_name".to_string();

        let message_result = ShinkaiMessageBuilder::initial_registration_with_no_code_for_device(
            my_device_encryption_sk.clone(),
            my_device_identity_sk,
            profile_encryption_sk,
            profile_identity_sk,
            registration_name.clone(),
            sender_subidentity.clone(),
            sender.clone(),
            recipient.clone(),
        );
        assert!(message_result.is_ok());
        let message = message_result.unwrap();

        assert_eq!(message.encryption, EncryptionMethod::None);

        if let MessageBody::Unencrypted(shinkai_body) = message.body {
            if let MessageData::Unencrypted(shinkai_data) = shinkai_body.message_data {
                // Parse the decrypted content from a JSON string to a RegistrationCode struct
                let parsed_content: Result<RegistrationCode, _> =
                    serde_json::from_str(&shinkai_data.message_raw_content);
                match &parsed_content {
                    Ok(registration_code) => {
                        println!("Parsed content: {:?}", registration_code);
                    }
                    Err(e) => {
                        eprintln!("Failed to parse content: {:?}", e);
                    }
                }

                let registration_code = parsed_content.unwrap();
                assert_eq!(registration_code.code, "");
                assert_eq!(registration_code.registration_name, registration_name);
                assert_eq!(registration_code.permission_type, "admin");
                assert_eq!(registration_code.identity_type, "device");
            }
            assert_eq!(shinkai_body.internal_metadata.sender_subidentity, sender_subidentity);
        }

        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, recipient);
    }

    #[test]
    fn test_builder_missing_fields() {
        let (my_identity_sk, _my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk).build();
        assert!(message_result.is_err());
    }
}
