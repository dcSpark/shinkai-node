use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Enum to represent different wallet roles. Useful for the API.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
pub enum WalletRole {
    Payment,
    Receiving,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum WalletSource {
    Mnemonic(String),
    PrivateKey(String),
}
