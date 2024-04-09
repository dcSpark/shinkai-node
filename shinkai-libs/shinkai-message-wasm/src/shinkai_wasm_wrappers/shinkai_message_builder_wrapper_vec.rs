use shinkai_message_primitives::{shinkai_message::shinkai_message_schemas::{APIConvertFilesAndSaveToFolder, APIVecFSRetrieveVectorResource, APIVecFsCopyFolder, APIVecFsCopyItem, APIVecFsCreateFolder, APIVecFsMoveFolder, APIVecFsMoveItem, APIVecFsRetrievePathSimplifiedJson, APIVecFsRetrieveVectorSearchSimplifiedJson, MessageSchemaType}, shinkai_utils::shinkai_message_builder::ShinkaiNameString};
use wasm_bindgen::prelude::*;
use crate::shinkai_wasm_wrappers::shinkai_message_builder_wrapper::ShinkaiMessageBuilderWrapper;

#[wasm_bindgen]
impl ShinkaiMessageBuilderWrapper {
    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_create_folder(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        folder_name: String,
        path: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let folder_creation_info = APIVecFsCreateFolder { folder_name, path };
        let body = serde_json::to_string(&folder_creation_info).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::VecFsCreateFolder.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_move_folder(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        origin_path: String,
        destination_path: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let folder_move_info = APIVecFsMoveFolder {
            origin_path,
            destination_path,
        };
        let body = serde_json::to_string(&folder_move_info).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::VecFsMoveFolder.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_copy_folder(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        origin_path: String,
        destination_path: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let folder_copy_info = APIVecFsCopyFolder {
            origin_path,
            destination_path,
        };
        let body = serde_json::to_string(&folder_copy_info).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::VecFsCopyFolder.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_move_item(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        origin_path: String,
        destination_path: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let item_move_info = APIVecFsMoveItem {
            origin_path,
            destination_path,
        };
        let body = serde_json::to_string(&item_move_info).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::VecFsMoveItem.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_copy_item(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        origin_path: String,
        destination_path: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let item_copy_info = APIVecFsCopyItem {
            origin_path,
            destination_path,
        };
        let body = serde_json::to_string(&item_copy_info).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::VecFsCopyItem.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_create_items(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        destination_path: String,
        file_inbox: String,
        file_datetime_iso8601: Option<String>,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let file_datetime_option = file_datetime_iso8601.and_then(|dt| {
            chrono::DateTime::parse_from_rfc3339(&dt)
                .map(|parsed_dt| parsed_dt.with_timezone(&chrono::Utc))
                .ok()
        });

        let create_items_info = APIConvertFilesAndSaveToFolder {
            path: destination_path,
            file_inbox,
            file_datetime: file_datetime_option,
        };
        let body = serde_json::to_string(&create_items_info).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::ConvertFilesAndSaveToFolder.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_retrieve_resource(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        path: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let retrieve_resource_info = APIVecFSRetrieveVectorResource { path };
        let body = serde_json::to_string(&retrieve_resource_info).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::VecFsRetrieveVectorResource.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_retrieve_path_simplified(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        path: String,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let retrieve_path_info = APIVecFsRetrievePathSimplifiedJson { path };
        let body = serde_json::to_string(&retrieve_path_info).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::VecFsRetrievePathSimplifiedJson.to_str().to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
            other,
            schema,
        )
    }

    #[wasm_bindgen]
    #[allow(clippy::too_many_arguments)]
    pub fn vecfs_retrieve_vector_search_simplified(
        my_encryption_secret_key: String,
        my_signature_secret_key: String,
        receiver_public_key: String,
        search: String,
        path: Option<String>,
        max_results: Option<usize>,
        max_files_to_scan: Option<usize>,
        sender: ShinkaiNameString,
        sender_subidentity: ShinkaiNameString,
        receiver: ShinkaiNameString,
        receiver_subidentity: String,
    ) -> Result<String, JsValue> {
        let search_info = APIVecFsRetrieveVectorSearchSimplifiedJson {
            search,
            path,
            max_results,
            max_files_to_scan,
        };
        let body = serde_json::to_string(&search_info).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let schema = MessageSchemaType::VecFsRetrieveVectorSearchSimplifiedJson
            .to_str()
            .to_string();
        let other = "";

        ShinkaiMessageBuilderWrapper::create_custom_shinkai_message_to_node(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            body,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
            other,
            schema,
        )
    }
}