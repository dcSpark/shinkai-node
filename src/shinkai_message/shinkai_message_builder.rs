use super::{encryption::public_key_to_string, shinkai_message_handler::ShinkaiMessageHandler};
#[allow(unused_imports)]
use super::encryption::{decrypt_body_content, encrypt_body_if_needed};
use crate::shinkai_message_proto::{
    Body, ExternalMetadata, Field, InternalMetadata, MessageSchemaType, ShinkaiMessage, Topic,
};
use chrono::Utc;
use x25519_dalek::{PublicKey, StaticSecret};

pub struct ShinkaiMessageBuilder {
    body: Option<Body>,
    message_schema_type: Option<MessageSchemaType>,
    topic: Option<Topic>,
    internal_metadata_content: Option<String>,
    external_metadata: Option<ExternalMetadata>,
    encryption: Option<String>,
    my_secret_key: StaticSecret,
    my_public_key: PublicKey,
    receiver_public_key: PublicKey,
}

impl ShinkaiMessageBuilder {
    pub fn new(my_secret_key: StaticSecret, receiver_public_key: PublicKey) -> Self {
        let my_public_key: PublicKey = PublicKey::from(&my_secret_key);
        Self {
            body: None,
            message_schema_type: None,
            topic: None,
            internal_metadata_content: None,
            external_metadata: None,
            encryption: None,
            my_secret_key,
            my_public_key,
            receiver_public_key,
        }
    }

    pub fn encryption(mut self, encryption: String) -> Self {
        self.encryption = Some(encryption);
        self
    }

    pub fn no_encryption(mut self) -> Self {
        self.encryption = Some("no_encryption".to_string());
        self
    }

    pub fn body(mut self, content: String) -> Self {
        self.body = Some(Body {
            content,
            internal_metadata: None,
        });
        self
    }

    pub fn message_schema_type(mut self, type_name: String, fields: Vec<Field>) -> Self {
        self.message_schema_type = Some(MessageSchemaType { type_name, fields });
        self
    }

    pub fn topic(mut self, topic_id: String, channel_id: String) -> Self {
        self.topic = Some(Topic {
            topic_id,
            channel_id,
        });
        self
    }

    pub fn internal_metadata_content(mut self, content: String) -> Self {
        self.internal_metadata_content = Some(content);
        self
    }

    pub fn external_metadata(mut self, recipient: PublicKey) -> Self {
        let signature = "".to_string();
        let scheduled_time = ShinkaiMessageHandler::generate_time_now();
        self.external_metadata = Some(ExternalMetadata {
            sender: public_key_to_string(self.my_public_key),
            recipient: public_key_to_string(recipient),
            scheduled_time,
            signature,
        });
        self
    }

    pub fn external_metadata_with_schedule(
        mut self,
        recipient: PublicKey,
        scheduled_time: String,
    ) -> Self {
        let signature = "".to_string();
        self.external_metadata = Some(ExternalMetadata {
            sender: public_key_to_string(self.my_public_key),
            recipient: public_key_to_string(recipient),
            scheduled_time,
            signature,
        });
        self
    }

