mod tests {

    use serde_json;
    use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;
    use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::JobMessage;
    use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::MessageSchemaType;
    use shinkai_message_primitives::shinkai_utils::encryption::unsafe_deterministic_encryption_keypair;
    use shinkai_message_primitives::shinkai_utils::encryption::EncryptionMethod;
    use shinkai_message_primitives::shinkai_utils::shinkai_message_builder::ShinkaiMessageBuilder;
    use shinkai_message_primitives::shinkai_utils::shinkai_path::ShinkaiPath;
    use shinkai_message_primitives::shinkai_utils::signatures::clone_signature_secret_key;
    use shinkai_message_primitives::shinkai_utils::signatures::unsafe_deterministic_signature_keypair;

    #[test]
    fn test_encode_decode_message() {
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

    #[test]
    fn test_serialize_deserialize_job_message() {
        // Create a sample JobMessage
        let job_message = JobMessage {
            job_id: "test_job_id".to_string(),
            content: "This is a test message".to_string(),
            parent: Some("parent_id".to_string()),
            sheet_job_data: Some("sheet_data".to_string()),
            callback: None,
            metadata: None,
            tool_key: Some("tool_key".to_string()),
            fs_files_paths: vec![],
            job_filenames: vec![],
        };

        // Serialize the JobMessage to a JSON string
        let serialized = serde_json::to_string(&job_message).expect("Failed to serialize JobMessage");

        // Deserialize the JSON string back to a JobMessage
        let deserialized: JobMessage = serde_json::from_str(&serialized).expect("Failed to deserialize JobMessage");

        // Assert that the original and deserialized JobMessages are the same
        assert_eq!(job_message, deserialized);
    }

    #[test]
    fn test_serialize_deserialize_job_message_with_files() {
        // Create a sample JobMessage with a ShinkaiPath in files
        let job_message = JobMessage {
            job_id: "test_job_id".to_string(),
            content: "This is a test message with files".to_string(),
            parent: Some("parent_id".to_string()),
            sheet_job_data: Some("sheet_data".to_string()),
            callback: None,
            metadata: None,
            tool_key: Some("tool_key".to_string()),
            fs_files_paths: vec![ShinkaiPath::new("/path/to/file")],
            job_filenames: vec!["file1.txt".to_string()],
        };

        // Serialize the JobMessage to a JSON string
        let serialized = serde_json::to_string(&job_message).expect("Failed to serialize JobMessage");

        // Deserialize the JSON string back to a JobMessage
        let deserialized: JobMessage = serde_json::from_str(&serialized).expect("Failed to deserialize JobMessage");

        // Assert that the original and deserialized JobMessages are the same
        assert_eq!(job_message, deserialized);
    }
}
