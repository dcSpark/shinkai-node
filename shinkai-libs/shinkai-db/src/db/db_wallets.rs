use super::{db_errors::ShinkaiDBError, ShinkaiDB, Topic};
use serde_json::Value;

impl ShinkaiDB {
    /// Saves the wallet data as a JSON Value to the database
    pub fn save_wallet_data(&self, wallet_data: &Value) -> Result<(), ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"wallet_data";
        let serialized_wallet_data = serde_json::to_vec(wallet_data).map_err(ShinkaiDBError::JsonSerializationError)?;
        self.db
            .put_cf(cf, key, serialized_wallet_data)
            .map_err(ShinkaiDBError::RocksDBError)
    }

    /// Reads the wallet data as a JSON Value from the database
    pub fn read_wallet_data(&self) -> Result<Value, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let key = b"wallet_data";

        match self.db.get_cf(cf, key) {
            Ok(Some(value)) => {
                serde_json::from_slice(&value).map_err(ShinkaiDBError::JsonSerializationError)
            }
            Ok(None) => Err(ShinkaiDBError::DataNotFound),
            Err(e) => Err(ShinkaiDBError::RocksDBError(e)),
        }
    }
}
