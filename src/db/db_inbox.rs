use chrono::DateTime;
use rand::distributions::Standard;
use rocksdb::{Error, Options, WriteBatch};
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxName, shinkai_name::ShinkaiName, shinkai_time::ShinkaiTime},
    shinkai_message::shinkai_message::ShinkaiMessage,
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};

use crate::{
    schemas::{
        identity::{IdentityType, StandardIdentity},
        inbox_permission::InboxPermission,
        smart_inbox::SmartInbox,
    },
    utils::logging_helpers::print_content_time_messages,
};

use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};

impl ShinkaiDB {
    pub fn create_empty_inbox(&mut self, inbox_name: String) -> Result<(), Error> {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("Creating inbox: {}", inbox_name),
        );
        // Create Options for ColumnFamily
        let mut cf_opts = Options::default();
        cf_opts.create_if_missing(true);
        cf_opts.create_missing_column_families(true);

        // Create ColumnFamilyDescriptors for inbox and permission lists
        let cf_name_inbox = inbox_name.clone();
        let cf_name_perms = format!("{}_perms", &inbox_name);
        let cf_name_unread_list = format!("{}_unread_list", &inbox_name);
        let cf_name_smart_inbox_name = format!("{}_smart_inbox_name", &inbox_name);

        // Create column families
        self.db.create_cf(&cf_name_inbox, &cf_opts)?;
        self.db.create_cf(&cf_name_perms, &cf_opts)?;
        self.db.create_cf(&cf_name_unread_list, &cf_opts)?;
        self.db.create_cf(&cf_name_smart_inbox_name, &cf_opts)?;

        // Start a write batch
        let mut batch = WriteBatch::default();

        // Add inbox name to the list in the 'inbox' topic
        let cf_inbox = self
            .db
            .cf_handle(Topic::Inbox.as_str())
            .expect("to be able to access Topic::Inbox");
        batch.put_cf(cf_inbox, &inbox_name, &inbox_name);

        // Add inbox name to the 'smart_inbox_name' column family
        let cf_smart_inbox_name = self
            .db
            .cf_handle(&cf_name_smart_inbox_name)
            .expect("to be able to access smart inbox name column family");
        batch.put_cf(cf_smart_inbox_name, &inbox_name, &inbox_name);

        // Commit the write batch
        self.db.write(batch)?;

