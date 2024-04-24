use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use shinkai_message_primitives::{schemas::inbox_name::InboxName, shinkai_message::shinkai_message::ShinkaiMessage};
use shinkai_vector_resources::shinkai_time::ShinkaiStringTime;
use tracing::instrument;

impl ShinkaiDB {
    pub fn fetch_message_and_hash(&self, hash_key: &str) -> Result<(ShinkaiMessage, String), ShinkaiDBError> {
        // Fetch the column family for all messages directly
        let messages_cf = self.get_cf_handle(Topic::AllMessages).unwrap();

        match self.db.get_cf(messages_cf, hash_key.as_bytes())? {
            Some(bytes) => {
                let message = ShinkaiMessage::decode_message_result(bytes)?;
                // eprintln!(
                //     "Found for hash key: {:?} Message: {:?} \n",
                //     hash_key,
                //     message.get_message_content()
                // );
                let message_hash = message.calculate_message_hash_for_pagination();
                Ok((message, message_hash))
            }
            None => Err(ShinkaiDBError::MessageNotFound),
        }
    }

    pub fn get_parent_message_hash(&self, inbox_name: &str, hash_key: &str) -> Result<Option<String>, ShinkaiDBError> {
        // Convert the inbox name to its hash value first half for consistency with the new key format
        let inbox_hash = InboxName::new(inbox_name.to_string())?.hash_value_first_half();

        // Fetch the column family for Inbox, as we are now using the Inbox CF for parent messages as well
        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();

        // Construct the key for fetching the parent message using the new format
        let parent_message_key = format!("inbox_{}_parent_{}", inbox_hash, hash_key);

        // Attempt to fetch the parent message key using the constructed key
        match self.db.get_cf(cf_inbox, parent_message_key.as_bytes())? {
            Some(bytes) => {
                let parent_key_str = String::from_utf8(bytes.to_vec()).unwrap();
                // Split the composite key to get the hash key of the parent
                let split: Vec<&str> = parent_key_str.split(":::").collect();
                let parent_hash_key = if split.len() < 2 {
                    // If the key does not contain ":::", assume it's a hash key
                    parent_key_str
                } else {
                    split[1].to_string()
                };
                Ok(Some(parent_hash_key))
            }
            None => Ok(None), // No parent message found
        }
    }

    /// Extract the identifier key from the full key
    /// Input: inbox_53a92e9e4c9427f5becf26c1fd6ffe51_message_TIMEKEY:::HASHKEY
    /// Output: Some("TIMEKEY:::HASHKEY")
    fn extract_identifier_key(full_key: &str) -> Option<String> {
        let prefix_length = 47; // The fixed length of the prefix
        if full_key.len() > prefix_length {
            // Extract everything after the prefix and return it
            Some(full_key[prefix_length..].to_string())
        } else {
            // Return None if the key does not have the expected prefix length
            None
        }
    }

    fn get_message_offset_db_key(message: &ShinkaiMessage) -> Result<String, ShinkaiDBError> {
        // Calculate the hash of the message for the key
        let hash_key = message.calculate_message_hash_for_pagination();

        // Clone the external_metadata first, then unwrap
        let ext_metadata = message.external_metadata.clone();

        // Get the scheduled time or calculate current time
        let time_key = match ext_metadata.scheduled_time.is_empty() {
            true => ShinkaiStringTime::generate_time_now(),
            false => ext_metadata.scheduled_time.clone(),
        };

        // Create the composite key by concatenating the time_key and the hash_key, with a separator
        let composite_key = format!("{}:::{}", time_key, hash_key);

        Ok(composite_key)
    }

