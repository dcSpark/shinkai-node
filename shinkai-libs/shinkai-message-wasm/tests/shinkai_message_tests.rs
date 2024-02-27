#[cfg(test)]
mod tests {
    use serde_json::json;
    use serde_json::to_value;
    use shinkai_message_primitives::shinkai_message::shinkai_message::ExternalMetadata;
    use shinkai_message_primitives::shinkai_message::shinkai_message::InternalMetadata;
    use shinkai_message_primitives::shinkai_message::shinkai_message::MessageBody;
    use shinkai_message_primitives::shinkai_message::shinkai_message::MessageData;
    use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiBody;
    use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiData;
    use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
    use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiVersion;
    use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
    use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
    use shinkai_message_primitives::shinkai_utils::encryption::encryption_public_key_to_string;
    use shinkai_message_primitives::shinkai_utils::encryption::encryption_secret_key_to_string;
    use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
    use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
    use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
    use shinkai_message_primitives::shinkai_utils::signatures::signature_secret_key_to_string;
    use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;
    use shinkai_message_wasm::shinkai_wasm_wrappers::wasm_shinkai_message::ShinkaiMessageMethods;
    use shinkai_message_wasm::shinkai_wasm_wrappers::wasm_shinkai_message::SerdeWasmMethods;
    use shinkai_message_wasm::shinkai_wasm_wrappers::wasm_shinkai_message::ShinkaiBodyMethods;
    use shinkai_message_wasm::shinkai_wasm_wrappers::wasm_shinkai_message::InternalMetadataMethods;
    use shinkai_message_wasm::shinkai_wasm_wrappers::wasm_shinkai_message::ExternalMetadataMethods;
    use shinkai_message_wasm::ShinkaiMessageBuilderWrapper;
    use shinkai_message_wasm::ShinkaiMessageWrapper;
    use wasm_bindgen_test::*;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_wasm_serde_encode_decode_message_with_decode_message_result() {
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

        let message_wrapper = message_result.unwrap();
        let message_json = message_wrapper.to_json_str();

        // Convert the JSON string to bytes
        let message_bytes = message_json.into_bytes();

        // Deserialize the JSON string back to a ShinkaiMessage using decode_message_result
        let deserialized_message = ShinkaiMessage::decode_message_result(message_bytes).unwrap();

        // Check if the original and deserialized messages are the same
        assert_eq!(
            message_wrapper.calculate_blake3_hash(),
            deserialized_message.calculate_message_hash_for_pagination()
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_shinkai_message_to_jsvalue() {
        let internal_metadata = InternalMetadata::new(
            String::from("test_sender_subidentity"),
            String::from("test_recipient_subidentity"),
            String::from("part1|part2::part3|part4::part5|part6::true"),
            String::from("None"),
            String::from("test_signature"),
        )
        .unwrap();

        let message_data = MessageData::Unencrypted(ShinkaiData {
            message_raw_content: "test_content".into(),
            message_content_schema: MessageSchemaType::TextContent,
        });

        let body = MessageBody::Unencrypted(ShinkaiBody::new(message_data, internal_metadata));

        let external_metadata = ExternalMetadata::new(
            String::from("test_sender"),
            String::from("test_recipient"),
            String::from("2023-07-02T20:53:34Z"),
            String::from("test_signature"),
            String::from("test_other"),
            String::from("intra_sender"),
        );
        let message = ShinkaiMessage::new(
            body,
            external_metadata,
            EncryptionMethod::DiffieHellmanChaChaPoly1305,
            None,
        );
        let js_value_result = message.to_jsvalue();

        // Check if the result is Ok, if not fail the test.
        assert!(js_value_result.is_ok());

        // Unwrap the result.
        let js_value = js_value_result.unwrap();
        let js_value_serde: serde_json::Value = serde_wasm_bindgen::from_value(js_value).unwrap();

        assert_eq!(
            js_value_serde,
            json!({
                "body": {
                    "unencrypted": {
                        "internal_metadata": {
                            "sender_subidentity": "test_sender_subidentity",
                            "recipient_subidentity": "test_recipient_subidentity",
                            "inbox": "part1|part2::part3|part4::part5|part6::true",
                            "encryption": "None",
                            "signature": "test_signature"
                        },
                        "message_data": {
                            "unencrypted": {
                                "message_content_schema": "TextContent",
                                "message_raw_content": "test_content"
                            }
                        }
                    }
                },
                "external_metadata": {
                    "sender": "test_sender",
                    "recipient": "test_recipient",
                    "scheduled_time": "2023-07-02T20:53:34Z",
                    "signature": "test_signature",
                    "other": "test_other",
                    "intra_sender": "intra_sender"
                },
                "encryption": "DiffieHellmanChaChaPoly1305",
                "version": "V1_0"
            })
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_shinkai_message_from_jsvalue() {
        console_log::init_with_level(log::Level::Debug).expect("error initializing log");
        let json = r#"{
            "body": {
                "unencrypted": {
                    "internal_metadata": {
                        "sender_subidentity": "test_sender_subidentity",
                        "recipient_subidentity": "test_recipient_subidentity",
                        "inbox": "part1|part2::part3|part4::part5|part6::true",
                        "encryption": "None",
                        "signature": "test_signature"
                    },
                    "message_data": {
                        "unencrypted": {
                            "message_content_schema": "TextContent",
                            "message_raw_content": "test_content"
                        }
                    }
                }
            },
            "external_metadata": {
                "sender": "test_sender",
                "recipient": "test_recipient",
                "scheduled_time": "2023-07-02T20:53:34Z",
                "signature": "test_signature",
                "other": "test_other",
                "intra_sender": "intra_sender"
            },
            "encryption": "DiffieHellmanChaChaPoly1305",
            "version": "V1_0" 
        }"#;

        let shinkai_message = ShinkaiMessage::from_json_str(json).unwrap();
        let js_value = serde_wasm_bindgen::to_value(&shinkai_message).unwrap();
        let message_result = ShinkaiMessage::from_jsvalue(&js_value);

        // Check if the result is Ok, if not fail the test.
        assert!(message_result.is_ok());

        // Unwrap the result.
        let message = message_result.unwrap();

        assert_eq!(message.encryption, EncryptionMethod::DiffieHellmanChaChaPoly1305);
        assert_eq!(message.version, ShinkaiVersion::V1_0);
        let body = match message.body {
            MessageBody::Unencrypted(body) => body,
            _ => panic!("Unexpected MessageBody variant"),
        };

        let message_data = match body.message_data {
            MessageData::Unencrypted(ref data) => data,
            _ => panic!("Unexpected MessageData variant"),
        };

        assert_eq!(message_data.message_raw_content, String::from("test_content"));

        let internal_metadata = body.internal_metadata;
        assert_eq!(
            internal_metadata.sender_subidentity,
            String::from("test_sender_subidentity")
        );
        let message_data = match body.message_data {
            MessageData::Unencrypted(ref data) => data,
            _ => panic!("Unexpected MessageData variant"),
        };

        assert_eq!(message_data.message_content_schema, MessageSchemaType::TextContent);
        assert_eq!(
            internal_metadata.inbox,
            String::from("part1|part2::part3|part4::part5|part6::true")
        );
        assert_eq!(internal_metadata.encryption, EncryptionMethod::None);
        assert_eq!(internal_metadata.signature, String::from("test_signature"));

        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, String::from("test_sender"));
        assert_eq!(external_metadata.recipient, String::from("test_recipient"));
        assert_eq!(external_metadata.scheduled_time, String::from("2023-07-02T20:53:34Z"));
        assert_eq!(external_metadata.signature, String::from("test_signature"));
        assert_eq!(external_metadata.other, String::from("test_other"));
        assert_eq!(external_metadata.intra_sender, String::from("intra_sender"));
    }
}
