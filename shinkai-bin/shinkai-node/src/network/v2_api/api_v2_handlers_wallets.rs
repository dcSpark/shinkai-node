// use async_channel::Sender;
// use reqwest::StatusCode;
// use serde::Deserialize;
// use serde_json::{json, Value};
// use utoipa::OpenApi;
// use warp::Filter;

// use crate::{
//     network::{node_api_router::APIError, node_commands::NodeCommand},
//     wallet::{local_ether_wallet::{LocalEthersWallet, WalletSource}, mixed::NetworkIdentifier},
// };

// use super::api_v2_router::{create_success_response, with_sender};

// pub fn wallet_routes(
//     node_commands_sender: Sender<NodeCommand>,
// ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
//     let restore_wallet_route = warp::path("restore_wallet")
//         .and(warp::post())
//         .and(with_sender(node_commands_sender.clone()))
//         .and(warp::header::<String>("authorization"))
//         .and(warp::body::json())
//         .and_then(restore_wallet_handler);

//     let create_wallet_route = warp::path("create_wallet")
//         .and(warp::post())
//         .and(with_sender(node_commands_sender.clone()))
//         .and(warp::header::<String>("authorization"))
//         .and(warp::body::json())
//         .and_then(create_wallet_handler);

//     restore_wallet_route.or(create_wallet_route)
// }

// #[derive(Deserialize)]
// pub struct RestoreWalletRequest {
//     pub network: NetworkIdentifier,
//     pub source: WalletSource,
// }

// #[utoipa::path(
//     post,
//     path = "/v2/restore_wallet",
//     request_body = RestoreWalletRequest,
//     responses(
//         (status = 200, description = "Successfully restored wallet", body = Value),
//         (status = 500, description = "Internal server error", body = APIError)
//     )
// )]
// pub async fn restore_wallet_handler(
//     sender: Sender<NodeCommand>,
//     authorization: String,
//     payload: RestoreWalletRequest,
// ) -> Result<impl warp::Reply, warp::Rejection> {
//     let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
//     let (res_sender, res_receiver) = async_channel::bounded(1);
//     sender
//         .send(NodeCommand::V2ApiRestoreWallet {
//             bearer,
//             network: payload.network,
//             source: payload.source,
//             res: res_sender,
//         })
//         .await
//         .map_err(|_| warp::reject::reject())?;

//     let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

//     match result {
//         Ok(response) => Ok(warp::reply::json(&response)),
//         Err(error) => Err(warp::reject::custom(error)),
//     }
// }

// #[derive(Deserialize)]
// pub struct CreateWalletRequest {
//     pub network: NetworkIdentifier,
// }

// #[utoipa::path(
//     post,
//     path = "/v2/create_wallet",
//     request_body = CreateWalletRequest,
//     responses(
//         (status = 200, description = "Successfully created wallet", body = LocalEthersWallet),
//         (status = 500, description = "Internal server error", body = APIError)
//     )
// )]
// pub async fn create_wallet_handler(
//     sender: Sender<NodeCommand>,
//     authorization: String,
//     payload: CreateWalletRequest,
// ) -> Result<impl warp::Reply, warp::Rejection> {
//     let bearer = authorization.strip_prefix("Bearer ").unwrap_or("").to_string();
//     let (res_sender, res_receiver) = async_channel::bounded(1);
//     sender
//         .send(NodeCommand::V2ApiCreateWallet {
//             bearer,
//             network: payload.network,
//             res: res_sender,
//         })
//         .await
//         .map_err(|_| warp::reject::reject())?;

//     let result = res_receiver.recv().await.map_err(|_| warp::reject::reject())?;

//     match result {
//         Ok(response) => Ok(warp::reply::json(&response)),
//         Err(error) => Err(warp::reject::custom(error)),
//     }
// }

// // #[derive(OpenApi)]
// // #[openapi(
// //     paths(
// //         restore_wallet_handler,
// //         create_wallet_handler,
// //     ),
// //     components(
// //         schemas(Value, APIError)
// //     ),
// //     tags(
// //         (name = "wallet", description = "Wallet API endpoints")
// //     )
// // )]
// pub struct WalletApiDoc;
