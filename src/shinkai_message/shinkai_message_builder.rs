#[allow(unused_imports)]
use super::encryption::{decrypt_body_message, encrypt_body};
use super::{
    encryption::{encrypt_string_content, encryption_public_key_to_string, EncryptionMethod},
    shinkai_message_handler::ShinkaiMessageHandler,
    signatures::{sign_message, signature_public_key_to_string},
};
use crate::{
    managers::identity_manager::RegistrationCode,
    schemas::message_schemas::{JobCreation, JobScope, MessageSchemaType},
    shinkai_message_proto::{Body, ExternalMetadata, InternalMetadata, ShinkaiMessage},
};
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

pub type ProfileName = String;

pub struct ShinkaiMessageBuilder {
    body: Option<Body>,
    message_schema_type: String,
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
            message_schema_type: String::new(),
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

    pub fn body_encryption(mut self, encryption: EncryptionMethod) -> Self {
        self.encryption = encryption.as_str().to_string();
        self
    }

    pub fn no_body_encryption(mut self) -> Self {
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

    pub fn message_schema_type(mut self, content: String) -> Self {
        // TODO: add validation here of the content: String or maybe even switch it to the new enum
        self.message_schema_type = content.clone();
        self
    }

    pub fn internal_metadata(
        mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        encryption: EncryptionMethod,
    ) -> Self {
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            message_schema_type: self.message_schema_type.clone(),
            inbox,
            encryption: encryption.as_str().to_string(),
        });
        self
    }

    pub fn internal_metadata_with_schema(
        mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        message_schema: String,
        encryption: EncryptionMethod,
    ) -> Self {
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            message_schema_type: message_schema,
            inbox,
            encryption: encryption.as_str().to_string(),
        });
        self
    }

    pub fn empty_encrypted_internal_metadata(mut self) -> Self {
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity: String::new(),
            recipient_subidentity: String::new(),
            message_schema_type: String::new(),
            inbox: String::new(),
            encryption: EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str().to_string(),
        });
        self
    }

    pub fn empty_non_encrypted_internal_metadata(mut self) -> Self {
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity: String::new(),
            recipient_subidentity: String::new(),
            message_schema_type: String::new(),
            inbox: String::new(),
            encryption: EncryptionMethod::None.as_str().to_string(),
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
            other,
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
            other,
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
            other,
        });
        self
    }

    pub fn build(self) -> Result<ShinkaiMessage, &'static str> {
        if self.internal_metadata.is_none() {
            return Err("Internal metadata is required");
        }

        let encryption_method_none = EncryptionMethod::None.as_str().to_string();
        if self.encryption != encryption_method_none
            && self.internal_metadata.is_some()
            && self.internal_metadata.as_ref().unwrap().encryption != encryption_method_none
        {
            return Err("Encryption should not be set on both body and internal metadata simultaneously");
        }

        if let Some(mut body) = self.body {
            let body_content = body.content.clone();
            let internal_metadata_clone = self.internal_metadata.clone();
            body.internal_metadata = internal_metadata_clone.clone();

            // if self.internal_metadata.encryption is not None
            let new_content = if let Some(internal_metadata) = &self.internal_metadata {
                if internal_metadata.encryption.as_str() != &encryption_method_none {
                    let encrypted_content = encrypt_string_content(
                        body_content,
                        internal_metadata.message_schema_type.clone(),
                        &self.my_encryption_secret_key,
                        &self.receiver_public_key,
                        internal_metadata.encryption.as_str(),
                    )
                    .expect("Failed to encrypt body");

                    encrypted_content
                } else {
                    // If encryption method is None, just return body
                    body_content
                }
            } else {
                println!("No internal_metadata");
                body_content
            };

            if new_content != body.content.clone() {
                if let Some(mut internal_metadata) = internal_metadata_clone {
                    internal_metadata.message_schema_type = String::new();
                    body.internal_metadata = Some(internal_metadata);
                }
            }

            // if self.encryption is not None
            let new_body = if self.encryption.as_str() != &encryption_method_none {
                let encrypted_body = encrypt_body(
                    &ShinkaiMessageHandler::encode_body(body.clone()),
                    &self.my_encryption_secret_key,
                    &self.receiver_public_key,
                    self.encryption.as_str(),
                )
                .expect("Failed to encrypt body");

                Body {
                    content: encrypted_body,
                    internal_metadata: None,
                }
            } else {
                // println!("No encryption");
                // If encryption method is None, just return body
                Body {
                    content: new_content,
                    internal_metadata: body.internal_metadata,
                }
            };

            let mut external_metadata = self.external_metadata.clone().ok_or("Missing external metadata")?;

            let unsigned_msg = ShinkaiMessage {
                body: Some(new_body.clone()),
                encryption: self.encryption.clone(),
                external_metadata: self.external_metadata,
            };
            let signature = sign_message(&self.my_signature_secret_key, unsigned_msg);

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
        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .body("ACK".to_string())
            .empty_non_encrypted_internal_metadata()
            .no_body_encryption()
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
        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .body(message)
            .empty_non_encrypted_internal_metadata()
            .no_body_encryption()
            .external_metadata(receiver, sender)
            .build()
    }

    pub fn job_creation(
        scope: JobScope,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let job_creation = JobCreation { scope };
        let body = serde_json::to_string(&job_creation).map_err(|_| "Failed to serialize job creation to JSON")?;

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .body(body)
            .internal_metadata_with_schema(
                sender.clone(),
                receiver.clone(),
                "".to_string(),
                MessageSchemaType::JobCreationSchema.to_str().to_string(),
                EncryptionMethod::None,
            )
            .no_body_encryption()
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
        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .body("terminate".to_string())
            .empty_non_encrypted_internal_metadata()
            .no_body_encryption()
            .external_metadata(receiver, sender)
            .build()
    }

    pub fn code_registration(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        code: String,
        permission_type: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let my_subidentity_signature_pk = ed25519_dalek::PublicKey::from(&my_subidentity_signature_sk);
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk);

        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);
        let registration_code = RegistrationCode {
            code,
            profile_name: sender.clone(),
            identity_pk: signature_public_key_to_string(my_subidentity_signature_pk),
            encryption_pk: other.clone(),
            permission_type,
        };

        let body =
            serde_json::to_string(&registration_code).map_err(|_| "Failed to serialize registration code to JSON")?;

        println!(
            "code_registration> receiver_public_key = {:?}",
            encryption_public_key_to_string(receiver_public_key)
        );
        ShinkaiMessageBuilder::new(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
        )
        .body(body)
        .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
        .internal_metadata(sender, "".to_string(), "".to_string(), EncryptionMethod::None)
        // we are interacting with the associated node so the receiver and the sender are from the same base node
        .external_metadata_with_other(receiver.clone(), receiver, other)
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
        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .body(format!("{{error: \"{}\"}}", error_msg))
            .empty_encrypted_internal_metadata()
            .no_body_encryption()
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shinkai_message::{
        encryption::{decrypt_body_message, decrypt_content_message, unsafe_deterministic_encryption_keypair},
        signatures::{unsafe_deterministic_signature_keypair, verify_signature},
    };

    #[test]
    fn test_builder_with_all_fields_no_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk)
            .body("body content".to_string())
            .body_encryption(EncryptionMethod::None)
            .message_schema_type("schema type".to_string())
            .internal_metadata("".to_string(), "".to_string(), "".to_string(), EncryptionMethod::None)
            .external_metadata_with_schedule(recipient.clone(), sender.clone(), scheduled_time.clone())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();
        let body = message.body.as_ref().unwrap();
        assert_eq!(body.content, "body content");
        assert_eq!(message.encryption, EncryptionMethod::None.as_str().to_string());
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
    fn test_builder_with_all_fields_body_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk.clone(), my_identity_sk, node2_encryption_pk)
            .body("body content".to_string())
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .message_schema_type("schema type".to_string())
            .internal_metadata("".to_string(), "".to_string(), "".to_string(), EncryptionMethod::None)
            .external_metadata(recipient, sender.clone())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();
        // let body_content = message.body.as_ref().unwrap().content.clone();
        assert_eq!(
            message.encryption,
            EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str().to_string()
        );

        let decrypted_message = decrypt_body_message(&message.clone(), &my_encryption_sk, &node2_encryption_pk)
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
    fn test_builder_with_all_fields_content_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk.clone(), my_identity_sk, node2_encryption_pk)
            .body("body content".to_string())
            .no_body_encryption()
            .message_schema_type("schema type".to_string())
            .internal_metadata(
                "".to_string(),
                "".to_string(),
                "".to_string(),
                EncryptionMethod::DiffieHellmanChaChaPoly1305,
            )
            .external_metadata(recipient, sender.clone())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();
        let body_clone = message.body.clone().unwrap();

        assert_eq!(message.encryption, EncryptionMethod::None.as_str().to_string());
        assert_eq!(
            body_clone.internal_metadata.as_ref().unwrap().encryption,
            EncryptionMethod::DiffieHellmanChaChaPoly1305.as_str().to_string()
        );

        let decrypted_content = decrypt_content_message(
            // decrypt_content_message(
            message.clone().body.unwrap().content,
            &message.clone().body.unwrap().internal_metadata.unwrap().encryption,
            &my_encryption_sk,
            &node2_encryption_pk,
        )
        .expect("Failed to decrypt body content");

        println!("decrypted content: {}", decrypted_content.0);
        assert_eq!(decrypted_content.0, "body content");

        let external_metadata = message.external_metadata.as_ref().unwrap();
        assert_eq!(external_metadata.sender, sender);
        assert!(verify_signature(&my_identity_pk, &message_clone).unwrap())
    }

    #[test]
    fn test_builder_with_all_fields_onion_encryption() {}

    #[test]
    fn test_builder_missing_fields() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk).build();
        assert!(message_result.is_err());
    }
}
