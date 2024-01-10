use chrono::DateTime;
use rocksdb::{Error, Options, WriteBatch};
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxName, shinkai_name::ShinkaiName, shinkai_time::ShinkaiStringTime},
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::WSTopic},
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};

use crate::schemas::{
    identity::{IdentityType, StandardIdentity},
    inbox_permission::InboxPermission,
    smart_inbox::SmartInbox,
};

use super::{db::Topic, db_errors::ShinkaiDBError, ShinkaiDB};

impl ShinkaiDB {
    pub async fn create_empty_inbox(&mut self, inbox_name: String) -> Result<(), Error> {
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
        let cf_name_read_list = format!("{}_read_list", &inbox_name);
        let cf_name_smart_inbox_name = format!("{}_smart_inbox_name", &inbox_name);

        // Create column families
        self.db.create_cf(&cf_name_inbox, &cf_opts)?;
        self.db.create_cf(&cf_name_perms, &cf_opts)?;
        self.db.create_cf(&cf_name_read_list, &cf_opts)?;
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

        {
            // Note: this is the code for enabling WS
            if let Some(manager) = &self.ws_manager {
                let m = manager.lock().await;
                let _ = m
                    .queue_message(WSTopic::SmartInboxes, "".to_string(), inbox_name.clone())
                    .await;
            }
        }

        Ok(())
    }

    // This fn doesn't validate access to the inbox (not really a responsibility of this db fn) so it's unsafe in that regard
    pub async fn unsafe_insert_inbox_message(
        &mut self,
        message: &ShinkaiMessage,
        maybe_parent_message_key: Option<String>,
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

        // Insert the message
        let _ = self.insert_message_to_all(&message.clone())?;

        // Check if the inbox topic exists and if not, create it
        if self.db.cf_handle(&inbox_name).is_none() {
            self.create_empty_inbox(inbox_name.clone()).await?;
        }

        // Calculate the hash of the message for the key
        let hash_key = message.calculate_message_hash();
        // println!("Hash key: {}", hash_key);

        // Clone the external_metadata first, then unwrap
        let ext_metadata = message.external_metadata.clone();

        // Get the scheduled time or calculate current time
        let time_key = match ext_metadata.scheduled_time.is_empty() {
            true => ShinkaiStringTime::generate_time_now(),
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

        // If this message has a parent, add this message as a child of the parent
        let parent_key = match maybe_parent_message_key {
            Some(key) => Some(key),
            None => {
                // Fetch the most recent message from the inbox
                let last_messages = self.get_last_messages_from_inbox(inbox_name.clone(), 1, None)?;
                if let Some(first_batch) = last_messages.first() {
                    if let Some(last_message) = first_batch.first() {
                        Some(last_message.calculate_message_hash())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };

        // If this message has a parent, add this message as a child of the parent
        if let Some(parent_key) = parent_key {
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

        {
            // Note: this is the code for enabling WS
            if let Some(manager) = &self.ws_manager {
                let m = manager.lock().await;
                let inbox_name_string = inbox_name.to_string();
                if let Ok(msg_string) = message.to_string() {
                    let _ = m.queue_message(WSTopic::Inbox, inbox_name_string, msg_string).await;
                }
            }
        }

        self.db.write(batch)?;
        Ok(())
    }

    pub fn mark_as_read_up_to(
        &mut self,
        inbox_name: String,
        up_to_message_hash_offset: String,
    ) -> Result<(), ShinkaiDBError> {
        let cf_name_read_list = format!("{}_read_list", inbox_name);
        let read_list_cf = match self.db.cf_handle(&cf_name_read_list) {
            Some(cf) => cf,
            None => {
                return Err(ShinkaiDBError::InboxNotFound(format!(
                    "Inbox not found: {}",
                    inbox_name
                )))
            }
        };

        // Store the up_to_offset message in the read_list
        self.db
            .put_cf(read_list_cf, &up_to_message_hash_offset, &up_to_message_hash_offset)?;

        Ok(())
    }

    pub fn get_last_read_message_from_inbox(&self, inbox_name: String) -> Result<Option<String>, ShinkaiDBError> {
        let cf_name_read_list = format!("{}_read_list", inbox_name);
        let read_list_cf = match self.db.cf_handle(&cf_name_read_list) {
            Some(cf) => cf,
            None => {
                return Err(ShinkaiDBError::InboxNotFound(format!(
                    "Inbox not found: {}",
                    inbox_name
                )))
            }
        };

        let mut iter = self.db.iterator_cf(read_list_cf, rocksdb::IteratorMode::End);

        match iter.next() {
            Some(Ok((key, _))) => {
                let key_str = match String::from_utf8(key.to_vec()) {
                    Ok(s) => s,
                    Err(_) => return Err(ShinkaiDBError::SomeError("UTF-8 conversion error".to_string())),
                };
                Ok(Some(key_str))
            }
            _ => Ok(None),
        }
    }

    pub fn get_last_unread_messages_from_inbox(
        &self,
        inbox_name: String,
        n: usize,
        from_offset_hash_key: Option<String>,
    ) -> Result<Vec<ShinkaiMessage>, ShinkaiDBError> {
        // Fetch the last read message
        let last_read_message = self.get_last_read_message_from_inbox(inbox_name.clone())?;

        // Fetch the last n messages from the inbox
        let messages = self.get_last_messages_from_inbox(inbox_name, n, from_offset_hash_key)?;

        // Flatten the Vec<Vec<ShinkaiMessage>> to Vec<ShinkaiMessage>
        let messages: Vec<ShinkaiMessage> = messages.into_iter().flatten().collect();

        // Iterate over the messages in reverse order until you reach the message with the last_read_message
        let mut unread_messages = Vec::new();
        for message in messages.into_iter().rev() {
            if Some(message.calculate_message_hash()) == last_read_message {
                break;
            }
            unread_messages.push(message);
        }

        unread_messages.reverse();
        Ok(unread_messages)
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
