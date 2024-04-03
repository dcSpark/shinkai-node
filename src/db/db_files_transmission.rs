use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use chrono::Utc;
use rocksdb::{Error, IteratorMode, Options, WriteBatch};
use shinkai_message_primitives::shinkai_message::shinkai_message::{MessageBody, MessageData};
use shinkai_message_primitives::shinkai_message::shinkai_message_schemas::{JobMessage, MessageSchemaType};

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

    pub fn create_files_message_inbox(&mut self, hex_blake3_hash: String) -> Result<(), Error> {
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
        batch.put_cf(cf_message_box_symmetric_keys, cf_name_encrypted_inbox.as_bytes(), &[]);

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

    pub fn add_file_to_files_message_inbox(
        &mut self,
        hex_blake3_hash: String,
        file_name: String,
        file_content: Vec<u8>,
    ) -> Result<(), ShinkaiDBError> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(&hex_blake3_hash);

        // Use Topic::MessageBoxSymmetricKeys with a prefix for encrypted inbox
        let cf_name_encrypted_inbox = format!("encyptedinbox_{}_{}", encrypted_inbox_id, file_name);

        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");

        // Directly put the file content into the column family without using a write batch
        self.db
            .put_cf(cf_inbox, &cf_name_encrypted_inbox.as_bytes(), &file_content)
            .map_err(|_| ShinkaiDBError::FailedFetchingValue)?;

        Ok(())
    }

    pub fn get_all_files_from_inbox(&self, hex_blake3_hash: String) -> Result<Vec<(String, Vec<u8>)>, ShinkaiDBError> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(&hex_blake3_hash);

        // Use the same prefix for encrypted inbox as in add_file_to_files_message_inbox
        let prefix = format!("encyptedinbox_{}_", encrypted_inbox_id);

        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::TempFilesInbox.as_str())
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

    pub fn get_all_filenames_from_inbox(&self, hex_blake3_hash: String) -> Result<Vec<String>, ShinkaiDBError> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(&hex_blake3_hash);

        // Use the same prefix for encrypted inbox as in add_file_to_files_message_inbox
        let prefix = format!("encyptedinbox_{}_", encrypted_inbox_id);

        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::TempFilesInbox.as_str())
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

    pub fn get_file_from_inbox(&self, hex_blake3_hash: String, file_name: String) -> Result<Vec<u8>, ShinkaiDBError> {
        let encrypted_inbox_id = Self::hex_blake3_to_half_hash(&hex_blake3_hash);

        // Use the same prefix for encrypted inbox as in add_file_to_files_message_inbox
        let prefix = format!("encyptedinbox_{}_{}", encrypted_inbox_id, file_name);

        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");

        // Get the file content directly using the constructed key
        match self.db.get_cf(cf_inbox, prefix.as_bytes()) {
            Ok(Some(file_content)) => Ok(file_content),
            Ok(None) => Err(ShinkaiDBError::DataNotFound),
            Err(_) => Err(ShinkaiDBError::FailedFetchingValue),
        }
    }

    pub async fn get_kai_file_from_inbox(
        &self,
        inbox_name: String,
    ) -> Result<Option<(String, Vec<u8>)>, ShinkaiDBError> {
        let mut offset_key: Option<String> = None;
        let page_size = 20;

        loop {
            // Get a page of messages from the inbox
            let mut messages = self.get_last_messages_from_inbox(inbox_name.clone(), page_size, offset_key.clone())?;
            // Note so messages are from most recent to oldest instead
            messages.reverse();

            // If there are no more messages, break the loop
            if messages.is_empty() {
                break;
            }

            // Iterate over the messages
            for message_branch in &messages {
                let message = match message_branch.first() {
                    Some(message) => message,
                    None => continue,
                };

                // Check if the message body is unencrypted
                if let MessageBody::Unencrypted(body) = &message.body {
                    // Check if the message data is unencrypted
                    if let MessageData::Unencrypted(data) = &body.message_data {
                        // Check if the message is of type JobMessageSchema
                        if data.message_content_schema == MessageSchemaType::JobMessageSchema {
                            // Parse the raw content into a JobMessage
                            let job_message: JobMessage = serde_json::from_str(&data.message_raw_content)?;

                            // Get all file names from the file inbox
                            match self.get_all_filenames_from_inbox(job_message.files_inbox.clone()) {
                                Ok(file_names) => {
                                    // Check if any file ends with .jobkai
                                    for file_name in file_names {
                                        if file_name.ends_with(".jobkai") {
                                            // Get the file content
                                            if let Ok(file_content) = self
                                                .get_file_from_inbox(job_message.files_inbox.clone(), file_name.clone())
                                            {
                                                return Ok(Some((file_name, file_content)));
                                            }
                                        }
                                    }
                                }
                                Err(_) => {} // Ignore the error and continue
                            }
                        }
                    }
                }
            }

            // Set the offset key for the next page to the key of the last message in the current page
            offset_key = messages
                .last()
                .and_then(|path| path.first())
                .map(|message| message.calculate_message_hash_for_pagination());
        }

        Ok(None)
    }
}
