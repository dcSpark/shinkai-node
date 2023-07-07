#[allow(unused_imports)]
use super::encryption::{decrypt_message, encrypt_body_if_needed};
use super::{
    encryption::{encryption_public_key_to_string, EncryptionMethod},
    shinkai_message_handler::ShinkaiMessageHandler,
    signatures::{sign_message, signature_public_key_to_string},
};
use crate::{shinkai_message_proto::{
    Body, ExternalMetadata, Field, InternalMetadata, MessageSchemaType, ShinkaiMessage,
}, network::subidentities::RegistrationCode};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub type ProfileName = String;

pub struct ShinkaiMessageBuilder {
    body: Option<Body>,
    message_schema_type: Option<MessageSchemaType>,
    internal_metadata: Option<InternalMetadata>,
    external_metadata: Option<ExternalMetadata>,
    encryption: String,
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
            internal_metadata: None,
            external_metadata: None,
            encryption: EncryptionMethod::None.as_str().to_string(),
            my_encryption_secret_key,
            my_encryption_public_key,
            my_signature_public_key,
            my_signature_secret_key,
            receiver_public_key,
        }
    }

    pub fn encryption(mut self, encryption: EncryptionMethod) -> Self {
        self.encryption = encryption.as_str().to_string();
        self
    }

    pub fn no_encryption(mut self) -> Self {
        self.encryption = EncryptionMethod::None.as_str().to_string();
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

    pub fn internal_metadata(
        mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
    ) -> Self {
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            message_schema_type: self.message_schema_type.take(),
            inbox,
        });
        self
    }

    pub fn external_metadata(mut self, recipient: ProfileName, sender: ProfileName) -> Self {
        let signature = "".to_string();
        let other = "".to_string();
        let scheduled_time = ShinkaiMessageHandler::generate_time_now();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other
        });
        self
    }

    pub fn external_metadata_with_other(mut self, recipient: ProfileName, sender: ProfileName, other: String) -> Self {
        let signature = "".to_string();
        let scheduled_time = ShinkaiMessageHandler::generate_time_now();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other
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
        let other = "".to_string();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other
        });
        self
    }

    pub fn build(self) -> Result<ShinkaiMessage, &'static str> {
        if let Some(mut body) = self.body {
            body.internal_metadata = self.internal_metadata;

            let encryption_method_none = EncryptionMethod::None.as_str().to_string();
            let new_body = if self.encryption.as_str() != &encryption_method_none {
                // Convert the body to bytes and encrypt the entire body
                let encrypted_body = encrypt_body_if_needed(
                    &ShinkaiMessageHandler::encode_body(body.clone()),
                    &self.my_encryption_secret_key,
                    &self.receiver_public_key,
                    self.encryption.as_str(),
                )
                .expect("Failed to encrypt body");

                // Convert the encrypted body to base58 and then to content of Body
                Body {
                    content: encrypted_body,
                    internal_metadata: None,
                }
            } else {
                println!("No encryption");
                // If encryption method is None, just return body
                body
            };

            let mut external_metadata = self
                .external_metadata
                .clone()
                .ok_or("Missing external metadata")?;

            let unsigned_msg = ShinkaiMessage {
                body: Some(new_body.clone()),
                encryption: self.encryption.clone(),
                external_metadata: self.external_metadata,
            };
            let unsigned_msg_bytes = ShinkaiMessageHandler::encode_message(unsigned_msg);
            let signature = sign_message(&self.my_signature_secret_key, &unsigned_msg_bytes);

            external_metadata.signature = signature;

            let signed_msg = ShinkaiMessage {
                body: Some(new_body.clone()),
                encryption: self.encryption.clone(),
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

    pub fn code_registration(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        code: String,
        sender: ProfileName,
        receiver: ProfileName
    ) -> Result<ShinkaiMessage, &'static str> {
        let my_subidentity_signature_pk = ed25519_dalek::PublicKey::from(&my_subidentity_signature_sk);
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk);

        let registration_code = RegistrationCode {
            code,
            profile_name: sender.clone(),
            identity_pk: signature_public_key_to_string(my_subidentity_signature_pk),
            encryption_pk: encryption_public_key_to_string(my_subidentity_encryption_pk),
        };

        let body = serde_json::to_string(&registration_code)
            .map_err(|_| "Failed to serialize registration code to JSON")?;

        ShinkaiMessageBuilder::new(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
        )
        .body(body)
        .encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
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
    use super::*;
    use crate::shinkai_message::{
        encryption::{decrypt_message, unsafe_deterministic_encryption_keypair},
        signatures::{unsafe_deterministic_signature_keypair, verify_signature},
    };

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

        let message_result =
            ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk)
                .body("body content".to_string())
                .encryption(EncryptionMethod::None)
                .message_schema_type("schema type".to_string(), fields)
                .internal_metadata("".to_string(), "".to_string(), "".to_string())
                .external_metadata_with_schedule(
                    recipient.clone(),
                    sender.clone(),
                    scheduled_time.clone(),
                )
                .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();
        let body = message.body.as_ref().unwrap();
        assert_eq!(body.content, "body content");
        assert_eq!(
            message.encryption,
            EncryptionMethod::None.as_str().to_string()
        );
        let internal_metadata = body.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.sender_subidentity, "");
        assert_eq!(internal_metadata.recipient_subidentity, "");
        assert_eq!(internal_metadata.inbox, "");
        let external_metadata = message.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.scheduled_time, scheduled_time);
        assert_eq!(external_metadata.recipient, recipient);
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

        let message_result = ShinkaiMessageBuilder::new(
            my_encryption_sk.clone(),
            my_identity_sk,
            node2_encryption_pk,
        )
        .body("body content".to_string())
        .encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
        .message_schema_type("schema type".to_string(), fields)
        .internal_metadata("".to_string(), "".to_string(), "".to_string())
        .external_metadata(recipient, sender.clone())
        .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();
        // let body_content = message.body.as_ref().unwrap().content.clone();
        assert_eq!(
            message.encryption,
            EncryptionMethod::DiffieHellmanChaChaPoly1305
                .as_str()
                .to_string()
        );

        let decrypted_message =
            decrypt_message(&message.clone(), &my_encryption_sk, &node2_encryption_pk)
                .expect("Failed to decrypt body content");

        let binding = decrypted_message.body.clone().unwrap();
        let decrypted_content = binding.content.as_str();
        println!("decrypted content: {}", decrypted_content);
        assert_eq!(decrypted_content, "body content");

        let binding = decrypted_message.body.clone().unwrap();
        let internal_metadata = binding.internal_metadata.as_ref().unwrap();
        assert_eq!(internal_metadata.sender_subidentity, "");
        assert_eq!(internal_metadata.recipient_subidentity, "");
        assert_eq!(internal_metadata.inbox, "");
        let external_metadata = message.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, sender);
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

        let message_result =
            ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk)
                .build();
        assert!(message_result.is_err());
    }
}
