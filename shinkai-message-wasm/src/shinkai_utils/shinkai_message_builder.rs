use serde::Serialize;
use wasm_bindgen::prelude::*;

use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{
    schemas::{inbox_name::InboxName, registration_code::RegistrationCode, shinkai_time::ShinkaiTime},
    shinkai_message::{
        shinkai_message::{
            EncryptedShinkaiBody, EncryptedShinkaiData, ExternalMetadata, InternalMetadata, MessageBody, MessageData,
            ShinkaiBody, ShinkaiData, ShinkaiMessage, ShinkaiVersion,
        },
        shinkai_message_schemas::{
            APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, IdentityPermissions, JobCreation, JobMessage,
            JobScope, MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType,
        },
    },
    shinkai_utils::{
        encryption::{encryption_public_key_to_string, EncryptionMethod},
        signatures::{sign_message, signature_public_key_to_string},
    },
};

use super::{
    encryption::{clone_static_secret_key, encryption_secret_key_to_string},
    signatures::{clone_signature_secret_key, signature_secret_key_to_string},
};

pub type ProfileName = String;

pub struct ShinkaiMessageBuilder {
    message_raw_content: String,
    message_content_schema: MessageSchemaType,
    internal_metadata: Option<InternalMetadata>,
    external_metadata: Option<ExternalMetadata>,
    encryption: EncryptionMethod,
    my_encryption_secret_key: EncryptionStaticKey,
    my_encryption_public_key: EncryptionPublicKey,
    my_signature_secret_key: SignatureStaticKey,
    my_signature_public_key: SignaturePublicKey,
    receiver_public_key: EncryptionPublicKey,
    version: ShinkaiVersion,
}

impl ShinkaiMessageBuilder {
    pub fn new(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
    ) -> Self {
        let version = ShinkaiVersion::V1_0;
        let my_encryption_public_key = x25519_dalek::PublicKey::from(&my_encryption_secret_key);
        let my_signature_public_key = ed25519_dalek::PublicKey::from(&my_signature_secret_key);
        Self {
            message_raw_content: "".to_string(),
            message_content_schema: MessageSchemaType::Empty,
            internal_metadata: None,
            external_metadata: None,
            encryption: EncryptionMethod::None,
            my_encryption_secret_key,
            my_encryption_public_key,
            my_signature_public_key,
            my_signature_secret_key,
            receiver_public_key,
            version,
        }
    }

    pub fn body_encryption(mut self, encryption: EncryptionMethod) -> Self {
        self.encryption = encryption;
        self
    }

    pub fn no_body_encryption(mut self) -> Self {
        self.encryption = EncryptionMethod::None;
        self
    }

    pub fn message_raw_content(mut self, message_raw_content: String) -> Self {
        self.message_raw_content = message_raw_content;
        self
    }

    pub fn message_schema_type(mut self, content: MessageSchemaType) -> Self {
        self.message_content_schema = content.clone();
        self
    }

