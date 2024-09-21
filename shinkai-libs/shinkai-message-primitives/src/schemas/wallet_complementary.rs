use serde::{Deserialize, Serialize};

/// Enum to represent different wallet roles. Useful for the API.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum WalletRole {
    Payment,
    Receiving,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletSource {
    Mnemonic(String),
    PrivateKey(String),
}