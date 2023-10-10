use crate::shinkai_utils::job_scope::JobScope;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use serde::Serialize;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{
    schemas::{
        agents::serialized_agent::SerializedAgent, inbox_name::InboxName, registration_code::RegistrationCode,
        shinkai_time::ShinkaiTime,
    },
    shinkai_message::{
        shinkai_message::{
            EncryptedShinkaiBody, EncryptedShinkaiData, ExternalMetadata, InternalMetadata, MessageBody, MessageData,
            ShinkaiBody, ShinkaiData, ShinkaiMessage, ShinkaiVersion,
        },
        shinkai_message_schemas::{
            APIAddAgentRequest, APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, IdentityPermissions,
            JobCreationInfo, JobMessage, MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType,
        },
    },
    shinkai_utils::{
        encryption::{encryption_public_key_to_string, EncryptionMethod},
        signatures::{sign_message, signature_public_key_to_string},
    },
};

use super::{
    encryption::{clone_static_secret_key, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair},
    signatures::{clone_signature_secret_key, signature_secret_key_to_string},
};

pub type ProfileName = String;

// TODO: refactor this so you don't need to give all the keys to the builder in new
// but rather give them to the build function that way you can have the two level of encryptions
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
    optional_second_public_key_receiver_node: Option<EncryptionPublicKey>,
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
            optional_second_public_key_receiver_node: None,
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
        self.message_content_schema = message_schema.clone();
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
        let intra_sender = "".to_string();
        let scheduled_time = ShinkaiTime::generate_time_now();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other,
            intra_sender,
        });
        self
    }

    pub fn external_metadata_with_other(mut self, recipient: ProfileName, sender: ProfileName, other: String) -> Self {
        let signature = "".to_string();
        let intra_sender = "".to_string();
        let scheduled_time = ShinkaiTime::generate_time_now();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other,
            intra_sender,
        });
        self
    }

    pub fn external_metadata_with_other_and_intra_sender(
        mut self,
        recipient: ProfileName,
        sender: ProfileName,
        other: String,
        intra_sender: String,
    ) -> Self {
        let signature = "".to_string();
        let scheduled_time = ShinkaiTime::generate_time_now();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other,
            intra_sender,
        });
        self
    }

    pub fn external_metadata_with_intra_sender(
        mut self,
        recipient: ProfileName,
        sender: ProfileName,
        intra_sender: String,
    ) -> Self {
        let signature = "".to_string();
        let other = "".to_string();
        let scheduled_time = ShinkaiTime::generate_time_now();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other,
            intra_sender,
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
        let intra_sender = "".to_string();
        self.external_metadata = Some(ExternalMetadata {
            sender,
            recipient,
            scheduled_time,
            signature,
            other,
            intra_sender,
        });
        self
    }

    pub fn update_intra_sender(mut self, intra_sender: String) -> Self {
        if let Some(external_metadata) = &mut self.external_metadata {
            external_metadata.intra_sender = intra_sender;
        }
        self
    }

    pub fn set_optional_second_public_key_receiver_node(
        mut self,
        optional_second_public_key_receiver_node: EncryptionPublicKey,
    ) -> Self {
        self.optional_second_public_key_receiver_node = Some(optional_second_public_key_receiver_node);
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
            optional_second_public_key_receiver_node: self.optional_second_public_key_receiver_node.clone(),
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
            && new_self.optional_second_public_key_receiver_node == None
        {
            return Err("Encryption should not be set on both body and internal metadata simultaneously without optional_second_public_key_receiver_node.");
        }

        // Fix inbox name if it's empty
        if let Some(internal_metadata) = &mut new_self.internal_metadata {
            if internal_metadata.inbox.is_empty() {
                if let Some(external_metadata) = &new_self.external_metadata {
                    // Generate a new inbox name
                    // Print the value of external_metadata.sender to the browser console
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
            let data = ShinkaiData {
                message_raw_content: new_self.message_raw_content.clone(),
                message_content_schema: new_self.message_content_schema.clone(),
            };

            // if self.internal_metadata.encryption is not None
            let new_message_data = if internal_metadata.encryption != encryption_method_none {
                let encrypted_content = MessageData::encrypt_message_data(
                    &data,
                    &new_self.my_encryption_secret_key,
                    &new_self.receiver_public_key,
                )
                .expect("Failed to encrypt data content");

                encrypted_content
            } else {
                // If encryption method is None, just return body
                MessageData::Unencrypted(data.clone())
            };

            let mut unsigned_msg = ShinkaiMessage {
                body: MessageBody::Unencrypted(ShinkaiBody {
                    message_data: new_message_data.clone(),
                    internal_metadata: internal_metadata.clone(),
                }),
                encryption: new_self.encryption.clone(),
                external_metadata: new_self.external_metadata.clone().unwrap(),
                version: new_self.version.clone(),
            };

            // Sign inner layer
            unsigned_msg
                .sign_inner_layer(&new_self.my_signature_secret_key)
                .map_err(|_| "Failed to sign body")?;

            let signed_body = match unsigned_msg.body {
                MessageBody::Unencrypted(ref body) => ShinkaiBody {
                    message_data: new_message_data.clone(),
                    internal_metadata: body.internal_metadata.clone(),
                },
                _ => return Err("Expected unencrypted message body"),
            };

            // if self.encryption is not None
            let new_body = if new_self.encryption != encryption_method_none {
                let second_public_key = new_self
                    .optional_second_public_key_receiver_node
                    .as_ref()
                    .unwrap_or(&new_self.receiver_public_key);

                let encrypted_body = MessageBody::encrypt_message_body(
                    &signed_body,
                    &new_self.my_encryption_secret_key,
                    &second_public_key,
                )
                .expect("Failed to encrypt body");

                encrypted_body
            } else {
                // println!("No encryption");
                // If encryption method is None, just return body
                MessageBody::Unencrypted(signed_body.clone())
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
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let job_creation = JobCreationInfo { scope };
        let body = serde_json::to_string(&job_creation).map_err(|_| "Failed to serialize job creation to JSON")?;

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(body)
            .internal_metadata_with_schema(
                sender_subidentity.to_string(),
                node_receiver_subidentity.clone(),
                "".to_string(),
                MessageSchemaType::JobCreationSchema,
                EncryptionMethod::None,
            )
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .external_metadata_with_intra_sender(node_receiver, sender, sender_subidentity)
            .build()
    }

    pub fn job_message(
        job_id: String,
        content: String,
        files_inbox: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        node_sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let job_id_clone = job_id.clone();
        let job_message = JobMessage {
            job_id,
            content,
            files_inbox,
        };
        let body = serde_json::to_string(&job_message).map_err(|_| "Failed to serialize job message to JSON")?;

        let inbox = InboxName::get_job_inbox_name_from_params(job_id_clone)
            .map_err(|_| "Failed to get job inbox name")?
            .to_string();

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(body)
            .internal_metadata_with_schema(
                sender_subidentity.to_string(),
                node_receiver_subidentity.clone(),
                inbox,
                MessageSchemaType::JobMessageSchema,
                EncryptionMethod::None,
            )
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .external_metadata_with_intra_sender(node_receiver, node_sender, sender_subidentity)
            .build()
    }

    pub fn job_message_from_agent(
        job_id: String,
        content: String,
        my_signature_secret_key: SignatureStaticKey,
        node_sender: ProfileName,
        node_receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let job_id_clone = job_id.clone();
        let job_message = JobMessage {
            job_id,
            content,
            files_inbox: "".to_string(),
        };
        let body = serde_json::to_string(&job_message).map_err(|_| "Failed to serialize job message to JSON")?;

        let inbox = InboxName::get_job_inbox_name_from_params(job_id_clone)
            .map_err(|_| "Failed to get job inbox name")?
            .to_string();

        // Use for placeholder. These messages *are not* encrypted so it's not required
        let (placeholder_encryption_sk, placeholder_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        ShinkaiMessageBuilder::new(
            placeholder_encryption_sk,
            my_signature_secret_key,
            placeholder_encryption_pk,
        )
        .message_raw_content(body)
        .internal_metadata_with_schema(
            "".to_string(),
            "".to_string(),
            inbox,
            MessageSchemaType::JobMessageSchema,
            EncryptionMethod::None,
        )
        .body_encryption(EncryptionMethod::None)
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
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let registration_code_request = RegistrationCodeRequest { permissions, code_type };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            registration_code_request,
            sender_subidentity,
            sender,
            receiver,
            MessageSchemaType::CreateRegistrationCode,
        )
    }

    pub fn use_code_registration_for_profile(
        profile_encryption_sk: EncryptionStaticKey,
        profile_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let profile_signature_pk = ed25519_dalek::PublicKey::from(&profile_signature_sk);
        let profile_encryption_pk = x25519_dalek::PublicKey::from(&profile_encryption_sk);

        let registration_code = RegistrationCode {
            code,
            registration_name: registration_name.clone(),
            device_identity_pk: "".to_string(),
            device_encryption_pk: "".to_string(),
            profile_identity_pk: signature_public_key_to_string(profile_signature_pk),
            profile_encryption_pk: encryption_public_key_to_string(profile_encryption_pk),
            identity_type,
            permission_type,
        };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            profile_encryption_sk,
            profile_signature_sk,
            receiver_public_key,
            registration_code,
            sender_subidentity,
            sender,
            receiver,
            MessageSchemaType::UseRegistrationCode,
        )
    }

    pub fn use_code_registration_for_device(
        my_device_encryption_sk: EncryptionStaticKey,
        my_device_signature_sk: SignatureStaticKey,
        profile_encryption_sk: EncryptionStaticKey,
        profile_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let my_device_signature_pk = ed25519_dalek::PublicKey::from(&my_device_signature_sk);
        let my_device_encryption_pk = x25519_dalek::PublicKey::from(&my_device_encryption_sk);
        let profile_signature_pk = ed25519_dalek::PublicKey::from(&profile_signature_sk);
        let profile_encryption_pk = x25519_dalek::PublicKey::from(&profile_encryption_sk);
        let other = encryption_public_key_to_string(my_device_encryption_pk);

        let registration_code = RegistrationCode {
            code,
            registration_name: registration_name.clone(),
            device_identity_pk: signature_public_key_to_string(my_device_signature_pk),
            device_encryption_pk: other.clone(),
            profile_identity_pk: signature_public_key_to_string(profile_signature_pk),
            profile_encryption_pk: encryption_public_key_to_string(profile_encryption_pk),
            identity_type,
            permission_type,
        };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_device_encryption_sk,
            my_device_signature_sk,
            receiver_public_key,
            registration_code,
            sender_subidentity,
            sender,
            receiver,
            MessageSchemaType::UseRegistrationCode,
        )
    }

    pub fn initial_registration_with_no_code_for_device(
        my_device_encryption_sk: EncryptionStaticKey,
        my_device_signature_sk: SignatureStaticKey,
        profile_encryption_sk: EncryptionStaticKey,
        profile_signature_sk: SignatureStaticKey,
        registration_name: String,
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let my_device_signature_pk = ed25519_dalek::PublicKey::from(&my_device_signature_sk);
        let my_device_encryption_pk = x25519_dalek::PublicKey::from(&my_device_encryption_sk);
        let profile_signature_pk = ed25519_dalek::PublicKey::from(&profile_signature_sk);
        let profile_encryption_pk = x25519_dalek::PublicKey::from(&profile_encryption_sk);
        let other = encryption_public_key_to_string(my_device_encryption_pk);

        let identity_type = "device".to_string();
        let permission_type = "admin".to_string();

        let registration_code = RegistrationCode {
            code: "".to_string(),
            registration_name: registration_name.clone(),
            device_identity_pk: signature_public_key_to_string(my_device_signature_pk),
            device_encryption_pk: other.clone(),
            profile_identity_pk: signature_public_key_to_string(profile_signature_pk),
            profile_encryption_pk: encryption_public_key_to_string(profile_encryption_pk),
            identity_type,
            permission_type,
        };

        let body = serde_json::to_string(&registration_code).map_err(|_| "Failed to serialize data to JSON")?;
        let other = encryption_public_key_to_string(my_device_encryption_pk.clone());

        ShinkaiMessageBuilder::new(my_device_encryption_sk, my_device_signature_sk, my_device_encryption_pk)
            .message_raw_content(body)
            .body_encryption(EncryptionMethod::None)
            .internal_metadata_with_schema(
                sender_subidentity,
                "".to_string(),
                "".to_string(),
                MessageSchemaType::UseRegistrationCode,
                EncryptionMethod::None,
            )
            .external_metadata_with_other(receiver.clone(), sender, other)
            .build()
    }

    pub fn create_files_inbox_with_sym_key(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        symmetric_key_sk: String,
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
        )
        .message_raw_content(symmetric_key_sk)
        .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
        .internal_metadata_with_schema(
            sender_subidentity.clone(),
            "".to_string(),
            inbox.to_string(),
            MessageSchemaType::SymmetricKeyExchange,
            EncryptionMethod::None,
        )
        .external_metadata_with_intra_sender(receiver.clone(), sender, sender_subidentity)
        .build()
    }

    pub fn get_all_inboxes_for_profile(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        full_profile: String,
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
        )
        .message_raw_content(full_profile)
        .internal_metadata_with_schema(
            sender_subidentity.clone(),
            "".to_string(),
            "".to_string(),
            MessageSchemaType::TextContent,
            EncryptionMethod::None,
        )
        .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
        .external_metadata_with_intra_sender(receiver, sender, sender_subidentity)
        .build()
    }

    pub fn get_last_messages_from_inbox(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        count: usize,
        offset: Option<String>,
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let inbox_name = InboxName::new(inbox).map_err(|_| "Failed to create inbox name")?;
        let get_last_messages_from_inbox = APIGetMessagesFromInboxRequest {
            inbox: inbox_name.to_string(),
            count,
            offset,
        };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            get_last_messages_from_inbox,
            sender_subidentity,
            sender,
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
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let inbox_name = InboxName::new(inbox).map_err(|_| "Failed to create inbox name")?;
        let get_last_unread_messages_from_inbox = APIGetMessagesFromInboxRequest {
            inbox: inbox_name.to_string(),
            count,
            offset,
        };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            get_last_unread_messages_from_inbox,
            sender_subidentity,
            sender,
            receiver,
            MessageSchemaType::APIGetMessagesFromInboxRequest,
        )
    }

    pub fn request_add_agent(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        agent: SerializedAgent,
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let add_agent = APIAddAgentRequest { agent };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            add_agent,
            sender_subidentity,
            sender,
            receiver,
            MessageSchemaType::APIAddAgentRequest,
        )
    }

    pub fn read_up_to_time(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        up_to_time: String,
        sender_subidentity: String,
        sender: ProfileName,
        receiver: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let inbox_name = InboxName::new(inbox).map_err(|_| "Failed to create inbox name")?;
        let read_up_to_time = APIReadUpToTimeRequest { inbox_name, up_to_time };

        ShinkaiMessageBuilder::create_custom_shinkai_message_to_node(
            my_subidentity_encryption_sk,
            my_subidentity_signature_sk,
            receiver_public_key,
            read_up_to_time,
            sender_subidentity,
            sender,
            receiver,
            MessageSchemaType::APIReadUpToTimeRequest,
        )
    }

    pub fn create_custom_shinkai_message_to_node<T: Serialize>(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SignatureStaticKey,
        receiver_public_key: EncryptionPublicKey,
        data: T,
        sender_subidentity: String,
        sender: ProfileName,
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
            sender_subidentity,
            "".to_string(),
            "".to_string(),
            schema,
            EncryptionMethod::None,
        )
        .external_metadata_with_other(receiver.clone(), sender, other)
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
        let scheduled_time = "2023-07-02T20:53:34Z".to_string();

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
        let scheduled_time = "2023-07-02T20:53:34Z".to_string();

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
        let scheduled_time = "2023-07-02T20:53:34Z".to_string();

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

        let (profile_identity_sk, profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (profile_encryption_sk, profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(2);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = recipient.clone();
        let sender_subidentity = "main".to_string();

        let code = "registration_code".to_string();
        let identity_type = IdentityPermissions::Admin.to_string();
        let permission_type = "profile".to_string();
        let registration_name = "registration_name".to_string();

        let message_result = ShinkaiMessageBuilder::use_code_registration_for_device(
            my_encryption_sk.clone(),
            my_identity_sk,
            profile_encryption_sk,
            profile_identity_sk,
            node2_encryption_pk,
            code,
            identity_type,
            permission_type,
            registration_name,
            sender_subidentity.clone(),
            sender.clone(),
            recipient.clone(),
        );
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
            assert_eq!(shinkai_body.internal_metadata.sender_subidentity, sender_subidentity);
        }

        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, recipient);
        assert!(verify_signature(&my_identity_pk, &message_clone).unwrap())
    }

    #[test]
    fn test_initial_registration_with_no_code_for_device() {
        let (my_device_identity_sk, my_device_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_device_encryption_sk, my_device_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (profile_identity_sk, profile_identity_pk) = unsafe_deterministic_signature_keypair(1);
        let (profile_encryption_sk, profile_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = recipient.clone();
        let sender_subidentity = "main".to_string();

        let registration_name = "registration_name".to_string();

        let message_result = ShinkaiMessageBuilder::initial_registration_with_no_code_for_device(
            my_device_encryption_sk.clone(),
            my_device_identity_sk,
            profile_encryption_sk,
            profile_identity_sk,
            registration_name.clone(),
            sender_subidentity.clone(),
            sender.clone(),
            recipient.clone(),
        );
        assert!(message_result.is_ok());
        let message = message_result.unwrap();

        assert_eq!(message.encryption, EncryptionMethod::None);

        if let MessageBody::Unencrypted(shinkai_body) = message.body {
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
                assert_eq!(registration_code.code, "");
                assert_eq!(registration_code.registration_name, registration_name);
                assert_eq!(registration_code.permission_type, "admin");
                assert_eq!(registration_code.identity_type, "device");
            }
            assert_eq!(shinkai_body.internal_metadata.sender_subidentity, sender_subidentity);
        }

        let external_metadata = message.external_metadata;
        assert_eq!(external_metadata.sender, recipient);
    }

    #[test]
    fn test_builder_missing_fields() {
        let (my_identity_sk, my_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (my_encryption_sk, my_encryption_pk) = unsafe_deterministic_encryption_keypair(0);
        let (_, node2_encryption_pk) = unsafe_deterministic_encryption_keypair(1);

        let recipient = "@@other_node.shinkai".to_string();
        let sender = "@@my_node.shinkai".to_string();
        let scheduled_time = "2023-07-02T20:53:34Z".to_string();

        let message_result = ShinkaiMessageBuilder::new(my_encryption_sk, my_identity_sk, node2_encryption_pk).build();
        assert!(message_result.is_err());
    }
}
