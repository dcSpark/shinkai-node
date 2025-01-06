use crate::shinkai_message::shinkai_message::NodeApiData;
use ed25519_dalek::{SigningKey, VerifyingKey};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{
    schemas::inbox_name::InboxName,
    shinkai_message::{
        shinkai_message::{
            ExternalMetadata, InternalMetadata, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiMessage,
            ShinkaiVersion,
        },
        shinkai_message_schemas::MessageSchemaType,
    },
    shinkai_utils::{
        encryption::{encryption_public_key_to_string, EncryptionMethod},
        signatures::signature_public_key_to_string,
    },
};

use super::{
    encryption::{clone_static_secret_key, encryption_secret_key_to_string}, shinkai_time::ShinkaiStringTime, signatures::{clone_signature_secret_key, signature_secret_key_to_string}
};

pub type ShinkaiNameString = String;

// TODO: refactor this so you don't need to give all the keys to the builder in new
// but rather give them to the build function that way you can have the two level of encryptions
#[derive(Clone)]
pub struct ShinkaiMessageBuilder {
    message_raw_content: String,
    message_content_schema: MessageSchemaType,
    internal_metadata: Option<InternalMetadata>,
    external_metadata: Option<ExternalMetadata>,
    encryption: EncryptionMethod,
    my_encryption_secret_key: EncryptionStaticKey,
    my_encryption_public_key: EncryptionPublicKey,
    my_signature_secret_key: SigningKey,
    my_signature_public_key: VerifyingKey,
    receiver_public_key: EncryptionPublicKey,
    version: ShinkaiVersion,
    optional_second_public_key_receiver_node: Option<EncryptionPublicKey>,
}

