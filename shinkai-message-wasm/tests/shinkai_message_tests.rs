#[cfg(test)]
mod tests {
    use serde_json::json;
    use serde_json::to_value;
    use shinkai_message_wasm::shinkai_message::shinkai_message::Body;
    use shinkai_message_wasm::shinkai_message::shinkai_message::ExternalMetadata;
    use shinkai_message_wasm::shinkai_message::shinkai_message::InternalMetadata;
    use shinkai_message_wasm::shinkai_message::shinkai_message::ShinkaiMessage;
    use shinkai_message_wasm::shinkai_message::shinkai_message_schemas::MessageSchemaType;
    use shinkai_message_wasm::shinkai_utils::encryption::EncryptionMethod;
    use wasm_bindgen_test::*;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_shinkai_message_to_jsvalue() {
        let internal_metadata = InternalMetadata::new(
            String::from("test_sender_subidentity"),
            String::from("test_recipient_subidentity"),
            String::from("TextContent"),
            String::from("part1|part2::part3|part4::part5|part6::true"),
            String::from("None"),
        )
        .unwrap();
        let body = Body::new(String::from("test_content"), Some(internal_metadata));
        let external_metadata = ExternalMetadata::new(
            String::from("test_sender"),
            String::from("test_recipient"),
            String::from("20230702T20533481345"),
            String::from("test_signature"),
            String::from("test_other"),
        );
        let message = ShinkaiMessage::new(
            Some(body),
            Some(external_metadata),
            EncryptionMethod::DiffieHellmanChaChaPoly1305,
        );
        let js_value_result = message.to_jsvalue();

        // Check if the result is Ok, if not fail the test.
        assert!(js_value_result.is_ok());

        // Unwrap the result.
        let js_value = js_value_result.unwrap();
        log::debug!("first test>> js_value: {:?}", js_value);
        let js_value_serde: serde_json::Value = serde_wasm_bindgen::from_value(js_value).unwrap();

        assert_eq!(
            js_value_serde,
            json!({
                "body": {
                    "content": "test_content",
                    "internal_metadata": {
                        "sender_subidentity": "test_sender_subidentity",
                        "recipient_subidentity": "test_recipient_subidentity",
                        "message_schema_type": "TextContent",
                        "inbox": "part1|part2::part3|part4::part5|part6::true",
                        "encryption": "None"
                    }
                },
                "external_metadata": {
                    "sender": "test_sender",
                    "recipient": "test_recipient",
                    "scheduled_time": "20230702T20533481345",
                    "signature": "test_signature",
                    "other": "test_other"
                },
                "encryption": "DiffieHellmanChaChaPoly1305"
            })
        );
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_shinkai_message_from_jsvalue() {
        console_log::init_with_level(log::Level::Debug).expect("error initializing log");
        let json = r#"{
            "body": {
                "content": "test_content",
                "internal_metadata": {
                    "sender_subidentity": "test_sender_subidentity",
                    "recipient_subidentity": "test_recipient_subidentity",
                    "message_schema_type": "TextContent",
                    "inbox": "part1|part2::part3|part4::part5|part6::true",
                    "encryption": "None"
                }
            },
            "external_metadata": {
                "sender": "test_sender",
                "recipient": "test_recipient",
                "scheduled_time": "20230702T20533481345",
                "signature": "test_signature",
                "other": "test_other"
            },
            "encryption": "DiffieHellmanChaChaPoly1305"
        }"#;

        let shinkai_message = ShinkaiMessage::from_json_str(json).unwrap();
        let js_value = serde_wasm_bindgen::to_value(&shinkai_message).unwrap();
        let message_result = ShinkaiMessage::from_jsvalue(&js_value);

        // Check if the result is Ok, if not fail the test.
        assert!(message_result.is_ok());

        // Unwrap the result.
        let message = message_result.unwrap();

        assert_eq!(message.encryption, EncryptionMethod::DiffieHellmanChaChaPoly1305);
        let body = message.body.unwrap();
        assert_eq!(body.content, String::from("test_content"));

        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(
            internal_metadata.sender_subidentity,
            String::from("test_sender_subidentity")
        );
        assert_eq!(
            internal_metadata.recipient_subidentity,
            String::from("test_recipient_subidentity")
        );
        assert_eq!(internal_metadata.message_schema_type, MessageSchemaType::TextContent);
        assert_eq!(internal_metadata.inbox, String::from("part1|part2::part3|part4::part5|part6::true"));
        assert_eq!(internal_metadata.encryption, EncryptionMethod::None);

        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, String::from("test_sender"));
        assert_eq!(external_metadata.recipient, String::from("test_recipient"));
        assert_eq!(external_metadata.scheduled_time, String::from("20230702T20533481345"));
        assert_eq!(external_metadata.signature, String::from("test_signature"));
        assert_eq!(external_metadata.other, String::from("test_other"));
    }
}
