use wasm_bindgen_test::*;

#[cfg(test)]
mod tests {
    use js_sys::Uint8Array;
    use serde_wasm_bindgen::from_value;
    use shinkai_message_primitives::schemas::inbox_name::InboxName;
    use shinkai_message_primitives::schemas::registration_code::RegistrationCode;
    use shinkai_message_primitives::shinkai_message::shinkai_message::{
        ExternalMetadata, MessageBody, MessageData, ShinkaiBody, ShinkaiMessage,
    };
    use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{AssociatedUI, JobCreationInfo, JobMessage, RegistrationCodeRequest};
    use shinkai_message_primitives::shinkai_utils::encryption::{
        encryption_public_key_to_string, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair,
        EncryptionMethod,
    };
    use shinkai_message_primitives::shinkai_utils::job_scope::JobScope;
    use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiNameString;
    use shinkai_message_primitives::shinkai_utils::signatures::{
        signature_secret_key_to_string, unsafe_deterministic_signature_keypair,
    };
    use shinkai_message_wasm::shinkai_utils::encryption::convert_encryption_sk_string_to_encryption_pk_string;
    use shinkai_message_wasm::shinkai_wasm_wrappers::inbox_name_wrapper::InboxNameWrapper;
    use shinkai_message_wasm::shinkai_wasm_wrappers::wasm_shinkai_message::SerdeWasmMethods;
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
        let scheduled_time = "2023-07-02T20:53:34Z".to_string();
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

        let _ = builder.message_raw_content("body content".into());
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

