use async_channel::Sender;
use futures::StreamExt;
use futures::TryFutureExt;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use shinkai_message_primitives::{
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::APIAvailableSharedItems},
    shinkai_utils::{
        encryption::encryption_public_key_to_string,
        shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
        signatures::signature_public_key_to_string,
    },
};
use utoipa::ToSchema;
use warp::Buf;

use crate::network::node_commands::NodeCommand;
use crate::network::node_api_router::handle_node_command;
use crate::network::node_api_router::APIError;
use crate::network::node_api_router::GetPublicKeysResponse;
use crate::network::node_api_router::SendResponseBody;
use crate::network::node_api_router::SendResponseBodyData;

#[derive(serde::Deserialize)]
pub struct NameToExternalProfileData {
    name: String,
}

#[derive(serde::Serialize)]
pub struct IdentityNameToExternalProfileDataResponse {
    pub signature_public_key: String,
    pub encryption_public_key: String,
}

pub async fn ping_all_handler(node_commands_sender: Sender<NodeCommand>) -> Result<impl warp::Reply, warp::Rejection> {
    match node_commands_sender.send(NodeCommand::PingAll).await {
        Ok(_) => Ok(warp::reply::json(&json!({
            "result": "Pinged all nodes successfully"
        }))),
        Err(_) => Ok(warp::reply::json(&json!({
            "error": "Error occurred while pinging all nodes"
        }))),
    }
}

