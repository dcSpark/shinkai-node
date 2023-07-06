#[allow(unused_imports)]
use super::encryption::{decrypt_body_content, encrypt_body_if_needed};
use super::{
    encryption::{encryption_public_key_to_string, EncryptionMethod},
    shinkai_message_handler::ShinkaiMessageHandler,
    signatures::sign_message,
};
use crate::shinkai_message_proto::{
    Body, ExternalMetadata, Field, InternalMetadata, MessageSchemaType, ShinkaiMessage, Topic,
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub type ProfileName = String;

pub struct ShinkaiMessageBuilder {
    body: Option<Body>,
    message_schema_type: Option<MessageSchemaType>,
    topic: Option<Topic>,
    internal_metadata_content: Option<String>,
    external_metadata: Option<ExternalMetadata>,
    encryption: Option<String>,
    my_encryption_secret_key: EncryptionStaticKey,
    my_encryption_public_key: EncryptionPublicKey,
    my_signature_secret_key: SignatureStaticKey,
    my_signature_public_key: SignaturePublicKey,
    receiver_public_key: EncryptionPublicKey,
}

impl ShinkaiMessageBuilder {
    pub fn new(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
    ) -> Self {
        let my_encryption_public_key = x25519_dalek::PublicKey::from(&my_encryption_secret_key);
        let my_signature_public_key = ed25519_dalek::PublicKey::from(&my_signature_secret_key);
        Self {
            body: None,
            message_schema_type: None,
            topic: None,
            internal_metadata_content: None,
            external_metadata: None,
            encryption: None,
            my_encryption_secret_key,
            my_encryption_public_key,
            my_signature_public_key,
            my_signature_secret_key,
            receiver_public_key,
        }
    }

    pub fn encryption(mut self, encryption: EncryptionMethod) -> Self {
        self.encryption = Some(encryption.as_str().to_string());
        self
    }

    pub fn no_encryption(mut self) -> Self {
        self.encryption = Some(EncryptionMethod::None.as_str().to_string());
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

    pub fn external_metadata(mut self, recipient: ProfileName, sender: ProfileName) -> Self {
        let signature = "".to_string();
        let scheduled_time = ShinkaiMessageHandler::generate_time_now();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
        });
        self
    }

    pub fn external_metadata_with_schedule(
        mut self,
        recipient: ProfileName,
        sender: ProfileName,
        scheduled_time: String,
    ) -> Self {
        let signature = "".to_string();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
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

            let encryption_method = EncryptionMethod::DiffieHellmanChaChaPoly1305
                .as_str()
                .to_string();

            if self.encryption == Some(encryption_method) {
                let encrypted_body = encrypt_body_if_needed(
                    body.content.as_bytes(),
                    &self.my_encryption_secret_key,
                    &self.receiver_public_key,
                    self.encryption.as_deref(),
                )
                .expect("Failed to encrypt body content");
                body.content = encrypted_body;
            }

            let mut external_metadata =
                self.external_metadata.clone().ok_or("Missing external metadata")?;

            let unsigned_msg = ShinkaiMessage {
                body: Some(body.clone()),
                encryption: self.encryption.clone().unwrap_or_else(|| String::from("")),
                external_metadata: self.external_metadata,
            };
            let unsigned_msg_bytes = ShinkaiMessageHandler::encode_message(unsigned_msg);
            let signature = sign_message(&self.my_signature_secret_key, &unsigned_msg_bytes);

            external_metadata.signature = signature;

            let signed_msg = ShinkaiMessage {
                body: Some(body.clone()),
                encryption: self.encryption.clone().unwrap_or_else(|| String::from("")),
                external_metadata: Some(external_metadata),
            };

            Ok(signed_msg)
        } else {
            Err("Missing fields")
        }
    }

    pub fn ack_message(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
        )
        .body("ACK".to_string())
        .no_encryption()
        .external_metadata(receiver, sender)
        .build()
    }

    pub fn ping_pong_message(
        message: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        if message != "Ping" && message != "Pong" {
            return Err("Invalid message: must be 'Ping' or 'Pong'");
        }
        ShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
        )
        .body(message)
        .no_encryption()
        .external_metadata(receiver, sender)
        .build()
    }

    pub fn terminate_message(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
        )
        .body("terminate".to_string())
        .no_encryption()
        .external_metadata(receiver, sender)
        .build()
    }

    pub fn error_message(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        receiver: ProfileName,
        error_msg: String,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
        )
        .body(format!("{{error: \"{}\"}}", error_msg))
        .no_encryption()
        .build()
    }
}

#[cfg(test)]
mod tests {
    use crate::shinkai_message::{signatures::{unsafe_deterministic_signature_keypair, verify_signature}, encryption::unsafe_deterministic_encryption_keypair};
    use super::*;

    #[test]
    fn test_builder_with_all_fields_no_encryption() {
        let fields = vec![
            Field {
                name: "field1".to_string(),
                field_type: "type1".to_string(),
            },
            // more fields...
        ];

        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk,my_identity_sk, node2_encryption_pk)
            .body("body content".to_string())
            .encryption(EncryptionMethod::None)
            .message_schema_type("schema type".to_string(), fields)
            .topic("topic_id".to_string(), "channel_id".to_string())
            .internal_metadata_content("internal metadata content".to_string())
            .external_metadata_with_schedule(recipient.clone(), sender.clone(), scheduled_time.clone())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();
        let body = message.body.as_ref().unwrap();
        assert_eq!(body.content, "body content");
        assert_eq!(message.encryption, EncryptionMethod::None.as_str().to_string());
        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "internal metadata content");
        let external_metadata = message.external_metadata.as_ref().unwrap();
        assert_eq!(
            external_metadata.sender,
            sender
        );
        assert_eq!(
            external_metadata.scheduled_time,
            scheduled_time
        );
        assert_eq!(
            external_metadata.recipient,
            recipient
        );
        assert!(verify_signature(&my_identity_pk, &message_clone).unwrap())
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

        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk.clone(),my_identity_sk, node2_encryption_pk)
            .body("body content".to_string())
            .encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .message_schema_type("schema type".to_string(), fields)
            .topic("topic_id".to_string(), "channel_id".to_string())
            .internal_metadata_content("internal metadata content".to_string())
            .external_metadata(recipient, sender.clone())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();
        let body = message.body.as_ref().unwrap();
        assert_eq!(message.encryption, EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str().to_string());

        let decrypted_content = decrypt_body_content(
            &body.content.as_bytes(),
            &my_encryption_sk,
            &node2_encryption_pk,
            Some(&message.encryption),
        )
        .expect("Failed to decrypt body content");
        assert_eq!(decrypted_content, "body content");

        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.content, "internal metadata content");
        let external_metadata = message.external_metadata.as_ref().unwrap();
        assert_eq!(
            external_metadata.sender,
            sender
        );
        assert!(verify_signature(&my_identity_pk, &message_clone).unwrap())
    }

    #[test]
    fn test_builder_missing_fields() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk,my_identity_sk, node2_encryption_pk).build();
        assert!(message_result.is_err());
    }
}
