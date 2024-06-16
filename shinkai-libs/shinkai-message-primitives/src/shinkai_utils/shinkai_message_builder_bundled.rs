use crate::{schemas::shinkai_name::ShinkaiName, shinkai_utils::job_scope::JobScope};
use ed25519_dalek::SigningKey;
use serde::Serialize;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{
    schemas::{llm_providers::serialized_llm_provider::SerializedLLMProvider, inbox_name::InboxName, registration_code::RegistrationCode},
    shinkai_message::{
        shinkai_message::ShinkaiMessage,
        shinkai_message_schemas::{
            APIAddAgentRequest, APIGetMessagesFromInboxRequest, APIReadUpToTimeRequest, IdentityPermissions,
            JobCreationInfo, JobMessage, MessageSchemaType, RegistrationCodeRequest, RegistrationCodeType,
        },
    },
    shinkai_utils::{
        encryption::{encryption_public_key_to_string, EncryptionMethod},
        signatures::signature_public_key_to_string,
    },
};

use super::{
    encryption::unsafe_deterministic_encryption_keypair,
    shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString},
};

impl ShinkaiMessageBuilder {
    #[allow(dead_code)]
    pub fn ack_message(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content("ACK".to_string())
            .empty_non_encrypted_internal_metadata()
            .no_body_encryption()
            .external_metadata(receiver, sender)
            .build()
    }

    #[allow(dead_code)]
    pub fn ping_pong_message(
        message: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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


    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn change_node_name(
        new_name: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        node_sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let new_name = ShinkaiName::new(new_name).map_err(|_| "Failed to create new name")?;
        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(new_name.node_name)
            .internal_metadata_with_schema(
                sender_subidentity.to_string(),
                node_receiver_subidentity.clone(),
                "".to_string(),
                MessageSchemaType::ChangeNodesName,
                EncryptionMethod::None,
                None,
            )
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .external_metadata_with_intra_sender(node_receiver, node_sender, sender_subidentity)
            .build()
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn job_creation(
        scope: JobScope,
        is_hidden: bool,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let job_creation = JobCreationInfo {
            scope,
            is_hidden: Some(is_hidden),
        };
        let body = serde_json::to_string(&job_creation).map_err(|_| "Failed to serialize job creation to JSON")?;

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(body)
            .internal_metadata_with_schema(
                sender_subidentity.to_string(),
                node_receiver_subidentity.clone(),
                "".to_string(),
                MessageSchemaType::JobCreationSchema,
                EncryptionMethod::None,
                None,
            )
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .external_metadata_with_intra_sender(node_receiver, sender, sender_subidentity)
            .build()
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn job_message(
        job_id: String,
        content: String,
        files_inbox: String,
        parent_hash: String,
        workflow: Option<String>,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        node_sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let job_id_clone = job_id.clone();
        let job_message = JobMessage {
            job_id,
            content,
            files_inbox,
            parent: Some(parent_hash),
            workflow,
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
                None,
            )
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .external_metadata_with_intra_sender(node_receiver, node_sender, sender_subidentity)
            .build()
    }

    #[allow(dead_code)]
    pub fn job_message_from_llm_provider(
        job_id: String,
        content: String,
        files_inbox: String,
        my_signature_secret_key: SigningKey,
        node_sender: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let job_id_clone = job_id.clone();
        let job_message = JobMessage {
            job_id,
            content,
            files_inbox,
            parent: None,
            workflow: None, // the agent wont be sending you a workflow
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
            None,
        )
        .body_encryption(EncryptionMethod::None)
        .external_metadata(node_receiver, node_sender)
        .build()
    }

    #[allow(dead_code)]
    pub fn terminate_message(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content("terminate".to_string())
            .empty_non_encrypted_internal_metadata()
            .no_body_encryption()
            .external_metadata(receiver, sender)
            .build()
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn request_code_registration(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        permissions: IdentityPermissions,
        code_type: RegistrationCodeType,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn use_code_registration_for_profile(
        profile_encryption_sk: EncryptionStaticKey,
        profile_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let profile_signature_pk = profile_signature_sk.verifying_key();
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn use_code_registration_for_device(
        my_device_encryption_sk: EncryptionStaticKey,
        my_device_signature_sk: SigningKey,
        profile_encryption_sk: EncryptionStaticKey,
        profile_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        code: String,
        identity_type: String,
        permission_type: String,
        registration_name: String,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let my_device_signature_pk = my_device_signature_sk.verifying_key();
        let my_device_encryption_pk = x25519_dalek::PublicKey::from(&my_device_encryption_sk);
        let profile_signature_pk = profile_signature_sk.verifying_key();
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn initial_registration_with_no_code_for_device(
        my_device_encryption_sk: EncryptionStaticKey,
        my_device_signature_sk: SigningKey,
        profile_encryption_sk: EncryptionStaticKey,
        profile_signature_sk: SigningKey,
        registration_name: String,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let my_device_signature_pk = my_device_signature_sk.verifying_key();
        let my_device_encryption_pk = x25519_dalek::PublicKey::from(&my_device_encryption_sk);
        let profile_signature_pk = profile_signature_sk.verifying_key();
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

        ShinkaiMessageBuilder::new(my_device_encryption_sk, my_device_signature_sk, my_device_encryption_pk)
            .message_raw_content(body)
            .body_encryption(EncryptionMethod::None)
            .internal_metadata_with_schema(
                sender_subidentity,
                "".to_string(),
                "".to_string(),
                MessageSchemaType::UseRegistrationCode,
                EncryptionMethod::None,
                None,
            )
            .external_metadata_with_other(receiver.clone(), sender, other)
            .build()
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn create_files_inbox_with_sym_key(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        symmetric_key_sk: String,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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
            None,
        )
        .external_metadata_with_intra_sender(receiver.clone(), sender, sender_subidentity)
        .build()
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn get_all_inboxes_for_profile(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        full_profile: String,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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
            None,
        )
        .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
        .external_metadata_with_intra_sender(receiver, sender, sender_subidentity)
        .build()
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn get_last_messages_from_inbox(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        count: usize,
        offset: Option<String>,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn get_last_unread_messages_from_inbox(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        count: usize,
        offset: Option<String>,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn request_add_agent(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        agent: SerializedLLMProvider,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn read_up_to_time(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        inbox: String,
        up_to_time: String,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn create_custom_shinkai_message_to_node<T: Serialize>(
        my_subidentity_encryption_sk: EncryptionStaticKey,
        my_subidentity_signature_sk: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        data: T,
        sender_subidentity: ShinkaiNameString,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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
            None,
        )
        .external_metadata_with_other(receiver.clone(), sender, other)
        .build()
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn error_message(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        receiver: ShinkaiNameString,
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