    pub fn internal_metadata(
        mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        encryption: EncryptionMethod,
    ) -> Self {
        let signature = "".to_string();
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            inbox: "".to_string(),
            signature,
            encryption,
        });
        self
    }

    pub fn internal_metadata_with_inbox(
        mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        encryption: EncryptionMethod,
    ) -> Self {
        let signature = "".to_string();
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            inbox,
            signature,
            encryption,
        });
        self
    }

    pub fn internal_metadata_with_schema(
        mut self,
        sender_subidentity: String,
        recipient_subidentity: String,
        inbox: String,
        message_schema: MessageSchemaType,
        encryption: EncryptionMethod,
    ) -> Self {
        let signature = "".to_string();
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            inbox,
            signature,
            encryption,
        });
        self
    }

    pub fn empty_encrypted_internal_metadata(mut self) -> Self {
        let signature = "".to_string();
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity: String::new(),
            recipient_subidentity: String::new(),
            inbox: String::new(),
            signature,
            encryption: EncryptionMethod::DiffieHellmanChaChaPoly1305,
        });
        self
    }

    pub fn empty_non_encrypted_internal_metadata(mut self) -> Self {
        let signature = "".to_string();
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity: String::new(),
            recipient_subidentity: String::new(),
            inbox: String::new(),
            signature,
            encryption: EncryptionMethod::None,
        });
        self
    }

    pub fn external_metadata(mut self, recipient: ProfileName, sender: ProfileName) -> Self {
        let signature = "".to_string();
        let other = "".to_string();
        let scheduled_time = ShinkaiTime::generate_time_now();
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
        let scheduled_time = ShinkaiTime::generate_time_now();
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

    pub fn clone(&self) -> Self {
        let my_encryption_secret_key_clone = clone_static_secret_key(&self.my_encryption_secret_key);
        let my_signature_secret_key_clone = clone_signature_secret_key(&self.my_signature_secret_key);
        let my_encryption_public_key_clone = x25519_dalek::PublicKey::from(&my_encryption_secret_key_clone);
        let my_signature_public_key_clone = ed25519_dalek::PublicKey::from(&my_signature_secret_key_clone);
        let receiver_public_key_clone = self.receiver_public_key.clone();

        Self {
            message_raw_content: self.message_raw_content.clone(),
            message_content_schema: self.message_content_schema.clone(),
            internal_metadata: self.internal_metadata.clone(),
            external_metadata: self.external_metadata.clone(),
            encryption: self.encryption.clone(),
            my_encryption_secret_key: my_encryption_secret_key_clone,
            my_encryption_public_key: my_encryption_public_key_clone,
            my_signature_secret_key: my_signature_secret_key_clone,
            my_signature_public_key: my_signature_public_key_clone,
            receiver_public_key: receiver_public_key_clone,
            version: self.version.clone(),
        }
    }

    //
    // Build
    //

    pub fn build(&self) -> Result<ShinkaiMessage, &'static str> {
        let mut new_self = self.clone();

        // Validations
        if new_self.internal_metadata.is_none() {
            return Err("Internal metadata is required");
        }

        let encryption_method_none = EncryptionMethod::None;
        if new_self.encryption != encryption_method_none
            && new_self.internal_metadata.is_some()
            && new_self.internal_metadata.as_ref().unwrap().encryption != encryption_method_none
        {
            // TODO: we can extend this later on but the builder will have to be able to take more keys
            return Err("Encryption should not be set on both body and internal metadata simultaneously");
        }

        // Fix inbox name if it's empty
        if let Some(internal_metadata) = &mut new_self.internal_metadata {
            if internal_metadata.inbox.is_empty() {
                if let Some(external_metadata) = &new_self.external_metadata {
                    // Generate a new inbox name
                    let new_inbox_name = InboxName::get_regular_inbox_name_from_params(
                        external_metadata.sender.clone(),
                        internal_metadata.sender_subidentity.clone(),
                        external_metadata.recipient.clone(),
                        internal_metadata.recipient_subidentity.clone(),
                        internal_metadata.encryption != EncryptionMethod::None,
                    )
                    .map_err(|_| "Failed to generate inbox name")?;

                    // Update the inbox name in the internal metadata
                    internal_metadata.inbox = match new_inbox_name {
                        InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
                    };
                } else {
                    return Err("Inbox is required");
                }
            }
        }

        // encrypted body or data if necessary
        if let Some(internal_metadata) = &mut new_self.internal_metadata {
            // if self.internal_metadata.encryption is not None
            let new_message_data = if internal_metadata.encryption != encryption_method_none {
                println!("Encrypting data content");

                let data = ShinkaiData {
                    message_raw_content: new_self.message_raw_content.clone(),
                    message_content_schema: new_self.message_content_schema.clone(),
                };

                let encrypted_content = MessageData::encrypt_message_data(
                    &data,
                    &new_self.my_encryption_secret_key,
                    &new_self.receiver_public_key,
                )
                .expect("Failed to encrypt data content");

                encrypted_content
            } else {
                // If encryption method is None, just return body
                MessageData::Unencrypted(ShinkaiData {
                    message_raw_content: new_self.message_raw_content.clone(),
                    message_content_schema: new_self.message_content_schema.clone(),
                })
            };

            // if self.encryption is not None
            let new_body = if new_self.encryption != encryption_method_none {
                let encrypted_body = MessageBody::encrypt_message_body(
                    &ShinkaiBody {
                        message_data: new_message_data.clone(),
                        internal_metadata: internal_metadata.clone(),
                    },
                    &new_self.my_encryption_secret_key,
                    &new_self.receiver_public_key,
                )
                .expect("Failed to encrypt body");

                encrypted_body
            } else {
                // println!("No encryption");
                // If encryption method is None, just return body

                let body = ShinkaiBody {
                    message_data: new_message_data.clone(),
                    internal_metadata: internal_metadata.clone(),
                };

                MessageBody::Unencrypted(body)
            };

            let unsigned_msg = ShinkaiMessage {
                body: new_body,
                encryption: new_self.encryption.clone(),
                external_metadata: new_self.external_metadata.clone().unwrap(),
                version: new_self.version.clone(),
            };
            let signed_msg = unsigned_msg
                .sign_outer_layer(&new_self.my_signature_secret_key)
                .map_err(|_| "Failed to sign message")?;

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
            .message_raw_content("ACK".to_string())
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
            .message_raw_content(message)
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
        node_sender: ProfileName,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let job_creation = JobCreation { scope };
        let body = serde_json::to_string(&job_creation).map_err(|_| "Failed to serialize job creation to JSON")?;

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(body)
            .internal_metadata_with_schema(
                "".to_string(),
                node_receiver_subidentity.clone(),
                "".to_string(),
                MessageSchemaType::JobCreationSchema,
                EncryptionMethod::None,
            )
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .external_metadata(node_receiver, node_sender)
            .build()
    }

    pub fn job_message(
        job_id: String,
        content: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        node_sender: ProfileName,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let job_id_clone = job_id.clone();
        let job_message = JobMessage { job_id, content };
        let body = serde_json::to_string(&job_message).map_err(|_| "Failed to serialize job message to JSON")?;

        let inbox = InboxName::get_job_inbox_name_from_params(job_id_clone)
            .map_err(|_| "Failed to get job inbox name")?
            .get_value();

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(body)
            .internal_metadata_with_schema(
                "".to_string(),
                node_receiver_subidentity.clone(),
                inbox,
                MessageSchemaType::JobMessageSchema,
                EncryptionMethod::None,
            )
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .external_metadata(node_receiver, node_sender)
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
            .message_raw_content("terminate".to_string())
            .empty_non_encrypted_internal_metadata()
            .no_body_encryption()
            .external_metadata(receiver, sender)
            .build()
    }

    pub fn request_code_registration(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        permissions: IdentityPermissions,
        code_type: RegistrationCodeType,
        sender_profile_name: String,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let registration_code_request = RegistrationCodeRequest { permissions, code_type };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            registration_code_request,
            sender_profile_name,
            receiver,
            MessageSchemaType::CreateRegistrationCode,
        )
    }

    pub fn use_code_registration(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender_profile_name: String,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let my_subidentity_signature_pk = ed25519_dalek::PublicKey::from(&my_subidentity_signature_sk);
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk);
        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);

        let registration_code = RegistrationCode {
            code,
            registration_name: registration_name.clone(),
            identity_pk: signature_public_key_to_string(my_subidentity_signature_pk),
            encryption_pk: other.clone(),
            identity_type,
            permission_type,
        };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            registration_code,
            sender_profile_name,
            receiver,
            MessageSchemaType::TextContent,
        )
    }

    pub fn get_last_messages_from_inbox(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        count: usize,
        offset: Option<String>,
        sender_profile_name: String,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let inbox_name = InboxName::new(inbox).map_err(|_| "Failed to create inbox name")?;
        let get_last_messages_from_inbox = APIGetMessagesFromInboxRequest {
            inbox: inbox_name,
            count,
            offset,
        };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            get_last_messages_from_inbox,
            sender_profile_name,
            receiver,
            MessageSchemaType::APIGetMessagesFromInboxRequest,
        )
    }

    pub fn get_last_unread_messages_from_inbox(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        count: usize,
        offset: Option<String>,
        sender_profile_name: String,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let inbox_name = InboxName::new(inbox).map_err(|_| "Failed to create inbox name")?;
        let get_last_unread_messages_from_inbox = APIGetMessagesFromInboxRequest {
            inbox: inbox_name,
            count,
            offset,
        };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            get_last_unread_messages_from_inbox,
            sender_profile_name,
            receiver,
            MessageSchemaType::APIGetMessagesFromInboxRequest,
        )
    }

    pub fn read_up_to_time(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        up_to_time: String,
        sender_profile_name: String,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let inbox_name = InboxName::new(inbox).map_err(|_| "Failed to create inbox name")?;
        let read_up_to_time = APIReadUpToTimeRequest { inbox_name, up_to_time };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            read_up_to_time,
            sender_profile_name,
            receiver,
            MessageSchemaType::APIReadUpToTimeRequest,
        )
    }

    pub fn create_custom_shinkai_message_to_node<T: Serialize>(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        data: T,
        sender_profile_name: String,
        receiver: ProfileName,
        schema: MessageSchemaType,
    ) -> Result<ShinkaiMessage, &'static str> {
        let body = serde_json::to_string(&data).map_err(|_| "Failed to serialize data to JSON")?;
        let my_subidentity_encryption_pk = x25519_dalek::PublicKey::from(&my_subidentity_encryption_sk);
        let other = encryption_public_key_to_string(my_subidentity_encryption_pk);

        ShinkaiMessageBuilder::new(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
        )
        .message_raw_content(body)
        .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
        .internal_metadata_with_schema(
            sender_profile_name,
            "".to_string(),
            "".to_string(),
            schema,
            EncryptionMethod::None,
        )
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
            .message_raw_content(format!("{{error: \"{}\"}}", error_msg))
            .empty_encrypted_internal_metadata()
            .external_metadata(receiver, sender)
            .no_body_encryption()
            .build()
    }
}