        Ok(())
    }

    // This fn doesn't validate access to the inbox (not really a responsibility of the db) so it's unsafe in that regard
    pub fn unsafe_insert_inbox_message(
        &mut self,
        message: &ShinkaiMessage,
        parent_message_key: Option<String>,
    ) -> Result<(), ShinkaiDBError> {
        let inbox_name_manager = InboxName::from_message(message).map_err(ShinkaiDBError::from)?;

        // If the inbox name is empty, use the get_inbox_name function
        let inbox_name = match &inbox_name_manager {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value.clone(),
        };

        // If the inbox name is empty, use the get_inbox_name function
        if inbox_name.is_empty() {
            return Err(ShinkaiDBError::SomeError("Inbox name is empty".to_string()));
        }

        if let Err(_) | Ok(false) = inbox_name_manager.has_sender_creation_access(message.clone()) {
            // TODO: check if it has "manual" permissions for this adding identity is required as an input to this fn
            return Err(ShinkaiDBError::SomeError(
                "Sender doesn't have creation access".to_string(),
            ));
        }

        // TODO: should be check that the recipient also has access?
        // Insert the message
        let insert_result = self.insert_message_to_all(&message.clone())?;
        // println!("Insert result: {:?}", insert_result);

        // Check if the inbox topic exists and if not, create it
        if self.db.cf_handle(&inbox_name).is_none() {
            self.create_empty_inbox(inbox_name.clone())?;

            // TODO: review how to add permissions to keep stuff in sync
            // we need the identity as an input to this fn
        }

        // Calculate the hash of the message for the key
        let hash_key = message.calculate_message_hash();
        // println!("Hash key: {}", hash_key);

        // Clone the external_metadata first, then unwrap
        let ext_metadata = message.external_metadata.clone();

        // Get the scheduled time or calculate current time
        let time_key = match ext_metadata.scheduled_time.is_empty() {
            true => ShinkaiTime::generate_time_now(),
            false => ext_metadata.scheduled_time.clone(),
        };

        // Create the composite key by concatenating the time_key and the hash_key, with a separator
        let composite_key = format!("{}:::{}", time_key, hash_key);
        // println!("Composite key: {}", composite_key);

        let mut batch = rocksdb::WriteBatch::default();

        // Add the message to the inbox
        let cf_inbox = self
            .db
            .cf_handle(&inbox_name)
            .expect("Failed to get cf handle for inbox");
        batch.put_cf(cf_inbox, &composite_key, &hash_key);

        // Add the message to the unread_list inbox
        let cf_unread_list = self
            .db
            .cf_handle(&format!("{}_unread_list", inbox_name))
            .expect("Failed to get cf handle for unread_list");
        batch.put_cf(cf_unread_list, &composite_key, &hash_key);

        // If this message has a parent, add this message as a child of the parent
        if let Some(parent_key) = parent_message_key {
            // eprintln!("Adding child: {} to parent: {}", composite_key, parent_key);
            // eprintln!("Inbox name: {}", inbox_name);

            let cf_children_name = format!("{}_children", inbox_name);
            let cf_children = match self.db.cf_handle(&cf_children_name) {
                Some(cf) => cf,
                None => {
                    // eprintln!("Creating cf for children: {}", cf_children_name);
                    // Create Options for ColumnFamily
                    let mut cf_opts = Options::default();
                    cf_opts.create_if_missing(true);
                    cf_opts.create_missing_column_families(true);

                    // Create column family if it doesn't exist
                    self.db
                        .create_cf(&cf_children_name, &cf_opts)
                        .expect("Failed to create cf for children");
                    self.db
                        .cf_handle(&cf_children_name)
                        .expect("Failed to get cf handle for children")
                }
            };

            let existing_children_bytes = self.db.get_cf(cf_children, &parent_key)?.unwrap_or_default();
            let existing_children = String::from_utf8(existing_children_bytes)
                .unwrap()
                .split(',')
                .map(String::from)
                .collect::<Vec<String>>();

            let mut children = vec![composite_key];
            children.extend_from_slice(&existing_children);
            batch.put_cf(cf_children, &parent_key, children.join(","));

            // Create column family for parents if it doesn't exist
            let cf_parents_name = format!("{}_parents", inbox_name);
            let cf_parents = match self.db.cf_handle(&cf_parents_name) {
                Some(cf) => cf,
                None => {
                    // Create Options for ColumnFamily
                    let mut cf_opts = Options::default();
                    cf_opts.create_if_missing(true);
                    cf_opts.create_missing_column_families(true);

                    // Create column family if it doesn't exist
                    self.db
                        .create_cf(&cf_parents_name, &cf_opts)
                        .expect("Failed to create cf for parents");
                    self.db
                        .cf_handle(&cf_parents_name)
                        .expect("Failed to get cf handle for parents")
                }
            };

            // Add the parent key to the parents column family with the child key
            batch.put_cf(cf_parents, &hash_key, parent_key);
        }

        self.db.write(batch)?;

        // Call get_last_messages_from_inbox and print the results
        // eprintln!("Calling get_last_messages_from_inbox");
        // let _ = self.get_last_messages_from_inbox(inbox_name.clone(), 10, None)?;
        // println!("Last messages: {:?}", last_messages);

        Ok(())
    }

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
        let mut skip_first = until_offset_key.is_some();
        let mut paths = Vec::new();

        // Get the next key from the iterator, unless we're skipping the first one
        let mut current_key: Option<String> = match iter.next() {
            Some(Ok((key, _))) if !skip_first => Some(String::from_utf8(key.to_vec()).unwrap()),
            _ => None, // No more messages, so break the loop
        };
        skip_first = false;

        // Loop through the messages
        // This loop is for fetching 'n' messages
        for _ in 0..n {
            let mut path = Vec::new();

            if current_key.clone().is_none() {
                continue;
            }

            let key = current_key.clone().unwrap();
            // This loop is for traversing up the tree from the current message
            loop {
                println!("Fetching message with key: {}", key);
                // Fetch the message from the AllMessages CF
                // Split the composite key to get the hash key
                let split: Vec<&str> = key.split(":::").collect();
                let hash_key = if split.len() < 2 {
                    // If the key does not contain ":::", assume it's a hash key
                    key.clone()
                } else {
                    split[1].to_string()
                };
                eprintln!("Current hash key: {}", hash_key);

                let mut added_message_hash: Option<String> = None;
                // Fetch the message from the AllMessages CF using the hash key
                match self.db.get_cf(messages_cf, hash_key.as_bytes())? {
                    Some(bytes) => {
                        let message = ShinkaiMessage::decode_message_result(bytes)?;
                        eprintln!(
                            "Found for hash key: {:?} Message: {:?} \n",
                            hash_key,
                            message.get_message_content()
                        );
                        added_message_hash = Some(message.calculate_message_hash());
                        path.push(message);
                    }
                    None => {
                        println!("Failed to find message with key: {}", hash_key);
                        return Err(ShinkaiDBError::MessageNotFound);
                    }
                }

                // Fetch the parent message key from the parents CF
                if let Some(cf_parents) = &cf_parents {
                    match self.db.get_cf(cf_parents, hash_key.as_bytes())? {
                        Some(bytes) => {
                            let parent_key = String::from_utf8(bytes.to_vec()).unwrap();
                            eprintln!("Parent key: {}", parent_key);
                            if !parent_key.is_empty() {
                                // Update the current key to the parent key
                                current_key = Some(parent_key.clone());

                                // Fetch the children of the parent message
                                if let Some(cf_children) = &cf_children {
                                    match self.db.get_cf(cf_children, parent_key.as_bytes())? {
                                        Some(bytes) => {
                                            let children_keys = String::from_utf8(bytes.to_vec()).unwrap();
                                            eprintln!("Children keys: {}", children_keys);
                                            for child_key in children_keys.split(',') {
                                                let child_key = child_key.trim(); // Remove any leading/trailing whitespace
                                                eprintln!("Child key: {}", child_key);
                                                if !child_key.is_empty() {
                                                    // Split the composite key to get the hash key
                                                    let split: Vec<&str> = child_key.split(":::").collect();
                                                    let hash_key = if split.len() < 2 {
                                                        // If the key does not contain ":::", assume it's a hash key
                                                        child_key.to_string()
                                                    } else {
                                                        split[1].to_string()
                                                    };

                                                    if hash_key != key {
                                                        // Fetch the child message from the AllMessages CF using the hash key
                                                        match self.db.get_cf(messages_cf, hash_key.as_bytes())? {
                                                            Some(bytes) => {
                                                                let message =
                                                                    ShinkaiMessage::decode_message_result(bytes)?;
                                                                eprintln!(
                                                                    "Found for child key: {:?} Message: {:?} \n",
                                                                    child_key,
                                                                    message.get_message_content()
                                                                );
                                                                // Check if the message to be added is the same as the last added message
                                                                // This is to avoid adding duplicate messages in the path
                                                                if Some(message.calculate_message_hash())
                                                                    != added_message_hash
                                                                {
                                                                    path.push(message);
                                                                }
                                                            }
                                                            None => {
                                                                println!(
                                                                    "Failed to find message with key: {}",
                                                                    hash_key
                                                                );
                                                                return Err(ShinkaiDBError::MessageNotFound);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        None => {} // No children messages, so do nothing
                                    }
                                }
                                break; // Break the loop once we've processed the parent and its children
                            }
                        }
                        None => break, // No parent message, so we've reached the root of the path
                    }
                } else {
                    break; // No parents CF, so we've reached the root of the path
                }
            }

            // Add the path to the list of paths
            paths.push(path);
        }

        // Reverse the paths to match the desired output order. Most recent at the end.
        paths.reverse();
        Ok(paths)
    }

    pub fn mark_as_read_up_to(&mut self, inbox_name: String, up_to_offset: String) -> Result<(), ShinkaiDBError> {
        // Fetch the column family for the specified unread_list
        let cf_name_unread_list = format!("{}_unread_list", inbox_name);
        let unread_list_cf = match self.db.cf_handle(&cf_name_unread_list) {
            Some(cf) => cf,
            None => {
                return Err(ShinkaiDBError::InboxNotFound(format!(
                    "Inbox not found: {}",
                    inbox_name
                )))
            }
        };

        // Create an iterator for the specified unread_list, starting from the beginning
        let iter = self.db.iterator_cf(unread_list_cf, rocksdb::IteratorMode::Start);

        // Iterate through the unread_list and delete all messages up to the specified offset
        for item in iter {
            // Handle the Result returned by the iterator
            match item {
                Ok((key, _)) => {
                    let key_str = match String::from_utf8(key.to_vec()) {
                        Ok(s) => s,
                        Err(_) => return Err(ShinkaiDBError::SomeError("UTF-8 conversion error".to_string())),
                    };

                    if key_str <= up_to_offset {
                        // Delete the message from the unread_list
                        self.db.delete_cf(unread_list_cf, key)?;
                    } else {
                        // We've passed the up_to_offset, so we can break the loop
                        break;
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    pub fn get_last_unread_messages_from_inbox(
        &self,
        inbox_name: String,
        mut n: usize,
        from_offset_key: Option<String>,
    ) -> Result<Vec<ShinkaiMessage>, ShinkaiDBError> {
        // Fetch the column family for the specified unread_list
        let cf_name_unread_list = format!("{}_unread_list", inbox_name);
        let unread_list_cf = match self.db.cf_handle(&cf_name_unread_list) {
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

        // Create an iterator for the specified unread_list
        let mut iter = match &from_offset_key {
            Some(from_key) => self.db.iterator_cf(
                unread_list_cf,
                rocksdb::IteratorMode::From(from_key.as_bytes(), rocksdb::Direction::Forward),
            ),
            None => self.db.iterator_cf(unread_list_cf, rocksdb::IteratorMode::Start),
        };

        let offset_hash = match &from_offset_key {
            Some(offset_key) => {
                let split: Vec<&str> = offset_key.split(":::").collect();
                if split.len() < 2 {
                    return Err(ShinkaiDBError::SomeError("Invalid offset key format".to_string()));
                }
                Some(split[1].to_string())
            }
            None => None,
        };

        let mut messages = Vec::new();
        let mut first_message = true;
        if from_offset_key.is_some() {
            n += 1;
        }
        for item in iter.take(n) {
            // Handle the Result returned by the iterator
            match item {
                Ok((_, value)) => {
                    // The value of the unread_list CF is the key in the AllMessages CF
                    let message_key = value.to_vec();

                    // Fetch the message from the AllMessages CF
                    match self.db.get_cf(messages_cf, &message_key)? {
                        Some(bytes) => {
                            let message = ShinkaiMessage::decode_message_result(bytes)?;

                            // Check if the message hash matches the offset's
                            if first_message {
                                if let Some(offset_hash) = &offset_hash {
                                    if message.calculate_message_hash() == *offset_hash {
                                        first_message = false;
                                        continue;
                                    }
                                }
                                first_message = false;
                            }

                            messages.push(message);
                        }
                        None => return Err(ShinkaiDBError::MessageNotFound),
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        // print_content_time_messages(messages.clone());
        Ok(messages)
    }

    pub fn add_permission(
        &mut self,
        inbox_name: &str,
        identity: &StandardIdentity,
        perm: InboxPermission,
    ) -> Result<(), ShinkaiDBError> {
        // Call the new function with the extracted profile name
        let shinkai_profile = identity.full_identity_name.extract_profile()?;
        self.add_permission_with_profile(inbox_name, shinkai_profile, perm)
    }

    pub fn add_permission_with_profile(
        &mut self,
        inbox_name: &str,
        profile: ShinkaiName,
        perm: InboxPermission,
    ) -> Result<(), ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name()
            .clone()
            .ok_or(ShinkaiDBError::InvalidIdentityName(profile.to_string()))?;

        // Fetch column family for identity
        let cf_identity =
            self.db
                .cf_handle(Topic::ProfilesIdentityKey.as_str())
                .ok_or(ShinkaiDBError::IdentityNotFound(format!(
                    "Identity not found for: {}",
                    profile_name
                )))?;

        // Check if the identity exists
        if self.db.get_cf(cf_identity, profile_name.clone().to_string())?.is_none() {
            return Err(ShinkaiDBError::IdentityNotFound(format!(
                "Identity not found for: {}",
                profile_name
            )));
        }

        // Handle the original permission addition
        let cf_name = format!("{}_perms", inbox_name);
        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::InboxNotFound(format!(
                "Inbox not found: {}",
                inbox_name
            )))?;
        let perm_val = perm.to_i32().to_string(); // Convert permission to i32 and then to String
        self.db.put_cf(cf, profile_name.to_string(), perm_val)?;
        Ok(())
    }

    pub fn remove_permission(&mut self, inbox_name: &str, identity: &StandardIdentity) -> Result<(), ShinkaiDBError> {
        let profile_name =
            identity
                .full_identity_name
                .get_profile_name()
                .clone()
                .ok_or(ShinkaiDBError::InvalidIdentityName(
                    identity.full_identity_name.to_string(),
                ))?;

        // Fetch column family for identity
        let cf_identity =
            self.db
                .cf_handle(Topic::ProfilesIdentityKey.as_str())
                .ok_or(ShinkaiDBError::IdentityNotFound(format!(
                    "Identity not found for: {}",
                    identity.full_identity_name
                )))?;

        // Check if the identity exists
        if self.db.get_cf(cf_identity, profile_name.clone())?.is_none() {
            return Err(ShinkaiDBError::IdentityNotFound(format!(
                "Identity not found for: {}",
                identity.full_identity_name
            )));
        }

        // Handle the original permission removal
        let cf_name = format!("{}_perms", inbox_name);
        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::InboxNotFound(format!(
                "Inbox not found: {}",
                inbox_name
            )))?;
        self.db.delete_cf(cf, profile_name.clone())?;
        Ok(())
    }

    pub fn has_permission(
        &self,
        inbox_name: &str,
        identity: &StandardIdentity,
        perm: InboxPermission,
    ) -> Result<bool, ShinkaiDBError> {
        let profile_name =
            identity
                .full_identity_name
                .get_profile_name()
                .clone()
                .ok_or(ShinkaiDBError::InvalidIdentityName(
                    identity.full_identity_name.to_string(),
                ))?;

        // Fetch column family for identity
        let cf_identity =
            self.db
                .cf_handle(Topic::ProfilesIdentityKey.as_str())
                .ok_or(ShinkaiDBError::IdentityNotFound(format!(
                    "Identity not found for: {}",
                    identity.full_identity_name
                )))?;

        // Check if the identity exists
        if self.db.get_cf(cf_identity, profile_name.clone())?.is_none() {
            return Err(ShinkaiDBError::IdentityNotFound(format!(
                "Identity not found for: {}",
                identity.full_identity_name
            )));
        }

        // Fetch column family for permissions
        let cf_permission =
            self.db
                .cf_handle(Topic::ProfilesIdentityType.as_str())
                .ok_or(ShinkaiDBError::PermissionNotFound(format!(
                    "Permission not found for: {}",
                    identity.full_identity_name
                )))?;

        // Get the permission type for the identity
        let perm_type_bytes =
            self.db
                .get_cf(cf_permission, profile_name.clone())?
                .ok_or(ShinkaiDBError::PermissionNotFound(format!(
                    "Permission not found for: {}",
                    identity.full_identity_name
                )))?;
        let perm_type_str = String::from_utf8(perm_type_bytes.to_vec())
            .map_err(|_| ShinkaiDBError::SomeError("UTF-8 conversion error".to_string()))?;

        // TODO: perm_type not used?
        // TODO(?): if it's admin it should be able to access anything :?
        let perm_type = IdentityType::to_enum(&perm_type_str).ok_or(ShinkaiDBError::InvalidIdentityType(format!(
            "Invalid identity type for: {}",
            identity.full_identity_name
        )))?;

        // Handle the original permission check
        let cf_name = format!("{}_perms", inbox_name);
        let cf = self
            .db
            .cf_handle(&cf_name)
            .ok_or(ShinkaiDBError::InboxNotFound(format!(
                "Inbox not found: {}",
                inbox_name
            )))?;
        match self.db.get_cf(cf, profile_name.clone())? {
            Some(val) => {
                let val_str = String::from_utf8(val.to_vec())
                    .map_err(|_| ShinkaiDBError::SomeError("UTF-8 conversion error".to_string()))?;
                let val_perm = InboxPermission::from_i32(
                    val_str
                        .parse::<i32>()
                        .map_err(|_| ShinkaiDBError::SomeError("UTF-8 conversion error".to_string()))?,
                )?;
                Ok(val_perm >= perm)
            }
            None => Ok(false),
        }
    }

    pub fn get_inboxes_for_profile(
        &self,
        profile_name_identity: StandardIdentity,
    ) -> Result<Vec<String>, ShinkaiDBError> {
        // Fetch the column family for the 'inbox' topic
        let cf_inbox = match self.db.cf_handle(Topic::Inbox.as_str()) {
            Some(cf) => cf,
            None => {
                return Err(ShinkaiDBError::InboxNotFound(format!(
                    "Inbox not found: {}",
                    profile_name_identity
                )))
            }
        };

        // Create an iterator for the 'inbox' topic
        let iter = self.db.iterator_cf(cf_inbox, rocksdb::IteratorMode::Start);

        let mut inboxes = Vec::new();
        for item in iter {
            // Handle the Result returned by the iterator
            match item {
                Ok((key, _)) => {
                    let key_str = String::from_utf8_lossy(&key);
                    if key_str.contains(&profile_name_identity.full_identity_name.to_string()) {
                        inboxes.push(key_str.to_string());
                    } else {
                        // Check if the identity has read permission for the inbox
                        match self.has_permission(&key_str, &profile_name_identity, InboxPermission::Read) {
                            Ok(has_perm) => {
                                if has_perm {
                                    inboxes.push(key_str.to_string());
                                }
                            }
                            Err(e) => return Err(e),
                        }
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
        shinkai_log(
            ShinkaiLogOption::API,
            ShinkaiLogLevel::Info,
            &format!("Inboxes: {}", inboxes.join(", ")),
        );
        Ok(inboxes)
    }

    pub fn get_all_smart_inboxes_for_profile(
        &self,
        profile_name_identity: StandardIdentity,
    ) -> Result<Vec<SmartInbox>, ShinkaiDBError> {
        let inboxes = self.get_inboxes_for_profile(profile_name_identity)?;

        let mut smart_inboxes = Vec::new();

        for inbox_id in inboxes {
            shinkai_log(
                ShinkaiLogOption::API,
                ShinkaiLogLevel::Info,
                &format!("Inbox: {}", inbox_id),
            );

            let last_message = self
                .get_last_messages_from_inbox(inbox_id.clone(), 1, None)?
                .into_iter()
                .next()
                .and_then(|mut v| v.pop());

            // Fetch the custom name from the smart_inbox_name column family
            let cf_name_smart_inbox_name = format!("{}_smart_inbox_name", &inbox_id);
            let cf_smart_inbox_name = self
                .db
                .cf_handle(&cf_name_smart_inbox_name)
                .expect("to be able to access smart inbox name column family");
            let custom_name = match self.db.get_cf(cf_smart_inbox_name, &inbox_id)? {
                Some(val) => String::from_utf8(val.to_vec())
                    .map_err(|_| ShinkaiDBError::SomeError("UTF-8 conversion error".to_string()))?,
                None => inbox_id.clone(), // Use the inbox_id as the default value if the custom name is not found
            };

            // Determine if the inbox is finished
            let is_finished = if inbox_id.starts_with("job::") {
                let job = self.get_job(&inbox_id)?;
                job.is_finished
            } else {
                false
            };

            let smart_inbox = SmartInbox {
                inbox_id: inbox_id.clone(),
                custom_name,
                last_message,
                is_finished,
            };

            smart_inboxes.push(smart_inbox);
        }

        // Sort the smart_inboxes by the timestamp of the last message
        smart_inboxes.sort_by(|a, b| match (&a.last_message, &b.last_message) {
            (Some(a_msg), Some(b_msg)) => {
                let a_time = DateTime::parse_from_rfc3339(&a_msg.external_metadata.scheduled_time).unwrap();
                let b_time = DateTime::parse_from_rfc3339(&b_msg.external_metadata.scheduled_time).unwrap();
                b_time.cmp(&a_time)
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        Ok(smart_inboxes)
    }

    pub fn update_smart_inbox_name(&mut self, inbox_id: &str, new_name: &str) -> Result<(), ShinkaiDBError> {
        // Fetch the column family for the smart_inbox_name
        let cf_name_smart_inbox_name = format!("{}_smart_inbox_name", inbox_id);
        let cf_smart_inbox_name = self
            .db
            .cf_handle(&cf_name_smart_inbox_name)
            .ok_or(ShinkaiDBError::InboxNotFound(format!("Inbox not found: {}", inbox_id)))?;

        // Update the name in the column family
        self.db.put_cf(cf_smart_inbox_name, inbox_id, new_name)?;

        Ok(())
    }
}
