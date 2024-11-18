use std::sync::Arc;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::{json, Value};
use shinkai_db::db::ShinkaiDB;
use shinkai_http_api::node_api_router::APIError;
use shinkai_message_primitives::schemas::{
    coinbase_mpc_config::CoinbaseMPCWalletConfig,
    wallet_complementary::{WalletRole, WalletSource},
    wallet_mixed::{Network, NetworkIdentifier},
};
use shinkai_sqlite::SqliteManager;
use tokio::sync::Mutex;

use crate::{
    network::{node_error::NodeError, Node},
    wallet::wallet_manager::WalletManager,
};

impl Node {
    pub async fn v2_api_restore_local_ethers_wallet(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<SqliteManager>,
        wallet_manager: Arc<Mutex<Option<WalletManager>>>,
        bearer: String,
        network_identifier: NetworkIdentifier,
        source: WalletSource,
        role: WalletRole,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut wallet_manager_lock = wallet_manager.lock().await;

        // Logic to restore wallet
        let network = Network::new(network_identifier);
        let restored_wallet_manager = WalletManager::recover_local_ethers_wallet_manager(network, source);

        match restored_wallet_manager {
            Ok(new_wallet_manager) => {
                match &mut *wallet_manager_lock {
                    Some(existing_wallet_manager) => {
                        // Update existing wallet manager based on role
                        match role {
                            WalletRole::Payment => {
                                existing_wallet_manager.update_payment_wallet(new_wallet_manager.payment_wallet);
                            }
                            WalletRole::Receiving => {
                                existing_wallet_manager.update_receiving_wallet(new_wallet_manager.receiving_wallet);
                            }
                            WalletRole::Both => {
                                *existing_wallet_manager = new_wallet_manager;
                            }
                        }
                    }
                    None => {
                        *wallet_manager_lock = Some(new_wallet_manager);
                    }
                }

                // Save the updated WalletManager to the database
                if let Some(ref wallet_manager) = *wallet_manager_lock {
                    match serde_json::to_value(wallet_manager) {
                        Ok(wallet_manager_value) => {
                            if let Err(e) = sqlite_manager.save_wallet_manager(&wallet_manager_value) {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Failed to save wallet manager: {}", e),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to serialize wallet manager: {}", e),
                            };
                            let _ = res.send(Err(api_error)).await;
                            return Ok(());
                        }
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

    pub async fn v2_api_create_local_ethers_wallet(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<SqliteManager>,
        wallet_manager: Arc<Mutex<Option<WalletManager>>>,
        bearer: String,
        network_identifier: NetworkIdentifier,
        role: WalletRole,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut wallet_manager_lock = wallet_manager.lock().await;

        // Logic to create wallet
        let network = Network::new(network_identifier);
        let created_wallet_manager = WalletManager::create_local_ethers_wallet_manager(network);

        match created_wallet_manager {
            Ok(new_wallet_manager) => {
                match &mut *wallet_manager_lock {
                    Some(existing_wallet_manager) => {
                        // Update existing wallet manager based on role
                        match role {
                            WalletRole::Payment => {
                                existing_wallet_manager.update_payment_wallet(new_wallet_manager.payment_wallet);
                            }
                            WalletRole::Receiving => {
                                existing_wallet_manager.update_receiving_wallet(new_wallet_manager.receiving_wallet);
                            }
                            WalletRole::Both => {
                                *existing_wallet_manager = new_wallet_manager;
                            }
                        }
                    }
                    None => {
                        *wallet_manager_lock = Some(new_wallet_manager);
                    }
                }

                // Save the updated WalletManager to the database
                if let Some(ref wallet_manager) = *wallet_manager_lock {
                    match serde_json::to_value(wallet_manager) {
                        Ok(wallet_manager_value) => {
                            if let Err(e) = sqlite_manager.save_wallet_manager(&wallet_manager_value) {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Failed to save wallet manager: {}", e),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to serialize wallet manager: {}", e),
                            };
                            let _ = res.send(Err(api_error)).await;
                            return Ok(());
                        }
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

    pub async fn v2_api_restore_coinbase_mpc_wallet(
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<SqliteManager>,
        wallet_manager: Arc<Mutex<Option<WalletManager>>>,
        bearer: String,
        network_identifier: NetworkIdentifier,
        config: Option<CoinbaseMPCWalletConfig>,
        wallet_id: String,
        role: WalletRole,
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
        let network = Network::new(network_identifier);
        let restored_wallet_manager =
            WalletManager::recover_coinbase_mpc_wallet_manager(network, sqlite_manager, config, wallet_id).await;

        match restored_wallet_manager {
            Ok(new_wallet_manager) => {
                match &mut *wallet_manager_lock {
                    Some(existing_wallet_manager) => {
                        // Update existing wallet manager based on role
                        match role {
                            WalletRole::Payment => {
                                existing_wallet_manager.update_payment_wallet(new_wallet_manager.payment_wallet);
                            }
                            WalletRole::Receiving => {
                                existing_wallet_manager.update_receiving_wallet(new_wallet_manager.receiving_wallet);
                            }
                            WalletRole::Both => {
                                *existing_wallet_manager = new_wallet_manager;
                            }
                        }
                    }
                    None => {
                        *wallet_manager_lock = Some(new_wallet_manager);
                    }
                }

                // Save the updated WalletManager to the database
                if let Some(ref wallet_manager) = *wallet_manager_lock {
                    match serde_json::to_value(wallet_manager) {
                        Ok(wallet_manager_value) => {
                            if let Err(e) = sqlite_manager.save_wallet_manager(&wallet_manager_value) {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Failed to save wallet manager: {}", e),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to serialize wallet manager: {}", e),
                            };
                            let _ = res.send(Err(api_error)).await;
                            return Ok(());
                        }
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
        db: Arc<ShinkaiDB>,
        sqlite_manager: Arc<SqliteManager>,
        wallet_manager: Arc<Mutex<Option<WalletManager>>>,
        bearer: String,
        network_identifier: NetworkIdentifier,
        config: Option<CoinbaseMPCWalletConfig>,
        role: WalletRole,
        res: Sender<Result<Value, APIError>>,
    ) -> Result<(), NodeError> {
        // Validate the bearer token
        if Self::validate_bearer_token(&bearer, db.clone(), &res).await.is_err() {
            return Ok(());
        }

        let mut wallet_manager_lock = wallet_manager.lock().await;

        // Logic to create Coinbase MPC wallet
        let network = Network::new(network_identifier);
        let created_wallet_manager =
            WalletManager::create_coinbase_mpc_wallet_manager(network, sqlite_manager, config).await;

        match created_wallet_manager {
            Ok(new_wallet_manager) => {
                match &mut *wallet_manager_lock {
                    Some(existing_wallet_manager) => {
                        // Update existing wallet manager based on role
                        match role {
                            WalletRole::Payment => {
                                existing_wallet_manager.update_payment_wallet(new_wallet_manager.payment_wallet);
                            }
                            WalletRole::Receiving => {
                                existing_wallet_manager.update_receiving_wallet(new_wallet_manager.receiving_wallet);
                            }
                            WalletRole::Both => {
                                *existing_wallet_manager = new_wallet_manager;
                            }
                        }
                    }
                    None => {
                        *wallet_manager_lock = Some(new_wallet_manager);
                    }
                }

                // Save the updated WalletManager to the database
                if let Some(ref wallet_manager) = *wallet_manager_lock {
                    match serde_json::to_value(wallet_manager) {
                        Ok(wallet_manager_value) => {
                            if let Err(e) = sqlite_manager.save_wallet_manager(&wallet_manager_value) {
                                let api_error = APIError {
                                    code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                    error: "Internal Server Error".to_string(),
                                    message: format!("Failed to save wallet manager: {}", e),
                                };
                                let _ = res.send(Err(api_error)).await;
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            let api_error = APIError {
                                code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                                error: "Internal Server Error".to_string(),
                                message: format!("Failed to serialize wallet manager: {}", e),
                            };
                            let _ = res.send(Err(api_error)).await;
                            return Ok(());
                        }
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
        db: Arc<ShinkaiDB>,
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
}
