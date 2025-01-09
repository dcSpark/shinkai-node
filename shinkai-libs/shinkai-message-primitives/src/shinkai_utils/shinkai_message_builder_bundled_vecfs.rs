use crate::{
    schemas::{shinkai_proxy_builder_info::ShinkaiProxyBuilderInfo, shinkai_subscription_req::SubscriptionPayment},
    shinkai_message::shinkai_message_schemas::{
        APIAvailableSharedItems, APIConvertFilesAndSaveToFolder, APICreateShareableFolder, APIGetMySubscribers,
        APISubscribeToSharedFolder, APIUnshareFolder, APIUnsubscribeToSharedFolder, APIVecFSRetrieveVectorResource,
        APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsDeleteFolder, APIVecFsDeleteItem,
        APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson,
        APIVecFsRetrieveVectorSearchSimplifiedJson, SubscriptionGenericResponse,
    },
    shinkai_utils::encryption::encryption_public_key_to_string,
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
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    fn create_vecfs_message_with_proxy(
        payload: impl Serialize,
        schema_type: MessageSchemaType,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
        proxy_info: Option<ShinkaiProxyBuilderInfo>,
    ) -> Result<ShinkaiMessage, &'static str> {
        let body = serde_json::to_string(&payload).map_err(|_| "Failed to serialize job creation to JSON")?;

        // It will encrypt the message with the proxy's pk if the sender is localhost and we have a proxy
        let effective_receiver_public_key = if let Some(proxy) = proxy_info {
            if !sender.starts_with("@@localhost.") {
                receiver_public_key
            } else {
                proxy.proxy_enc_public_key
            }
        } else {
            receiver_public_key
        };

        // Convert the encryption secret key to a public key and print it
        let my_encryption_public_key = EncryptionPublicKey::from(&my_encryption_secret_key);
        let my_enc_string = encryption_public_key_to_string(my_encryption_public_key);

        ShinkaiMessageBuilder::new(
            my_encryption_secret_key,
            my_signature_secret_key,
            effective_receiver_public_key,
        )
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
        .external_metadata_with_other_and_intra_sender(node_receiver, sender, my_enc_string, sender_subidentity)
        .build()
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn vecfs_create_items(
        destination_path: &str,
        file_inbox: &str,
        file_datetime_iso8601: Option<&str>,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        // Note: upgrade from the deprecated methods
        let file_datetime_option = file_datetime_iso8601.and_then(|dt| {
            chrono::DateTime::parse_from_rfc3339(dt)
                .map(|parsed_dt| parsed_dt.with_timezone(&chrono::Utc))
                .ok()
        });
        let payload = APIConvertFilesAndSaveToFolder {
            path: destination_path.to_string(),
            file_inbox: file_inbox.to_string(),
            file_datetime: file_datetime_option,
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn vecfs_delete_item(
        path: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsDeleteItem { path: path.to_string() };

        Self::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsDeleteItem,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn vecfs_delete_folder(
        path: &str,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsDeleteFolder { path: path.to_string() };

        Self::create_vecfs_message(
            payload,
            MessageSchemaType::VecFsDeleteFolder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn vecfs_retrieve_path_simplified(
        path: &str,
        depth: Option<usize>,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIVecFsRetrievePathSimplifiedJson {
            path: path.to_string(),
            depth: depth,
        };

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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn subscriptions_create_share_folder(
        payload: APICreateShareableFolder,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        Self::create_vecfs_message(
            payload,
            MessageSchemaType::CreateShareableFolder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn subscriptions_unshare_folder(
        path: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIUnshareFolder { path };
        Self::create_vecfs_message(
            payload,
            MessageSchemaType::UnshareFolder,
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
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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
        proxy_info: Option<ShinkaiProxyBuilderInfo>,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIAvailableSharedItems {
            path: path.unwrap_or_else(|| "/".to_string()),
            streamer_node_name,
            streamer_profile_name,
        };

        Self::create_vecfs_message_with_proxy(
            payload,
            MessageSchemaType::AvailableSharedItems,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            proxy_info,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn vecfs_subscribe_to_shared_folder(
        shared_folder: String,
        requirements: SubscriptionPayment,
        http_preferred: Option<bool>,
        base_folder: Option<String>,
        streamer_node: String,
        streamer_profile: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
        proxy_info: Option<ShinkaiProxyBuilderInfo>,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APISubscribeToSharedFolder {
            path: shared_folder,
            streamer_node_name: streamer_node,
            streamer_profile_name: streamer_profile,
            payment: requirements,
            http_preferred,
            base_folder,
        };

        Self::create_vecfs_message_with_proxy(
            payload,
            MessageSchemaType::SubscribeToSharedFolder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            proxy_info,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn vecfs_unsubscribe_to_shared_folder(
        shared_folder: String,
        streamer_node: String,
        streamer_profile: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
        proxy_info: Option<ShinkaiProxyBuilderInfo>,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIUnsubscribeToSharedFolder {
            path: shared_folder,
            streamer_node_name: streamer_node,
            streamer_profile_name: streamer_profile,
        };

        Self::create_vecfs_message_with_proxy(
            payload,
            MessageSchemaType::UnsubscribeToSharedFolder,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            proxy_info,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn get_my_subscribers(
        path: Option<String>,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: ShinkaiNameString,
    ) -> Result<ShinkaiMessage, &'static str> {
        let payload = APIGetMySubscribers {
            path: path.unwrap_or_else(|| "/".to_string()),
        };

        Self::create_vecfs_message(
            payload,
            MessageSchemaType::GetMySubscribers,
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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
