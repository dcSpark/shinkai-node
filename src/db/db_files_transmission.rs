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

    // TODO: Use ProfileBatching so it's associated with a specific profile
    pub fn create_files_message_inbox(&mut self, hex_blake3_has: String) -> Result<(), Error> {
        // Create Options for ColumnFamily
        let cf_opts = Self::create_cf_options();

        // Create ColumnFamilyDescriptors for encrypted inbox
        let cf_name_encrypted_inbox = hex_blake3_has.clone();

        // Create column families
        self.db.create_cf(&cf_name_encrypted_inbox, &cf_opts)?;

        // Start a write batch
        let mut batch = WriteBatch::default();

        // Add encrypted inbox name to the list in the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");
        batch.put_cf(cf_inbox, &hex_blake3_has, &cf_name_encrypted_inbox);

        // Add current time to MessageBoxSymmetricKeysTimes with public_key as the key
        let current_time = Utc::now().to_rfc3339();
        let cf_times = self
            .db
            .cf_handle(Topic::MessageBoxSymmetricKeysTimes.as_str())
            .expect("to be able to access Topic::MessageBoxSymmetricKeysTimes");
        batch.put_cf(cf_times, &hex_blake3_has, current_time.as_bytes());

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
        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");
        let cf_name_encrypted_inbox = self
            .db
            .get_cf(cf_inbox, &hex_blake3_hash)?
            .ok_or(ShinkaiDBError::InboxNotFound(hex_blake3_hash.clone()))?;

        // Check if the column family exists
        let cf_name_encrypted_inbox_str =
            std::str::from_utf8(&cf_name_encrypted_inbox).map_err(|_| ShinkaiDBError::DataNotFound)?; // handle the error appropriately

        if self.db.cf_handle(cf_name_encrypted_inbox_str).is_none() {
            return Err(ShinkaiDBError::InboxNotFound(cf_name_encrypted_inbox_str.to_string()));
        }

        // Start a write batch
        let mut batch = WriteBatch::default();

        // Add the file to the encrypted inbox
        let cf_encrypted_inbox = self
            .db
            .cf_handle(&cf_name_encrypted_inbox_str)
            .ok_or(ShinkaiDBError::FailedFetchingCF)?;
        batch.put_cf(cf_encrypted_inbox, &file_name, &file_content);

        // Commit the write batch
        self.db.write(batch).map_err(|_| ShinkaiDBError::FailedFetchingValue)?;

        Ok(())
    }

    pub fn get_all_files_from_inbox(&self, hex_blake3_hash: String) -> Result<Vec<(String, Vec<u8>)>, ShinkaiDBError> {
        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");
        let cf_name_encrypted_inbox = self
            .db
            .get_cf(cf_inbox, &hex_blake3_hash)?
            .ok_or(ShinkaiDBError::InboxNotFound(hex_blake3_hash.clone()))?;

        // Check if the column family exists
        let cf_name_encrypted_inbox_str =
            std::str::from_utf8(&cf_name_encrypted_inbox).map_err(|_| ShinkaiDBError::DataNotFound)?; // handle the error appropriately

        if self.db.cf_handle(cf_name_encrypted_inbox_str).is_none() {
            return Err(ShinkaiDBError::InboxNotFound(cf_name_encrypted_inbox_str.to_string()));
        }

        // Get an iterator over the column family
        let cf_encrypted_inbox = self
            .db
            .cf_handle(&cf_name_encrypted_inbox_str)
            .ok_or(ShinkaiDBError::FailedFetchingCF)?;
        let iter = self.db.iterator_cf(cf_encrypted_inbox, IteratorMode::Start);

        // Collect all key-value pairs in the column family
        let files: Result<Vec<(String, Vec<u8>)>, _> = iter
            .map(|res| res.map(|(key, value)| (String::from_utf8(key.to_vec()).unwrap(), value.to_vec())))
            .collect();

        files.map_err(|_| ShinkaiDBError::FailedFetchingValue)
    }

    pub fn get_all_filenames_from_inbox(&self, hex_blake3_hash: String) -> Result<Vec<String>, ShinkaiDBError> {
        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");
        let cf_name_encrypted_inbox = self
            .db
            .get_cf(cf_inbox, &hex_blake3_hash)?
            .ok_or(ShinkaiDBError::InboxNotFound(hex_blake3_hash.clone()))?;

        // Check if the column family exists
        let cf_name_encrypted_inbox_str =
            std::str::from_utf8(&cf_name_encrypted_inbox).map_err(|_| ShinkaiDBError::DataNotFound)?; // handle the error appropriately

        if self.db.cf_handle(cf_name_encrypted_inbox_str).is_none() {
            return Err(ShinkaiDBError::InboxNotFound(cf_name_encrypted_inbox_str.to_string()));
        }

        // Get an iterator over the column family
        let cf_encrypted_inbox = self
            .db
            .cf_handle(&cf_name_encrypted_inbox_str)
            .ok_or(ShinkaiDBError::FailedFetchingCF)?;
        let iter = self.db.iterator_cf(cf_encrypted_inbox, IteratorMode::Start);

        // Collect all keys (filenames) in the column family
        let filenames: Result<Vec<String>, _> = iter
            .map(|res| res.map(|(key, _)| String::from_utf8(key.to_vec()).unwrap()))
            .collect();

        filenames.map_err(|_| ShinkaiDBError::FailedFetchingValue)
    }

    pub fn get_file_from_inbox(&self, hex_blake3_hash: String, file_name: String) -> Result<Vec<u8>, ShinkaiDBError> {
        // Get the name of the encrypted inbox from the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::TempFilesInbox.as_str())
            .expect("to be able to access Topic::TempFilesInbox");
        let cf_name_encrypted_inbox = self
            .db
            .get_cf(cf_inbox, &hex_blake3_hash)?
            .ok_or(ShinkaiDBError::InboxNotFound(hex_blake3_hash.clone()))?;

        // Check if the column family exists
        let cf_name_encrypted_inbox_str =
            std::str::from_utf8(&cf_name_encrypted_inbox).map_err(|_| ShinkaiDBError::DataNotFound)?; // handle the error appropriately

        if self.db.cf_handle(cf_name_encrypted_inbox_str).is_none() {
            return Err(ShinkaiDBError::InboxNotFound(cf_name_encrypted_inbox_str.to_string()));
        }

        // Get the file from the column family
        let cf_encrypted_inbox = self
            .db
            .cf_handle(&cf_name_encrypted_inbox_str)
            .ok_or(ShinkaiDBError::FailedFetchingCF)?;
        let file_content = self.db.get_cf(cf_encrypted_inbox, file_name)?;

        file_content.ok_or(ShinkaiDBError::DataNotFound)
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
