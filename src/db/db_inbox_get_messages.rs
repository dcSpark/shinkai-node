use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};
use shinkai_message_primitives::shinkai_message::shinkai_message::ShinkaiMessage;

impl ShinkaiDB {
    fn fetch_message_and_hash(
        &self,
        messages_cf: &rocksdb::ColumnFamily,
        hash_key: &str,
    ) -> Result<(ShinkaiMessage, String), ShinkaiDBError> {
        match self.db.get_cf(messages_cf, hash_key.as_bytes())? {
            Some(bytes) => {
                let message = ShinkaiMessage::decode_message_result(bytes)?;
                // eprintln!(
                //     "Found for hash key: {:?} Message: {:?} \n",
                //     hash_key,
                //     message.get_message_content()
                // );
                let message_hash = message.calculate_message_hash();
                Ok((message, message_hash))
            }
            None => {
                // println!("Failed to find message with key: {}", hash_key);
                Err(ShinkaiDBError::MessageNotFound)
            }
        }
    }

    fn fetch_parent_message(
        &self,
        cf_parents: &rocksdb::ColumnFamily,
        hash_key: &str,
    ) -> Result<Option<String>, ShinkaiDBError> {
        match self.db.get_cf(cf_parents, hash_key.as_bytes())? {
            Some(bytes) => {
                let parent_key = String::from_utf8(bytes.to_vec()).unwrap();
                Ok(Some(parent_key))
            }
            None => Ok(None),
        }
    }

    fn fetch_children_messages(
        &self,
        cf_children: &rocksdb::ColumnFamily,
        parent_key: &str,
        messages_cf: &rocksdb::ColumnFamily,
    ) -> Result<Vec<ShinkaiMessage>, ShinkaiDBError> {
        // eprintln!("Fetching children for parent: {:?}", parent_key);
        let mut children_messages = Vec::new();
        match self.db.get_cf(cf_children, parent_key.as_bytes())? {
            Some(bytes) => {
                let children_keys = String::from_utf8(bytes.to_vec()).unwrap();
                for child_key in children_keys.split(',') {
                    let child_key = child_key.trim(); // Remove any leading/trailing whitespace
                    if !child_key.is_empty() {
                        // Split the composite key to get the hash key
                        let split: Vec<&str> = child_key.split(":::").collect();
                        let hash_key = if split.len() < 2 {
                            // If the key does not contain ":::", assume it's a hash key
                            child_key.to_string()
                        } else {
                            split[1].to_string()
                        };

                        // Fetch the child message from the AllMessages CF using the hash key
                        match self.db.get_cf(messages_cf, hash_key.as_bytes())? {
                            Some(bytes) => {
                                let message = ShinkaiMessage::decode_message_result(bytes)?;
                                children_messages.push(message);
                            }
                            None => return Err(ShinkaiDBError::MessageNotFound),
                        }
                    }
                }
            }
            None => {} // No children messages, so do nothing
        }
        Ok(children_messages)
    }

