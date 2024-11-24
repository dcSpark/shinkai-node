mod tests {

    use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
    use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
    use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
    use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
    use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
    use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
    use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;

    #[test]
    fn test_encode_decode_message() {
        // Initialize the message
        let (my_encryption_secret_key, my_encryption_public_key) = unsafe_deterministic_encryption_keypair(0);
        let (my_signature_secret_key, my_signature_public_key) = unsafe_deterministic_signature_keypair(0);
        let receiver_public_key = my_encryption_public_key.clone();

        let message = ShinkaiMessageBuilder::new(
            my_encryption_secret_key.clone(),
            clone_signature_secret_key(&my_signature_secret_key),
            receiver_public_key,
        )
        .message_raw_content("Hello World".to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata_with_inbox(
            "".to_string(),
            "main_profile_node1".to_string(),
            "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false".to_string(),
            EncryptionMethod::None,
            None,
        )
        .external_metadata_with_schedule(
            "@@node1.shinkai".to_string(),
            "@@node1.shinkai".to_string(),
            "2023-07-02T20:53:34Z".to_string(),
        )
        .build()
        .unwrap();

        // Encode the message
        let encoded_message = message.encode_message().unwrap();

        // Decode the message
        let decoded_message = ShinkaiMessage::decode_message_result(encoded_message).unwrap();

        // Check if the original and decoded messages are the same
        assert_eq!(
            message.calculate_message_hash_for_pagination(),
            decoded_message.calculate_message_hash_for_pagination()
        );
    }

    #[test]
    fn test_serde_encode_decode_message() {
        // Initialize the message
        let (my_encryption_secret_key, my_encryption_public_key) = unsafe_deterministic_encryption_keypair(0);
        let (my_signature_secret_key, my_signature_public_key) = unsafe_deterministic_signature_keypair(0);
        let receiver_public_key = my_encryption_public_key.clone();

        let message = ShinkaiMessageBuilder::new(
            my_encryption_secret_key.clone(),
            clone_signature_secret_key(&my_signature_secret_key),
            receiver_public_key,
        )
        .message_raw_content("Hello World".to_string())
        .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata_with_inbox(
            "".to_string(),
            "main_profile_node1".to_string(),
            "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false".to_string(),
            EncryptionMethod::None,
            None,
        )
        .external_metadata_with_schedule(
            "@@node1.shinkai".to_string(),
            "@@node1.shinkai".to_string(),
            "2023-07-02T20:53:34Z".to_string(),
        )
        .build()
        .unwrap();

        // Serialize the message to a JSON string
        let serialized_message = serde_json::to_string(&message).unwrap();

        // Deserialize the JSON string back to a ShinkaiMessage
        let deserialized_message: ShinkaiMessage = serde_json::from_str(&serialized_message).unwrap();

        // Check if the original and deserialized messages are the same
        assert_eq!(
            message.calculate_message_hash_for_pagination(),
            deserialized_message.calculate_message_hash_for_pagination()
        );
    }

    #[test]
    fn test_serde_encode_decode_message_with_decode_message_result() {
        // Initialize the message
        let (my_encryption_secret_key, my_encryption_public_key) = unsafe_deterministic_encryption_keypair(0);
        let (my_signature_secret_key, _my_signature_public_key) = unsafe_deterministic_signature_keypair(0);
        let receiver_public_key = my_encryption_public_key.clone();

        let message = ShinkaiMessageBuilder::new(
            my_encryption_secret_key.clone(),
            clone_signature_secret_key(&my_signature_secret_key),
            receiver_public_key,
        )
        .message_raw_content("Hello World".to_string())
        .body_encryption(EncryptionMethod::None)
        .message_schema_type(MessageSchemaType::TextContent)
        .internal_metadata_with_inbox(
            "".to_string(),
            "main_profile_node1".to_string(),
            "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false".to_string(),
            EncryptionMethod::DiffieHellmanChaChaPoly1305,
            None,
        )
        .external_metadata_with_schedule(
            "@@node1.shinkai".to_string(),
            "@@node1.shinkai".to_string(),
            "2023-07-02T20:53:34Z".to_string(),
        )
        .build()
        .unwrap();

        // Serialize the message to a JSON string
        let serialized_message = serde_json::to_string(&message).unwrap();

        // Convert the JSON string to bytes
        let serialized_message_bytes = serialized_message.into_bytes();

        // Deserialize the JSON string back to a ShinkaiMessage using decode_message_result
        let deserialized_message = ShinkaiMessage::decode_message_result(serialized_message_bytes).unwrap();

        // Check if the original and deserialized messages are the same
        assert_eq!(
            message.calculate_message_hash_for_pagination(),
            deserialized_message.calculate_message_hash_for_pagination()
        );
    }
}
