use wasm_bindgen_test::*;

#[cfg(test)]
mod tests {
    use js_sys::Uint8Array;
    use serde_wasm_bindgen::from_value;
    use shinkai_message_wasm::schemas::inbox_name::InboxName;
    use shinkai_message_wasm::shinkai_message::shinkai_message::{Body, ExternalMetadata, ShinkaiMessage};
    use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::{JobMessage, JobScope};
    use shinkai_message_wasm::shinkai_utils::encryption::{
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
        let _ = builder.internal_metadata(sender_subidentity.clone().into(), recipient_subidentity.clone().into(), "".into(), "None".into());
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
        assert_eq!(internal_metadata.inbox, "");

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
        let _ = builder.internal_metadata(sender_subidentity.clone().into(), recipient_subidentity.clone().into(), "".into(), "None".into());
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
        assert_eq!(internal_metadata.inbox, "");

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
        console_log::init_with_level(log::Level::Debug).expect("error initializing log");
        // Initialize test data
        let (my_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, receiver_public_key) = unsafe_deterministic_encryption_keypair(1);

        let my_encryption_sk_string = encryption_secret_key_to_string(my_encryption_sk);
        let my_identity_sk_string = signature_secret_key_to_string(my_identity_sk);
        let receiver_public_key_string = encryption_public_key_to_string(receiver_public_key);

        let scope = JobScope {
            buckets: vec![InboxName::new("inbox::@@node1.shinkai|test_name::@@|::false".to_string()).unwrap()],
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
            panic!(
                "job_message() returned an error: {:?}\n\
                 my_encryption_sk_string: {}\n\
                 my_identity_sk_string: {}\n\
                 receiver_public_key_string: {}",
                e,
                my_encryption_sk_string,
                my_identity_sk_string,
                receiver_public_key_string
            );
        }
        assert!(message_result.is_ok());
        
        let message_result_string = message_result.unwrap();
        let message: ShinkaiMessage = ShinkaiMessage::from_json_str(&message_result_string).unwrap();

        // Deserialize the body and check its content
        let body = message.body.unwrap();

        let job_message: JobMessage = serde_json::from_str(&body.content).unwrap();
        assert_eq!(job_message.job_id, job_id);
        assert_eq!(job_message.content, content);

        // Check internal metadata
        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(internal_metadata.sender_subidentity, "".to_string());
        assert_eq!(internal_metadata.recipient_subidentity, node_receiver_subidentity);

        // Check external metadata
        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, node_sender);
        assert_eq!(external_metadata.recipient, node_receiver);
    }

        // #[wasm_bindgen_test]
    // fn test_builder_missing_fields() {
    //     // Setup code with keys goes here.

    //     let mut builder = ShinkaiMessageBuilderWrapper::new(/* Insert your keys here */);
    //     let message_result = builder.build();
    //     assert!(message_result.is_err());
    // }
}
