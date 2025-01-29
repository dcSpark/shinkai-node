use rusqlite::{params, Result, Row, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;

use crate::SqliteManager;

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiWallet {
    pub id: i64,
    pub secret_key: String,
    pub is_encrypted: bool,
    pub key_hash: Option<String>,
    pub wallet_type: WalletType,
    pub compatible_networks: Vec<String>,
    pub wallet_data: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WalletType {
    Mnemonic,
    Hex,
    Mpc,
}

impl FromStr for WalletType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mnemonic" => Ok(WalletType::Mnemonic),
            "hex" => Ok(WalletType::Hex),
            "mpc" => Ok(WalletType::Mpc),
            _ => Err(format!("Invalid wallet type: {}", s)),
        }
    }
}

impl SqliteManager {
    /// Adds a new wallet to the database
    pub fn add_wallet(&self, wallet: &MultiWallet) -> Result<i64> {
        let conn = self.get_connection()?;
        let compatible_networks = serde_json::to_string(&wallet.compatible_networks)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let wallet_data = serde_json::to_string(&wallet.wallet_data)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        // Convert wallet type to string directly
        let wallet_type = match wallet.wallet_type {
            WalletType::Mnemonic => "mnemonic",
            WalletType::Hex => "hex",
            WalletType::Mpc => "mpc",
        };

        conn.execute(
            "INSERT INTO shinkai_multi_wallets (
                secret_key, is_encrypted, key_hash, wallet_type, compatible_networks, wallet_data
            ) VALUES (?, ?, ?, ?, ?, ?)",
            params![
                wallet.secret_key,
                wallet.is_encrypted as i32,
                wallet.key_hash,
                wallet_type,
                compatible_networks,
                wallet_data,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Gets a wallet by its ID
    pub fn get_wallet(&self, id: i64) -> Result<Option<MultiWallet>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, secret_key, is_encrypted, key_hash, wallet_type, compatible_networks, 
                    wallet_data, created_at, updated_at 
             FROM shinkai_multi_wallets 
             WHERE id = ?",
        )?;

        let wallet = stmt.query_row(params![id], |row| self.row_to_wallet(row)).optional()?;
        Ok(wallet)
    }

    /// Gets all wallets from the database
    pub fn get_all_wallets(&self) -> Result<Vec<MultiWallet>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, secret_key, is_encrypted, key_hash, wallet_type, compatible_networks, 
                    wallet_data, created_at, updated_at 
             FROM shinkai_multi_wallets",
        )?;

        let wallets = stmt
            .query_map([], |row| self.row_to_wallet(row))?
            .collect::<Result<Vec<_>>>()?;

        Ok(wallets)
    }

    /// Removes a wallet by its ID
    pub fn remove_wallet(&self, id: i64) -> Result<bool> {
        let conn = self.get_connection()?;
        let rows_affected = conn.execute("DELETE FROM shinkai_multi_wallets WHERE id = ?", params![id])?;
        Ok(rows_affected > 0)
    }

    /// Helper function to convert a database row to a MultiWallet struct
    fn row_to_wallet(&self, row: &Row) -> Result<MultiWallet> {
        let compatible_networks: String = row.get(5)?;
        let wallet_data: String = row.get(6)?;
        let wallet_type: String = row.get(4)?;

        // Convert string to WalletType directly
        let wallet_type = match wallet_type.as_str() {
            "mnemonic" => WalletType::Mnemonic,
            "hex" => WalletType::Hex,
            "mpc" => WalletType::Mpc,
            _ => return Err(rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid wallet type")),
            )),
        };

        Ok(MultiWallet {
            id: row.get(0)?,
            secret_key: row.get(1)?,
            is_encrypted: row.get::<_, i32>(2)? != 0,
            key_hash: row.get(3)?,
            wallet_type,
            compatible_networks: serde_json::from_str(&compatible_networks)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
            wallet_data: serde_json::from_str(&wallet_data)
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use shinkai_embedding::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};

    async fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    #[tokio::test]
    async fn test_wallet_crud_operations() {
        let manager = setup_test_db().await;

        // Create a test wallet
        let mut wallet_data = serde_json::Map::new();
        wallet_data.insert(
            "derivation_path".to_string(),
            serde_json::Value::String("m/44'/0'/0'/0/0".to_string()),
        );
        wallet_data.insert(
            "provider_url".to_string(),
            serde_json::Value::String("https://example.com".to_string()),
        );

        let mut public_addresses = serde_json::Map::new();
        public_addresses.insert(
            "evm".to_string(),
            serde_json::Value::String("0x123...".to_string()),
        );
        wallet_data.insert(
            "public_address".to_string(),
            serde_json::Value::Object(public_addresses),
        );

        let wallet = MultiWallet {
            id: 0, // Will be set by the database
            secret_key: "encrypted_secret_key".to_string(),
            is_encrypted: true,
            key_hash: Some("key_hash_value".to_string()),
            wallet_type: WalletType::Mnemonic,
            compatible_networks: vec!["evm".to_string()],
            wallet_data: serde_json::Value::Object(wallet_data),
            created_at: "".to_string(), // Will be set by the database
            updated_at: "".to_string(), // Will be set by the database
        };

        // Test add_wallet
        let wallet_id = manager.add_wallet(&wallet).unwrap();
        assert!(wallet_id > 0);

        // Test get_wallet
        let retrieved_wallet = manager.get_wallet(wallet_id).unwrap().unwrap();
        assert_eq!(retrieved_wallet.secret_key, wallet.secret_key);
        assert_eq!(retrieved_wallet.is_encrypted, wallet.is_encrypted);
        assert_eq!(retrieved_wallet.key_hash, wallet.key_hash);

        // Test get_all_wallets
        let all_wallets = manager.get_all_wallets().unwrap();
        assert_eq!(all_wallets.len(), 1);
        assert_eq!(all_wallets[0].id, wallet_id);

        // Test remove_wallet
        let removed = manager.remove_wallet(wallet_id).unwrap();
        assert!(removed);

        // Verify wallet was removed
        let wallet_after_remove = manager.get_wallet(wallet_id).unwrap();
        assert!(wallet_after_remove.is_none());
    }
} 