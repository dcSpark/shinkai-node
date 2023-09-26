use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use crate::schemas::identity::{DeviceIdentity, Identity, IdentityType, StandardIdentity, StandardIdentityType};
use chrono::Utc;
use ed25519_dalek::{PublicKey as SignaturePublicKey, SecretKey as SignatureStaticKey};
use mupdf::device;
use rocksdb::{Error, Options, WriteBatch, IteratorMode};
use serde_json::to_vec;
use shinkai_message_primitives::schemas::shinkai_name::ShinkaiName;
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::IdentityPermissions;
use shinkai_message_primitives::shinkai_utils::encryption::{
    encryption_public_key_to_string, encryption_public_key_to_string_ref, string_to_encryption_public_key,
};
use shinkai_message_primitives::shinkai_utils::signatures::{
    signature_public_key_to_string, signature_public_key_to_string_ref, string_to_signature_public_key,
};
use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

impl ShinkaiDB {
    pub fn write_symmetric_key(&self, public_key: &str, private_key: &[u8]) -> Result<(), ShinkaiDBError> {
        // Get the ColumnFamily handle for MessageBoxSymmetricKeys
        let cf = self
            .db
            .cf_handle(Topic::MessageBoxSymmetricKeys.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(Topic::MessageBoxSymmetricKeys.as_str().to_string()))?;

        // Write the private key to the database with the public key as the key
        self.db.put_cf(cf, public_key, private_key).map_err(|_| ShinkaiDBError::FailedFetchingValue)
    }

    pub fn read_symmetric_key(&self, public_key: &str) -> Result<Vec<u8>, ShinkaiDBError> {
        // Get the ColumnFamily handle for MessageBoxSymmetricKeys
        let cf = self
            .db
            .cf_handle(Topic::MessageBoxSymmetricKeys.as_str())
            .ok_or(ShinkaiDBError::ColumnFamilyNotFound(Topic::MessageBoxSymmetricKeys.as_str().to_string()))?;

        // Read the private key from the database using the public key
        match self.db.get_cf(cf, public_key)? {
            Some(private_key) => Ok(private_key),
            None => Err(ShinkaiDBError::DataNotFound),
        }
    }
    
    // TODO: Use ProfileBatching so it's associated with a specific profile
    pub fn create_files_message_inbox(&mut self, public_key: String) -> Result<(), Error> {
        println!("Creating encrypted inbox");
    
        // Create Options for ColumnFamily
        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);
        cf_opts.create_missing_column_families(true);
    
        // Create ColumnFamilyDescriptors for encrypted inbox
        let current_time = Utc::now().to_rfc3339();
        let cf_name_encrypted_inbox = format!("{}:::{}", public_key, current_time);
    
        // Create column families
        self.db.create_cf(&cf_name_encrypted_inbox, &cf_opts)?;
    
        // Start a write batch
        let mut batch = WriteBatch::default();
    
        // Add encrypted inbox name to the list in the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::Inbox.as_str())
            .expect("to be able to access Topic::Inbox");
        batch.put_cf(cf_inbox, &cf_name_encrypted_inbox, &cf_name_encrypted_inbox);
    
        // Commit the write batch
        self.db.write(batch)?;
    
        Ok(())
    }
    
    pub fn add_file_to_files_message_inbox(&mut self, public_key: String, file_name: String, file_content: Vec<u8>) -> Result<(), ShinkaiDBError> {
        println!("Adding file to encrypted inbox");
    
        // Create the name of the encrypted inbox
        let current_time = Utc::now().to_rfc3339();
        let cf_name_encrypted_inbox = format!("{}:::{}", public_key, current_time);
    
        // Check if the column family exists
        if self.db.cf_handle(&cf_name_encrypted_inbox).is_none() {
            return Err(ShinkaiDBError::InboxNotFound(cf_name_encrypted_inbox));
        }
    
        // Start a write batch
        let mut batch = WriteBatch::default();
    
        // Add the file to the encrypted inbox
        let cf_encrypted_inbox = self
            .db
            .cf_handle(&cf_name_encrypted_inbox)
            .ok_or(ShinkaiDBError::FailedFetchingCF)?;
        batch.put_cf(cf_encrypted_inbox, &file_name, &file_content);
    
        // Commit the write batch
        self.db.write(batch).map_err(|_| ShinkaiDBError::FailedFetchingValue)?;
    
        Ok(())
    }

    pub fn get_all_files_from_inbox(&self, public_key: String) -> Result<Vec<(String, Vec<u8>)>, ShinkaiDBError> {
        println!("Getting all files from encrypted inbox");
    
        // Create the name of the encrypted inbox
        let current_time = Utc::now().to_rfc3339();
        let cf_name_encrypted_inbox = format!("{}:::{}", public_key, current_time);
    
        // Check if the column family exists
        let cf_encrypted_inbox = self
            .db
            .cf_handle(&cf_name_encrypted_inbox)
            .ok_or(ShinkaiDBError::InboxNotFound(cf_name_encrypted_inbox))?;
    
        // Get an iterator over the column family
        let iter = self.db.iterator_cf(cf_encrypted_inbox, IteratorMode::Start);
    
        // Collect all key-value pairs in the column family
        let files: Result<Vec<(String, Vec<u8>)>, _> = iter.map(|res| {
            res.map(|(key, value)| (String::from_utf8(key.to_vec()).unwrap(), value.to_vec()))
        }).collect();
    
        files.map_err(|_| ShinkaiDBError::FailedFetchingValue)
    }
    
    pub fn get_file_from_inbox(&self, public_key: String, file_name: String) -> Result<Vec<u8>, ShinkaiDBError> {
        println!("Getting file from encrypted inbox");
    
        // Create the name of the encrypted inbox
        let current_time = Utc::now().to_rfc3339();
        let cf_name_encrypted_inbox = format!("{}:::{}", public_key, current_time);
    
        // Check if the column family exists
        let cf_encrypted_inbox = self
            .db
            .cf_handle(&cf_name_encrypted_inbox)
            .ok_or(ShinkaiDBError::InboxNotFound(cf_name_encrypted_inbox))?;
    
        // Get the file from the column family
        let file_content = self.db.get_cf(cf_encrypted_inbox, file_name)?;
    
        file_content.ok_or(ShinkaiDBError::DataNotFound)
    }
}
