use std::sync::Arc;
use std::time::Instant;

use chrono::DateTime;
use chrono::Utc;
use rocksdb::{Error, WriteBatch};

use serde_json::Value;
use shinkai_message_primitives::schemas::identity::StandardIdentity;
use shinkai_message_primitives::schemas::job_config::JobConfig;
use shinkai_message_primitives::schemas::smart_inbox::LLMProviderSubset;
use shinkai_message_primitives::schemas::smart_inbox::SmartInbox;
use shinkai_message_primitives::shinkai_message::shinkai_message::NodeApiData;
use shinkai_message_primitives::{
    schemas::{inbox_name::InboxName, shinkai_name::ShinkaiName, shinkai_time::ShinkaiStringTime},
    shinkai_message::{shinkai_message::ShinkaiMessage, shinkai_message_schemas::WSTopic},
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use tokio::sync::Mutex;

use crate::schemas::inbox_permission::InboxPermission;
use crate::schemas::ws_types::WSMessageType;
use crate::schemas::ws_types::WSUpdateHandler;

use super::{db_main::Topic, db_errors::ShinkaiDBError, ShinkaiDB};

impl ShinkaiDB {
    pub fn create_empty_inbox(&self, inbox_name: String) -> Result<(), Error> {
        shinkai_log(
            ShinkaiLogOption::JobExecution,
            ShinkaiLogLevel::Info,
            &format!("Creating inbox: {}", inbox_name),
        );
        // Use shared CFs
        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();

        // Start a write a batch
        let mut batch = WriteBatch::default();

        // Construct keys with inbox_name as part of the key
        let inbox_key = format!("inbox_placeholder_value_to_match_prefix_abcdef_{}", inbox_name);
        let inbox_read_list_key = format!("{}_read_list", inbox_name);
        let inbox_smart_inbox_name_key = format!("{}_smart_inbox_name", inbox_name);

        // Content
        let initial_inbox_name = format!("New Inbox: {}", inbox_name);

        // Put Inbox Data into the DB
        batch.put_cf(cf_inbox, inbox_key.as_bytes(), "".as_bytes());
        batch.put_cf(cf_inbox, inbox_read_list_key.as_bytes(), "".as_bytes());
        batch.put_cf(
            cf_inbox,
            inbox_smart_inbox_name_key.as_bytes(),
            initial_inbox_name.as_bytes(),
        );

        // Commit the write batch
        self.db.write(batch)?;
        Ok(())
    }

    // This fn doesn't validate access to the inbox (not really a responsibility of this db fn) so it's unsafe in that regard
    pub async fn unsafe_insert_inbox_message(
        &self,
        message: &ShinkaiMessage,
        maybe_parent_message_key: Option<String>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
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

        // Use shared CFs
        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();

        // Construct keys with inbox_name as part of the key
        let inbox_key = format!("inbox_placeholder_value_to_match_prefix_abcdef_{}", inbox_name);
        let fixed_inbox_key = format!("inbox_{}", inbox_name_manager.hash_value_first_half());

        // Check if the inbox exists and if not, create it
        if self.db.get_cf(cf_inbox, inbox_key.as_bytes())?.is_none() {
            self.create_empty_inbox(inbox_name.clone())?;
        }

        // println!("Hash key: {}", hash_key);

        // Clone the external_metadata first, then unwrap
        let ext_metadata = message.external_metadata.clone();

        // Get the scheduled time or calculate current time
        let mut time_key = match ext_metadata.scheduled_time.is_empty() {
            true => ShinkaiStringTime::generate_time_now(),
            false => ext_metadata.scheduled_time.clone(),
        };

        // If this message has a parent, add this message as a child of the parent
        let parent_key = match maybe_parent_message_key {
            Some(key) if !key.is_empty() => Some(key),
            _ => {
                // Fetch the most recent message from the inbox
                let last_messages = self.get_last_messages_from_inbox(inbox_name.clone(), 1, None)?;
                if let Some(first_batch) = last_messages.first() {
                    first_batch
                        .first()
                        .map(|last_message| last_message.calculate_message_hash_for_pagination())
                } else {
                    None
                }
            }
        };

        // Previous code was here
        if let InboxName::JobInbox { .. } = inbox_name_manager {
            if let Some(parent_key) = &parent_key.clone() {
                let (parent_message, _) = self.fetch_message_and_hash(parent_key)?;
                let parent_time = parent_message.external_metadata.scheduled_time;
                let parsed_time_key: DateTime<Utc> = DateTime::parse_from_rfc3339(&time_key)?.into();
                let parsed_parent_time: DateTime<Utc> = DateTime::parse_from_rfc3339(&parent_time)?.into();
                if parsed_time_key < parsed_parent_time {
                    time_key = ShinkaiStringTime::generate_time_now();
                }
            }
        }

        // Calculate the hash of the message for the key
        let hash_key = message.calculate_message_hash_for_pagination();

        // We update the message with some extra information api_node_data
        let updated_message = {
            let node_api_data = NodeApiData {
                parent_hash: parent_key.clone().unwrap_or_default(),
                node_message_hash: hash_key.clone(), // this is safe because hash_key doesn't use node_api_data
                node_timestamp: time_key.clone(),
            };

            let updated_message = message.clone();
            updated_message.update_node_api_data(Some(node_api_data))?
        };

        // Create the composite key by concatenating the time_key and the hash_key, with a separator
        let composite_key = format!("{}_message_{}:::{}", fixed_inbox_key, time_key, hash_key);

        let mut batch = rocksdb::WriteBatch::default();

        // Add the message to the shared column family with a key that includes the inbox name
        batch.put_cf(cf_inbox, composite_key.as_bytes(), &hash_key);

        // Insert the message
        self.insert_message_to_all(&updated_message.clone())?;

        // If this message has a parent, add this message as a child of the parent
        if let Some(parent_key) = parent_key {
            // Construct a key for storing child messages of a parent
            let parent_children_key = format!("{}_children_{}", fixed_inbox_key, parent_key);

            // Fetch existing children for the parent, if any
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

            let mut children = vec![hash_key.clone()];
            children.extend_from_slice(&existing_children);

            batch.put_cf(cf_inbox, parent_children_key.as_bytes(), children.join(","));

            let message_parent_key = format!("{}_parent_{}", fixed_inbox_key, hash_key);

            // Add the parent key to the parents column family with the child key
            self.db.put_cf(cf_inbox, message_parent_key.as_bytes(), parent_key)?;
        }

        {
            // Note: this is the code for enabling WS
            if let Some(manager) = ws_manager {
                let m = manager.lock().await;
                let inbox_name_string = inbox_name.to_string();
                if let Ok(msg_string) = message.to_string() {
                    let _ = m.queue_message(WSTopic::Inbox, inbox_name_string, msg_string, WSMessageType::None, false).await;
                }
            }
        }

        self.db.write(batch)?;
        Ok(())
    }

    pub fn mark_as_read_up_to(
        &self,
        inbox_name: String,
        up_to_message_hash_offset: String,
    ) -> Result<(), ShinkaiDBError> {
        // Use the Inbox CF for marking messages as read
        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();

        // Construct the key for the read list within the Inbox CF
        let inbox_read_list_key = format!("{}_read_list", inbox_name);

        // Store the up_to_message_hash_offset as the value for the read list key
        // This represents the last message that has been read up to
        self.db.put_cf(
            cf_inbox,
            inbox_read_list_key.as_bytes(),
            up_to_message_hash_offset.as_bytes(),
        )?;

        Ok(())
    }

    pub fn get_last_read_message_from_inbox(&self, inbox_name: String) -> Result<Option<String>, ShinkaiDBError> {
        // Use the Inbox CF for fetching the last read message
        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();

        // Construct the key for the last read message within the Inbox CF
        let inbox_read_list_key = format!("{}_read_list", inbox_name);

        // Directly fetch the value associated with the last read message key
        match self.db.get_cf(cf_inbox, inbox_read_list_key.as_bytes())? {
            Some(value) => {
                // Convert the value to a String
                let last_read_message = String::from_utf8(value.to_vec())
                    .map_err(|_| ShinkaiDBError::SomeError("UTF-8 conversion error".to_string()))?;
                Ok(Some(last_read_message))
            }
            None => Ok(None), // If there's no value, return None
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
            if Some(message.calculate_message_hash_for_pagination()) == last_read_message {
                break;
            }
            unread_messages.push(message);
        }

        unread_messages.reverse();
        Ok(unread_messages)
    }

    pub fn add_permission(
        &self,
        inbox_name: &str,
        identity: &StandardIdentity,
        perm: InboxPermission,
    ) -> Result<(), ShinkaiDBError> {
        // Call the new function with the extracted profile name
        let shinkai_profile = identity.full_identity_name.extract_profile()?;
        self.add_permission_with_profile(inbox_name, shinkai_profile, perm)
    }

    pub fn add_permission_with_profile(
        &self,
        inbox_name: &str,
        profile: ShinkaiName,
        perm: InboxPermission,
    ) -> Result<(), ShinkaiDBError> {
        let profile_name = profile
            .get_profile_name_string()
            .clone()
            .ok_or(ShinkaiDBError::InvalidIdentityName(profile.to_string()))?;

        // Check if profile exists using does_identity_exists
        let profile_exists = self.does_identity_exists(&profile)?;
        if !profile_exists {
            return Err(ShinkaiDBError::ProfileNotFound(format!(
                "Profile not found for: {}",
                profile_name
            )));
        }

        // Check if inbox exists
        if !self.does_inbox_exists(inbox_name)? {
            return Err(ShinkaiDBError::InboxNotFound(format!(
                "Inbox not found for: {}",
                inbox_name
            )));
        }

        // Handle the original permission addition
        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();
        let perms_key = format!("{}_perms_{}", inbox_name, profile_name);
        let perm_val = perm.to_i32().to_string(); // Convert permission to i32 and then to String

        match self.db.put_cf(cf_inbox, perms_key.as_bytes(), perm_val.as_bytes()) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Error adding permission: {}", e);
            }
        }

        Ok(())
    }

    pub fn does_inbox_exists(&self, inbox_name: &str) -> Result<bool, ShinkaiDBError> {
        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();
        let inbox_name_manager = InboxName::new(inbox_name.to_string()).map_err(ShinkaiDBError::from)?;
        // let fixed_inbox_key = format!("inbox_{}", inbox_name_manager.hash_value_first_half());
        let fixed_inbox_key = format!(
            "inbox_placeholder_value_to_match_prefix_abcdef_{}",
            inbox_name_manager.get_value()
        );

        Ok(self.db.get_cf(cf_inbox, fixed_inbox_key.as_bytes())?.is_some())
    }

    pub fn remove_permission(&self, inbox_name: &str, identity: &StandardIdentity) -> Result<(), ShinkaiDBError> {
        let profile_name = identity.full_identity_name.get_profile_name_string().clone().ok_or(
            ShinkaiDBError::InvalidIdentityName(identity.full_identity_name.to_string()),
        )?;

        // Check if profile exists using does_identity_exists
        let profile_exists = self.does_identity_exists(&identity.full_identity_name)?;
        if !profile_exists {
            return Err(ShinkaiDBError::ProfileNotFound(format!(
                "Profile not found for: {}",
                profile_name
            )));
        }

        // Check if inbox exists
        if !self.does_inbox_exists(inbox_name)? {
            return Err(ShinkaiDBError::InboxNotFound(format!(
                "Inbox not found for: {}",
                inbox_name
            )));
        }

        // Permission removal logic remains the same
        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();
        let perms_key = format!("{}_perms_{}", inbox_name, profile_name);
        self.db.delete_cf(cf_inbox, perms_key)?;
        Ok(())
    }

    pub fn has_permission(
        &self,
        inbox_name: &str,
        identity: &StandardIdentity,
        perm: InboxPermission,
    ) -> Result<bool, ShinkaiDBError> {
        let profile_name = identity.full_identity_name.get_profile_name_string().clone().ok_or(
            ShinkaiDBError::InvalidIdentityName(identity.full_identity_name.to_string()),
        )?;

        // Check if profile exists using does_identity_exists
        let profile_exists = self.does_identity_exists(&identity.full_identity_name)?;
        if !profile_exists {
            return Err(ShinkaiDBError::ProfileNotFound(format!(
                "Profile not found for: {}",
                profile_name
            )));
        }

        // Check if inbox exists
        if !self.does_inbox_exists(inbox_name)? {
            return Err(ShinkaiDBError::InboxNotFound(format!(
                "Inbox not found for: {}",
                inbox_name
            )));
        }

        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();

        // Construct the permissions key similar to how it's done in add_permission_with_profile
        // TODO: perm_type not used?
        // TODO(?): if it's admin it should be able to access anything :?
        // Construct the permissions key similar to how it's done in add_permission_with_profile
        let perms_key = format!("{}_perms_{}", inbox_name, profile_name);

        // Attempt to fetch the permission value for the constructed key
        match self.db.get_cf(cf_inbox, perms_key.as_bytes())? {
            Some(val) => {
                // Convert the stored permission value back to an integer, then to an InboxPermission enum
                let val_str = String::from_utf8(val.to_vec())
                    .map_err(|_| ShinkaiDBError::SomeError("UTF-8 conversion error".to_string()))?;
                let stored_perm_val = val_str
                    .parse::<i32>()
                    .map_err(|_| ShinkaiDBError::SomeError("Permission value parse error".to_string()))?;
                let stored_perm = InboxPermission::from_i32(stored_perm_val)?;

                // Check if the stored permission is greater than or equal to the requested permission
                Ok(stored_perm >= perm)
            }
            None => {
                // If no permission is found, the identity does not have the requested permission
                Ok(false)
            }
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

        // Check if profile exists using does_identity_exists
        let profile_exists = self.does_identity_exists(&profile_name_identity.full_identity_name)?;
        if !profile_exists {
            return Err(ShinkaiDBError::ProfileNotFound(format!(
                "Profile not found for: {}",
                profile_name_identity.full_identity_name
            )));
        }

        // Create ReadOptions and set the prefix_sa
        let prefix = "inbox_placeholder_value_to_match_prefix_abcdef_"; // Define the prefix for the iterator

        // Create an iterator for the 'inbox' topic with the specified prefix
        let iter = self.db.prefix_iterator_cf(cf_inbox, prefix.as_bytes());

        let mut inboxes = Vec::new();
        for item in iter {
            // Handle the Result returned by the iterator
            match item {
                Ok((key, _)) => {
                    let key_str = String::from_utf8_lossy(&key);

                    // Attempt to strip the prefix from the key_str
                    if let Some(stripped_key_str) = key_str.strip_prefix(prefix) {
                        if stripped_key_str.contains(&profile_name_identity.full_identity_name.to_string()) {
                            inboxes.push(stripped_key_str.to_string());
                        } else {
                            // Check if the identity has read permission for the inbox
                            if let Ok(has_perm) =
                                self.has_permission(stripped_key_str, &profile_name_identity, InboxPermission::Read)
                            {
                                if has_perm {
                                    inboxes.push(stripped_key_str.to_string());
                                }
                            }
                        }
                    } else {
                        // nothing to do here. not expected
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
        shinkai_log(
            ShinkaiLogOption::Api,
            ShinkaiLogLevel::Debug,
            &format!("Inboxes: {}", inboxes.join(", ")),
        );
        Ok(inboxes)
    }

    pub fn get_all_smart_inboxes_for_profile(
        &self,
        profile_name_identity: StandardIdentity,
    ) -> Result<Vec<SmartInbox>, ShinkaiDBError> {
        let inboxes = self.get_inboxes_for_profile(profile_name_identity.clone())?;

        let mut smart_inboxes = Vec::new();

        for inbox_id in inboxes {
            // Start the timer
            let start = Instant::now();

            let last_message = self
                .get_last_messages_from_inbox(inbox_id.clone(), 1, None)?
                .into_iter()
                .next()
                .and_then(|mut v| v.pop());

            // Measure the elapsed time
            let duration = start.elapsed();
            println!("Time taken to get last message: {:?}", duration);

            let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();
            let inbox_smart_inbox_name_key = format!("{}_smart_inbox_name", &inbox_id);
            let custom_name = match self.db.get_cf(cf_inbox, inbox_smart_inbox_name_key.as_bytes())? {
                Some(val) => String::from_utf8(val.to_vec())
                    .map_err(|_| ShinkaiDBError::SomeError("UTF-8 conversion error".to_string()))?,
                None => inbox_id.clone(), // Use the inbox_id as the default value if the custom name is not found
            };

            let mut job_scope_value: Option<Value> = None;
            let mut datetime_created = String::new();
            let mut job_config_value: Option<JobConfig> = None;

            // Determine if the inbox is finished
            let is_finished = if inbox_id.starts_with("job_inbox::") {
                match InboxName::new(inbox_id.clone())? {
                    InboxName::JobInbox { unique_id, .. } => {
                        let job = self.get_job_with_options(&unique_id, false, false)?;
                        let scope_value = job.scope.to_json_value()?;
                        job_scope_value = Some(scope_value);
                        job_config_value = job.config;
                        datetime_created.clone_from(&job.datetime_created);
                        job.is_finished || job.is_hidden
                    }
                    _ => false,
                }
            } else {
                false
            };

            // Start the timer
            let start = Instant::now();

            let agent_subset = {
                let profile_result = profile_name_identity.full_identity_name.clone().extract_profile();
                match profile_result {
                    Ok(p) => {
                        if inbox_id.starts_with("job_inbox::") {
                            match InboxName::new(inbox_id.clone())? {
                                InboxName::JobInbox { unique_id, .. } => {
                                    // Start the timer
                                    let job = self.get_job_with_options(&unique_id, false, false)?;
                                    let agent_id = job.parent_agent_or_llm_provider_id;

                                    // TODO: add caching so we don't call this every time for the same agent_id
                                    match self.get_llm_provider(&agent_id, &p) {
                                        Ok(agent) => agent.map(LLMProviderSubset::from_serialized_llm_provider),
                                        Err(_) => None,
                                    }
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            };

            let smart_inbox = SmartInbox {
                inbox_id: inbox_id.clone(),
                custom_name,
                last_message,
                datetime_created,
                is_finished,
                job_scope: job_scope_value,
                agent: agent_subset,
                job_config: job_config_value,
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

    pub fn update_smart_inbox_name(&self, inbox_id: &str, new_name: &str) -> Result<(), ShinkaiDBError> {
        // Fetch the column family for the Inbox topic
        let cf_inbox = self.get_cf_handle(Topic::Inbox).unwrap();

        // The current CF name is used as a key
        let inbox_smart_inbox_name_key = format!("{}_smart_inbox_name", inbox_id);

        // Update the name in the column family
        self.db
            .put_cf(cf_inbox, inbox_smart_inbox_name_key.as_bytes(), new_name.as_bytes())?;

        Ok(())
    }
}
