use async_channel::Sender;
use serde::Deserialize;
use serde_json::Value;
use shinkai_message_primitives::schemas::{wallet_complementary::WalletRole, wallet_mixed::NetworkIdentifier};
use utoipa::{OpenApi, ToSchema};
use warp::Filter;

use crate::{node_api_router::APIError, node_commands::NodeCommand};

use super::api_v2_router::with_sender;

pub fn tool_wallets_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let add_wallet_route = warp::path("add_wallet")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(add_wallet_handler);

    let get_wallet_route = warp::path("get_wallet")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<GetWalletRequest>())
        .and_then(get_wallet_handler);

    let remove_wallet_route = warp::path("remove_wallet")
        .and(warp::delete())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::query::<RemoveWalletRequest>())
        .and_then(remove_wallet_handler);

    let unlock_wallets_route = warp::path("unlock_wallets")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(unlock_wallets_handler);

    let get_wallets_route = warp::path("get_wallets")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(get_wallets_handler);

    add_wallet_route
        .or(get_wallet_route)
        .or(remove_wallet_route)
        .or(unlock_wallets_route)
        .or(get_wallets_route)
}

#[derive(Deserialize, ToSchema)]
pub struct CreateLocalWallet {
    pub network: NetworkIdentifier,
    pub role: WalletRole,
}

#[derive(Deserialize, ToSchema)]
pub struct AddWalletRequest {
    pub secret_key: String,
    pub is_encrypted: bool,
    pub key_hash: Option<String>,
    pub wallet_type: String,
    pub compatible_networks: Vec<String>,
    pub wallet_data: Value,
}

#[derive(Deserialize, ToSchema)]
pub struct GetWalletRequest {
    pub wallet_id: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct RemoveWalletRequest {
    pub wallet_id: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct UnlockWalletsRequest {
    pub password: String,
}

#[utoipa::path(
    post,
    path = "/v2/add_wallet",
    request_body = AddWalletRequest,
    responses(
        (status = 200, description = "Successfully added wallet", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn add_wallet_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: AddWalletRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiAddWallet {
            bearer,
            secret_key: payload.secret_key,
            is_encrypted: payload.is_encrypted,
            key_hash: payload.key_hash,
            wallet_type: payload.wallet_type,
            compatible_networks: payload.compatible_networks,
            wallet_data: payload.wallet_data,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    get,
    path = "/v2/get_wallet",
    responses(
        (status = 200, description = "Successfully retrieved wallet", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_wallet_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query: GetWalletRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetWallet {
            bearer,
            wallet_id: query.wallet_id,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    delete,
    path = "/v2/remove_wallet",
    responses(
        (status = 200, description = "Successfully removed wallet", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn remove_wallet_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    query: RemoveWalletRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiRemoveWallet {
            bearer,
            wallet_id: query.wallet_id,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    post,
    path = "/v2/unlock_wallets",
    request_body = UnlockWalletsRequest,
    responses(
        (status = 200, description = "Successfully unlocked wallets", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn unlock_wallets_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: UnlockWalletsRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiUnlockWallets {
            bearer,
            password: payload.password,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[utoipa::path(
    get,
    path = "/v2/get_wallets",
    responses(
        (status = 200, description = "Successfully retrieved all wallets", body = Vec<Value>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn get_wallets_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiGetWallets {
            bearer,
            res: res_sender,
        })
        .await
        .map_err(|_| warp::reject::reject())?;

    let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

    match result {
        Ok(response) => Ok(warp::reply::json(&response)),
        Err(error) => Err(warp::reject::custom(error)),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        add_wallet_handler,
        get_wallet_handler,
        remove_wallet_handler,
        unlock_wallets_handler,
        get_wallets_handler,
    ),
    components(
        schemas(APIError, AddWalletRequest, GetWalletRequest, RemoveWalletRequest, UnlockWalletsRequest)
    ),
    tags(
        (name = "tool_wallets", description = "Tool Wallet API endpoints")
    )
)]
pub struct ToolWalletApiDoc;