    pub fn build(self) -> Result<ShinkaiMessage, &'static str> {
        if let Some(mut body) = self.body {
            let internal_metadata = InternalMetadata {
                message_schema_type: self.message_schema_type,
                topic: self.topic,
                content: self
                    .internal_metadata_content
                    .unwrap_or_else(|| String::from("")),
            };
            body.internal_metadata = Some(internal_metadata);

            if self.encryption == Some("default".to_string()) {
                let encrypted_body = encrypt_body_if_needed(
                    body.content.as_bytes(),
                    &self.my_secret_key,
                    &self.receiver_public_key,
                    self.encryption.as_deref(),
                )
                .expect("Failed to encrypt body content");
                body.content = encrypted_body;
            }

            Ok(ShinkaiMessage {
                body: Some(body),
                encryption: self.encryption.unwrap_or_else(|| String::from("")),
                external_metadata: self.external_metadata,
            })
        } else {
            Err("Missing fields")
        }
    }

    pub fn ack_message(
        my_secret_key: StaticSecret,
        receiver_public_key: PublicKey,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(my_secret_key, receiver_public_key)
            .body("ACK".to_string())
            .no_encryption()
            .external_metadata(receiver_public_key)
            .build()
    }

    pub fn ping_pong_message(
        message: String,
        my_secret_key: StaticSecret,
        receiver_public_key: PublicKey,
    ) -> Result<ShinkaiMessage, &'static str> {
        if message != "Ping" && message != "Pong" {
            return Err("Invalid message: must be 'Ping' or 'Pong'");
        }

        ShinkaiMessageBuilder::new(my_secret_key, receiver_public_key)
            .body(message)
            .no_encryption()
            .external_metadata(receiver_public_key)
            .build()
    }

    pub fn terminate_message(
        my_secret_key: StaticSecret,
        receiver_public_key: PublicKey,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(my_secret_key, receiver_public_key)
            .body("terminate".to_string())
            .no_encryption()
            .external_metadata(receiver_public_key)
            .build()
    }

    pub fn error_message(
        my_secret_key: StaticSecret,
        receiver_public_key: PublicKey,
        error_msg: String,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(my_secret_key, receiver_public_key)
            .body(format!("{{error: \"{}\"}}", error_msg))
            .no_encryption()
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(deprecated)]
    use rand_os::OsRng;
    use x25519_dalek::StaticSecret; // Provides a random number generator that implements RngCore and CryptoRng

    #[test]
    fn test_builder_with_all_fields_no_encryption() {
        let fields = vec![
            Field {
                name: "field1".to_string(),
                field_type: "type1".to_string(),
            },
            // more fields...
        ];

        #[allow(deprecated)]
        let mut csprng = OsRng::new().unwrap();
        let secret_key = StaticSecret::new(&mut csprng);
        let public_key = PublicKey::from(&secret_key);

        let message_result = ShinkaiMessageBuilder::new(secret_key, public_key)
            .body("body content".to_string())
            .encryption("no_encryption".to_string())
            .message_schema_type("schema type".to_string(), fields)
            .topic("topic_id".to_string(), "channel_id".to_string())
            .internal_metadata_content("internal metadata content".to_string())
            .external_metadata_with_schedule(public_key, "20230702T20533481345".to_string())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let body = message.body.as_ref().unwrap();
        assert_eq!(body.content, "body content");
        assert_eq!(message.encryption, "no_encryption");
        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "internal metadata content");
        let external_metadata = message.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, public_key_to_string(public_key));
    }

    #[test]
    fn test_builder_with_all_fields_encryption() {
        let fields = vec![
            Field {
                name: "field1".to_string(),
                field_type: "type1".to_string(),
            },
            // more fields...
        ];

        #[allow(deprecated)]
        let mut csprng = OsRng::new().unwrap();
        let secret_key = StaticSecret::new(&mut csprng);
        let secret_key_clone = secret_key.clone();
        let public_key = PublicKey::from(&secret_key);

        let message_result = ShinkaiMessageBuilder::new(secret_key, public_key)
            .body("body content".to_string())
            .encryption("default".to_string())
            .message_schema_type("schema type".to_string(), fields)
            .topic("topic_id".to_string(), "channel_id".to_string())
            .internal_metadata_content("internal metadata content".to_string())
            .external_metadata(public_key)
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let body = message.body.as_ref().unwrap();
        assert_eq!(message.encryption, "default");

        print!(
            "test encryption 'body content'> {:?} ",
            &body.content.as_bytes()
        );
        let decrypted_content = decrypt_body_content(
            &body.content.as_bytes(),
            &secret_key_clone,
            &public_key,
            Some(&message.encryption),
        )
        .expect("Failed to decrypt body content");
        assert_eq!(decrypted_content, "body content");

        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "internal metadata content");
        let external_metadata = message.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, public_key_to_string(public_key));
    }

    #[test]
    fn test_builder_missing_fields() {
        #[allow(deprecated)]
        let mut csprng = OsRng::new().unwrap();
        let secret_key = StaticSecret::new(&mut csprng);
        let public_key = PublicKey::from(&secret_key);

        let message_result = ShinkaiMessageBuilder::new(secret_key, public_key).build();
        assert!(message_result.is_err());
    }
}
