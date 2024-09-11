use std::sync::Arc;

use async_channel::Sender;
use reqwest::StatusCode;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{
    db::ShinkaiDB, lance_db::shinkai_lance_db::LanceShinkaiDb, network::{node_api_router::APIError, node_error::NodeError, Node}, wallet::{
        coinbase_mpc_wallet::CoinbaseMPCWalletConfig, local_ether_wallet::WalletSource, mixed::{Network, NetworkIdentifier}, wallet_manager::{WalletManager, WalletRole}
    }
};

impl Node {
    pub async fn v2_api_restore_local_ethers_wallet(
        db: Arc<ShinkaiDB>,
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
                    if let Err(e) = db.save_wallet_manager(wallet_manager) {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to save wallet manager: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
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

    pub async fn v2_api_create_local_ethers_wallet(
        db: Arc<ShinkaiDB>,
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
                    if let Err(e) = db.save_wallet_manager(wallet_manager) {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to save wallet manager: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
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

    pub async fn v2_api_restore_coinbase_mpc_wallet(
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
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

        let mut wallet_manager_lock = wallet_manager.lock().await;

        // Logic to restore Coinbase MPC wallet
        let network = Network::new(network_identifier);
        let lance_db_weak = Arc::downgrade(&lance_db);
        let restored_wallet_manager = WalletManager::recover_coinbase_mpc_wallet_manager(network, lance_db_weak, config, wallet_id).await;

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
                    if let Err(e) = db.save_wallet_manager(wallet_manager) {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to save wallet manager: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
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
        db: Arc<ShinkaiDB>,
        lance_db: Arc<Mutex<LanceShinkaiDb>>,
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
        let lance_db_weak = Arc::downgrade(&lance_db);
        let created_wallet_manager = WalletManager::create_coinbase_mpc_wallet_manager(network, lance_db_weak, config).await;

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
                    if let Err(e) = db.save_wallet_manager(wallet_manager) {
                        let api_error = APIError {
                            code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                            error: "Internal Server Error".to_string(),
                            message: format!("Failed to save wallet manager: {}", e),
                        };
                        let _ = res.send(Err(api_error)).await;
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
}
