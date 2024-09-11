use crate::wallet::wallet_manager::WalletManager;

use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};

impl ShinkaiDB {
    /// Saves the WalletManager to the database
    pub fn save_wallet_manager(&self, wallet_manager: &WalletManager) -> Result<(), ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"wallet_manager";
        let serialized_wallet_manager =
            serde_json::to_vec(wallet_manager).map_err(ShinkaiDBError::JsonSerializationError)?;
        self.db
            .put_cf(cf, key, serialized_wallet_manager)
            .map_err(ShinkaiDBError::RocksDBError)
    }

    /// Reads the WalletManager from the database
    pub fn read_wallet_manager(&self) -> Result<WalletManager, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"wallet_manager";

        match self.db.get_cf(cf, key) {
            Ok(Some(value)) => {
                let wallet_manager: WalletManager =
                    serde_json::from_slice(&value).map_err(ShinkaiDBError::JsonSerializationError)?;
                Ok(wallet_manager)
            }
            Ok(None) => Err(ShinkaiDBError::DataNotFound),
            Err(e) => Err(ShinkaiDBError::RocksDBError(e)),
        }
    }
}
