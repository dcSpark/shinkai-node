use wasm_bindgen_test::*;

#[cfg(test)]
mod tests {
    use js_sys::Uint8Array;
    use serde_wasm_bindgen::from_value;
    use shinkai_message_wasm::shinkai_message::shinkai_message::{Body, ExternalMetadata, ShinkaiMessage};
    use shinkai_message_wasm::shinkai_utils::encryption::{
        encryption_public_key_to_jsvalue, encryption_secret_key_to_jsvalue, unsafe_deterministic_encryption_keypair,
        EncryptionMethod,
    };
    use shinkai_message_wasm::shinkai_utils::signatures::{
        signature_secret_key_to_jsvalue, unsafe_deterministic_signature_keypair, verify_signature,
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

        let my_encryption_sk_js = encryption_secret_key_to_jsvalue(&my_encryption_sk);
        let my_identity_sk_js = signature_secret_key_to_jsvalue(&my_identity_sk);
        let node2_encryption_pk_js = encryption_public_key_to_jsvalue(&node2_encryption_pk);

        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_sk_js, my_identity_sk_js, node2_encryption_pk_js).unwrap();

        let _ = builder.body("body content".into());
        let _ = builder.body_encryption("None".into());
        let _ = builder.message_schema_type("TextContent".into());
        let _ = builder.internal_metadata("".into(), "".into(), "".into(), "None".into());
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
        assert_eq!(internal_metadata.sender_subidentity, "");
        assert_eq!(internal_metadata.recipient_subidentity, "");
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

    // #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_jsvalue_builder_with_all_fields_no_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let my_encryption_sk_js = encryption_secret_key_to_jsvalue(&my_encryption_sk);
        let my_identity_sk_js = signature_secret_key_to_jsvalue(&my_identity_sk);
        let node2_encryption_pk_js = encryption_public_key_to_jsvalue(&node2_encryption_pk);

        let mut builder =
            ShinkaiMessageBuilderWrapper::new(my_encryption_sk_js, my_identity_sk_js, node2_encryption_pk_js).unwrap();

        let _ = builder.body("body content".into());
        let _ = builder.body_encryption("None".into());
        let _ = builder.message_schema_type("TextContent".into());
        let _ = builder.internal_metadata("".into(), "".into(), "".into(), "None".into());
        let _ = builder.external_metadata_with_schedule(
            recipient.clone().into(),
            sender.clone().into(),
            scheduled_time.clone().into(),
        );

        let message_result = builder.build_to_jsvalue();
        assert!(message_result.is_ok());

        let message_jsvalue = message_result.unwrap();
        let message: ShinkaiMessageWrapper = ShinkaiMessageWrapper::from_jsvalue(&message_jsvalue).unwrap();

        let body_jsvalue = message.body().unwrap();
        let body: Body = Body::from_jsvalue(&body_jsvalue).unwrap();
        let internal_metadata = body.internal_metadata.unwrap();
        let encryption = EncryptionMethod::from_str(&message.encryption().as_str().to_string());

        assert_eq!(body.content, "body content");
        assert_eq!(encryption, EncryptionMethod::None);
        assert_eq!(internal_metadata.sender_subidentity, "");
        assert_eq!(internal_metadata.recipient_subidentity, "");
        assert_eq!(internal_metadata.inbox, "");

        let external_metadata_jsvalue = message.external_metadata().unwrap();
        let external_metadata: ExternalMetadata = ExternalMetadata::from_jsvalue(&external_metadata_jsvalue).unwrap();

        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.scheduled_time, scheduled_time);
        assert_eq!(external_metadata.recipient, recipient);

        // Convert ShinkaiMessage back to JSON
        let message_clone_jsvalue = message.to_jsvalue().unwrap();
        let message_clone: ShinkaiMessageWrapper = ShinkaiMessageWrapper::from_jsvalue(&message_clone_jsvalue).unwrap();
    }

    // More tests, similar to the one above, go here.

    // #[wasm_bindgen_test]
    // fn test_builder_missing_fields() {
    //     // Setup code with keys goes here.

    //     let mut builder = ShinkaiMessageBuilderWrapper::new(/* Insert your keys here */);
    //     let message_result = builder.build();
    //     assert!(message_result.is_err());
    // }
}