    /*
    Get the last messages from an inbox
    Note: This code is messy because the messages could be in a tree, sequential or a mix of both
     */
    pub fn get_last_messages_from_inbox(
        &self,
        inbox_name: String,
        n: usize,
        until_offset_key: Option<String>,
    ) -> Result<Vec<Vec<ShinkaiMessage>>, ShinkaiDBError> {
        // println!("Getting last {} messages from inbox: {}", n, inbox_name);
        // println!("Offset key: {:?}", until_offset_key);
        // println!("n: {:?}", n);

        // Fetch the column family for the specified inbox
        let inbox_cf = match self.db.cf_handle(&inbox_name) {
            Some(cf) => cf,
            None => {
                return Err(ShinkaiDBError::InboxNotFound(format!(
                    "Inbox not found: {}",
                    inbox_name
                )))
            }
        };

        // Fetch the column family for all messages
        let messages_cf = self.cf_handle(Topic::AllMessages.as_str())?;

        // Fetch the column family for parents and children
        let cf_parents_name = format!("{}_parents", inbox_name);
        let cf_parents = self.db.cf_handle(&cf_parents_name);
        let cf_children_name = format!("{}_children", inbox_name);
        let cf_children = self.db.cf_handle(&cf_children_name);

        // Create an iterator for the specified inbox
        let mut iter = match &until_offset_key {
            Some(offset_key) => self.db.iterator_cf(
                inbox_cf,
                rocksdb::IteratorMode::From(offset_key.as_bytes(), rocksdb::Direction::Reverse),
            ),
            None => self.db.iterator_cf(inbox_cf, rocksdb::IteratorMode::End),
        };

        // Skip the first message if an offset key is provided so it doesn't get included
        let skip_first = until_offset_key.is_some();
        let mut paths = Vec::new();

        // Get the next key from the iterator, unless we're skipping the first one
        let mut current_key: Option<String> = match iter.next() {
            Some(Ok((key, _))) if !skip_first => Some(String::from_utf8(key.to_vec()).unwrap()),
            _ => None, // No more messages, so break the loop
        };

         // If skip_first is true, get the next key
         if skip_first {
            current_key = match iter.next() {
                Some(Ok((key, _))) => Some(String::from_utf8(key.to_vec()).unwrap()),
                _ => None, // No more messages, so break the loop
            };
        }

        // If empty return early
        if current_key.is_none() {
            return Ok(paths);
        }

        // Loop through the messages
        // This loop is for fetching 'n' messages
        let mut first_iteration = true;
        let mut tree_found = false;
        // eprintln!("n: {}", n);
        for i in 0..n {
            // eprintln!("\n\n------\niteration: {}", i);
            let mut path = Vec::new();

            let key = current_key.clone().unwrap();
            current_key = None;
            // This loop is for traversing up the tree from the current message
            // println!("Fetching message with key: {}", key);

            // Fetch the message from the AllMessages CF
            // Split the composite key to get the hash key
            let split: Vec<&str> = key.split(":::").collect();
            let hash_key = if split.len() < 2 {
                // If the key does not contain ":::", assume it's a hash key
                key.clone()
            } else {
                split[1].to_string()
            };
            // eprintln!("Current hash key: {}", hash_key);

            let mut added_message_hash_tmp: Option<String> = None;
            // Fetch the message from the AllMessages CF using the hash key
            match self.fetch_message_and_hash(messages_cf, &hash_key) {
                Ok((message, added_message_hash)) => {
                    added_message_hash_tmp = Some(added_message_hash);
                    path.push(message.clone());
                    // eprintln!(
                    //     "Message fetched and added to path. Message content: {}",
                    //     message.clone().get_message_content().unwrap()
                    // );
                }
                Err(e) => return Err(e),
            }

            // Fetch the parent message key from the parents CF
            if let Some(cf_parents) = &cf_parents {
                if let Some(parent_key) = self.fetch_parent_message(cf_parents, &hash_key)? {
                    if !parent_key.is_empty() {
                        tree_found = true;
                        // Update the current key to the parent key
                        current_key = Some(parent_key.clone());
                        // eprintln!("Parent key fetched: {}", parent_key);

                        // Fetch the children of the parent message
                        if let Some(cf_children) = &cf_children {
                            // eprintln!("first_iteration? {:?}", first_iteration);
                            // Skip fetching children for the first message
                            if !first_iteration {
                                let children_messages =
                                    self.fetch_children_messages(cf_children, &parent_key, messages_cf)?;
                                for message in children_messages {
                                    if Some(message.calculate_message_hash()) != added_message_hash_tmp {
                                        path.push(message.clone());
                                        // eprintln!(
                                        //     "Child message added to path. Message content: {}",
                                        //     message.clone().get_message_content().unwrap()
                                        // );
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // eprintln!("No parent message, reached the root of the path");
                }
            } else {
                // eprintln!("No parents CF, reached the root of the path");
            }

            // Add the path to the list of paths
            paths.push(path);

            // We check if no parent was found, which means we reached the root of the path
            // If so, let's check if there is a solitary message if not then break
            if current_key.clone().is_none() {
                // eprintln!("current key is None. Key: {:?}", key);
                // Move the iterator forward until it matches the current key
                if tree_found {
                    while let Some(Ok((new_key, _))) = iter.next() {
                        let new_key_str = String::from_utf8(new_key.to_vec()).unwrap();
                        let new_key_hash = new_key_str.split(":::").nth(1).unwrap_or("");
                        // eprintln!("new_key_hash: {:?}", new_key_hash);
                        if new_key_hash == key {
                            // eprintln!("Found the current key in the iterator: {:?}", new_key_str);
                            break;
                        }
                    }
                }

                // Get the next key from the iterator
                current_key = match iter.next() {
                    Some(Ok((key, _))) => Some(String::from_utf8(key.to_vec()).unwrap()),
                    _ => None, // No more messages, so break the loop
                };

                if current_key.is_none() {
                    // eprintln!("Couldn't find a new key");
                    break;
                }
                // eprintln!("New key found: {:?}", current_key);
            }

            // First iteration false
            first_iteration = false;
        }

        // Reverse the paths to match the desired output order. Most recent at the end.
        paths.reverse();
        Ok(paths)
    }
}