impl ShinkaiMessageBuilder {
    pub fn new(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
    ) -> Self {
        let version = ShinkaiVersion::V1_0;
        let my_encryption_public_key = x25519_dalek::PublicKey::from(&my_encryption_secret_key);
        let my_signature_public_key = my_signature_secret_key.verifying_key();
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

    #[allow(dead_code)]
    pub fn message_schema_type(mut self, content: MessageSchemaType) -> Self {
        self.message_content_schema = content.clone();
        self
    }

    #[allow(dead_code)]
    pub fn internal_metadata(
        mut self,
        sender_subidentity: ShinkaiNameString,
        recipient_subidentity: String,
        encryption: EncryptionMethod,
        node_api_data: Option<NodeApiData>,
    ) -> Self {
        let signature = "".to_string();
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            inbox: "".to_string(),
            signature,
            encryption,
            node_api_data,
        });
        self
    }

    #[allow(dead_code)]
    pub fn internal_metadata_with_inbox(
        mut self,
        sender_subidentity: ShinkaiNameString,
        recipient_subidentity: String,
        inbox: String,
        encryption: EncryptionMethod,
        node_api_data: Option<NodeApiData>,
    ) -> Self {
        let signature = "".to_string();
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            inbox,
            signature,
            encryption,
            node_api_data,
        });
        self
    }

    pub fn internal_metadata_with_schema(
        mut self,
        sender_subidentity: ShinkaiNameString,
        recipient_subidentity: String,
        inbox: String,
        message_schema: MessageSchemaType,
        encryption: EncryptionMethod,
        node_api_data: Option<NodeApiData>,
    ) -> Self {
        let signature = "".to_string();
        self.message_content_schema = message_schema.clone();
        self.internal_metadata = Some(InternalMetadata {
            sender_subidentity,
            recipient_subidentity,
            inbox,
            signature,
            encryption,
            node_api_data,
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
            node_api_data: None,
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
            node_api_data: None,
        });
        self
    }

    pub fn external_metadata(mut self, recipient: ShinkaiNameString, sender: ShinkaiNameString) -> Self {
        let signature = "".to_string();
        let other = "".to_string();
        let intra_sender = "".to_string();
        let scheduled_time = ShinkaiStringTime::generate_time_now();
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

    pub fn external_metadata_with_other(
        mut self,
        recipient: ShinkaiNameString,
        sender: ShinkaiNameString,
        other: String,
    ) -> Self {
        let signature = "".to_string();
        let intra_sender = "".to_string();
        let scheduled_time = ShinkaiStringTime::generate_time_now();
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

    #[allow(dead_code)]
    pub fn external_metadata_with_other_and_intra_sender(
        mut self,
        recipient: ShinkaiNameString,
        sender: ShinkaiNameString,
        other: String,
        intra_sender: String,
    ) -> Self {
        let signature = "".to_string();
        let scheduled_time = ShinkaiStringTime::generate_time_now();
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
        recipient: ShinkaiNameString,
        sender: ShinkaiNameString,
        intra_sender: String,
    ) -> Self {
        let signature = "".to_string();
        let other = "".to_string();
        let scheduled_time = ShinkaiStringTime::generate_time_now();
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

    #[allow(dead_code)]
    pub fn external_metadata_with_schedule(
        mut self,
        recipient: ShinkaiNameString,
        sender: ShinkaiNameString,
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

    #[allow(dead_code)]
    pub fn update_intra_sender(mut self, intra_sender: String) -> Self {
        if let Some(external_metadata) = &mut self.external_metadata {
            external_metadata.intra_sender = intra_sender;
        }
        self
    }

    #[allow(dead_code)]
    pub fn update_node_api_data(mut self, node_api_data: NodeApiData) -> Self {
        if let Some(internal_metadata) = &mut self.internal_metadata {
            internal_metadata.node_api_data = Some(node_api_data);
        }
        self
    }

    #[allow(dead_code)]
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
        let my_signature_public_key_clone = my_signature_secret_key_clone.verifying_key();
        let receiver_public_key_clone = self.receiver_public_key;

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
            optional_second_public_key_receiver_node: self.optional_second_public_key_receiver_node,
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
            && new_self.optional_second_public_key_receiver_node.is_none()
        {
            return Err("Encryption should not be set on both body and internal metadata simultaneously without optional_second_public_key_receiver_node.");
        }

        // Fix inbox name if it's empty
        if let Some(internal_metadata) = &mut new_self.internal_metadata {
            if internal_metadata.inbox.is_empty() {
                if let Some(external_metadata) = &new_self.external_metadata {
                    // Generate a new inbox name
                    // Print the value of external_metadata.sender to the browser console
                    let new_inbox_name_result = InboxName::get_regular_inbox_name_from_params(
                        external_metadata.sender.clone(),
                        internal_metadata.sender_subidentity.clone(),
                        external_metadata.recipient.clone(),
                        internal_metadata.recipient_subidentity.clone(),
                        internal_metadata.encryption != EncryptionMethod::None,
                    );

                    if let Ok(new_inbox_name) = new_inbox_name_result {
                        // Update the inbox name in the internal metadata if generation was successful
                        internal_metadata.inbox = match new_inbox_name {
                            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
                        };
                    }
                }
                // If the inbox name generation fails, do not return an error and allow the inbox to remain empty
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
                MessageData::encrypt_message_data(
                    &data,
                    &new_self.my_encryption_secret_key,
                    &new_self.receiver_public_key,
                )
                .expect("Failed to encrypt data content")
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

                MessageBody::encrypt_message_body(&signed_body, &new_self.my_encryption_secret_key, second_public_key)
                    .expect("Failed to encrypt body")
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
}

impl std::fmt::Debug for ShinkaiMessageBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let encryption_sk_string = encryption_secret_key_to_string(self.my_encryption_secret_key.clone());
        let encryption_pk_string = encryption_public_key_to_string(self.my_encryption_public_key);
        let signature_sk_clone = clone_signature_secret_key(&self.my_signature_secret_key);
        let signature_sk_string = signature_secret_key_to_string(signature_sk_clone);
        let signature_pk_string = signature_public_key_to_string(self.my_signature_public_key);
        let receiver_pk_string = encryption_public_key_to_string(self.receiver_public_key);

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
