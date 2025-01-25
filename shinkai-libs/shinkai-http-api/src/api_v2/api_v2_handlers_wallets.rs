use async_channel::Sender;
use serde::Deserialize;
use serde_json::Value;
use shinkai_message_primitives::schemas::{
    coinbase_mpc_config::CoinbaseMPCWalletConfig,
    wallet_complementary::{WalletRole, WalletSource},
    wallet_mixed::{Address, Asset, Network, NetworkIdentifier, NetworkProtocolFamilyEnum},
};
use utoipa::{OpenApi, ToSchema};
use warp::Filter;

use crate::{node_api_router::APIError, node_commands::NodeCommand};

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

    restore_local_wallet_route
        .or(create_local_wallet_route)
        .or(pay_invoice_route)
        .or(restore_coinbase_mpc_wallet_route)
        .or(list_wallets_route)
        .or(add_wallet_route)
        .or(get_wallet_route)
        .or(remove_wallet_route)
        .or(unlock_wallets_route)
        .or(get_wallets_route)
}

#[derive(Deserialize, ToSchema)]
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

#[derive(Deserialize, ToSchema)]
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

#[derive(Deserialize, ToSchema)]
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

#[derive(Deserialize, ToSchema)]
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

#[derive(Deserialize, ToSchema)]
pub struct AddWalletRequest {
    pub secret_key: String,
    pub is_encrypted: bool,
    pub key_hash: Option<String>,
    pub wallet_type: String,
    pub compatible_networks: Vec<String>,
    pub wallet_data: Value,
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

#[derive(Deserialize, ToSchema)]
pub struct GetWalletRequest {
    pub wallet_id: i64,
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

#[derive(Deserialize, ToSchema)]
pub struct RemoveWalletRequest {
    pub wallet_id: i64,
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

#[derive(Deserialize, ToSchema)]
pub struct UnlockWalletsRequest {
    pub password: String,
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
        restore_local_wallet_handler,
        create_local_wallet_handler,
        pay_invoice_handler,
        restore_coinbase_mpc_wallet_handler,
        add_wallet_handler,
        get_wallet_handler,
        remove_wallet_handler,
        unlock_wallets_handler,
        get_wallets_handler,
    ),
    components(
        schemas(APIError, CreateLocalWalletRequest, PayInvoiceRequest, RestoreCoinbaseMPCWalletRequest, RestoreLocalWalletRequest,
            Network, NetworkIdentifier, NetworkProtocolFamilyEnum, WalletRole, WalletSource, CoinbaseMPCWalletConfig, Address, Asset,
            AddWalletRequest, GetWalletRequest, RemoveWalletRequest, UnlockWalletsRequest)
    ),
    tags(
        (name = "wallet", description = "Wallet API endpoints")
    )
)]
pub struct WalletApiDoc;
