use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use chrono::Utc;
use rocksdb::{Error, WriteBatch};

impl ShinkaiDB {
    pub fn write_symmetric_key(&self, hex_blake3_hash: &str, private_key: &[u8]) -> Result<(), ShinkaiDBError> {
        // Get the ColumnFamily handle for MessageBoxSymmetricKeys
        let cf =
            self.db
                .cf_handle(Topic::MessageBoxSymmetricKeys.as_str())
                .ok_or(ShinkaiDBError::ColumnFamilyNotFound(
                    Topic::MessageBoxSymmetricKeys.as_str().to_string(),
                ))?;

        // Write the private key to the database with the public key as the key
        self.db
            .put_cf(cf, hex_blake3_hash, private_key)
            .map_err(|_| ShinkaiDBError::FailedFetchingValue)
    }

    pub fn read_symmetric_key(&self, hex_blake3_hash: &str) -> Result<Vec<u8>, ShinkaiDBError> {
        // Get the ColumnFamily handle for MessageBoxSymmetricKeys
        let cf =
            self.db
                .cf_handle(Topic::MessageBoxSymmetricKeys.as_str())
                .ok_or(ShinkaiDBError::ColumnFamilyNotFound(
                    Topic::MessageBoxSymmetricKeys.as_str().to_string(),
                ))?;

        // Read the private key from the database using the public key
        match self.db.get_cf(cf, hex_blake3_hash)? {
            Some(private_key) => Ok(private_key),
            None => Err(ShinkaiDBError::DataNotFound),
        }
    }

    /// Returns the first half of the blake3 hash of the hex blake3 inbox id
    pub fn hex_blake3_to_half_hash(hex_blake3_hash: &str) -> String {
        let full_hash = blake3::hash(hex_blake3_hash.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    pub fn create_files_message_inbox(&self, hex_blake3_hash: String) -> Result<(), Error> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(&hex_blake3_hash);

        // Use Topic::MessageBoxSymmetricKeys with a prefix for encrypted inbox
        let cf_name_encrypted_inbox = format!("encyptedinbox_{}_", encrypted_inbox_id);

        // Ensure the MessageBoxSymmetricKeys column family exists
        let cf_message_box_symmetric_keys = self
            .db
            .cf_handle(Topic::MessageBoxSymmetricKeys.as_str())
            .expect("to be able to access Topic::MessageBoxSymmetricKeys");

        // Start a write batch
        let mut batch = WriteBatch::default();

        // Add encrypted inbox name to the MessageBoxSymmetricKeys column family with a prefix
        batch.put_cf(cf_message_box_symmetric_keys, cf_name_encrypted_inbox.as_bytes(), []);

        // Add current time to MessageBoxSymmetricKeys with the encrypted inbox name as the key
        let current_time = Utc::now().to_rfc3339();

        let cf_name_encrypted_inbox_time = format!(
            "encyptedinbox_sorted_by_time_extraplaceholder__{}_{}",
            current_time, encrypted_inbox_id
        );
        batch.put_cf(
            cf_message_box_symmetric_keys,
            cf_name_encrypted_inbox_time.as_bytes(),
            current_time.as_bytes(),
        );

        // Commit the write batch
        self.db.write(batch)?;

        Ok(())
    }
}
