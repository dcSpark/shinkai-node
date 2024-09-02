use aes_gcm::aead::generic_array::GenericArray;
use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic, Seed};
use ethers::core::k256::ecdsa::SigningKey;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{Address as EthersAddress, NameOrAddress};
use ethers::utils::{format_units, hex, to_checksum};
use ethers::{core::k256::SecretKey, prelude::*};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;
use uuid::Uuid;

use ethers::signers::LocalWallet as EthersLocalWallet;
use futures::TryFutureExt;

use crate::lance_db::shinkai_lance_db::LanceShinkaiDb;
use crate::tools::js_toolkit_headers::ToolConfig;
use crate::tools::shinkai_tool::ShinkaiTool;
use crate::wallet::erc20_abi::ERC20_ABI;
use crate::wallet::wallet_error::WalletError;

use super::mixed::{self, Address, AddressBalanceList, Asset, AssetType, Balance, Network, PublicAddress};
use super::wallet_traits::{CommonActions, IsWallet, PaymentWallet, ReceivingWallet, SendActions, TransactionHash};

#[derive(Debug, Clone)]
pub struct CoinbaseMPCWallet {
    pub id: String,
    pub network: Network,
    pub address: Address,
    pub config: CoinbaseMPCWalletConfig,
    pub lance_db: Weak<Mutex<LanceShinkaiDb>>, // Added field to store Weak reference
                                               // pub wallet: Wallet<SigningKey>,
                                               // pub provider: LocalWalletProvider,

                                               // Note: do we need access to ToolRouter? (maybe not, since we can call the Coinbase SDK directly)
                                               // Should we create a new manager that calls the Coinbase MPC SDK directly? (Probably)
                                               // So we still need access to lancedb so we can get the code for each tool
                                               // If we use lancedb each time (it's slightly slower) but we can have everything in sync

                                               // We could have an UI in Settings, where we can select the Coinbase Wallet or the Ethers Local Wallet

                                               // Note: maybe we should create a new struct that holds the information about Config + Params + Results (for each tool)
                                               // based on what we have in the typescript tools
}

