use crate::{
    shinkai_message::shinkai_message_schemas::{
        APIConvertFilesAndSaveToFolder, APIVecFSRetrieveVectorResource, APIVecFsCopyFolder, APIVecFsCopyItem,
        APIVecFsCreateFolder, APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson,
        APIVecFsRetrieveVectorSearchSimplifiedJson,
    },
    shinkai_utils::job_scope::JobScope,
};
use ed25519_dalek::{SigningKey, VerifyingKey};
use serde::Serialize;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{
    schemas::{
        agents::serialized_agent::SerializedAgent, inbox_name::InboxName, registration_code::RegistrationCode,
        shinkai_time::ShinkaiStringTime,
    },
    shinkai_message::{
        shinkai_message::{
            ExternalMetadata, InternalMetadata, MessageBody, MessageData, ShinkaiBody, ShinkaiData, ShinkaiMessage,
            ShinkaiVersion,
        },
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
    encryption::{clone_static_secret_key, encryption_secret_key_to_string, unsafe_deterministic_encryption_keypair},
    shinkai_message_builder::{ProfileName, ShinkaiMessageBuilder},
    signatures::{clone_signature_secret_key, signature_secret_key_to_string},
};

impl ShinkaiMessageBuilder {
    fn create_vecfs_message(
        payload: impl Serialize,
        schema_type: MessageSchemaType,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let body = serde_json::to_string(&payload).map_err(|_| "Failed to serialize job creation to JSON")?;

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(body)
            .internal_metadata_with_schema(
                sender_subidentity.clone(),
                node_receiver_subidentity.clone(),
                "".to_string(),
                schema_type,
                EncryptionMethod::None,
                None,
            )
            .body_encryption(EncryptionMethod::DiffieHellmanChaChaPoly1305)
            .external_metadata_with_intra_sender(node_receiver, sender, sender_subidentity)
            .build()
    }

    pub fn vecfs_create_folder(
        folder_name: &str,
        path: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsCreateFolder {
            path: path.to_string(),
            folder_name: folder_name.to_string(),
        };

        ShinkaiMessageBuilder::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsCreateFolder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_move_folder(
        origin_path: &str,
        destination_path: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsMoveFolder {
            origin_path: origin_path.to_string(),
            destination_path: destination_path.to_string(),
        };

        ShinkaiMessageBuilder::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsMoveFolder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_copy_folder(
        origin_path: &str,
        destination_path: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsCopyFolder {
            origin_path: origin_path.to_string(),
            destination_path: destination_path.to_string(),
        };

        ShinkaiMessageBuilder::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsCopyFolder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_move_item(
        origin_path: &str,
        destination_path: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsMoveItem {
            origin_path: origin_path.to_string(),
            destination_path: destination_path.to_string(),
        };

        ShinkaiMessageBuilder::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsMoveItem,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_copy_item(
        origin_path: &str,
        destination_path: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsCopyItem {
            origin_path: origin_path.to_string(),
            destination_path: destination_path.to_string(),
        };

        ShinkaiMessageBuilder::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsCopyItem,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_create_items(
        destination_path: &str,
        file_inbox: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIConvertFilesAndSaveToFolder {
            path: destination_path.to_string(),
            file_inbox: file_inbox.to_string(),
        };

        ShinkaiMessageBuilder::create_vecfs_message(
            payload,
            MessageSchemaType::ConvertFilesAndSaveToFolder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_retrieve_resource(
        path: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFSRetrieveVectorResource { path: path.to_string() };

        ShinkaiMessageBuilder::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsRetrieveVectorResource,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_retrieve_path_simplified(
        path: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsRetrievePathSimplifiedJson { path: path.to_string() };

        ShinkaiMessageBuilder::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsRetrievePathSimplifiedJson,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_retrieve_vector_search_simplified(
        search: &str,
        path: Option<&str>,
        max_results: Option<&usize>,
        max_files_to_scan: Option<&usize>,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ProfileName,
        sender_subidentity: String,
        node_receiver: ProfileName,
        node_receiver_subidentity: ProfileName,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsRetrieveVectorSearchSimplifiedJson {
            search: search.to_string(),
            path: path.map(|x| x.to_string()),
            max_results: max_results.copied(),
            max_files_to_scan: max_files_to_scan.copied(),
        };

        ShinkaiMessageBuilder::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsRetrieveVectorSearchSimplifiedJson,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }
}
