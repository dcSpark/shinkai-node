use wasm_bindgen_test::*;

#[cfg(test)]
mod tests {
    use js_sys::Uint8Array;
    use serde_wasm_bindgen::from_value;
    use shinkai_message_wasm::schemas::inbox_name::InboxName;
    use shinkai_message_wasm::schemas::registration_code::RegistrationCode;
    use shinkai_message_wasm::shinkai_message::shinkai_message::{Body, ExternalMetadata, ShinkaiMessage};
    use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::{JobMessage, JobScope, RegistrationCodeRequest};
    use shinkai_message_wasm::shinkai_utils::encryption::{
        convert_encryption_sk_string_to_encryption_pk_string, decrypt_body_message, decrypt_content_message,
        encryption_public_key_to_jsvalue, encryption_public_key_to_string, encryption_secret_key_to_jsvalue,
        encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair, EncryptionMethod,
    };
    use shinkai_message_wasm::shinkai_utils::signatures::{
        signature_secret_key_to_jsvalue, signature_secret_key_to_string, unsafe_deterministic_signature_keypair,
        verify_signature,
    };
    use shinkai_message_wasm::{ShinkaiMessageBuilderWrapper, ShinkaiMessageWrapper};
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_test::*;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_builder_with_all_fields_no_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();
        let recipient_subidentity = "recipient_user1".to_string();
        let sender_subidentity = "sender_user2".to_string();

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk);
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let node2_encryption_pk_string = encryption_public_key_to_string(node2_encryption_pk);

        let mut builder = ShinkaiMessageBuilderWrapper::new(
            my_encryption_sk_string,
            my_identity_sk_string,
            node2_encryption_pk_string,
        )
        .unwrap();

        let _ = builder.body("body content".into());
        let _ = builder.body_encryption("None".into());
        let _ = builder.message_schema_type("TextContent".into());
        let _ = builder.internal_metadata(
            sender_subidentity.clone().into(),
            recipient_subidentity.clone().into(),
            "None".into(),
        );
        let _ = builder.external_metadata_with_schedule(
            recipient.clone().into(),
            sender.clone().into(),
            scheduled_time.clone().into(),
        );

        let message_result = builder.build();
        assert!(message_result.is_ok());

        let message_json = message_result.unwrap().to_json_str().unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_json).unwrap();

        let body = message.clone().body.unwrap();
        let internal_metadata = body.internal_metadata.unwrap();
        let encryption = EncryptionMethod::from_str(&message.encryption.as_str().to_string());

        assert_eq!(body.content, "body content");
        assert_eq!(encryption, EncryptionMethod::None);
        assert_eq!(internal_metadata.sender_subidentity, sender_subidentity);
        assert_eq!(internal_metadata.recipient_subidentity, recipient_subidentity);
        assert_eq!(
            internal_metadata.inbox,
            "inbox::@@my_node.shinkai/sender_user2::@@other_node.shinkai/recipient_user1::false"
        );

        let external_metadata = message.clone().external_metadata.unwrap();

        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.scheduled_time, scheduled_time);
        assert_eq!(external_metadata.recipient, recipient);

        // Convert ShinkaiMessage back to JSON
        let message_clone_json = message.to_json_str().unwrap();
        let message_clone: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_clone_json).unwrap();
        assert!(verify_signature(&my_identity_pk, &message_clone).unwrap())
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_jsvalue_builder_with_all_fields_no_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();
        let recipient_subidentity = "recipient_user1".to_string();
        let sender_subidentity = "sender_user2".to_string();

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk);
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let node2_encryption_pk_string = encryption_public_key_to_string(node2_encryption_pk);

        let mut builder = ShinkaiMessageBuilderWrapper::new(
            my_encryption_sk_string,
            my_identity_sk_string,
            node2_encryption_pk_string,
        )
        .unwrap();

        let _ = builder.body("body content".into());
        let _ = builder.body_encryption("None".into());
        let _ = builder.message_schema_type("TextContent".into());
        let _ = builder.internal_metadata(
            sender_subidentity.clone().into(),
            recipient_subidentity.clone().into(),
            "None".into(),
        );
        let _ = builder.external_metadata_with_schedule(
            recipient.clone().into(),
            sender.clone().into(),
            scheduled_time.clone().into(),
        );

        let message_result = builder.build_to_jsvalue();
        assert!(message_result.is_ok());

        let message_string = message_result.unwrap();
        let message: ShinkaiMessageWrapper = ShinkaiMessageWrapper::from_jsvalue(&message_string).unwrap();

        let body_string = message.body().unwrap();
        let body: Body = Body::from_jsvalue(&body_string).unwrap();
        let internal_metadata = body.internal_metadata.unwrap();
        let encryption = EncryptionMethod::from_str(&message.encryption().as_str().to_string());

        assert_eq!(body.content, "body content");
        assert_eq!(encryption, EncryptionMethod::None);
        assert_eq!(internal_metadata.sender_subidentity, sender_subidentity);
        assert_eq!(internal_metadata.recipient_subidentity, recipient_subidentity);
        assert_eq!(
            internal_metadata.inbox,
            "inbox::@@my_node.shinkai/sender_user2::@@other_node.shinkai/recipient_user1::false"
        );

        let external_metadata_string = message.external_metadata().unwrap();
        let external_metadata: ExternalMetadata = ExternalMetadata::from_jsvalue(&external_metadata_string).unwrap();

        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.scheduled_time, scheduled_time);
        assert_eq!(external_metadata.recipient, recipient);

        // Convert ShinkaiMessage back to JSON
        let message_clone_string = message.to_json_str().unwrap();
        let message_clone: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_clone_string).unwrap();
        assert!(verify_signature(&my_identity_pk, &message_clone).unwrap())
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_job_message_creation() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk);
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let scope = JobScope {
            // get_job_inbox_name_from_params
            buckets: vec![InboxName::new("job_inbox::job2::false".to_string()).unwrap()],
            documents: vec!["document1".to_string(), "document2".to_string()],
        };

        let job_id = "job123".to_string();
        let content = scope.to_json_str().unwrap();
        let node_sender = "@@sender_node.shinkai".to_string();
        let node_receiver = "@@receiver_node.shinkai".to_string();
        let node_receiver_subidentity = "@@receiver_subidentity.shinkai".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::job_message(
            job_id.clone(),
            content.clone(),
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            node_sender.clone(),
            node_receiver.clone(),
            node_receiver_subidentity.clone(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!("job_message() returned an error: {:?}", e);
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Deserialize the body and check its content
        let body = message.body.unwrap();

        let job_message: JobMessage = serde_json::from_str(&body.content).unwrap();
        assert_eq!(job_message.job_id, job_id);
        assert_eq!(job_message.content, content);

        let job_message_scope: JobScope = serde_json::from_str(&job_message.content).unwrap();
        assert_eq!(job_message_scope, scope);

        // Check internal metadata
        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(internal_metadata.sender_subidentity, "".to_string());
        assert_eq!(internal_metadata.recipient_subidentity, node_receiver_subidentity);

        // Check external metadata
        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, node_sender);
        assert_eq!(external_metadata.recipient, node_receiver);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_ack_message_creation() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk);
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_node = "@@sender_node.shinkai".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::ack_message(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            sender_node.clone(),
            receiver_node.clone(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!("ack_message() returned an error: {:?}", e);
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Deserialize the body and check its content
        let body = message.body.unwrap();
        assert_eq!(body.content, "ACK".to_string());

        // Check internal metadata
        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(internal_metadata.sender_subidentity, "".to_string());
        assert_eq!(internal_metadata.recipient_subidentity, "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, sender_node);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_ping_pong_message_creation() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk);
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_node = "@@sender_node.shinkai".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();

        // Test both "Ping" and "Pong" messages
        for message_content in vec!["Ping", "Pong"] {
            // Call the function and check the result
            let message_result = ShinkaiMessageBuilderWrapper::ping_pong_message(
                message_content.to_string(),
                my_encryption_sk_string.clone(),
                my_identity_sk_string.clone(),
                receiver_public_key_string.clone(),
                sender_node.clone(),
                receiver_node.clone(),
            );

            if let Err(e) = &message_result {
                eprintln!("Error occurred: {:?}", e);
                panic!("ping_pong_message() returned an error: {:?}", e);
            }
            assert!(message_result.is_ok());

            let message_result_string = message_result.unwrap();
            let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

            // Deserialize the body and check its content
            let body = message.body.unwrap();
            assert_eq!(body.content, message_content);

            // Check internal metadata
            let internal_metadata = body.internal_metadata.unwrap();
            assert_eq!(internal_metadata.sender_subidentity, "".to_string());
            assert_eq!(internal_metadata.recipient_subidentity, "".to_string());

            // Check external metadata
            let external_metadata = message.external_metadata.unwrap();
            assert_eq!(external_metadata.sender, sender_node);
            assert_eq!(external_metadata.recipient, receiver_node);
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_create_custom_shinkai_message_to_node() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_profile = "sender_profile".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();
        let data = "Test data".to_string();
        let schema = "TextContent".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            data.clone(),
            sender_profile.clone(),
            receiver_node.clone(),
            schema.clone(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!("create_custom_shinkai_message_to_node() returned an error: {:?}", e);
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = decrypt_body_message(&message, &my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // log::debug!("decrypted_message: {:?}", decrypted_message);
        // Deserialize the body and check its content
        let body = decrypted_message.body.unwrap();
        assert_eq!(body.content, data);

        // Check internal metadata
        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(internal_metadata.sender_subidentity, sender_profile);
        assert_eq!(internal_metadata.recipient_subidentity, "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, receiver_node);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_use_code_registration() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_profile = "sender_profile".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();
        let code = "test_code".to_string();
        let identity_type = "profile".to_string();
        let permission_type = "admin".to_string();
        let registration_name = "test_registration".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::use_code_registration(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            code.clone(),
            identity_type.clone(),
            permission_type.clone(),
            registration_name.clone(),
            sender_profile.clone(),
            receiver_node.clone(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!("use_code_registration() returned an error: {:?}", e);
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = decrypt_body_message(&message, &my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let body = decrypted_message.body.unwrap();

        let registration_code: RegistrationCode = serde_json::from_str(&body.content).unwrap();
        assert_eq!(registration_code.code, code);
        assert_eq!(registration_code.registration_name, registration_name);
        assert_eq!(registration_code.identity_type, identity_type);
        assert_eq!(registration_code.permission_type, permission_type);
        let encryption_pk_string =
            convert_encryption_sk_string_to_encryption_pk_string(my_encryption_sk_string.clone()).unwrap();
        assert_eq!(registration_code.encryption_pk, encryption_pk_string);

        // Check internal metadata
        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(internal_metadata.sender_subidentity, sender_profile);
        assert_eq!(internal_metadata.recipient_subidentity, "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, receiver_node);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_request_code_registration() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_profile = "sender_profile".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();
        let permissions = "admin".to_string();
        let code_type = "profile".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::request_code_registration(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            permissions.clone(),
            code_type.clone(),
            sender_profile.clone(),
            receiver_node.clone(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!("request_code_registration() returned an error: {:?}", e);
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = decrypt_body_message(&message, &my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let body = decrypted_message.body.unwrap();

        let registration_code_request: RegistrationCodeRequest = serde_json::from_str(&body.content).unwrap();
        assert_eq!(registration_code_request.permissions.to_string(), permissions);
        assert_eq!(registration_code_request.code_type.to_string(), code_type);

        // Check internal metadata
        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(internal_metadata.sender_subidentity, sender_profile);
        assert_eq!(internal_metadata.recipient_subidentity, "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, receiver_node);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_terminate_message_creation() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk);
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_node = "@@sender_node.shinkai".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::terminate_message(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            sender_node.clone(),
            receiver_node.clone(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!(
                "terminate_message() returned an error: {:?}\n\
            my_encryption_sk_string: {}\n\
            my_identity_sk_string: {}\n\
            receiver_public_key_string: {}",
                e, my_encryption_sk_string, my_identity_sk_string, receiver_public_key_string
            );
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Check the body content
        let body = message.body.unwrap();
        assert_eq!(body.content, "terminate");

        // Check internal metadata
        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(internal_metadata.sender_subidentity, "".to_string());
        assert_eq!(internal_metadata.recipient_subidentity, "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, sender_node);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_error_message_creation() {
        console_log::init_with_level(log::Level::Debug).expect("error initializing log");

        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_node = "@@sender_node.shinkai".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();

        let error_msg = "Some error occurred.".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::error_message(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            sender_node.clone(),
            receiver_node.clone(),
            error_msg.clone(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!(
                "error_message() returned an error: {:?}\n\
            my_encryption_sk_string: {}\n\
            my_identity_sk_string: {}\n\
            receiver_public_key_string: {}",
                e, my_encryption_sk_string, my_identity_sk_string, receiver_public_key_string
            );
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Check the body content
        let body = message.body.unwrap();

        // Decrypt the content
        let decrypted_content = decrypt_content_message(
            body.content,
            &body.internal_metadata.clone().unwrap().encryption.as_str().to_string(),
            &my_encryption_sk,
            &receiver_public_key,
        )
        .expect("Failed to decrypt body content");

        assert_eq!(decrypted_content.0, format!("{{error: \"{}\"}}", error_msg));

        // Check internal metadata
        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(internal_metadata.sender_subidentity, "".to_string());
        assert_eq!(internal_metadata.recipient_subidentity, "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, sender_node);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    // #[wasm_bindgen_test]
    // fn test_builder_missing_fields() {
    //     // Setup code with keys goes here.

    //     let mut builder = ShinkaiMessageBuilderWrapper::new(/* Insert your keys here */);
    //     let message_result = builder.build();
    //     assert!(message_result.is_err());
    // }
}