    /*
    Get the last messages from an inbox
    Note: This code is messy because the messages could be in a tree, sequential or a mix of both
     */
    // TODO: clean up and add comments. Complex code!
    #[instrument]
    pub fn get_last_messages_from_inbox(
        &self,
        inbox_name: String,
        n: usize,
        until_offset_hash_key: Option<String>,
    ) -> Result<Vec<Vec<ShinkaiMessage>>, ShinkaiDBError> {
        // Fetch the column family for Inbox
        let cf_inbox = self.db.cf_handle(Topic::Inbox.as_str()).unwrap();
        let inbox_hash = InboxName::new(inbox_name.clone())?.hash_value_first_half();

        // Create an iterator for the specified inbox, using a key prefix to filter messages
        let inbox_key_prefix = format!("inbox_{}_message_", inbox_hash);

        let iter = self.db.prefix_iterator_cf(cf_inbox, inbox_key_prefix.as_bytes());

        // Initialize current_key as None. It will be updated with the last key encountered.
        let mut current_key: Option<String> = None;

        // prefix_iterator_cf doesn't allow to iterate in reverse order so we need to collect all keys
        // So we collect only keys into a vector and then iterate in reverse order
        let mut keys = Vec::new();
        for item in iter {
            match item {
                Ok((key, _)) => {
                    let key_str = String::from_utf8(key.to_vec()).unwrap();
                    // Use the new function to extract the identifier key
                    if let Some(identifier_key) = Self::extract_identifier_key(&key_str) {
                        keys.push(identifier_key.clone());
                        // Update current_key with the latest identifier key encountered
                        current_key = Some(identifier_key);
                    }
                }
                Err(e) => return Err(ShinkaiDBError::from(e)),
            }
        }

        let mut start_index = 0;
        // If an until_offset_hash_key is provided, find its position in the keys vector
        if let Some(ref until_hash) = until_offset_hash_key {
            // Iterate over keys to find the key that contains the until_offset_hash_key
            for (index, key) in keys.iter().enumerate() {
                if let Some((_, hash_key)) = key.rsplit_once(":::") {
                    if hash_key == until_hash {
                        start_index = index;
                        current_key = key.clone().into();
                        break;
                    }
                }
            }
        }

        // Skip the first message if an offset key is provided so it doesn't get included
        let mut paths = Vec::new();

        // If empty return early
        if current_key.is_none() {
            return Ok(paths);
        }

        // Loop through the messages
        // This loop is for fetching 'n' messages
        let mut first_iteration = true;
        let mut tree_found = false;
        // eprintln!("n: {}", n);
        let total_elements = until_offset_hash_key.is_some().then(|| n + 1).unwrap_or(n);
        let keys = keys.clone().into_iter().rev().collect::<Vec<String>>();

        for _i in 0..total_elements {
            let mut path = Vec::new();

            let key = match current_key.clone() {
                Some(k) => k,
                None => break,
            };
            current_key = None;

            // Split the composite key to get the hash key
            let split: Vec<&str> = key.split(":::").collect();
            let hash_key = if split.len() < 2 {
                // If the key does not contain ":::", assume it's a hash key
                key.clone()
            } else {
                split[1].to_string()
            };

            let mut added_message_hash_tmp: Option<String> = None;
            // Fetch the message from the AllMessages CF using the hash key
            match self.fetch_message_and_hash(&hash_key) {
                Ok((message, added_message_hash)) => {
                    added_message_hash_tmp = Some(added_message_hash);
                    path.push(message.clone());
                }
                Err(e) => return Err(e),
            }

            // Fetch the parent message key from the Inbox CF using the specific prefix
            let message_parent_key = format!("inbox_{}_parent_{}", inbox_hash, hash_key);
            if let Some(parent_key) = self.db.get_cf(cf_inbox, message_parent_key.as_bytes())? {
                let parent_key_str = String::from_utf8(parent_key.to_vec()).unwrap();
                if !parent_key_str.is_empty() {
                    tree_found = true;
                    // Update the current key to the parent key
                    current_key = Some(parent_key_str.clone());

                    // Fetch the children of the parent message
                    let parent_children_key = format!("inbox_{}_children_{}", inbox_hash, parent_key_str);
                    let existing_children_bytes = self
                        .db
                        .get_cf(cf_inbox, parent_children_key.as_bytes())?
                        .unwrap_or_default();
                    let existing_children = String::from_utf8(existing_children_bytes)
                        .unwrap()
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect::<Vec<String>>();

                    // Skip fetching children for the first message
                    if !first_iteration {
                        for child_key in existing_children {
                            // Fetch and add the child message to the path
                            if let Ok((child_message, _)) = self.fetch_message_and_hash(&child_key) {
                                if Some(child_message.calculate_message_hash_for_pagination()) != added_message_hash_tmp
                                {
                                    path.push(child_message);
                                }
                            }
                        }
                    }
                }
            } else {
                // eprintln!("No parent message, reached the root of the path");
            }

            // Add the path to the list of paths
            paths.push(path);

            // We check if no parent was found, which means we reached the root of the path
            // If so, let's check if there is a solitary message if not then break
            if current_key.clone().is_none() {
                // Move the iterator forward until it matches the current key
                if tree_found {
                    let mut found = false;
                    for potential_next_key in &keys {
                        if found {
                            current_key = Some(potential_next_key.clone());
                            break;
                        }
                        if let Some((_, hash_key)) = potential_next_key.rsplit_once(":::") {
                            if hash_key == &key {
                                found = true;
                            }
                        }
                    }
                } else {
                    // If no tree was found, simply move to the next key in the list
                    if let Some(index) = keys.iter().position(|r| r == &key) {
                        if index + 1 < keys.len() {
                            current_key = Some(keys[index + 1].clone());
                        }
                    }
                }

                if current_key.is_none() {
                    break;
                }
            }

            // First iteration false
            first_iteration = false;
        }

        // Reverse the paths to match the desired output order. Most recent at the end.
        paths.reverse();

        // If an until_offset_key is provided, drop the last element of the paths array
        if until_offset_hash_key.is_some() {
            paths.pop();
        }

        Ok(paths)
    }
}