impl CoinbaseMPCWallet {
    pub async fn create_wallet(
        network: Network,
        lance_db: Weak<Mutex<LanceShinkaiDb>>, // Changed to Weak
        config: Option<CoinbaseMPCWalletConfig>,
    ) -> Result<Self, WalletError> {
        let lance_db_strong = lance_db.upgrade().ok_or(WalletError::ConfigNotFound)?;
        let mut config = match config {
            Some(cfg) => cfg,
            None => {
                let db = lance_db_strong.lock().await;
                let tool_id = ShinkaiToolCoinbase::CreateWallet.definition_id();
                let shinkai_tool = db.get_tool(tool_id).await?.ok_or(WalletError::ConfigNotFound)?;

                // Extract the required configuration from the JSTool
                let mut name = String::new();
                let mut private_key = String::new();
                let mut use_server_signer = String::new();
                if let ShinkaiTool::JS(js_tool, _) = shinkai_tool {
                    for cfg in js_tool.config {
                        match cfg {
                            ToolConfig::BasicConfig(basic_config) => match basic_config.key_name.as_str() {
                                "name" => name = basic_config.key_value.clone().unwrap_or_default(),
                                "private_key" => private_key = basic_config.key_value.clone().unwrap_or_default(),
                                "useServerSigner" => {
                                    use_server_signer = basic_config.key_value.clone().unwrap_or_default()
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                } else {
                    return Err(WalletError::ConfigNotFound);
                }

                CoinbaseMPCWalletConfig {
                    name,
                    private_key,
                    wallet_id: None,
                    use_server_signer: Some(use_server_signer),
                }
            }
        };

        // Call the function to create the wallet
        let params = serde_json::json!({
            "name": config.name,
            "privateKey": config.private_key,
            "useServerSigner": config.use_server_signer,
        });

        let response = Self::call_function(
            config.clone(),
            lance_db.clone(),
            ShinkaiToolCoinbase::CreateWallet,
            params,
        )
        .await?;

        // Extract the necessary fields from the response
        let wallet_id = response
            .get("walletId")
            .and_then(|v| v.as_str())
            .ok_or(WalletError::ConfigNotFound)?
            .to_string();
        let address_id = response
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or(WalletError::ConfigNotFound)?
            .to_string();

        // Update the config with the wallet_id
        config.wallet_id = Some(wallet_id.clone());

        // Use the extracted fields to create the wallet
        let wallet = CoinbaseMPCWallet {
            id: wallet_id.clone(),
            config,
            network: network.clone(),
            address: Address {
                wallet_id: wallet_id,
                network_id: network.id,
                public_key: None,
                address_id,
            },
            lance_db, // Store the Weak reference
        };

        Ok(wallet)
    }

    pub async fn restore_wallet(
        network: Network,
        lance_db: Weak<Mutex<LanceShinkaiDb>>, // Changed to Weak
        config: Option<CoinbaseMPCWalletConfig>,
        wallet_id: String,
    ) -> Result<Self, WalletError> {
        let lance_db_strong = lance_db.upgrade().ok_or(WalletError::ConfigNotFound)?;
        let config = match config {
            Some(cfg) => cfg,
            None => {
                let db = lance_db_strong.lock().await;
                let tool_id = ShinkaiToolCoinbase::CreateWallet.definition_id();
                let shinkai_tool = db.get_tool(tool_id).await?.ok_or(WalletError::ConfigNotFound)?;

                // Extract the required configuration from the JSTool
                let mut name = String::new();
                let mut private_key = String::new();
                let mut use_server_signer = String::new();
                if let ShinkaiTool::JS(js_tool, _) = shinkai_tool {
                    for cfg in js_tool.config {
                        match cfg {
                            ToolConfig::BasicConfig(basic_config) => match basic_config.key_name.as_str() {
                                "name" => name = basic_config.key_value.clone().unwrap_or_default(),
                                "private_key" => private_key = basic_config.key_value.clone().unwrap_or_default(),
                                "useServerSigner" => {
                                    use_server_signer = basic_config.key_value.clone().unwrap_or_default()
                                }
                                "walletId" => {
                                    if basic_config.key_value.is_none() {
                                        return Err(WalletError::ConfigNotFound);
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                } else {
                    return Err(WalletError::ConfigNotFound);
                }

                CoinbaseMPCWalletConfig {
                    name,
                    private_key,
                    wallet_id: Some(wallet_id.clone()),
                    use_server_signer: Some(use_server_signer),
                }
            }
        };

        // Call the function to restore the wallet
        let params = serde_json::json!({
            "name": config.name,
            "privateKey": config.private_key,
            "useServerSigner": config.use_server_signer,
            "walletId": wallet_id,
        });

        let response = Self::call_function(
            config.clone(),
            lance_db.clone(),
            ShinkaiToolCoinbase::CreateWallet,
            params,
        )
        .await?;

        // Extract the necessary fields from the response
        let address_id = response
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or(WalletError::ConfigNotFound)?
            .to_string();

        // Use the extracted fields to create the wallet
        let wallet = CoinbaseMPCWallet {
            id: wallet_id.clone(),
            network: network.clone(),
            config,
            address: Address {
                wallet_id: wallet_id,
                network_id: network.id,
                public_key: None,
                address_id,
            },
            lance_db, // Store the Weak reference
        };

        Ok(wallet)
    }

    pub async fn call_function(
        config: CoinbaseMPCWalletConfig,
        lance_db: Weak<Mutex<LanceShinkaiDb>>, // Changed to Weak
        function_name: ShinkaiToolCoinbase,
        params: Value,
    ) -> Result<Value, WalletError> {
        let lance_db_strong = lance_db.upgrade().ok_or(WalletError::ConfigNotFound)?;
        let db = lance_db_strong.lock().await;
        let tool_id = function_name.definition_id();
        let shinkai_tool = db.get_tool(tool_id).await?.ok_or(WalletError::ConfigNotFound)?;
        let function_config = shinkai_tool.get_config_from_env();

        // Convert function_config from String to Value
        let mut function_config_value: Value = match function_config {
            Some(config_str) => {
                serde_json::from_str(&config_str).map_err(|e| WalletError::FunctionExecutionError(e.to_string()))?
            }
            None => Value::Object(serde_json::Map::new()),
        };

        // Overwrite function_config_value with values from config
        function_config_value["name"] = Value::String(config.name);
        function_config_value["privateKey"] = Value::String(config.private_key);
        if let Some(use_server_signer) = config.use_server_signer {
            function_config_value["useServerSigner"] = Value::String(use_server_signer);
        }
        if let Some(wallet_id) = config.wallet_id {
            function_config_value["walletId"] = Value::String(wallet_id);
        }

        // Convert function_config_value back to String
        let function_config_str = serde_json::to_string(&function_config_value)
            .map_err(|e| WalletError::FunctionExecutionError(e.to_string()))?;

        if let ShinkaiTool::JS(js_tool, _) = shinkai_tool {
            let result = js_tool
                .run(params, Some(function_config_str))
                .map_err(|e| WalletError::FunctionExecutionError(e.to_string()))?;
            let result_str =
                serde_json::to_string(&result).map_err(|e| WalletError::FunctionExecutionError(e.to_string()))?;
            return Ok(
                serde_json::from_str(&result_str).map_err(|e| WalletError::FunctionExecutionError(e.to_string()))?
            );
        }

        Err(WalletError::FunctionNotFound(tool_id.to_string()))
    }
}

impl IsWallet for CoinbaseMPCWallet {}

impl PaymentWallet for CoinbaseMPCWallet {
    // No additional methods needed, as they are covered by SendActions and CommonActions
}

impl ReceivingWallet for CoinbaseMPCWallet {
    // No additional methods needed, as they are covered by SendActions and CommonActions
}

impl CommonActions for CoinbaseMPCWallet {
    fn get_payment_address(&self) -> PublicAddress {
        self.address.clone().into()
    }

    fn get_address(&self) -> Address {
        self.address.clone()
    }

    fn get_balance(&self) -> Pin<Box<dyn Future<Output = Result<f64, WalletError>> + Send + 'static>> {
        let config = self.config.clone();
        let lance_db = self.lance_db.clone(); // Use the Weak reference

        Box::pin(async move {
            let params = serde_json::json!({
                "walletId": config.wallet_id,
            });

            let response =
                CoinbaseMPCWallet::call_function(config, lance_db, ShinkaiToolCoinbase::GetBalance, params).await?;

            let balance_str = response
                .get("balance")
                .and_then(|v| v.as_str())
                .ok_or(WalletError::ConfigNotFound)?;

            let balance: f64 = balance_str
                .parse()
                .map_err(|e: std::num::ParseFloatError| WalletError::ConversionError(e.to_string()))?;
            Ok(balance)
        })
    }

    fn check_balances(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<AddressBalanceList, WalletError>> + Send + 'static>> {
        let config = self.config.clone();
        let lance_db = self.lance_db.clone(); // Use the Weak reference

        Box::pin(async move {
            let params = serde_json::json!({
                "walletId": config.wallet_id,
            });

            let response =
                CoinbaseMPCWallet::call_function(config, lance_db, ShinkaiToolCoinbase::GetBalance, params).await?;

            let balances = response
                .get("balances")
                .and_then(|v| v.as_array())
                .ok_or(WalletError::ConfigNotFound)?;

            // let data: Vec<Balance> = balances
            //     .iter()
            //     .map(|balance| {
            //         let amount = balance
            //             .get("amount")
            //             .and_then(|v| v.as_str())
            //             .unwrap_or_default()
            //             .parse::<f64>()
            //             .unwrap_or(0.0);
            //         let decimals = balance.get("decimals").and_then(|v| v.as_u64()).map(|d| d as u32);
            //         let asset = balance
            //             .get("asset")
            //             .and_then(|v| v.as_str())
            //             .unwrap_or_default()
            //             .to_string();
            //         Balance {
            //             amount: amount.to_string(),
            //             decimals,
            //             asset,
            //         }
            //     })
            //     .collect();

            let has_more = response.get("has_more").and_then(|v| v.as_bool()).unwrap_or(false);

            let next_page = response
                .get("next_page")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let total_count = response.get("total_count").and_then(|v| v.as_u64()).unwrap_or(0);

            let address_balance_list = AddressBalanceList {
                data: vec![],
                has_more,
                next_page,
                total_count,
            };

            Ok(address_balance_list)
        })
    }

    fn check_asset_balance(
        &self,
        public_address: PublicAddress,
        asset: Asset,
    ) -> Pin<Box<dyn Future<Output = Result<Balance, WalletError>> + Send + 'static>> {
        let config = self.config.clone();
        let lance_db = self.lance_db.clone(); // Use the Weak reference

        Box::pin(async move {
            let params = serde_json::json!({
                "walletId": config.wallet_id,
                "publicAddress": public_address.address_id,
                "asset": asset.asset_id,
            });

            let response =
                CoinbaseMPCWallet::call_function(config, lance_db, ShinkaiToolCoinbase::GetBalance, params).await?;

            let amount = response
                .get("amount")
                .and_then(|v| v.as_str())
                .ok_or(WalletError::ConfigNotFound)?
                .parse::<f64>()
                .map_err(|e| WalletError::ConversionError(e.to_string()))?;

            let decimals = response.get("decimals").and_then(|v| v.as_u64()).unwrap_or(18);

            let balance = Balance {
                amount: amount.to_string(),
                decimals: Some(decimals as u32),
                asset: asset.asset_id,
            };

            Ok(balance)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseMPCWalletConfig {
    pub name: String,
    pub private_key: String,
    pub wallet_id: Option<String>,
    pub use_server_signer: Option<String>,
}

pub enum ShinkaiToolCoinbase {
    CreateWallet,
    GetMyAddress,
    GetBalance,
    GetTransactions,
    SendTx,
    CallFaucet,
}

impl ShinkaiToolCoinbase {
    pub fn definition_id(&self) -> &'static str {
        match self {
            ShinkaiToolCoinbase::CreateWallet => "shinkai-tool-coinbase-create-wallet",
            ShinkaiToolCoinbase::GetMyAddress => "shinkai-tool-coinbase-get-my-address",
            ShinkaiToolCoinbase::GetBalance => "shinkai-tool-coinbase-get-balance",
            ShinkaiToolCoinbase::GetTransactions => "shinkai-tool-coinbase-get-transactions",
            ShinkaiToolCoinbase::SendTx => "shinkai-tool-coinbase-send-tx",
            ShinkaiToolCoinbase::CallFaucet => "shinkai-tool-coinbase-call-faucet",
        }
    }
}