pub async fn api_subscription_available_shared_items_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIAvailableSharedItems {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_subscription_available_shared_items_open_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: APIAvailableSharedItems,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIAvailableSharedItemsOpen {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_subscription_create_shareable_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APICreateShareableFolder {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_subscription_update_shareable_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIUpdateShareableFolder {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_subscription_unshare_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIUnshareFolder {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn add_toolkit_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIAddToolkit {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn retrieve_vrkai_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::RetrieveVRKai {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn retrieve_vrpack_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::RetrieveVRPack {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_retrieve_path_simplified_json_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSRetrievePathSimplifiedJson {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_retrieve_path_minimal_json_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSRetrievePathMinimalJson {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_retrieve_vector_search_simplified_json_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSRetrieveVectorSearchSimplifiedJson {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_search_item_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSSearchItems {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_create_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSCreateFolder {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_move_item_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSMoveItem {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_copy_item_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSCopyItem {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_remove_item_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSDeleteItem {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_move_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSMoveFolder {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_remove_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSDeleteFolder {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_copy_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSCopyFolder {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_vec_fs_retrieve_vector_resource_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIVecFSRetrieveVectorResource {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_convert_files_and_save_to_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIConvertFilesAndSaveToFolder {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn scan_ollama_models_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APIScanOllamaModels {
            msg: message,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(models) => Ok(warp::reply::json(&models)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

pub async fn add_ollama_models_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APIAddOllamaModels {
            msg: message,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(_) => Ok(warp::reply::json(&json!({"status": "success"}))),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

pub async fn subscribe_to_shared_folder_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APISubscribeToSharedFolder {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn search_workflows_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APISearchWorkflows {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn add_workflow_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIAddWorkflow {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn update_workflow_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIUpdateWorkflow {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn delete_workflow_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIRemoveWorkflow {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn set_column_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APISetColumn {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn remove_column_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIRemoveColumn {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn add_row_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIAddRows {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn remove_row_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIRemoveRows {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn user_sheets_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIUserSheets {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn create_sheet_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APICreateSheet {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn remove_sheet_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIRemoveSheet {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn set_cell_value_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APISetCellValue {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn get_sheet_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIGetSheet {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn get_workflow_info_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIGetWorkflowInfo {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn list_all_workflows_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIListAllWorkflows {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_update_default_embedding_model_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIUpdateDefaultEmbeddingModel {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn api_update_supported_embedding_models_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIUpdateSupportedEmbeddingModels {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn unsubscribe_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIUnsubscribe {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn change_nodes_name_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIChangeNodesName {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn api_my_subscriptions_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(
        node_commands_sender,
        message,
        |_node_commands_sender, message, res_sender| NodeCommand::APIMySubscriptions {
            msg: message,
            res: res_sender,
        },
    )
    .await
}

pub async fn get_my_subscribers_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APIGetMySubscribers {
            msg: message,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let subscribers = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    Ok(warp::reply::json(&subscribers))
}

#[allow(clippy::type_complexity)]
pub async fn send_msg_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_send_msg_sender, res_send_msg_receiver): (
        async_channel::Sender<Result<SendResponseBodyData, APIError>>,
        async_channel::Receiver<Result<SendResponseBodyData, APIError>>,
    ) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::SendOnionizedMessage {
            msg: message,
            res: res_send_msg_sender,
        })
        .await
        .map_err(|e| warp::reject::custom(APIError::from(e)))?;
    let send_result = res_send_msg_receiver
        .recv()
        .await
        .map_err(|e| warp::reject::custom(APIError::from(format!("{}", e))))?;
    match send_result {
        Ok(data) => {
            let response_body = SendResponseBody {
                status: "success".to_string(),
                message: "Message sent successfully".to_string(),
                data: Some(data),
            };
            Ok(warp::reply::json(&response_body))
        }
        Err(api_error) => Err(warp::reject::custom(api_error)),
    }
}

pub async fn identity_name_to_external_profile_data_handler(
    node_commands_sender: Sender<NodeCommand>,
    body: NameToExternalProfileData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::IdentityNameToExternalProfileData {
            name: body.name,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let external_profile_data = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;
    Ok(warp::reply::json(&IdentityNameToExternalProfileDataResponse {
        signature_public_key: signature_public_key_to_string(external_profile_data.node_signature_public_key),
        encryption_public_key: encryption_public_key_to_string(external_profile_data.node_encryption_public_key),
    }))
}

pub async fn get_last_messages_from_inbox_with_branches_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIGetLastMessagesFromInboxWithBranches {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn handle_file_upload(
    node_commands_sender: Sender<NodeCommand>,
    public_key: String,
    encrypted_nonce: String,
    form: warp::multipart::FormData,
) -> Result<Box<dyn warp::Reply + Send>, warp::Rejection> {
    let mut stream = Box::pin(form.filter_map(|part_result| async move {
        if let Ok(part) = part_result {
            shinkai_log(
                ShinkaiLogOption::Identity,
                ShinkaiLogLevel::Debug,
                format!("Received file: {:?}", part).as_str(),
            );
            if let Some(filename) = part.filename() {
                let filename = filename.to_string();
                let stream = part
                    .stream()
                    .map(|res| res.map(|mut buf| buf.copy_to_bytes(buf.remaining()).to_vec()));
                return Some((filename, stream));
            }
        }
        None
    }));

    if let Some((filename, mut file_stream)) = stream.next().await {
        let mut file_data = Vec::new();
        while let Some(Ok(node)) = file_stream.next().await {
            file_data.extend(node);
        }

        let (res_sender, res_receiver) = async_channel::bounded(1);
        node_commands_sender
            .clone()
            .send(NodeCommand::APIAddFileToInboxWithSymmetricKey {
                filename,
                file: file_data,
                public_key,
                encrypted_nonce,
                res: res_sender,
            })
            .map_err(|_| warp::reject::reject())
            .await?;
        let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

        match result {
            Ok(message) => Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&message),
                StatusCode::OK,
            ))),
            Err(error) => Ok(Box::new(warp::reply::with_status(
                warp::reply::json(&error),
                StatusCode::from_u16(error.code).unwrap(),
            ))),
        }
    } else {
        Err(warp::reject::reject())
    }
}

pub async fn get_public_key_handler(
    node_commands_sender: Sender<NodeCommand>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::GetPublicKeys(res_sender))
        .await
        .map_err(|_| warp::reject::reject())?; // Send the command to Node
    let (signature_public_key, encryption_public_key) =
        res_receiver.recv().await.map_err(|_| warp::reject::reject())?;
    let signature_public_key_string = signature_public_key_to_string(signature_public_key);
    let encryption_public_key_string = encryption_public_key_to_string(encryption_public_key);
    Ok(warp::reply::json(&GetPublicKeysResponse {
        signature_public_key: signature_public_key_string,
        encryption_public_key: encryption_public_key_string,
    }))
}

pub async fn get_last_messages_from_inbox_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIGetLastMessagesFromInbox {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn get_last_unread_messages_from_inbox_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIGetLastUnreadMessagesFromInbox {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn get_all_inboxes_for_profile_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIGetAllInboxesForProfile {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn get_all_smart_inboxes_for_profile_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIGetAllSmartInboxesForProfile {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn update_smart_inbox_name_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIUpdateSmartInboxName {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn update_job_to_finished_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIUpdateJobToFinished {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn create_job_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APICreateJob {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn add_agent_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIAddAgent {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn modify_agent_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIModifyAgent {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn remove_agent_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIRemoveAgent {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn available_llm_providers_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIAvailableLLMProviders {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

#[allow(clippy::type_complexity)]
pub async fn job_message_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_job_msg_sender, res_job_msg_receiver): (
        async_channel::Sender<Result<SendResponseBodyData, APIError>>,
        async_channel::Receiver<Result<SendResponseBodyData, APIError>>,
    ) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APIJobMessage {
            msg: message,
            res: res_job_msg_sender,
        })
        .await
        .map_err(|e| warp::reject::custom(APIError::from(e)))?;
    let job_result = res_job_msg_receiver
        .recv()
        .await
        .map_err(|e| warp::reject::custom(APIError::from(format!("{}", e))))?;
    match job_result {
        Ok(data) => {
            let response_body = SendResponseBody {
                status: "Success".to_string(),
                message: "Job message processed successfully".to_string(),
                data: Some(data),
            };
            Ok(warp::reply::json(&response_body))
        }
        Err(api_error) => Err(warp::reject::custom(api_error)),
    }
}

pub async fn get_filenames_message_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIGetFilenamesInInbox {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn mark_as_read_up_to_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIMarkAsReadUpTo {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn create_files_inbox_with_symmetric_key_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APICreateFilesInboxWithSymmetricKey {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn create_registration_code_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APICreateRegistrationCode {
            msg: message,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?; // Send the command to Node
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(code) => {
            let response = serde_json::json!({ "code": code });
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

pub async fn get_subscription_links_handler(
    node_commands_sender: Sender<NodeCommand>,
    subscription_id: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APIGetHttpFreeSubscriptionLinks {
            subscription_profile_path: subscription_id,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;
    let links = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    Ok(warp::reply::json(&links))
}

pub async fn change_job_agent_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_, message, res_sender| {
        NodeCommand::APIChangeJobAgent {
            msg: message,
            res: res_sender,
        }
    })
    .await
}

pub async fn get_local_processing_preference_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_sender, msg, res| {
        NodeCommand::APIGetLocalProcessingPreference { msg, res }
    })
    .await
}

pub async fn get_last_notifications_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_sender, msg, res| {
        NodeCommand::APIGetLastNotifications { msg, res }
    })
    .await
}

pub async fn get_notifications_before_timestamp_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_sender, msg, res| {
        NodeCommand::APIGetNotificationsBeforeTimestamp { msg, res }
    })
    .await
}

pub async fn update_local_processing_preference_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    handle_node_command(node_commands_sender, message, |_sender, msg, res| {
        NodeCommand::APIUpdateLocalProcessingPreference { preference: msg, res }
    })
    .await
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct APIUseRegistrationCodeSuccessResponse {
    pub message: String,
    pub node_name: String,
    pub encryption_public_key: String,
    pub identity_public_key: String,
}

pub async fn use_registration_code_handler(
    node_commands_sender: Sender<NodeCommand>,
    message: ShinkaiMessage,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    node_commands_sender
        .send(NodeCommand::APIUseRegistrationCode {
            msg: message,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?; // Send the command to Node
    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(success_response) => {
            let data = serde_json::json!({
                "message": success_response.message,
                "node_name": success_response.node_name,
                "encryption_public_key": success_response.encryption_public_key,
                "identity_public_key": success_response.identity_public_key
            });
            let response = serde_json::json!({
                "status": "success",
                "data": data,
                // TODO: remove the below repeated data  once the Apps have updated
                "message": success_response.message,
                "node_name": success_response.node_name,
                "encryption_public_key": success_response.encryption_public_key,
                "identity_public_key": success_response.identity_public_key
            });
            Ok(warp::reply::with_status(warp::reply::json(&response), StatusCode::OK))
        }
        Err(error) => Ok(warp::reply::with_status(
            warp::reply::json(&error),
            StatusCode::from_u16(error.code).unwrap(),
        )),
    }
}

pub async fn shinkai_health_handler(
    node_commands_sender: Sender<NodeCommand>,
    node_name: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let version = env!("CARGO_PKG_VERSION");

    // Create a channel to receive the result
    let (res_sender, res_receiver) = async_channel::bounded(1);

    // Send the command to the node
    node_commands_sender
        .send(NodeCommand::APIIsPristine { res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    // Receive the result
    let pristine_state = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    // Check if there was an error
    if let Err(error) = pristine_state {
        return Ok(warp::reply::json(&json!({ "status": "error", "error": error })));
    }

    // If there was no error, proceed as usual
    Ok(warp::reply::json(
        &json!({ "status": "ok", "version": version, "node_name": node_name, "is_pristine": pristine_state.unwrap() }),
    ))
}

pub async fn get_all_subidentities_handler(
    node_commands_sender: Sender<NodeCommand>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let node_commands_sender = node_commands_sender.clone();
    let (res_sender, res_receiver) = async_channel::bounded(1);

    node_commands_sender
        .send(NodeCommand::APIGetAllSubidentities { res: res_sender })
        .await
        .map_err(|_| warp::reject::reject())?;

    match res_receiver.recv().await {
        Ok(subidentities) => Ok(warp::reply::json(&subidentities)),
        Err(_) => Err(warp::reject::reject()),
    }
}