impl std::fmt::Debug for ShinkaiMessageBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let encryption_sk_string = encryption_secret_key_to_string(self.my_encryption_secret_key.clone());
        let encryption_pk_string = encryption_public_key_to_string(self.my_encryption_public_key.clone());
        let signature_sk_clone = clone_signature_secret_key(&self.my_signature_secret_key);
        let signature_sk_string = signature_secret_key_to_string(signature_sk_clone);
        let signature_pk_string = signature_public_key_to_string(self.my_signature_public_key.clone());
        let receiver_pk_string = encryption_public_key_to_string(self.receiver_public_key.clone());

        f.debug_struct("ShinkaiMessageBuilder")
            .field("message_raw_content", &self.message_raw_content)
            .field("message_schema_type", &self.message_content_schema)
            .field("internal_metadata", &self.internal_metadata)
            .field("external_metadata", &self.external_metadata)
            .field("encryption", &self.encryption)
            .field("my_encryption_secret_key", &encryption_sk_string)
            .field("my_encryption_public_key", &encryption_pk_string)
            .field("my_signature_secret_key", &signature_sk_string)
            .field("my_signature_public_key", &signature_pk_string)
            .field("receiver_public_key", &receiver_pk_string)
            .field("version", &self.version)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::shinkai_utils::{
        encryption::unsafe_deterministic_encryption_keypair,
        signatures::{unsafe_deterministic_signature_keypair, verify_signature},
    };

    use super::*;

    #[test]
    fn test_builder_with_all_fields_no_encryption() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk)
            .message_raw_content("body content".to_string())
            .body_encryption(EncryptionMethod::None)
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata("".to_string(), "".to_string(), EncryptionMethod::None)
            .external_metadata_with_schedule(recipient.clone(), sender.clone(), scheduled_time.clone())
            .build();

        println!("message_result = {:?}", message_result);
        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();

        if let MessageBody::Unencrypted(shinkai_body) = message.body {
            if let MessageData::Unencrypted(shinkai_data) = shinkai_body.message_data {
                assert_eq!(shinkai_data.message_raw_content, "body content");
            }
            assert_eq!(shinkai_body.internal_metadata.sender_subidentity, "");
            assert_eq!(shinkai_body.internal_metadata.recipient_subidentity, "");
            assert_eq!(
                shinkai_body.internal_metadata.inbox,
                "inbox::@@my_node.shinkai::@@other_node.shinkai::false"
            );
        }

        assert_eq!(message.encryption, EncryptionMethod::None);
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender);
        assert_eq!(external_metadata.scheduled_time, scheduled_time);
        assert_eq!(external_metadata.recipient, recipient);
        assert!(verify_signature(&my_identity_pk, &message_clone).unwrap())
    }

    #[test]
    fn test_builder_with_all_fields_body_encryption() {
        println!("\n\n\ntest_builder_with_all_fields_body_encryption");
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "20230702T20533481345".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk.clone(), my_identity_sk, node2_encryption_pk)
            .message_raw_content("body content".to_string())
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata("".to_string(), "".to_string(), EncryptionMethod::None)
            .external_metadata(recipient, sender.clone())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();
        assert_eq!(message.encryption, EncryptionMethod::DiffieHellmanChaChaPoly1305);

        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &node2_encryption_pk)
            .expect("Failed to decrypt body content");

        let binding = decrypted_message.body.clone();
        if let MessageBody::Unencrypted(shinkai_body) = binding {
            let message_data = shinkai_body.message_data;
            if let MessageData::Unencrypted(shinkai_data) = message_data {
                let decrypted_content = shinkai_data.message_raw_content;
                eprintln!("\n\ndecrypted content: {}", decrypted_content);
                assert_eq!(decrypted_content, "body content");
            }
        }

        let binding = decrypted_message.body.clone();
        if let MessageBody::Unencrypted(shinkai_body) = binding {
            let internal_metadata = &shinkai_body.internal_metadata;
            assert_eq!(internal_metadata.sender_subidentity, "");
            assert_eq!(internal_metadata.recipient_subidentity, "");
            assert_eq!(
                internal_metadata.inbox,
                "inbox::@@my_node.shinkai::@@other_node.shinkai::false"
            );
        }
        let external_metadata = message.external_metadata;
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
            .message_raw_content("body content".to_string())
            .no_body_encryption()
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata(
                "".to_string(),
                "".to_string(),
                EncryptionMethod::DiffieHellmanChaChaPoly1305,
            )
            .external_metadata(recipient, sender.clone())
            .build();

        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();

        if let MessageBody::Unencrypted(shinkai_body) = message.clone().body {
            assert_eq!(
                shinkai_body.internal_metadata.encryption,
                EncryptionMethod::DiffieHellmanChaChaPoly1305
            );
        }

        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &node2_encryption_pk)
            .expect("Failed to decrypt body content");

        if let MessageBody::Unencrypted(shinkai_body) = decrypted_message.body {
            if let MessageData::Unencrypted(shinkai_data) = shinkai_body.message_data {
                assert_eq!(shinkai_data.message_raw_content, "body content");
            }
        }

        assert_eq!(message.encryption, EncryptionMethod::None);
        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, sender);
        assert!(verify_signature(&my_identity_pk, &message_clone).unwrap())
    }

    #[test]
    fn test_builder_with_all_fields_onion_encryption() {}

    #[test]
    fn test_builder_use_code_registration() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "main".to_string();

        let code = "registration_code".to_string();
        let identity_type = IdentityPermissions::Admin.to_string();
        let permission_type = "profile".to_string();
        let registration_name = "registration_name".to_string();

        let message_result = ShinkaiMessageBuilder::use_code_registration(
            my_encryption_sk.clone(),
            my_identity_sk,
            node2_encryption_pk,
            code,
            identity_type,
            permission_type,
            registration_name,
            sender.clone(),
            recipient.clone(),
        );
        println!("message_result: {:?}", message_result);
        assert!(message_result.is_ok());
        let message = message_result.unwrap();
        let message_clone = message.clone();

        assert_eq!(message.encryption, EncryptionMethod::DiffieHellmanChaChaPoly1305);

        let decrypted_message = message
            .decrypt_outer_layer(&my_encryption_sk, &node2_encryption_pk)
            .expect("Failed to decrypt body content");

        if let MessageBody::Unencrypted(shinkai_body) = decrypted_message.body {
            if let MessageData::Unencrypted(shinkai_data) = shinkai_body.message_data {
                // Parse the decrypted content from a JSON string to a RegistrationCode struct
                let parsed_content: Result<RegistrationCode, _> =
                    serde_json::from_str(&shinkai_data.message_raw_content);
                match &parsed_content {
                    Ok(registration_code) => {
                        println!("Parsed content: {:?}", registration_code);
                    }
                    Err(e) => {
                        eprintln!("Failed to parse content: {:?}", e);
                    }
                }

                let registration_code = parsed_content.unwrap();
                assert_eq!(registration_code.code, "registration_code");
                assert_eq!(registration_code.registration_name, "registration_name");
                assert_eq!(registration_code.permission_type, "profile");
                assert_eq!(registration_code.identity_type, "admin");
            }
            assert_eq!(shinkai_body.internal_metadata.sender_subidentity, sender);
        }

        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, recipient);
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

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk).build();
        assert!(message_result.is_err());
    }
}
