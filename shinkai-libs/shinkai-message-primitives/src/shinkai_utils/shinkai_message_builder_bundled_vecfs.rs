use crate::{
    schemas::shinkai_subscription_req::SubscriptionPayment,
    shinkai_message::shinkai_message_schemas::{
        APIAvailableSharedItems, APIConvertFilesAndSaveToFolder, APISubscribeToSharedFolder,
        APIVecFSRetrieveVectorResource, APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsMoveFolder,
        APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson, APIVecFsRetrieveVectorSearchSimplifiedJson,
        SubscriptionGenericResponse, SubscriptionResponseStatus,
    },
};
use ed25519_dalek::SigningKey;
use serde::Serialize;
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

use crate::{
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::MessageSchemaType},
    shinkai_utils::encryption::EncryptionMethod,
};

use super::shinkai_message_builder::{ShinkaiMessageBuilder, ShinkaiNameString};

impl ShinkaiMessageBuilder {
    fn create_vecfs_message(
        payload: impl Serialize,
        schema_type: MessageSchemaType,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsCreateFolder {
            path: path.to_string(),
            folder_name: folder_name.to_string(),
        };

        Self::create_vecfs_message(
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsMoveFolder {
            origin_path: origin_path.to_string(),
            destination_path: destination_path.to_string(),
        };

        Self::create_vecfs_message(
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsCopyFolder {
            origin_path: origin_path.to_string(),
            destination_path: destination_path.to_string(),
        };

        Self::create_vecfs_message(
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsMoveItem {
            origin_path: origin_path.to_string(),
            destination_path: destination_path.to_string(),
        };

        Self::create_vecfs_message(
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsCopyItem {
            origin_path: origin_path.to_string(),
            destination_path: destination_path.to_string(),
        };

        Self::create_vecfs_message(
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIConvertFilesAndSaveToFolder {
            path: destination_path.to_string(),
            file_inbox: file_inbox.to_string(),
        };

        Self::create_vecfs_message(
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFSRetrieveVectorResource { path: path.to_string() };

        Self::create_vecfs_message(
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsRetrievePathSimplifiedJson { path: path.to_string() };

        Self::create_vecfs_message(
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
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsRetrieveVectorSearchSimplifiedJson {
            search: search.to_string(),
            path: path.map(|x| x.to_string()),
            max_results: max_results.copied(),
            max_files_to_scan: max_files_to_scan.copied(),
        };

        Self::create_vecfs_message(
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

    // TODO: to be able to manage an error as well
    // for that we need a new struct to manage the resp and error
    pub fn vecfs_available_shared_items_response(
        results: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        Self::create_vecfs_message(
            results,
            MessageSchemaType::AvailableSharedItemsResponse,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_available_shared_items(
        path: Option<String>,
        streamer_node_name: String,
        streamer_profile_name: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIAvailableSharedItems {
            path: path.unwrap_or_else(|| "/".to_string()),
            streamer_node_name,
            streamer_profile_name,
        };

        Self::create_vecfs_message(
            payload,
            MessageSchemaType::AvailableSharedItems,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_subscribe_to_shared_folder(
        shared_folder: String,
        requirements: SubscriptionPayment,
        streamer_node: String,
        streamer_profile: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APISubscribeToSharedFolder {
            path: shared_folder,
            streamer_node_name: streamer_node,
            streamer_profile_name: streamer_profile,
            payment: requirements,
        };

        Self::create_vecfs_message(
            payload,
            MessageSchemaType::SubscribeToSharedFolder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_request_share_current_shared_folder_state(
        shared_folder_path: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        Self::create_vecfs_message(
            shared_folder_path,
            MessageSchemaType::SubscriptionRequiresTreeUpdate,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn vecfs_share_current_shared_folder_state(
        tree_item_response: SubscriptionGenericResponse,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_profile: String,
        node_receiver: ShinkaiNameString,
        node_receiver_profile: String,
    ) -> Result<ShinkaiMessage, &'static str> {
        Self::create_vecfs_message(
            tree_item_response,
            MessageSchemaType::SubscriptionRequiresTreeUpdateResponse,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_profile,
            node_receiver,
            node_receiver_profile,
        )
    }

    pub fn my_subscriptions(
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        Self::create_vecfs_message(
            // Note(Nico): we could change this but it works for now
            "".to_string(),
            MessageSchemaType::MySubscriptions,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    pub fn p2p_subscription_generic_response(
        response: SubscriptionGenericResponse,
        schema_type: MessageSchemaType,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        Self::create_vecfs_message(
            response,
            schema_type,
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
