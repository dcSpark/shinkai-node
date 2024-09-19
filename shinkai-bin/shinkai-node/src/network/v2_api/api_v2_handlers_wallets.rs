use async_channel::Sender;
use serde::Deserialize;
use serde_json::Value;
use shinkai_message_primitives::schemas::wallet_mixed::NetworkIdentifier;
use utoipa::OpenApi;
use warp::Filter;

use crate::{
    network::{node_api_router::APIError, node_commands::NodeCommand},
    wallet::{
        coinbase_mpc_wallet::CoinbaseMPCWalletConfig, local_ether_wallet::WalletSource, wallet_manager::WalletRole,
    },
};

use super::api_v2_router::with_sender;

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

    let pay_invoice_route = warp::path("pay_invoice")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(pay_invoice_handler);

    let restore_coinbase_mpc_wallet_route = warp::path("restore_coinbase_mpc_wallet")
        .and(warp::post())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and(warp::body::json())
        .and_then(restore_coinbase_mpc_wallet_handler);

    let list_wallets_route = warp::path("list_wallets")
        .and(warp::get())
        .and(with_sender(node_commands_sender.clone()))
        .and(warp::header::<String>("authorization"))
        .and_then(list_wallets_handler);

    restore_local_wallet_route
        .or(create_local_wallet_route)
        .or(pay_invoice_route)
        .or(restore_coinbase_mpc_wallet_route)
        .or(list_wallets_route)
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

#[derive(Deserialize)]
pub struct PayInvoiceRequest {
    pub invoice_id: String,
    pub data_for_tool: Value,
}

#[utoipa::path(
    post,
    path = "/v2/pay_invoice",
    request_body = PayInvoiceRequest,
    responses(
        (status = 200, description = "Successfully paid invoice", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn pay_invoice_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: PayInvoiceRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiPayInvoice {
            bearer,
            invoice_id: payload.invoice_id,
            data_for_tool: payload.data_for_tool,
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
pub struct RestoreCoinbaseMPCWalletRequest {
    pub network: NetworkIdentifier,
    pub config: Option<CoinbaseMPCWalletConfig>,
    pub wallet_id: String,
    pub role: WalletRole,
}

#[utoipa::path(
    post,
    path = "/v2/restore_coinbase_mpc_wallet",
    request_body = RestoreCoinbaseMPCWalletRequest,
    responses(
        (status = 200, description = "Successfully restored Coinbase MPC wallet", body = Value),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn restore_coinbase_mpc_wallet_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
    payload: RestoreCoinbaseMPCWalletRequest,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiRestoreCoinbaseMPCWallet {
            bearer,
            network: payload.network,
            config: payload.config,
            wallet_id: payload.wallet_id,
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

#[utoipa::path(
    get,
    path = "/v2/list_wallets",
    responses(
        (status = 200, description = "Successfully listed wallets", body = Vec<Value>),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub async fn list_wallets_handler(
    sender: Sender<NodeCommand>,
    authorization: String,
) -> Result<impl warp::Reply, warp::Rejection> {
    let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
    let (res_sender, res_receiver) = async_channel::bounded(1);
    sender
        .send(NodeCommand::V2ApiListWallets {
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
        restore_local_wallet_handler,
        create_local_wallet_handler,
        pay_invoice_handler,
        restore_coinbase_mpc_wallet_handler,
    ),
    components(
        schemas(APIError)
    ),
    tags(
        (name = "wallet", description = "Wallet API endpoints")
    )
)]
pub struct WalletApiDoc;
