use crate::vector_fs::vector_fs_error::VectorFSError;

use super::fs_db::{FSTopic, VectorFSDB};

impl VectorFSDB {
    /// Returns the first half of the blake3 hash of the hex blake3 inbox id
    pub fn hex_blake3_to_half_hash(hex_blake3_hash: &str) -> String {
        let full_hash = blake3::hash(hex_blake3_hash.as_bytes()).to_hex().to_string();
        full_hash[..full_hash.len() / 2].to_string()
    }

    pub fn add_file_to_files_message_inbox(
        &self,
        hex_blake3_hash: String,
        file_name: String,
        file_content: Vec<u8>,
    ) -> Result<(), VectorFSError> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(&hex_blake3_hash);

        // Use Topic::MessageBoxSymmetricKeys with a prefix for encrypted inbox
        let cf_name_encrypted_inbox = format!("encyptedinbox_{}_{}", encrypted_inbox_id, file_name);

        // Get the name of the encrypted inbox from the 'inbox' topic
        // let cf_inbox = self
        //     .db
        //     .cf_handle()
        //     .expect("to be able to access Topic::TempFilesInbox");
        //
        // self.db
        //     .put_cf(cf_inbox, &cf_name_encrypted_inbox.as_bytes(), &file_content)
        //     .map_err(|_| VectorFSError::FailedFetchingValue)?;

        // Directly put the file content into the column family without using a write batch
        self.put_cf(
            FSTopic::TempFilesInbox.as_str(),
            cf_name_encrypted_inbox.as_bytes(),
            file_content,
        )
        .map_err(|_| VectorFSError::FailedFetchingValue)?;

        Ok(())
    }

    pub fn get_all_files_from_inbox(&self, hex_blake3_hash: String) -> Result<Vec<(String, Vec<u8>)>, VectorFSError> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(&hex_blake3_hash);

        // Use the same prefix for encrypted inbox as in add_file_to_files_message_inbox
        let prefix = format!("encyptedinbox_{}_", encrypted_inbox_id);

        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(FSTopic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");

        let mut files = Vec::new();

        // Get an iterator over the column family with a prefix search
        let iter = self.db.prefix_iterator_cf(cf_inbox, prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, value)) => {
                    // Attempt to convert the key to a String and strip the prefix
                    match String::from_utf8(key.to_vec()) {
                        Ok(key_str) => {
                            if let Some(file_name) = key_str.strip_prefix(&prefix) {
                                files.push((file_name.to_string(), value.to_vec()));
                            } else {
                                eprintln!("Error: Key does not start with the expected prefix.");
                            }
                        }
                        Err(e) => eprintln!("Error decoding key from UTF-8: {}", e),
                    }
                }
                Err(e) => eprintln!("Error reading from database: {}", e),
            }
        }

        Ok(files)
    }

    pub fn get_all_filenames_from_inbox(&self, hex_blake3_hash: String) -> Result<Vec<String>, VectorFSError> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(&hex_blake3_hash);

        // Use the same prefix for encrypted inbox as in add_file_to_files_message_inbox
        let prefix = format!("encyptedinbox_{}_", encrypted_inbox_id);

        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(FSTopic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");

        let mut filenames = Vec::new();

        // Get an iterator over the column family with a prefix search
        let iter = self.db.prefix_iterator_cf(cf_inbox, prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, _value)) => {
                    // Attempt to convert the key to a String and strip the prefix
                    match String::from_utf8(key.to_vec()) {
                        Ok(key_str) => {
                            if let Some(file_name) = key_str.strip_prefix(&prefix) {
                                filenames.push(file_name.to_string());
                            } else {
                                eprintln!("Error: Key does not start with the expected prefix.");
                            }
                        }
                        Err(e) => eprintln!("Error decoding key from UTF-8: {}", e),
                    }
                }
                Err(e) => eprintln!("Error reading from database: {}", e),
            }
        }

        Ok(filenames)
    }

    /// Removes an inbox and all its associated files.
    pub fn remove_inbox(&self, hex_blake3_hash: &str) -> Result<(), VectorFSError> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(hex_blake3_hash);

        // Use the same prefix for encrypted inbox as in add_file_to_files_message_inbox
        let prefix = format!("encyptedinbox_{}_", encrypted_inbox_id);

        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox =
            self.db
                .cf_handle(FSTopic::TempFilesInbox.as_str())
                .ok_or(VectorFSError::ColumnFamilyNotFound(
                    FSTopic::TempFilesInbox.as_str().to_string(),
                ))?;

        // Get an iterator over the column family with a prefix search to find all associated files
        let iter = self.db.prefix_iterator_cf(cf_inbox, prefix.as_bytes());

        // Start a write batch to delete all files in the inbox
        for item in iter {
            match item {
                Ok((key, _)) => {
                    // Since delete_cf does not return a result, we cannot use `?` here.
                    self.delete_cf(FSTopic::TempFilesInbox.as_str(), &key)?;
                }
                Err(_) => return Err(VectorFSError::FailedFetchingValue),
            }
        }

        Ok(())
    }

    pub fn get_file_from_inbox(&self, hex_blake3_hash: String, file_name: String) -> Result<Vec<u8>, VectorFSError> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(&hex_blake3_hash);

        // Use the same prefix for encrypted inbox as in add_file_to_files_message_inbox
        let prefix = format!("encyptedinbox_{}_{}", encrypted_inbox_id, file_name);

        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(FSTopic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");

        // Get the file content directly using the constructed key
        match self.db.get_cf(cf_inbox, prefix.as_bytes()) {
            Ok(Some(file_content)) => Ok(file_content),
            Ok(None) => Err(VectorFSError::DataNotFound),
            Err(_) => Err(VectorFSError::FailedFetchingValue),
        }
    }
}
