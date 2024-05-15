use crate::shinkai_wasm_wrappers::shinkai_message_builder_wrapper::ShinkaiMessageBuilderWrapper;
use serde_wasm_bindgen::from_value;
use shinkai_message_primitives::{
    schemas::shinkai_subscription_req::SubscriptionPayment,
    shinkai_message::shinkai_message_schemas::{
        APIAvailableSharedItems, APICreateShareableFolder, APISubscribeToSharedFolder, APIUnsubscribeToSharedFolder,
        MessageSchemaType, SubscriptionGenericResponse,
    },
    shinkai_utils::shinkai_message_builder::ShinkaiNameString,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl ShinkaiMessageBuilderWrapper {
    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn subscriptions_create_share_folder(
        payload_create_shareable_folder: JsValue,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let payload: APICreateShareableFolder =
            from_value(payload_create_shareable_folder).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let body = serde_json::to_string(&payload).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::CreateShareableFolder.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_subscribe_to_shared_folder(
        shared_folder: String,
        requirements: JsValue,
        http_preferred: Option<bool>,
        base_folder: Option<String>,
        streamer_node: String,
        streamer_profile: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let payment: SubscriptionPayment = from_value(requirements).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let payload = APISubscribeToSharedFolder {
            path: shared_folder,
            streamer_node_name: streamer_node,
            streamer_profile_name: streamer_profile,
            payment,
            http_preferred,
            base_folder,
        };
        let body = serde_json::to_string(&payload).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::SubscribeToSharedFolder.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn subscription_unsubscribe_to_shared_folder(
        shared_folder: String,
        streamer_node: String,
        streamer_profile: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let payload = APIUnsubscribeToSharedFolder {
            path: shared_folder,
            streamer_node_name: streamer_node,
            streamer_profile_name: streamer_profile,
        };
        let body = serde_json::to_string(&payload).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::UnsubscribeToSharedFolder.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn subscription_available_shared_items_response(
        results: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let schema = MessageSchemaType::AvailableSharedItemsResponse.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            results,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn subscription_available_shared_items(
        path: Option<String>,
        streamer_node_name: String,
        streamer_profile_name: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let payload = APIAvailableSharedItems {
            path: path.unwrap_or_else(|| "/".to_string()),
            streamer_node_name,
            streamer_profile_name,
        };
        let body = serde_json::to_string(&payload).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::AvailableSharedItems.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn subscription_request_share_current_shared_folder_state(
        shared_folder_path: String,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let schema = MessageSchemaType::SubscriptionRequiresTreeUpdate.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            shared_folder_path,
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            other,
            schema,
        )
    }
    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn subscription_share_current_shared_folder_state(
        tree_item_response: JsValue,
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_profile: String,
        node_receiver: ShinkaiNameString,
        node_receiver_profile: String,
    ) -> Result<String, JsValue> {
        let payload: SubscriptionGenericResponse =
            from_value(tree_item_response).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let body = serde_json::to_string(&payload).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::SubscriptionRequiresTreeUpdateResponse
            .to_str()
            .to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_profile,
            node_receiver,
            node_receiver_profile,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn subscription_my_subscriptions(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        node_receiver: ShinkaiNameString,
        node_receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let schema = MessageSchemaType::MySubscriptions.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            "".to_string(), // Empty string as per the original function's note
            sender,
            sender_subidentity,
            node_receiver,
            node_receiver_subidentity,
            other,
            schema,
        )
    }
}
