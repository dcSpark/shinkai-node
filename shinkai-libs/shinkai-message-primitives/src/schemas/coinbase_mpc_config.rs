use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinbaseMPCWalletConfig {
    pub name: String,
    pub private_key: String,
    pub wallet_id: Option<String>,
    pub use_server_signer: Option<String>,
}