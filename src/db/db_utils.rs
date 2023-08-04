use rocksdb::IteratorMode;
use rocksdb::{Error, Options};

use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};

impl ShinkaiDB {
    #[cfg(debug_assertions)]
    pub fn print_all_from_cf(&self, cf_name: &str) -> Result<(), ShinkaiDBError> {
        println!("printing all for {}", cf_name);
        let cf = self.db.cf_handle(cf_name).ok_or(ShinkaiDBError::ColumnFamilyNotFound(cf_name.to_string()))?;
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut is_empty = true;
        for item in iter {
            let (key, value) = item.map_err(|e| ShinkaiDBError::RocksDBError(e))?;
            let key_str = std::str::from_utf8(&key)?.to_string();
            let value_str = std::str::from_utf8(&value)?.to_string();
            println!("print_all_from_cf > Key: {}, Value: {}", key_str, value_str);
            is_empty = false;
        }
        if is_empty {
            println!("print_all_from_cf {}: empty bucket", cf_name);
        }
        Ok(())
    }
}
