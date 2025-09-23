use std::sync::Arc;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value};

use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::{
    coinbase_mpc_config::CoinbaseMPCWalletConfig,
    shinkai_name::ShinkaiName,
    wallet_complementary::{WalletRole, WalletSource},
    wallet_mixed::{Asset, AssetType},
    x402_types::Network,
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;

use crate::{
    network::{node_error::NodeError, Node},
    wallet::wallet_manager::WalletManager,
};

impl Node {
    /// Helper to update the wallet manager in-place based on role
    fn update_wallet_manager_by_role(
        wallet_manager_lock: &mut Option<WalletManager>,
        new_wallet_manager: WalletManager,
        role: WalletRole,
    ) {
        match wallet_manager_lock {
            Some(existing_wallet_manager) => match role {
                WalletRole::Payment => {
                    existing_wallet_manager.update_payment_wallet(new_wallet_manager.payment_wallet);
                }
                WalletRole::Receiving => {
                    existing_wallet_manager.update_receiving_wallet(new_wallet_manager.receiving_wallet);
                }
                WalletRole::Both => {
                    *existing_wallet_manager = new_wallet_manager;
                }
            },
            None => {
                *wallet_manager_lock = Some(new_wallet_manager);
            }
        }
    }

    /// Helper to serialize and save the wallet manager to the DB, sending errors via the channel
    async fn save_wallet_manager_to_db(
        db: &SqliteManager,
        wallet_manager: &WalletManager,
        res: &Sender<Result<Value, APIError>>,
    ) -> Result<(), ()> {
        match serde_json::to_value(wallet_manager) {
            Ok(wallet_manager_value) => {
                if let Err(e) = db.save_wallet_manager(&wallet_manager_value) {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to save wallet manager: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Err(());
                }
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to serialize wallet manager: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                return Err(());
            }
        }
        Ok(())
    }

    pub async fn v2_api_restore_coinbase_mpc_wallet(
        db: Arc<SqliteManager>,
        wallet_manager: Arc<Mutex<Option<WalletManager>>>,
        bearer: String,
        network: Network,
        config: Option<CoinbaseMPCWalletConfig>,
        wallet_id: String,
        role: WalletRole,
        node_name: ShinkaiName,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        // Note: for some reason, the private key is not unescaped for some requests and we get \\n instead of \n
        // This happens if you use --data-binary and you escape the content (you shouldn't do that)
        let config = config.map(|cfg| {
            let unescaped_private_key = cfg.private_key.replace("\\n", "\n");
            CoinbaseMPCWalletConfig {
                private_key: unescaped_private_key,
                ..cfg
            }
        });

        let mut wallet_manager_lock = wallet_manager.lock().await;

        // Logic to restore Coinbase MPC wallet
        let restored_wallet_manager =
            WalletManager::recover_coinbase_mpc_wallet_manager(network, db.clone(), config, wallet_id, node_name).await;

        match restored_wallet_manager {
            Ok(new_wallet_manager) => {
                Self::update_wallet_manager_by_role(&mut wallet_manager_lock, new_wallet_manager, role);

                // Save the updated WalletManager to the database
                if let Some(ref wallet_manager) = *wallet_manager_lock {
                    if Self::save_wallet_manager_to_db(&db, wallet_manager, &res)
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                let _ = res
                    .send(Ok(serde_json::json!({"status": "success"})))
                    .await
                    .map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to restore wallet: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_create_coinbase_mpc_wallet(
        db: Arc<SqliteManager>,
        wallet_manager: Arc<Mutex<Option<WalletManager>>>,
        bearer: String,
        network: Network,
        config: Option<CoinbaseMPCWalletConfig>,
        role: WalletRole,
        node_name: ShinkaiName,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut wallet_manager_lock = wallet_manager.lock().await;

        // Logic to create Coinbase MPC wallet
        let created_wallet_manager =
            WalletManager::create_coinbase_mpc_wallet_manager(network, db.clone(), config, node_name).await;

        match created_wallet_manager {
            Ok(new_wallet_manager) => {
                Self::update_wallet_manager_by_role(&mut wallet_manager_lock, new_wallet_manager, role);

                // Save the updated WalletManager to the database
                if let Some(ref wallet_manager) = *wallet_manager_lock {
                    if Self::save_wallet_manager_to_db(&db, wallet_manager, &res)
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                let _ = res
                    .send(Ok(serde_json::json!({"status": "success"})))
                    .await
                    .map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create wallet: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_list_wallets(
        db: Arc<SqliteManager>,
        wallet_manager: Arc<Mutex<Option<WalletManager>>>,
        bearer: String,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let wallet_manager_lock = wallet_manager.lock().await;

        // Check if wallet manager exists
        if let Some(ref wallet_manager) = *wallet_manager_lock {
            // Convert wallet manager to JSON
            let wallets_json = match serde_json::to_value(wallet_manager) {
                Ok(value) => value,
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to serialize wallet manager: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            };

            let _ = res.send(Ok(wallets_json)).await;
        } else {
            // Return null for payment_wallet and receiving_wallet
            let empty_wallets_json = json!({
                "payment_wallet": null,
                "receiving_wallet": null
            });

            let _ = res.send(Ok(empty_wallets_json)).await;
        }

        Ok(())
    }

    pub async fn v2_api_create_local_ethers_wallet(
        db: Arc<SqliteManager>,
        wallet_manager: Arc<tokio::sync::Mutex<Option<WalletManager>>>,
        bearer: String,
        network: Network,
        role: WalletRole,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let mut wallet_manager_lock = wallet_manager.lock().await;
        let created_wallet_manager =
            WalletManager::create_local_ethers_wallet_manager(network, db.clone(), role.clone()).await;

        match created_wallet_manager {
            Ok(new_wallet_manager) => {
                let wallet_details = new_wallet_manager.payment_wallet.to_wallet_enum();
                Self::update_wallet_manager_by_role(&mut wallet_manager_lock, new_wallet_manager, role.clone());
                if let Some(ref wallet_manager) = *wallet_manager_lock {
                    if Self::save_wallet_manager_to_db(&db, wallet_manager, &res)
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                let _ = res
                    .send(Ok(serde_json::json!({"status": "success", "wallet": wallet_details})))
                    .await
                    .map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to create wallet: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_restore_local_ethers_wallet(
        db: Arc<SqliteManager>,
        wallet_manager: Arc<tokio::sync::Mutex<Option<WalletManager>>>,
        bearer: String,
        network: Network,
        source: WalletSource,
        role: WalletRole,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }
        let mut wallet_manager_lock = wallet_manager.lock().await;
        let restored_wallet_manager =
            WalletManager::recover_local_ethers_wallet_manager(network, db.clone(), source.clone(), role.clone()).await;
        match restored_wallet_manager {
            Ok(new_wallet_manager) => {
                let wallet_details = new_wallet_manager.payment_wallet.to_wallet_enum();
                Self::update_wallet_manager_by_role(&mut wallet_manager_lock, new_wallet_manager, role.clone());
                if let Some(ref wallet_manager) = *wallet_manager_lock {
                    if Self::save_wallet_manager_to_db(&db, wallet_manager, &res)
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                let _ = res
                    .send(Ok(serde_json::json!({"status": "success", "wallet": wallet_details})))
                    .await
                    .map_err(|_| ());
                Ok(())
            }
            Err(e) => {
                let api_error = APIError {
                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    error: "Internal Server Error".to_string(),
                    message: format!("Failed to restore wallet: {}", e),
                };
                let _ = res.send(Err(api_error)).await;
                Ok(())
            }
        }
    }

    pub async fn v2_api_get_wallet_balance(
        db: Arc<SqliteManager>,
        wallet_manager: Arc<tokio::sync::Mutex<Option<WalletManager>>>,
        bearer: String,
        node_name: ShinkaiName,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let wallet_manager_lock = wallet_manager.lock().await;

        if let Some(ref wallet_manager) = *wallet_manager_lock {
            match wallet_manager.check_balances_payment_wallet(node_name.clone()).await {
                Ok(address_balance_list) => {
                    let mut balances_map = serde_json::Map::new();
                    for balance_item in address_balance_list.data {
                        match serde_json::to_value(balance_item.clone()) {
                            Ok(value) => {
                                balances_map.insert(balance_item.asset.asset_id.clone(), value);
                            }
                            Err(e) => {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!(
                                        "Failed to serialize balance item {}: {}",
                                        balance_item.asset.asset_id, e
                                    ),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        }
                    }
                    let _ = res.send(Ok(Value::Object(balances_map))).await;
                }
                Err(e) => {
                    let api_error = APIError {
                        code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                        error: "Internal Server Error".to_string(),
                        message: format!("Failed to get wallet balances: {}", e),
                    };
                    let _ = res.send(Err(api_error)).await;
                    return Ok(());
                }
            }
        } else {
            let api_error = APIError {
                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                error: "Internal Server Error".to_string(),
                message: "Wallet manager not initialized".to_string(),
            };
            let _ = res.send(Err(api_error)).await;
        }

        Ok(())
    }
}