        let message_json = message_result.unwrap().to_json_str();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_json).unwrap();

        let body = message.clone().body;
        let internal_metadata;
        let body_content;

        match body {
            MessageBody::Unencrypted(unencrypted_body) => {
                internal_metadata = unencrypted_body.internal_metadata;
                match unencrypted_body.message_data {
                    MessageData::Unencrypted(data) => {
                        body_content = data.message_raw_content;
                    }
                    _ => panic!("Unexpected MessageData variant"),
                }
            }
            _ => panic!("Unexpected MessageBody variant"),
        }

        let encryption = EncryptionMethod::from_str(&message.encryption.as_str().to_string());

        assert_eq!(body_content, "body content");
        assert_eq!(encryption, EncryptionMethod::None);
        assert_eq!(internal_metadata.sender_subidentity, sender_subidentity);
        assert_eq!(internal_metadata.recipient_subidentity, recipient_subidentity);
        assert_eq!(
            internal_metadata.inbox,
            "inbox::@@my_node.shinkai/sender_user2::@@other_node.shinkai/recipient_user1::false"
        );

        let external_metadata = message.clone().external_metadata;

        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.scheduled_time, scheduled_time);
        assert_eq!(external_metadata.recipient, recipient);

        // Convert ShinkaiMessage back to JSON
        let message_clone_json = message.to_json_str().unwrap();
        let message_clone: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_clone_json).unwrap();
        assert!(message_clone.verify_outer_layer_signature(&my_identity_pk).unwrap());
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_jsvalue_builder_with_all_fields_no_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "2023-07-02T20:53:34Z".to_string();
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

        let _ = builder.message_raw_content("body content".into());
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

        let body_jsvalue = message.message_body().unwrap();
        let body: ShinkaiBody = ShinkaiBody::from_jsvalue(&body_jsvalue).unwrap();
        let internal_metadata = body.internal_metadata;
        let encryption = EncryptionMethod::from_str(&message.encryption().as_str().to_string());

        let body_content = match body.message_data {
            MessageData::Unencrypted(data) => data.message_raw_content,
            _ => panic!("Unexpected MessageData variant"),
        };
        assert_eq!(body_content, "body content");
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
        let message_clone_string = message.to_json_str();
        let message_clone: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_clone_string).unwrap();
        assert!(message_clone.verify_outer_layer_signature(&my_identity_pk).unwrap())
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

        let scope = JobScope::new_default();
        let job_id = "job123".to_string();
        let content = scope.to_json_str().unwrap();
        let node_sender = "@@sender_node.shinkai".to_string();
        let sender_subidentity = "main".to_string();
        let node_receiver = "@@receiver_node.shinkai".to_string();
        let node_receiver_subidentity = "@@receiver_subidentity.shinkai".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::job_message(
            job_id.clone(),
            content.clone(),
            String::new(),
            String::new(),
            None,
            None,
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            node_sender.clone(),
            sender_subidentity.clone(),
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
        let body = match message.body {
            MessageBody::Unencrypted(body) => body,
            _ => panic!("Unexpected MessageBody variant"),
        };

        let job_message: JobMessage = match body.message_data {
            MessageData::Unencrypted(data) => serde_json::from_str(&data.message_raw_content).unwrap(),
            _ => panic!("Unexpected MessageData variant"),
        };
        assert_eq!(job_message.job_id, job_id);
        assert_eq!(job_message.content, content);

        let job_message_scope: JobScope = serde_json::from_str(&job_message.content).unwrap();
        assert_eq!(job_message_scope, scope);

        // Check internal metadata
        let internal_metadata = body.internal_metadata;
        assert_eq!(internal_metadata.sender_subidentity, sender_subidentity.to_string());
        assert_eq!(internal_metadata.recipient_subidentity, node_receiver_subidentity);

        // Check external metadata
        let external_metadata = message.external_metadata;
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
        let sender_subidentity = "main".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::ack_message(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            sender_node.clone(),
            sender_subidentity.clone(),
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
        let body_content = message.get_message_content().unwrap();
        assert_eq!(body_content, "ACK");

        // Check internal metadata
        assert_eq!(message.get_recipient_subidentity().unwrap(), "".to_string());
        assert_eq!(message.get_sender_subidentity().unwrap(), "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata;
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
            assert_eq!(message.get_message_content().unwrap(), message_content);

            // Check internal metadata
            assert_eq!(message.get_recipient_subidentity().unwrap(), "".to_string());
            assert_eq!(message.get_sender_subidentity().unwrap(), "".to_string());

            // Check external metadata
            let external_metadata = message.external_metadata;
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
        let sender = format!("{}/{}", receiver_node.clone(), sender_profile);
        let data = "Test data".to_string();
        let schema = "TextContent".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            data.clone(),
            sender.clone(),
            "".to_string(),
            receiver_node.clone(),
            "".to_string(),
            "",
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
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // log::debug!("decrypted_message: {:?}", decrypted_message);
        // Deserialize the body and check its content
        assert_eq!(decrypted_message.get_message_content().unwrap(), data);

        // Check internal metadata
        assert_eq!(decrypted_message.get_recipient_subidentity().unwrap(), "".to_string());
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), "".to_string());

        // Check external metadata
        let external_metadata = decrypted_message.external_metadata;
        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_use_code_registration_for_device() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (profile_identity_sk, _) = unsafe_deterministic_signature_keypair(1);
        let (profile_encryption_sk, _) = unsafe_deterministic_encryption_keypair(1);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(2);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let profile_encryption_sk_string = encryption_secret_key_to_string(profile_encryption_sk.clone());
        let profile_identity_sk_string = signature_secret_key_to_string(profile_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_profile = "sender_profile".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();
        let code = "test_code".to_string();
        let identity_type = "profile".to_string();
        let permission_type = "admin".to_string();
        let registration_name = "test_registration".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::use_code_registration_for_device(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            profile_encryption_sk_string.clone(),
            profile_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            code.clone(),
            identity_type.clone(),
            permission_type.clone(),
            registration_name.clone(),
            receiver_node.clone(),
            sender_profile.clone(),
            receiver_node.clone(),
            "".to_string(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!("use_code_registration() returned an error: {:?}", e);
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        let registration_code: RegistrationCode = serde_json::from_str(&content).unwrap();
        assert_eq!(registration_code.code, code);
        assert_eq!(registration_code.registration_name, registration_name);
        assert_eq!(registration_code.identity_type, identity_type);
        assert_eq!(registration_code.permission_type, permission_type);
        let encryption_pk_string =
            convert_encryption_sk_string_to_encryption_pk_string(my_encryption_sk_string.clone()).unwrap();
        let profile_encryption_pk_string =
            convert_encryption_sk_string_to_encryption_pk_string(profile_encryption_sk_string.clone()).unwrap();
        assert_eq!(registration_code.device_encryption_pk, encryption_pk_string);
        assert_eq!(registration_code.profile_encryption_pk, profile_encryption_pk_string);

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_profile);
        assert_eq!(decrypted_message.get_recipient_subidentity().unwrap(), "".to_string());

        // Check external metadata
        let external_metadata = decrypted_message.external_metadata;
        assert_eq!(external_metadata.sender, receiver_node);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_initial_registration_with_no_code_for_device() {
        // Initialize test data
        let (my_device_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_device_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (profile_sk, _) = unsafe_deterministic_signature_keypair(1);
        let (profile_encryption_sk, _) = unsafe_deterministic_encryption_keypair(1);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(2);

        let my_device_encryption_sk_string = encryption_secret_key_to_string(my_device_encryption_sk.clone());
        let my_device_sk_string = signature_secret_key_to_string(my_device_sk);
        let profile_encryption_sk_string = encryption_secret_key_to_string(profile_encryption_sk.clone());
        let profile_sk_string = signature_secret_key_to_string(profile_sk);

        let sender_profile = "sender_profile".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();
        let registration_name = "test_registration".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::initial_registration_with_no_code_for_device(
            my_device_encryption_sk_string.clone(),
            my_device_sk_string.clone(),
            profile_encryption_sk_string.clone(),
            profile_sk_string.clone(),
            registration_name.clone(),
            sender_profile.clone(),
            receiver_node.clone(),
            receiver_node.clone(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!(
                "initial_registration_with_no_code_for_device() returned an error: {:?}",
                e
            );
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Deserialize the body and check its content
        let content = message.get_message_content().unwrap();

        let registration_code: RegistrationCode = serde_json::from_str(&content).unwrap();
        assert_eq!(registration_code.registration_name, registration_name);
        assert_eq!(registration_code.identity_type, "device");
        assert_eq!(registration_code.permission_type, "admin");
        let encryption_pk_string =
            convert_encryption_sk_string_to_encryption_pk_string(my_device_encryption_sk_string.clone()).unwrap();
        let profile_encryption_pk_string =
            convert_encryption_sk_string_to_encryption_pk_string(profile_encryption_sk_string.clone()).unwrap();
        assert_eq!(registration_code.device_encryption_pk, encryption_pk_string);
        assert_eq!(registration_code.profile_encryption_pk, profile_encryption_pk_string);

        // Check internal metadata
        assert_eq!(message.get_sender_subidentity().unwrap(), sender_profile);
        assert_eq!(message.get_recipient_subidentity().unwrap(), "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata;
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
            receiver_node.clone(),
            sender_profile.clone(),
            receiver_node.clone(),
            "".to_string(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!("request_code_registration() returned an error: {:?}", e);
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        let registration_code_request: RegistrationCodeRequest = serde_json::from_str(&content).unwrap();
        assert_eq!(registration_code_request.permissions.to_string(), permissions);
        assert_eq!(registration_code_request.code_type.to_string(), code_type);

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_profile);
        assert_eq!(decrypted_message.get_recipient_subidentity().unwrap(), "".to_string());

        // Check external metadata
        let external_metadata = decrypted_message.external_metadata;
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
        let sender_subidentity = "main".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::terminate_message(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            sender_node.clone(),
            sender_subidentity.clone(),
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
        let content = message.get_message_content().unwrap();
        assert_eq!(content, "terminate");

        // Check internal metadata
        assert_eq!(message.get_recipient_subidentity().unwrap(), "".to_string());
        assert_eq!(message.get_sender_subidentity().unwrap(), "".to_string());
        assert_eq!(message.get_sender_intra_sender(), sender_subidentity.to_string());

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender_node);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_error_message_creation() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_node = "@@sender_node.shinkai".to_string();
        let sender_subidentity = "main".to_string();
        let receiver_node = "@@receiver_node.shinkai".to_string();

        let error_msg = "Some error occurred.".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::error_message(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            sender_node.clone(),
            sender_subidentity.clone(),
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

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        let content = decrypted_message.get_message_content().unwrap();
        assert_eq!(content, format!("{{error: \"{}\"}}", error_msg));

        log::debug!("decrypted_message: {:?}", decrypted_message);

        // Check internal metadata
        assert_eq!(message.get_recipient_subidentity(), None);
        assert_eq!(message.get_sender_subidentity(), None);

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender_node);
        assert_eq!(external_metadata.recipient, receiver_node);
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_get_last_messages_from_inbox() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_profile_name = "sender_profile".to_string();
        let receiver = "@@receiver_node.shinkai".to_string();
        let inbox = "inbox::@@node.shinkai::true".to_string();
        let count = 10;
        let offset = Some("offset_string".to_string());

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::get_last_messages_from_inbox(
            my_encryption_sk_string,
            my_identity_sk_string,
            receiver_public_key_string,
            inbox.clone(),
            count,
            offset,
            receiver.clone(),
            sender_profile_name.clone(),
            receiver.clone(),
            "".to_string(),
        );

        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Check the content
        let inbox_name_wrapper = InboxNameWrapper::new(&JsValue::from_str(&inbox)).unwrap();
        assert_eq!(inbox_name_wrapper.get_value(), "inbox::@@node.shinkai::true");
        assert_eq!(inbox_name_wrapper.get_is_e2e(), true);
        assert_eq!(
            serde_wasm_bindgen::from_value::<Vec<String>>(inbox_name_wrapper.get_identities().unwrap()).unwrap(),
            vec!["@@node.shinkai"]
        );
        assert_eq!(content["count"], 10);
        assert_eq!(content["offset"], "offset_string");

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_profile_name);
        assert_eq!(decrypted_message.get_recipient_subidentity().unwrap(), "".to_string());

        // Check external metadata
        let external_metadata = decrypted_message.external_metadata;
        assert_eq!(external_metadata.sender, receiver.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_get_last_unread_messages_from_inbox() {
        console_log::init_with_level(log::Level::Debug).expect("error initializing log");
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_subidentity_profile_name = "sender_profile".to_string();
        let receiver = "@@receiver_node.shinkai".to_string();
        let inbox = "inbox::@@node.shinkai::true".to_string();
        let count = 10;
        let offset = Some("offset_string".to_string());

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::get_last_unread_messages_from_inbox(
            my_encryption_sk_string,
            my_identity_sk_string,
            receiver_public_key_string,
            inbox,
            count,
            offset,
            receiver.clone(),
            sender_subidentity_profile_name.clone(),
            receiver.clone(),
            "".to_string(),
        );

        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();
        log::debug!("content: {}", content);

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();
        log::debug!("new content: {}", content);

        let inbox_name_js = serde_wasm_bindgen::to_value(&content["inbox"]).unwrap();
        let inbox_name_wrapper = InboxNameWrapper::new(&inbox_name_js).unwrap();
        log::debug!("new inbox_name: {:?}", inbox_name_wrapper);

        // Check the content
        // assert_eq!(content["inbox"], "inbox::@@node.shinkai::true");
        // assert_eq!(content["is_e2e"], true);
        // assert_eq!(content["identities"][0], "@@node.shinkai");
        // assert_eq!(content["identities"][0]["node_name"], "@@node.shinkai");
        // assert_eq!(content["count"], 10);
        // assert_eq!(content["offset"], "offset_string");
        assert_eq!(
            inbox_name_wrapper.get_value(),
            JsValue::from_str("inbox::@@node.shinkai::true")
        );
        assert_eq!(inbox_name_wrapper.get_is_e2e(), true);
        let identities_js = inbox_name_wrapper.get_identities().unwrap();
        let identities: Vec<String> = serde_wasm_bindgen::from_value(identities_js).unwrap();
        assert_eq!(identities, vec!["@@node.shinkai"]);
        assert_eq!(content["count"], 10);
        assert_eq!(content["offset"], "offset_string");

        // Check internal metadata
        assert_eq!(
            decrypted_message.get_sender_subidentity().unwrap(),
            sender_subidentity_profile_name
        );
        assert_eq!(decrypted_message.get_recipient_subidentity().unwrap(), "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, receiver.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_read_up_to_time() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let sender_profile_name = "sender_profile".to_string();
        let receiver = "@@receiver_node.shinkai".to_string();
        let inbox = "inbox::@@node.shinkai::true".to_string();
        let up_to_time = "2023-07-02T20:53:34Z".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::read_up_to_time(
            my_encryption_sk_string,
            my_identity_sk_string,
            receiver_public_key_string,
            inbox,
            up_to_time,
            receiver.clone(),
            sender_profile_name.clone(),
            receiver.clone(),
            "".to_string(),
        );

        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_profile_name);
        assert_eq!(decrypted_message.get_recipient_subidentity().unwrap(), "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, receiver.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Check the content
        assert_eq!(
            content["inbox_name"]["RegularInbox"]["value"],
            "inbox::@@node.shinkai::true"
        );
        assert_eq!(content["inbox_name"]["RegularInbox"]["is_e2e"], true);
        assert_eq!(content["inbox_name"]["RegularInbox"]["identities"][0], "@@node.shinkai");
        assert_eq!(content["inbox_name"]["RegularInbox"]["identities"][0], "@@node.shinkai");
        assert_eq!(content["up_to_time"], "2023-07-02T20:53:34Z");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_vecfs_create_folder() {
        // Initialize test data

        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let folder_name = "NewFolder".to_string();
        let path = "/root/path/".to_string();
        let sender = ShinkaiNameString::from("@@node.shinkai");
        let sender_subidentity = "main".to_string();
        let receiver = ShinkaiNameString::from("@@node.shinkai");
        let receiver_subidentity = "".to_string();

        // Call the function and check the result
        let result = ShinkaiMessageBuilderWrapper::vecfs_create_folder(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            folder_name.clone(),
            path.clone(),
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &result {
            eprintln!("Error occurred: {:?}", e);
            panic!("vecfs_create_folder() returned an error: {:?}", e);
        }
        assert!(result.is_ok());

        let message_result_string = result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_subidentity);
        assert_eq!(decrypted_message.get_recipient_subidentity().unwrap(), "".to_string());

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, receiver.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // log::debug!("decrypted_message: {:?}", content);

        // Further assertions based on the expected structure of the result
        assert_eq!(content["folder_name"], "NewFolder");
        assert_eq!(content["path"], "/root/path/");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_vecfs_move_folder() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let origin_path = "/root/origin/".to_string();
        let destination_path = "/root/destination/".to_string();
        let sender = ShinkaiNameString::from("@@sender_node.shinkai");
        let sender_subidentity = "main".to_string();
        let receiver = ShinkaiNameString::from("@@receiver_node.shinkai");
        let receiver_subidentity = "main".to_string();

        // Call the function and check the result
        let result = ShinkaiMessageBuilderWrapper::vecfs_move_folder(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            origin_path.clone(),
            destination_path.clone(),
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &result {
            eprintln!("Error occurred: {:?}", e);
            panic!("vecfs_move_folder() returned an error: {:?}", e);
        }
        assert!(result.is_ok());

        let message_result_string = result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_subidentity);
        assert_eq!(
            decrypted_message.get_recipient_subidentity().unwrap(),
            receiver_subidentity
        );

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Further assertions based on the expected structure of the result
        assert_eq!(content["origin_path"], "/root/origin/");
        assert_eq!(content["destination_path"], "/root/destination/");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_vecfs_copy_folder() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let origin_path = "/root/origin/".to_string();
        let destination_path = "/root/destination/".to_string();
        let sender = ShinkaiNameString::from("@@sender_node.shinkai");
        let sender_subidentity = "main".to_string();
        let receiver = ShinkaiNameString::from("@@receiver_node.shinkai");
        let receiver_subidentity = "main".to_string();

        // Call the function and check the result
        let result = ShinkaiMessageBuilderWrapper::vecfs_copy_folder(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            origin_path.clone(),
            destination_path.clone(),
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &result {
            eprintln!("Error occurred: {:?}", e);
            panic!("vecfs_copy_folder() returned an error: {:?}", e);
        }
        assert!(result.is_ok());

        let message_result_string = result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_subidentity);
        assert_eq!(
            decrypted_message.get_recipient_subidentity().unwrap(),
            receiver_subidentity
        );

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Further assertions based on the expected structure of the result
        assert_eq!(content["origin_path"], "/root/origin/");
        assert_eq!(content["destination_path"], "/root/destination/");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_vecfs_move_item() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let origin_path = "/root/origin/item".to_string();
        let destination_path = "/root/destination/item".to_string();
        let sender = ShinkaiNameString::from("@@sender_node.shinkai");
        let sender_subidentity = "main".to_string();
        let receiver = ShinkaiNameString::from("@@receiver_node.shinkai");
        let receiver_subidentity = "main".to_string();

        // Call the function and check the result
        let result = ShinkaiMessageBuilderWrapper::vecfs_move_item(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            origin_path.clone(),
            destination_path.clone(),
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &result {
            eprintln!("Error occurred: {:?}", e);
            panic!("vecfs_move_item() returned an error: {:?}", e);
        }
        assert!(result.is_ok());

        let message_result_string = result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_subidentity);
        assert_eq!(
            decrypted_message.get_recipient_subidentity().unwrap(),
            receiver_subidentity
        );

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Further assertions based on the expected structure of the result
        assert_eq!(content["origin_path"], "/root/origin/item");
        assert_eq!(content["destination_path"], "/root/destination/item");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_vecfs_copy_item() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let origin_path = "/root/origin/item".to_string();
        let destination_path = "/root/destination/item".to_string();
        let sender = ShinkaiNameString::from("@@sender_node.shinkai");
        let sender_subidentity = "main".to_string();
        let receiver = ShinkaiNameString::from("@@receiver_node.shinkai");
        let receiver_subidentity = "main".to_string(); // Updated as per instruction

        // Call the function and check the result
        let result = ShinkaiMessageBuilderWrapper::vecfs_copy_item(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            origin_path.clone(),
            destination_path.clone(),
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &result {
            eprintln!("Error occurred: {:?}", e);
            panic!("vecfs_copy_item() returned an error: {:?}", e);
        }
        assert!(result.is_ok());

        let message_result_string = result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_subidentity);
        assert_eq!(
            decrypted_message.get_recipient_subidentity().unwrap(),
            receiver_subidentity
        );

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Further assertions based on the expected structure of the result
        assert_eq!(content["origin_path"], "/root/origin/item");
        assert_eq!(content["destination_path"], "/root/destination/item");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_vecfs_create_items() {
        // console_log::init_with_level(log::Level::Debug).expect("error initializing log");

        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let destination_path = "/root/destination/".to_string();
        let file_inbox = "inbox_for_files".to_string();
        let sender = ShinkaiNameString::from("@@sender_node.shinkai");
        let sender_subidentity = "main".to_string();
        let receiver = ShinkaiNameString::from("@@receiver_node.shinkai");
        let receiver_subidentity = "main".to_string(); // Updated as per instruction
        let datetime = "2023-07-02T20:53:34Z".to_string();

        // Call the function and check the result
        let result = ShinkaiMessageBuilderWrapper::vecfs_create_items(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            destination_path.clone(),
            file_inbox.clone(),
            Some(datetime.clone()),
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &result {
            eprintln!("Error occurred: {:?}", e);
            panic!("vecfs_create_items() returned an error: {:?}", e);
        }
        assert!(result.is_ok());

        let message_result_string = result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_subidentity);
        assert_eq!(
            decrypted_message.get_recipient_subidentity().unwrap(),
            receiver_subidentity
        );

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // log::debug!("decrypted_message: {:?}", content);

        // Further assertions based on the expected structure of the result
        assert_eq!(content["path"], "/root/destination/");
        assert_eq!(content["file_inbox"], "inbox_for_files");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_vecfs_retrieve_resource() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let path = "/root/resource/path".to_string();
        let sender = ShinkaiNameString::from("@@sender_node.shinkai");
        let sender_subidentity = "main".to_string();
        let receiver = ShinkaiNameString::from("@@receiver_node.shinkai");
        let receiver_subidentity = "main".to_string(); // Updated as per instruction

        // Call the function and check the result
        let result = ShinkaiMessageBuilderWrapper::vecfs_retrieve_resource(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            path.clone(),
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &result {
            eprintln!("Error occurred: {:?}", e);
            panic!("vecfs_retrieve_resource() returned an error: {:?}", e);
        }
        assert!(result.is_ok());

        let message_result_string = result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_subidentity);
        assert_eq!(
            decrypted_message.get_recipient_subidentity().unwrap(),
            receiver_subidentity
        );

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Further assertions based on the expected structure of the result
        assert_eq!(content["path"], "/root/resource/path");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_vecfs_retrieve_path_simplified() {
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let path = "/root/simplified/path".to_string();
        let sender = ShinkaiNameString::from("@@sender_node.shinkai");
        let sender_subidentity = "main".to_string();
        let receiver = ShinkaiNameString::from("@@receiver_node.shinkai");
        let receiver_subidentity = "main".to_string(); // Updated as per instruction

        // Call the function and check the result
        let result = ShinkaiMessageBuilderWrapper::vecfs_retrieve_path_simplified(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            path.clone(),
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &result {
            eprintln!("Error occurred: {:?}", e);
            panic!("vecfs_retrieve_path_simplified() returned an error: {:?}", e);
        }
        assert!(result.is_ok());

        let message_result_string = result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_subidentity);
        assert_eq!(
            decrypted_message.get_recipient_subidentity().unwrap(),
            receiver_subidentity
        );

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Further assertions based on the expected structure of the result
        assert_eq!(content["path"], "/root/simplified/path");
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_vecfs_retrieve_vector_search_simplified() {
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let search = "keyword".to_string();
        let path = Some("/root/search/path".to_string());
        let max_results = Some(10);
        let max_files_to_scan = Some(100);
        let sender = ShinkaiNameString::from("@@sender_node.shinkai");
        let sender_subidentity = "main".to_string();
        let receiver = ShinkaiNameString::from("@@receiver_node.shinkai");
        let receiver_subidentity = "main".to_string(); // Updated as per instruction

        let result = ShinkaiMessageBuilderWrapper::vecfs_retrieve_vector_search_simplified(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            search.clone(),
            path.clone(),
            max_results.clone(),
            max_files_to_scan.clone(),
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &result {
            eprintln!("Error occurred: {:?}", e);
            panic!("vecfs_retrieve_vector_search_simplified() returned an error: {:?}", e);
        }
        assert!(result.is_ok());

        let message_result_string = result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Decrypt the content
        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &receiver_public_key)
            .expect("Failed to decrypt body content");

        // Deserialize the body and check its content
        let content = decrypted_message.get_message_content().unwrap();

        // Check internal metadata
        assert_eq!(decrypted_message.get_sender_subidentity().unwrap(), sender_subidentity);
        assert_eq!(
            decrypted_message.get_recipient_subidentity().unwrap(),
            receiver_subidentity
        );

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());

        // Deserialize the content into a JSON object
        let content: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Further assertions based on the expected structure of the result
        assert_eq!(content["search"], search);
        if let Some(p) = &path {
            assert_eq!(content["path"], *p);
        }
        assert_eq!(content["max_results"].as_i64(), max_results.map(|m| m as i64));
        assert_eq!(
            content["max_files_to_scan"].as_i64(),
            max_files_to_scan.map(|m| m as i64)
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_job_creation() {
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk.clone());
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let scope = JobScope::new_default();
        let scope_jsvalue = serde_wasm_bindgen::to_value(&scope).unwrap();
        let associated_ui = serde_wasm_bindgen::to_value(&AssociatedUI::Sheet("sheet_id".to_string())).unwrap();
        let sender = ShinkaiNameString::from("@@sender_node.shinkai");
        let sender_subidentity = ShinkaiNameString::from("main");
        let receiver = ShinkaiNameString::from("@@receiver_node.shinkai");
        let receiver_subidentity = "receiver_subidentity".to_string();

        // Call the function and check the result
        let message_result = ShinkaiMessageBuilderWrapper::job_creation(
            my_encryption_sk_string.clone(),
            my_identity_sk_string.clone(),
            receiver_public_key_string.clone(),
            scope_jsvalue,
            false,
            associated_ui,
            sender.clone(),
            sender_subidentity.clone(),
            receiver.clone(),
            receiver_subidentity.clone(),
        );

        if let Err(e) = &message_result {
            eprintln!("Error occurred: {:?}", e);
            panic!("job_creation() returned an error: {:?}", e);
        }
        assert!(message_result.is_ok());

        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Deserialize the body and check its content
        let body = match message.body {
            MessageBody::Unencrypted(body) => body,
            _ => panic!("Unexpected MessageBody variant"),
        };

        let job_creation_info: JobCreationInfo = match body.message_data {
            MessageData::Unencrypted(data) => serde_json::from_str(&data.message_raw_content).unwrap(),
            _ => panic!("Unexpected MessageData variant"),
        };
        assert_eq!(job_creation_info.scope, scope);
        assert_eq!(job_creation_info.is_hidden, Some(false));
        assert_eq!(job_creation_info.associated_ui, Some(AssociatedUI::Sheet("sheet_id".to_string())));

        // Check internal metadata
        let internal_metadata = body.internal_metadata;
        assert_eq!(internal_metadata.sender_subidentity, sender_subidentity.to_string());
        assert_eq!(internal_metadata.recipient_subidentity, receiver_subidentity);

        // Check external metadata
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender.to_string());
        assert_eq!(external_metadata.recipient, receiver.to_string());
    }
}
