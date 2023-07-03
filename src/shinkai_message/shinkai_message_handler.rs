// shinkai_message.rs

use crate::shinkai_message_proto::ShinkaiMessage;
use chrono::Utc;
use prost::Message;
use sha2::{Digest, Sha256};

pub struct ShinkaiMessageHandler;

impl ShinkaiMessageHandler {
    pub fn encode_message(message: ShinkaiMessage) -> Vec<u8> {
        let mut bytes = Vec::new();
        message.encode(&mut bytes).unwrap();
        bytes
    }

    pub fn decode_message(bytes: Vec<u8>) -> Result<ShinkaiMessage, prost::DecodeError> {
        ShinkaiMessage::decode(bytes.as_slice())
    }

    pub fn generate_time_now() -> String {
        let timestamp = Utc::now().format("%Y%m%dT%H%M%S%f").to_string();
        let scheduled_time = format!("{}{}", &timestamp[..17], &timestamp[17..20]);
        scheduled_time
    }

    pub fn calculate_hash(message: &ShinkaiMessage) -> String {
        let mut hasher = Sha256::new();

        hasher.update(format!("{:?}", message));
        let result = hasher.finalize();
        format!("{:x}", result)
    }
}

#[cfg(test)]
mod tests {
    use x25519_dalek::{PublicKey, StaticSecret};

    use crate::shinkai_message::encryption::{
        public_key_to_string, string_to_static_key, EncryptionMethod,
    };
    use crate::shinkai_message::{
        shinkai_message_builder::ShinkaiMessageBuilder,
        shinkai_message_handler::ShinkaiMessageHandler,
    };
    use crate::shinkai_message_proto::{Field, ShinkaiMessage};

    const SECRET_KEYS: [&str; 3] = [
        "yMA8duhbady14IzHUXyz4m9ZeX423UHxvfEFFRCFK04=",
        "GGyELi2jbj7K30kZoAgU13jJ445Z+Ua3hEgwOKeXE0s",
        "UEbkn/SV8f1DaBRs9gw44rFkGRFYwGn5fHHSeg0vVFY=",
    ];

    fn deterministic_keys(secret_string: String) -> (StaticSecret, PublicKey) {
        let secret_key = string_to_static_key(&secret_string).unwrap();
        let public_key = PublicKey::from(&secret_key);
        (secret_key, public_key)
    }

    fn build_message(
        my_secret_key: StaticSecret,
        receiver_public_key: PublicKey,
        encryption: String,
    ) -> ShinkaiMessage {
        let fields = vec![
            Field {
                name: "field1".to_string(),
                field_type: "type1".to_string(),
            },
            Field {
                name: "field2".to_string(),
                field_type: "type2".to_string(),
            },
        ];

        let message_result =
            ShinkaiMessageBuilder::new(my_secret_key.clone(), receiver_public_key.clone())
                .body("Hello World".to_string())
                .encryption(encryption.to_string())
                .message_schema_type("MyType".to_string(), fields)
                .topic("my_topic".to_string(), "my_channel".to_string())
                .internal_metadata_content("InternalContent".to_string())
                .external_metadata_with_schedule(receiver_public_key, "20230702T20533481345".to_string())
                .build();

        return message_result.unwrap();
    }

    #[test]
    fn test_encode_message() {
        let (my_secret_key, _) = deterministic_keys(SECRET_KEYS[0].to_owned());
        let (_, receiver_public_key) = deterministic_keys(SECRET_KEYS[1].to_owned());
        let message = build_message(
            my_secret_key,
            receiver_public_key,
            EncryptionMethod::None.as_str().to_owned(),
        );
        let encoded_message = ShinkaiMessageHandler::encode_message(message);
        assert!(encoded_message.len() > 0);
    }

    #[test]
    fn test_encode_message_with_encryption() {
        let (my_secret_key, _) = deterministic_keys(SECRET_KEYS[0].to_owned());
        let (_, receiver_public_key) = deterministic_keys(SECRET_KEYS[1].to_owned());
        let message = build_message(
            my_secret_key,
            receiver_public_key,
            EncryptionMethod::DiffieHellmanChaChaPoly1305
                .as_str()
                .to_owned(),
        );
        let encoded_message = ShinkaiMessageHandler::encode_message(message);
        assert!(encoded_message.len() > 0);
    }

    #[test]
    fn test_decode_message() {
        let (my_secret_key, my_public_key) = deterministic_keys(SECRET_KEYS[0].to_owned());
        let (_, receiver_public_key) = deterministic_keys(SECRET_KEYS[1].to_owned());
        let message = build_message(
            my_secret_key,
            receiver_public_key,
            EncryptionMethod::None.as_str().to_owned(),
        );
        let encoded_message = ShinkaiMessageHandler::encode_message(message);
        let decoded_message = ShinkaiMessageHandler::decode_message(encoded_message).unwrap();

        // Assert that the decoded message is the same as the original message
        let body = decoded_message.body.as_ref().unwrap();
        assert_eq!(body.content, "Hello World");

        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "InternalContent");
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .type_name,
            "MyType"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .name,
            "field1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .field_type,
            "type1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .name,
            "field2"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .field_type,
            "type2"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().topic_id,
            "my_topic"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().channel_id,
            "my_channel"
        );

        assert_eq!(decoded_message.encryption, "None");

        let external_metadata = decoded_message.external_metadata.as_ref().unwrap();
        assert_eq!(
            external_metadata.sender,
            public_key_to_string(my_public_key.to_owned())
        );
        assert_eq!(
            external_metadata.recipient,
            public_key_to_string(receiver_public_key.to_owned())
        );
        assert_eq!(external_metadata.scheduled_time, "20230702T20533481345");
        assert_eq!(external_metadata.signature, "");
    }

    #[test]
    fn test_decode_encrypted_message() {
        let (my_secret_key, my_public_key) = deterministic_keys(SECRET_KEYS[0].to_owned());
        let (_, receiver_public_key) = deterministic_keys(SECRET_KEYS[1].to_owned());
        let message = build_message(
            my_secret_key,
            receiver_public_key,
            EncryptionMethod::None.as_str().to_owned(),
        );
        let encoded_message = ShinkaiMessageHandler::encode_message(message);
        let decoded_message = ShinkaiMessageHandler::decode_message(encoded_message).unwrap();

        // Assert that the decoded message is the same as the original message
        let body = decoded_message.body.as_ref().unwrap();
        assert_eq!(body.content, "Hello World");

        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "InternalContent");
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .type_name,
            "MyType"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .name,
            "field1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[0]
                .field_type,
            "type1"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .name,
            "field2"
        );
        assert_eq!(
            internal_metadata
                .message_schema_type
                .as_ref()
                .unwrap()
                .fields[1]
                .field_type,
            "type2"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().topic_id,
            "my_topic"
        );
        assert_eq!(
            internal_metadata.topic.as_ref().unwrap().channel_id,
            "my_channel"
        );

        assert_eq!(decoded_message.encryption, "None");

        let external_metadata = decoded_message.external_metadata.as_ref().unwrap();
        assert_eq!(
            external_metadata.sender,
            public_key_to_string(my_public_key.to_owned())
        );
        assert_eq!(
            external_metadata.recipient,
            public_key_to_string(receiver_public_key.to_owned())
        );
        assert_eq!(external_metadata.scheduled_time, "20230702T20533481345");
        assert_eq!(external_metadata.signature, "");
    }
}
