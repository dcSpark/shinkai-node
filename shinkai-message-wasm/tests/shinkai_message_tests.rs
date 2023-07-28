
#[cfg(test)]
mod tests {
    use shinkai_message_wasm::shinkai_message::shinkai_message::Body;
    use shinkai_message_wasm::shinkai_message::shinkai_message::ShinkaiMessage;
    use wasm_bindgen_test::*;
    use serde_json::json;
    use serde_json::to_value;
    use shinkai_message_wasm::shinkai_message::shinkai_message::InternalMetadata;
    use shinkai_message_wasm::shinkai_message::shinkai_message::ExternalMetadata;

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_shinkai_message_to_jsvalue() {
        let internal_metadata = InternalMetadata::new(
            String::from("test_sender_subidentity"),
            String::from("test_recipient_subidentity"),
            String::from("test_type"),
            String::from("test_inbox"),
            String::from("test_encryption")
        );
        let body = Body::new(String::from("test_content"), Some(internal_metadata));
        let external_metadata = ExternalMetadata::new(
            String::from("test_sender"),
            String::from("test_recipient"),
            String::from("test_time"),
            String::from("test_signature"),
            String::from("test_other")
        );
        let message = ShinkaiMessage::new(Some(body), Some(external_metadata), String::from("test_encryption"));
        let js_value = message.to_jsvalue();
    
        let js_value_serde: serde_json::Value = serde_wasm_bindgen::from_value(js_value).unwrap();
    
        assert_eq!(js_value_serde, json!({
            "body": {
                "content": "test_content",
                "internal_metadata": {
                    "sender_subidentity": "test_sender_subidentity",
                    "recipient_subidentity": "test_recipient_subidentity",
                    "message_schema_type": "test_type",
                    "inbox": "test_inbox",
                    "encryption": "test_encryption"
                }
            },
            "external_metadata": {
                "sender": "test_sender",
                "recipient": "test_recipient",
                "scheduled_time": "test_time",
                "signature": "test_signature",
                "other": "test_other"
            },
            "encryption": "test_encryption"
        }));
    }
    
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen_test]
    fn test_shinkai_message_from_jsvalue() {
        let json_value = json!({
            "body": {
                "content": "test_content",
                "internal_metadata": {
                    "sender_subidentity": "test_sender_subidentity",
                    "recipient_subidentity": "test_recipient_subidentity",
                    "message_schema_type": "test_type",
                    "inbox": "test_inbox",
                    "encryption": "test_encryption"
                }
            },
            "external_metadata": {
                "sender": "test_sender",
                "recipient": "test_recipient",
                "scheduled_time": "test_time",
                "signature": "test_signature",
                "other": "test_other"
            },
            "encryption": "test_encryption"
        });
    
        let js_value = serde_wasm_bindgen::to_value(&json_value).unwrap();
        let message = ShinkaiMessage::from_jsvalue(&js_value);
    
        assert_eq!(message.encryption, String::from("test_encryption"));
    
        let body = message.body.unwrap();
        assert_eq!(body.content, String::from("test_content"));
    
        let internal_metadata = body.internal_metadata.unwrap();
        assert_eq!(internal_metadata.sender_subidentity, String::from("test_sender_subidentity"));
        assert_eq!(internal_metadata.recipient_subidentity, String::from("test_recipient_subidentity"));
        assert_eq!(internal_metadata.message_schema_type, String::from("test_type"));
        assert_eq!(internal_metadata.inbox, String::from("test_inbox"));
        assert_eq!(internal_metadata.encryption, String::from("test_encryption"));
    
        let external_metadata = message.external_metadata.unwrap();
        assert_eq!(external_metadata.sender, String::from("test_sender"));
        assert_eq!(external_metadata.recipient, String::from("test_recipient"));
        assert_eq!(external_metadata.scheduled_time, String::from("test_time"));
        assert_eq!(external_metadata.signature, String::from("test_signature"));
        assert_eq!(external_metadata.other, String::from("test_other"));
    }
}
