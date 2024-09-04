use async_channel::Sender;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use utoipa::OpenApi;
use warp::Filter;

use crate::{
    network::{node_api_router::APIError, node_commands::NodeCommand},
    wallet::{local_ether_wallet::{LocalEthersWallet, WalletSource}, mixed::NetworkIdentifier, coinbase_mpc_wallet::CoinbaseMPCWalletConfig, wallet_manager::WalletRole},
};

use super::api_v2_router::{create_success_response, with_sender};

pub fn wallet_routes(
    node_commands_sender: Sender<NodeCommand>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let restore_local_wallet_route = warp::path("restore_local_wallet")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(restore_local_wallet_handler);

    let create_local_wallet_route = warp::path("create_local_wallet")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(create_local_wallet_handler);

    restore_local_wallet_route.or(create_local_wallet_route)
}

#[derive(Deserialize)]
pub struct RestoreLocalWalletRequest {
    pub network: NetworkIdentifier,
    pub source: WalletSource,
    pub role: WalletRole,
}

#[utoipa::path(
    post,
    path = "/v2/restore_local_wallet",
    request_body = RestoreLocalWalletRequest,
    responses(
        (status = 200, description = "Successfully restored wallet", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn restore_local_wallet_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: RestoreLocalWalletRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiRestoreLocalEthersWallet {
            bearer,
            network: payload.network,
            source: payload.source,
            role: payload.role,
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

#[derive(Deserialize)]
pub struct CreateLocalWalletRequest {
    pub network: NetworkIdentifier,
    pub role: WalletRole,
}

#[utoipa::path(
    post,
    path = "/v2/create_local_wallet",
    request_body = CreateLocalWalletRequest,
    responses(
        (status = 200, description = "Successfully created wallet", body = LocalEthersWallet),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn create_local_wallet_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: CreateLocalWalletRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiCreateLocalEthersWallet {
            bearer,
            network: payload.network,
            role: payload.role,
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

// #[derive(OpenApi)]
// #[openapi(
//     paths(
//         restore_local_wallet_handler,
//         create_local_wallet_handler,
//     ),
//     components(
//         schemas(Value, APIError)
//     ),
//     tags(
//         (name = "wallet", description = "Wallet API endpoints")
//     )
// )]
pub struct WalletApiDoc;
