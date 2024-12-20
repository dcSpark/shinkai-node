use std::{collections::HashMap, str::FromStr, sync::Arc};

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde_json::Value;
use shinkai_message_primitives::{
    schemas::{
        identity::StandardIdentity,
        inbox_name::InboxName,
        inbox_permission::InboxPermission,
        job_config::JobConfig,
        shinkai_name::ShinkaiName,
        smart_inbox::{LLMProviderSubset, ProviderType, SmartInbox},
        ws_types::{WSMessageType, WSUpdateHandler},
    },
    shinkai_message::{
        shinkai_message::{NodeApiData, ShinkaiMessage},
        shinkai_message_schemas::WSTopic,
    },
};
use shinkai_vector_resources::shinkai_time::ShinkaiStringTime;
use tokio::sync::Mutex;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn create_empty_inbox(&self, inbox_name: String) -> Result<(), SqliteManagerError> {
        let smart_inbox_name = format!("New Inbox: {}", inbox_name);
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO inboxes (inbox_name, smart_inbox_name) VALUES (?1, ?2)",
            params![inbox_name, smart_inbox_name],
        )?;
        Ok(())
    }

    pub async fn unsafe_insert_inbox_message(
        &self,
        message: &ShinkaiMessage,
        maybe_parent_message_key: Option<String>,
        ws_manager: Option<Arc<Mutex<dyn WSUpdateHandler + Send>>>,
    ) -> Result<(), SqliteManagerError> {
        let inbox_name_manager =
            InboxName::from_message(message).map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;

        let inbox_name = match &inbox_name_manager {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value.clone(),
        };

        if inbox_name.is_empty() {
            return Err(SqliteManagerError::SomeError("Inbox name is empty".to_string()));
        }

        if !self.does_inbox_exist(&inbox_name)? {
            self.create_empty_inbox(inbox_name.clone())?;
        }

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

        let ext_metadata = message.external_metadata.clone();

        // Get the scheduled time or calculate current time
        let mut time_key = match ext_metadata.scheduled_time.is_empty() {
            true => ShinkaiStringTime::generate_time_now(),
            false => ext_metadata.scheduled_time.clone(),
        };

        if let InboxName::JobInbox { .. } = inbox_name_manager {
            if let Some(parent_key) = &parent_key.clone() {
                let (parent_message, _) = self.fetch_message_and_hash(parent_key)?;
                let parent_time = parent_message.external_metadata.scheduled_time;
                let parsed_time_key: DateTime<Utc> = DateTime::parse_from_rfc3339(&time_key)
                    .map_err(|e| SqliteManagerError::SomeError(e.to_string()))?
                    .into();
                let parsed_parent_time: DateTime<Utc> = DateTime::parse_from_rfc3339(&parent_time)
                    .map_err(|e| SqliteManagerError::SomeError(e.to_string()))?
                    .into();
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
            updated_message.update_node_api_data(Some(node_api_data)).map_err(|e| {
                SqliteManagerError::SomeError(format!("Error updating message with node_api_data: {}", e))
            })?
        };

        let encoded_message = updated_message
            .encode_message()
            .map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO inbox_messages (message_hash, inbox_name, shinkai_message, parent_message_hash, time_key) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![hash_key, inbox_name, encoded_message, parent_key, time_key],
        )?;

        {
            // Note: this is the code for enabling WS
            if let Some(manager) = ws_manager {
                let m = manager.lock().await;
                let inbox_name_string = inbox_name.to_string();
                if let Ok(msg_string) = message.to_string() {
                    let _ = m
                        .queue_message(
                            WSTopic::Inbox,
                            inbox_name_string,
                            msg_string,
                            WSMessageType::None,
                            false,
                        )
                        .await;
                }
            }
        }

        Ok(())
    }

    pub fn fetch_message_and_hash(&self, hash_key: &str) -> Result<(ShinkaiMessage, String), SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT shinkai_message FROM inbox_messages WHERE message_hash = ?1")?;
        let mut rows = stmt.query(params![hash_key])?;

        let encoded_message: Vec<u8> = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?.get(0)?;
        let message = ShinkaiMessage::decode_message_result(encoded_message)
            .map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;
        let message_hash = message.calculate_message_hash_for_pagination();

        Ok((message, message_hash))
    }

    pub fn get_parent_message_hash(
        &self,
        inbox_name: &str,
        hash_key: &str,
    ) -> Result<Option<String>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT parent_message_hash FROM inbox_messages WHERE inbox_name = ?1 AND message_hash = ?2")?;
        let mut rows = stmt.query(params![inbox_name, hash_key])?;

        let parent_key: Option<String> = match rows.next()? {
            Some(row) => row.get(0)?,
            None => return Ok(None),
        };

        Ok(parent_key)
    }

    pub fn get_last_messages_from_inbox(
        &self,
        inbox_name: String,
        n: usize,
        until_offset_hash_key: Option<String>,
    ) -> Result<Vec<Vec<ShinkaiMessage>>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT message_hash, parent_message_hash, shinkai_message FROM inbox_messages
                WHERE inbox_name = ?1
                ORDER BY time_key DESC",
        )?;

        let message_rows = stmt.query_map(params![inbox_name], |row| {
            let message_key: String = row.get(0)?;
            let parent_key: Option<String> = row.get(1)?;
            let encoded_message: Vec<u8> = row.get(2)?;
            let message = ShinkaiMessage::decode_message_result(encoded_message).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            Ok((message_key, parent_key, message))
        })?;

        // (key, parent_key, message)
        let mut messages = Vec::new();
        for message in message_rows {
            messages.push(message?);
        }

        let mut current_key: Option<String> = messages.first().map(|(key, _, _)| key.clone());

        if let Some(ref until_hash) = until_offset_hash_key {
            for (key, _, _) in messages.iter() {
                if key == until_hash {
                    current_key = Some(key.clone());
                    break;
                }
            }
        }

        let mut paths = Vec::new();

        if current_key.is_none() {
            return Ok(paths);
        }

        let mut first_iteration = true;
        let mut tree_found = false;
        let total_elements = until_offset_hash_key.is_some().then(|| n + 1).unwrap_or(n);

        for _i in 0..total_elements {
            let mut path = Vec::new();

            let key = match current_key.clone() {
                Some(k) => k,
                None => break,
            };
            current_key = None;

            let message = messages.iter().find(|(message_key, _, _)| message_key == &key).ok_or(
                SqliteManagerError::SomeError(format!("Message with key not found: {}", key)),
            )?;

            let added_message_hash = &message.0;
            path.push(message.2.clone());

            if let Some(parent_key) = &message.1 {
                tree_found = true;
                current_key = Some(parent_key.clone());

                if !first_iteration {
                    let child_messages = messages
                        .iter()
                        .filter(|(key, parent, _)| parent == &Some(parent_key.clone()) && key != added_message_hash)
                        .map(|(_, _, msg)| msg.clone())
                        .collect::<Vec<ShinkaiMessage>>();

                    path.extend(child_messages);
                }
            }

            paths.push(path);

            // We check if no parent was found, which means we reached the root of the path
            // If so, let's check if there is a solitary message if not then break
            if current_key.is_none() {
                // Move the iterator forward until it matches the current key
                if tree_found {
                    let mut found = false;
                    for (potential_next_key, _, _) in &messages {
                        if found {
                            current_key = Some(potential_next_key.clone());
                            break;
                        }
                        if potential_next_key == &key {
                            found = true;
                        }
                    }
                } else {
                    // If no tree was found, simply move to the next key in the list
                    if let Some(index) = messages.iter().position(|(k, _, _)| k == &key) {
                        if index + 1 < messages.len() {
                            current_key = Some(messages[index + 1].0.clone());
                        }
                    }
                }

                if current_key.is_none() {
                    break;
                }
            }

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

    pub fn does_inbox_exist(&self, inbox_name: &str) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM inboxes WHERE inbox_name = ?1")?;
        let mut rows = stmt.query(params![inbox_name])?;
        let count: i32 = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?.get(0)?;
        Ok(count > 0)
    }

    pub fn is_inbox_empty(&self, inbox_name: &str) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM inbox_messages WHERE inbox_name = ?1")?;
        let mut rows = stmt.query(params![inbox_name])?;
        let count: i32 = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?.get(0)?;
        Ok(count == 0)
    }

    pub fn mark_as_read_up_to(
        &self,
        inbox_name: String,
        up_to_message_hash_offset: String,
    ) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE inboxes SET read_up_to_message_hash = ?1 WHERE inbox_name = ?2",
            params![up_to_message_hash_offset, inbox_name],
        )?;
        Ok(())
    }

    pub fn get_last_read_message_from_inbox(&self, inbox_name: String) -> Result<Option<String>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT read_up_to_message_hash FROM inboxes WHERE inbox_name = ?1")?;
        let mut rows = stmt.query(params![inbox_name])?;
        let read_up_to_message_hash: Option<String> = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?.get(0)?;
        Ok(read_up_to_message_hash)
    }

    pub fn get_last_unread_messages_from_inbox(
        &self,
        inbox_name: String,
        n: usize,
        from_offset_hash_key: Option<String>,
    ) -> Result<Vec<ShinkaiMessage>, SqliteManagerError> {
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
    ) -> Result<(), SqliteManagerError> {
        let shinkai_profile = identity
            .full_identity_name
            .extract_profile()
            .map_err(|_| SqliteManagerError::InvalidProfileName(identity.full_identity_name.to_string()))?;
        self.add_permission_with_profile(inbox_name, shinkai_profile, perm)
    }

    pub fn add_permission_with_profile(
        &self,
        inbox_name: &str,
        profile: ShinkaiName,
        perm: InboxPermission,
    ) -> Result<(), SqliteManagerError> {
        let profile_name = profile
            .get_profile_name_string()
            .ok_or(SqliteManagerError::InvalidIdentityName(profile.to_string()))?;

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO inbox_profile_permissions (inbox_name, profile_name, permission) VALUES (?1, ?2, ?3)",
            params![inbox_name, profile_name, perm.to_string()],
        )?;
        Ok(())
    }

    pub fn remove_permission(&self, inbox_name: &str, identity: &StandardIdentity) -> Result<(), SqliteManagerError> {
        let profile_name = identity.full_identity_name.get_profile_name_string().clone().ok_or(
            SqliteManagerError::InvalidIdentityName(identity.full_identity_name.to_string()),
        )?;

        let profile_exists = self.does_identity_exist(&identity.full_identity_name)?;
        if !profile_exists {
            return Err(SqliteManagerError::ProfileNotFound(profile_name));
        }

        if !self.does_inbox_exist(inbox_name)? {
            return Err(SqliteManagerError::InboxNotFound(inbox_name.to_string()));
        }

        let conn = self.get_connection()?;
        conn.execute(
            "DELETE FROM inbox_profile_permissions WHERE inbox_name = ?1 AND profile_name = ?2",
            params![inbox_name, profile_name],
        )?;

        Ok(())
    }

    pub fn has_permission(
        &self,
        inbox_name: &str,
        identity: &StandardIdentity,
        perm: InboxPermission,
    ) -> Result<bool, SqliteManagerError> {
        let profile_name = identity.full_identity_name.get_profile_name_string().clone().ok_or(
            SqliteManagerError::InvalidIdentityName(identity.full_identity_name.to_string()),
        )?;

        let profile_exists = self.does_identity_exist(&identity.full_identity_name)?;
        if !profile_exists {
            return Err(SqliteManagerError::ProfileNotFound(profile_name));
        }

        if !self.does_inbox_exist(inbox_name)? {
            return Err(SqliteManagerError::InboxNotFound(inbox_name.to_string()));
        }

        let conn = self.get_connection()?;
        let mut stmt =
            conn.prepare("SELECT COUNT(*) FROM inbox_profile_permissions WHERE inbox_name = ?1 AND profile_name = ?2")?;
        let mut rows = stmt.query(params![inbox_name, profile_name])?;
        let count: i32 = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?.get(0)?;

        if count == 0 {
            return Ok(false);
        }

        let mut stmt = conn
            .prepare("SELECT permission FROM inbox_profile_permissions WHERE inbox_name = ?1 AND profile_name = ?2")?;
        let stored_permission = stmt.query_row(params![inbox_name, profile_name], |row| {
            let perm_str: String = row.get(0)?;
            let permission = InboxPermission::from_str(&perm_str).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;
            Ok(permission)
        })?;

        Ok(stored_permission >= perm)
    }

    pub fn get_inboxes_for_profile(
        &self,
        profile_name_identity: StandardIdentity,
    ) -> Result<Vec<String>, SqliteManagerError> {
        // Check if profile exists using does_identity_exists
        let profile_exists = self.does_identity_exist(&profile_name_identity.full_identity_name)?;
        if !profile_exists {
            return Err(SqliteManagerError::ProfileNotFound(format!(
                "Profile not found for: {}",
                profile_name_identity.full_identity_name
            )));
        }

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT inbox_name FROM inboxes")?;
        let mut rows = stmt.query([])?;

        let mut inboxes = Vec::new();
        while let Some(row) = rows.next()? {
            let inbox_name: String = row.get(0)?;

            inboxes.push(inbox_name);
        }

        Ok(inboxes)
    }

    pub fn get_all_smart_inboxes_for_profile(
        &self,
        profile_name_identity: StandardIdentity,
    ) -> Result<Vec<SmartInbox>, SqliteManagerError> {
        let conn = self.get_connection()?;

        let inboxes = self.get_inboxes_for_profile(profile_name_identity.clone())?;

        let smart_inbox_names = {
            let mut stmt = conn.prepare("SELECT inbox_name, smart_inbox_name FROM inboxes")?;
            let mut rows = stmt.query([])?;

            let mut smart_inbox_names = HashMap::new();
            while let Some(row) = rows.next()? {
                let inbox_name: String = row.get(0)?;
                let smart_inbox_name: String = row.get(1)?;

                smart_inbox_names.insert(inbox_name, smart_inbox_name);
            }

            smart_inbox_names
        };

        let mut smart_inboxes = Vec::new();

        for inbox_id in inboxes {
            let last_message = self
                .get_last_messages_from_inbox(inbox_id.clone(), 1, None)?
                .into_iter()
                .next()
                .and_then(|mut v| v.pop());

            let custom_name = smart_inbox_names.get(&inbox_id).unwrap_or(&inbox_id).to_string();

            let mut job_scope_value: Option<Value> = None;
            let mut datetime_created = String::new();
            let mut job_config_value: Option<JobConfig> = None;

            // Determine if the inbox is finished
            let is_finished = if inbox_id.starts_with("job_inbox::") {
                match InboxName::new(inbox_id.clone()).map_err(|e| SqliteManagerError::SomeError(e.to_string()))? {
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

            let (agent_subset, provider_type) = {
                let profile_result = profile_name_identity.full_identity_name.clone().extract_profile();
                match profile_result {
                    Ok(p) => {
                        if inbox_id.starts_with("job_inbox::") {
                            match InboxName::new(inbox_id.clone())
                                .map_err(|e| SqliteManagerError::SomeError(e.to_string()))?
                            {
                                InboxName::JobInbox { unique_id, .. } => {
                                    // Start the timer
                                    let job = self.get_job_with_options(&unique_id, false, false)?;
                                    let agent_id = job.parent_agent_or_llm_provider_id;

                                    // Check if the agent_id is an LLM provider
                                    match self.get_llm_provider(&agent_id, &p) {
                                        Ok(agent) => (
                                            agent.map(LLMProviderSubset::from_serialized_llm_provider),
                                            ProviderType::LLMProvider,
                                        ),
                                        Err(_) => {
                                            // If not found as an LLM provider, check if it exists as an agent
                                            match self.get_agent(&agent_id.to_lowercase()) {
                                                Ok(Some(agent)) => {
                                                    // Fetch the serialized LLM provider
                                                    if let Ok(Some(serialized_llm_provider)) =
                                                        self.get_llm_provider(&agent.llm_provider_id, &p)
                                                    {
                                                        (
                                                            Some(LLMProviderSubset::from_agent(
                                                                agent,
                                                                serialized_llm_provider,
                                                            )),
                                                            ProviderType::Agent,
                                                        )
                                                    } else {
                                                        (None, ProviderType::Unknown)
                                                    }
                                                }
                                                _ => (None, ProviderType::Unknown),
                                            }
                                        }
                                    }
                                }
                                _ => (None, ProviderType::Unknown),
                            }
                        } else {
                            (None, ProviderType::Unknown)
                        }
                    }
                    Err(_) => (None, ProviderType::Unknown),
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
                provider_type,
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

    pub fn update_smart_inbox_name(&self, inbox_id: &str, new_name: &str) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE inboxes SET smart_inbox_name = ?1 WHERE inbox_name = ?2",
            params![new_name, inbox_id],
        )?;
        Ok(())
    }

    pub fn get_last_messages_from_all(&self, n: usize) -> Result<Vec<ShinkaiMessage>, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT shinkai_message FROM inbox_messages
                ORDER BY time_key DESC
                LIMIT ?1",
        )?;
        let mut rows = stmt.query(params![n])?;

        let mut messages = Vec::new();
        while let Some(row) = rows.next()? {
            let encoded_message: Vec<u8> = row.get(0)?;
            let message = ShinkaiMessage::decode_message_result(encoded_message).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(SqliteManagerError::SerializationError(e.to_string())))
            })?;

            messages.push(message);
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use shinkai_message_primitives::{
        schemas::identity::StandardIdentityType,
        shinkai_message::{
            shinkai_message::{MessageBody, MessageData, ShinkaiVersion},
            shinkai_message_schemas::{IdentityPermissions, MessageSchemaType},
        },
        shinkai_utils::{
            encryption::{unsafe_deterministic_encryption_keypair, EncryptionMethod},
            shinkai_message_builder::ShinkaiMessageBuilder,
            signatures::{clone_signature_secret_key, unsafe_deterministic_signature_keypair},
        },
    };
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;
    use x25519_dalek::{PublicKey as EncryptionPublicKey, StaticSecret as EncryptionStaticKey};

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }

    fn generate_message_with_text_and_inbox(
        content: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        recipient_subidentity_name: String,
        origin_destination_identity_name: String,
        timestamp: String,
        inbox_name_value: String,
    ) -> ShinkaiMessage {
        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(content.to_string())
            .body_encryption(EncryptionMethod::None)
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata_with_inbox(
                "".to_string(),
                recipient_subidentity_name.clone().to_string(),
                inbox_name_value, // Use the passed inbox name
                EncryptionMethod::None,
                None,
            )
            .external_metadata_with_schedule(
                origin_destination_identity_name.clone().to_string(),
                origin_destination_identity_name.clone().to_string(),
                timestamp,
            )
            .build()
            .unwrap()
    }

    fn generate_message_with_text(
        content: String,
        my_encryption_secret_key: EncryptionStaticKey,
        my_signature_secret_key: SigningKey,
        receiver_public_key: EncryptionPublicKey,
        recipient_subidentity_name: String,
        origin_destination_identity_name: String,
        timestamp: String,
    ) -> ShinkaiMessage {
        let inbox_name = InboxName::get_regular_inbox_name_from_params(
            origin_destination_identity_name.clone().to_string(),
            "".to_string(),
            origin_destination_identity_name.clone().to_string(),
            recipient_subidentity_name.clone().to_string(),
            false,
        )
        .unwrap();

        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        ShinkaiMessageBuilder::new(my_encryption_secret_key, my_signature_secret_key, receiver_public_key)
            .message_raw_content(content.to_string())
            .body_encryption(EncryptionMethod::None)
            .message_schema_type(MessageSchemaType::TextContent)
            .internal_metadata_with_inbox(
                "".to_string(),
                recipient_subidentity_name.clone().to_string(),
                inbox_name_value,
                EncryptionMethod::None,
                None,
            )
            .external_metadata_with_schedule(
                origin_destination_identity_name.clone().to_string(),
                origin_destination_identity_name.clone().to_string(),
                timestamp,
            )
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn test_insert_single_inbox_message() {
        let db = setup_test_db();

        let node_identity_name = "@@node.shinkai";
        let subidentity_name = "main";
        let (node_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (node_encryption_sk, node_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let message = generate_message_with_text(
            "Only Message".to_string(),
            node_encryption_sk,
            clone_signature_secret_key(&node_identity_sk),
            node_encryption_pk,
            subidentity_name.to_string(),
            node_identity_name.to_string(),
            "2023-07-03T10:00:00.000Z".to_string(),
        );

        db.unsafe_insert_inbox_message(&message, None, None).await.unwrap();

        // Retrieve the message and check
        let inbox_name = InboxName::from_message(&message).unwrap();
        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        let messages = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 1, None)
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0][0].clone().get_message_content().unwrap(),
            "Only Message".to_string()
        );
    }

    #[tokio::test]
    async fn test_insert_two_messages_and_check_order_and_parent() {
        let db = setup_test_db();

        let node_identity_name = "@@node.shinkai";
        let subidentity_name = "main";
        let (node_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (node_encryption_sk, node_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        // Insert first message
        let message1 = generate_message_with_text(
            "First Message".to_string(),
            node_encryption_sk.clone(),
            clone_signature_secret_key(&node_identity_sk),
            node_encryption_pk,
            subidentity_name.to_string(),
            node_identity_name.to_string(),
            "2023-07-02T20:53:34.812Z".to_string(),
        );

        db.unsafe_insert_inbox_message(&message1, None, None).await.unwrap();

        // Insert second message with first message as parent
        let message2 = generate_message_with_text(
            "Second Message".to_string(),
            node_encryption_sk.clone(),
            clone_signature_secret_key(&node_identity_sk),
            node_encryption_pk,
            subidentity_name.to_string(),
            node_identity_name.to_string(),
            "2023-07-02T20:54:34.923Z".to_string(),
        );

        let parent_message_hash = Some(message1.calculate_message_hash_for_pagination());

        db.unsafe_insert_inbox_message(&message2, parent_message_hash.clone(), None)
            .await
            .unwrap();

        // Retrieve messages and check order
        let inbox_name = InboxName::from_message(&message1).unwrap();
        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        let messages = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 2, None)
            .unwrap();
        eprintln!("\n\n\n Messages: {:?}", messages);

        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0][0].clone().get_message_content().unwrap(),
            "First Message".to_string()
        );
        assert_eq!(
            messages[1][0].clone().get_message_content().unwrap(),
            "Second Message".to_string()
        );

        // Check parent of the second message
        let expected_parent_hash = if let MessageBody::Unencrypted(shinkai_body) = &messages[0][0].body {
            shinkai_body
                .internal_metadata
                .node_api_data
                .as_ref()
                .map(|data| data.node_message_hash.clone())
        } else {
            None
        };

        let actual_parent_hash = if let MessageBody::Unencrypted(shinkai_body) = &messages[1][0].body {
            shinkai_body
                .internal_metadata
                .node_api_data
                .as_ref()
                .map(|data| data.parent_hash.clone())
        } else {
            None
        };

        assert_eq!(actual_parent_hash, expected_parent_hash);

        // Retrieve messages with pagination using the last message's hash
        let pagination_hash = messages[1][0].calculate_message_hash_for_pagination();
        eprintln!("Pagination hash: {}", pagination_hash);
        let paginated_messages = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 2, Some(pagination_hash))
            .unwrap();

        eprintln!("Paginated messages: {:?}", paginated_messages);

        // Expecting to get only 1 message back due to pagination
        assert_eq!(paginated_messages.len(), 1);
        assert_eq!(
            paginated_messages[0][0].clone().get_message_content().unwrap(),
            "First Message".to_string()
        );
    }

    #[tokio::test]
    async fn test_insert_messages_with_tree_structure() {
        let db = setup_test_db();

        let node1_identity_name = "@@node1.shinkai";
        let node1_subidentity_name = "main_profile_node1";
        let (node1_identity_sk, _) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, _) = unsafe_deterministic_encryption_keypair(0);

        let (_, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let mut parent_message = None;

        eprintln!("Inserting messages...\n\n");
        let mut parent_message_hash: Option<String> = None;
        let mut parent_message_hash_2: Option<String> = None;
        let mut parent_message_hash_4: Option<String> = None;
        let mut parent_message_hash_5: Option<String> = None;
        /*
        The tree that we are creating looks like:
            1
            ├── 2
            │   ├── 4
            │   │   ├── 6
            │   │   └── 7
            │   │       └── 8
            │   └── 5
            └── 3
         */
        for i in 1..=8 {
            let message = generate_message_with_text(
                format!("Hello World {}", i),
                node1_encryption_sk.clone(),
                clone_signature_secret_key(&node1_identity_sk),
                node1_subencryption_pk,
                node1_subidentity_name.to_string(),
                node1_identity_name.to_string(),
                format!("2023-07-02T20:53:34.81{}Z", i),
            );

            // Necessary to extract the inbox
            parent_message = Some(message.clone());

            let parent_hash: Option<String> = match i {
                2 | 3 => parent_message_hash.clone(),
                4 | 5 => parent_message_hash_2.clone(),
                6 | 7 => parent_message_hash.clone(),
                8 => parent_message_hash_4.clone(),
                _ => None,
            };

            db.unsafe_insert_inbox_message(&message, parent_hash.clone(), None)
                .await
                .unwrap();

            // Update the parent message according to the tree structure
            if i == 1 {
                parent_message_hash = Some(message.calculate_message_hash_for_pagination());
            } else if i == 2 {
                parent_message_hash_2 = Some(message.calculate_message_hash_for_pagination());
            } else if i == 4 {
                parent_message_hash = Some(message.calculate_message_hash_for_pagination());
            } else if i == 7 {
                parent_message_hash_4 = Some(message.calculate_message_hash_for_pagination());
            } else if i == 5 {
                parent_message_hash_5 = Some(message.calculate_message_hash_for_pagination());
            }

            // Print the message hash, content, and parent hash
            println!(
                "message hash: {} message content: {} message parent hash: {}",
                message.calculate_message_hash_for_pagination(),
                message.get_message_content().unwrap(),
                parent_hash.as_deref().unwrap_or("None")
            );
        }

        let inbox_name = InboxName::from_message(&parent_message.unwrap()).unwrap();

        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        eprintln!("\n\n\n Getting messages...");

        let last_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 3, None)
            .unwrap();

        let last_messages_content: Vec<Vec<String>> = last_messages_inbox
            .iter()
            .map(|message_array| {
                message_array
                    .iter()
                    .map(|message| message.clone().get_message_content().unwrap())
                    .collect()
            })
            .collect();

        eprintln!("Last messages: {:?}", last_messages_content);

        assert_eq!(last_messages_inbox.len(), 3);

        // Check the content of the first message array
        assert_eq!(last_messages_inbox[0].len(), 2);
        assert_eq!(
            last_messages_inbox[0][0].clone().get_message_content().unwrap(),
            "Hello World 4".to_string()
        );
        assert_eq!(
            last_messages_inbox[0][1].clone().get_message_content().unwrap(),
            "Hello World 5".to_string()
        );

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[1].len(), 2);
        assert_eq!(
            last_messages_inbox[1][0].clone().get_message_content().unwrap(),
            "Hello World 7".to_string()
        );
        assert_eq!(
            last_messages_inbox[1][1].clone().get_message_content().unwrap(),
            "Hello World 6".to_string()
        );

        // Check the content of the third message array
        assert_eq!(last_messages_inbox[2].len(), 1);
        assert_eq!(
            last_messages_inbox[2][0].clone().get_message_content().unwrap(),
            "Hello World 8".to_string()
        );

        /*
        Now we are updating the tree to looks like this:
            1
            ├── 2
            │   ├── 4
            │   │   ├── 6
            │   │   └── 7
            │   │       └── 8
            │   └── 5
            |       └── 9
            └── 3

            So the new path should be: [1], [2,3], [5,4], [9] (if we request >5 for n)
         */

        // Add message 9 as a child of message 5
        let message = generate_message_with_text(
            "Hello World 9".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_subencryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.819Z".to_string(),
        );

        // Get the hash of message 5 to set as the parent of message 9
        let parent_hash = parent_message_hash_5.clone();

        db.unsafe_insert_inbox_message(&message, parent_hash.clone(), None)
            .await
            .unwrap();

        // Print the message hash, content, and parent hash
        println!(
            "message hash: {} message content: {} message parent hash: {}",
            message.calculate_message_hash_for_pagination(),
            message.get_message_content().unwrap(),
            parent_hash.as_deref().unwrap_or("None")
        );

        // Get the last 5 messages from the inbox
        let last_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 5, None)
            .unwrap();

        let last_messages_content: Vec<Vec<String>> = last_messages_inbox
            .iter()
            .map(|message_array| {
                message_array
                    .iter()
                    .map(|message| message.clone().get_message_content().unwrap())
                    .collect()
            })
            .collect();

        eprintln!("Last messages: {:?}", last_messages_content);

        assert_eq!(last_messages_inbox[3].len(), 1);
        assert_eq!(
            last_messages_inbox[3][0].clone().get_message_content().unwrap(),
            "Hello World 9".to_string()
        );

        // Check the content of the second message array
        assert_eq!(last_messages_inbox[2].len(), 2);
        assert_eq!(
            last_messages_inbox[2][0].clone().get_message_content().unwrap(),
            "Hello World 5".to_string()
        );
        assert_eq!(
            last_messages_inbox[2][1].clone().get_message_content().unwrap(),
            "Hello World 4".to_string()
        );

        // Check the content of the third message array
        assert_eq!(last_messages_inbox[1].len(), 2);
        assert_eq!(
            last_messages_inbox[1][0].clone().get_message_content().unwrap(),
            "Hello World 2".to_string()
        );
        assert_eq!(
            last_messages_inbox[1][1].clone().get_message_content().unwrap(),
            "Hello World 3".to_string()
        );

        assert_eq!(last_messages_inbox[0].len(), 1);
        assert_eq!(
            last_messages_inbox[0][0].clone().get_message_content().unwrap(),
            "Hello World 1".to_string()
        );
    }

    #[tokio::test]
    async fn db_inbox() {
        let db = setup_test_db();

        let node1_identity_name = "@@node1.shinkai";
        let node1_subidentity_name = "main_profile_node1";
        let (node1_identity_sk, node1_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node1_encryption_sk, node1_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (_, node1_subidentity_pk) = unsafe_deterministic_signature_keypair(100);
        let (_, node1_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);

        let message = generate_message_with_text(
            "Hello World".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_subencryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.812Z".to_string(),
        );

        let _ = db.unsafe_insert_inbox_message(&message.clone(), None, None).await;
        println!("Inserted message {:?}", message.encode_message());
        let result = ShinkaiMessage::decode_message_result(message.encode_message().unwrap());
        println!("Decoded message {:?}", result);

        let last_messages_all = db.get_last_messages_from_all(10).unwrap();
        assert_eq!(last_messages_all.len(), 1);
        assert_eq!(
            last_messages_all[0].clone().get_message_content().unwrap(),
            "Hello World".to_string()
        );

        let inbox_name = InboxName::from_message(&message).unwrap();

        let inbox_name_value = match inbox_name {
            InboxName::RegularInbox { value, .. } | InboxName::JobInbox { value, .. } => value,
        };

        println!("Inbox name: {}", inbox_name_value);
        assert_eq!(
            inbox_name_value,
            "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false".to_string()
        );

        println!("Inbox name: {}", inbox_name_value.to_string());
        let last_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name_value.to_string(), 10, None)
            .unwrap();
        assert_eq!(last_messages_inbox.len(), 1);
        assert_eq!(
            last_messages_inbox[0][0].clone().get_message_content().unwrap(),
            "Hello World".to_string()
        );

        // Get last unread messages
        let last_unread = db
            .get_last_unread_messages_from_inbox(inbox_name_value.clone().to_string(), 10, None)
            .unwrap();
        println!("Last unread messages: {:?}", last_unread);
        assert_eq!(last_unread.len(), 1);
        assert_eq!(
            last_unread[0].clone().get_message_content().unwrap(),
            "Hello World".to_string()
        );

        let message2 = generate_message_with_text(
            "Hello World 2".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_subencryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.813Z".to_string(),
        );
        let message3 = generate_message_with_text(
            "Hello World 3".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_subencryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.814Z".to_string(),
        );
        let message4 = generate_message_with_text(
            "Hello World 4".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_subencryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:54:34.814Z".to_string(),
        );
        let message5 = generate_message_with_text(
            "Hello World 5".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_subencryption_pk,
            node1_subidentity_name.to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:55:34.814Z".to_string(),
        );
        match db.unsafe_insert_inbox_message(&message2.clone(), None, None).await {
            Ok(_) => println!("message2 inserted successfully"),
            Err(e) => println!("Failed to insert message2: {}", e),
        }

        match db.unsafe_insert_inbox_message(&message3.clone(), None, None).await {
            Ok(_) => println!("message3 inserted successfully"),
            Err(e) => println!("Failed to insert message3: {}", e),
        }

        match db.unsafe_insert_inbox_message(&message4.clone(), None, None).await {
            Ok(_) => println!("message4 inserted successfully"),
            Err(e) => println!("Failed to insert message4: {}", e),
        }

        match db.unsafe_insert_inbox_message(&message5.clone(), None, None).await {
            Ok(_) => println!("message5 inserted successfully"),
            Err(e) => println!("Failed to insert message5: {}", e),
        }

        let all_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 6, None)
            .unwrap();
        assert_eq!(all_messages_inbox.len(), 5);

        let last_messages_inbox = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 2, None)
            .unwrap();
        assert_eq!(last_messages_inbox.len(), 2);

        let last_unread_messages_inbox = db
            .get_last_unread_messages_from_inbox(inbox_name_value.clone().to_string(), 2, None)
            .unwrap();
        assert_eq!(last_unread_messages_inbox.len(), 2);
        assert_eq!(
            last_unread_messages_inbox[0].clone().get_message_content().unwrap(),
            "Hello World 4".to_string()
        );
        assert_eq!(
            last_unread_messages_inbox[1].clone().get_message_content().unwrap(),
            "Hello World 5".to_string()
        );

        let offset = last_unread_messages_inbox[1]
            .clone()
            .calculate_message_hash_for_pagination();
        println!("\n\n ### Offset: {}", offset);
        println!("Last unread messages: {:?}", last_unread_messages_inbox[1]);
        // check pagination for last unread
        let last_unread_messages_inbox_page2 = db
            .get_last_unread_messages_from_inbox(inbox_name_value.clone().to_string(), 3, Some(offset.clone()))
            .unwrap();
        assert_eq!(last_unread_messages_inbox_page2.len(), 3);
        assert_eq!(
            last_unread_messages_inbox_page2[0]
                .clone()
                .get_message_content()
                .unwrap(),
            "Hello World 2".to_string()
        );

        // check pagination for inbox messages
        let last_unread_messages_inbox_page2 = db
            .get_last_messages_from_inbox(inbox_name_value.clone().to_string(), 3, Some(offset))
            .unwrap();
        assert_eq!(last_unread_messages_inbox_page2.len(), 3);
        assert_eq!(
            last_unread_messages_inbox_page2[0][0]
                .clone()
                .get_message_content()
                .unwrap(),
            "Hello World 2".to_string()
        );

        // Mark as read up to a certain time
        db.mark_as_read_up_to(
            inbox_name_value.clone().to_string(),
            last_unread_messages_inbox_page2[2][0]
                .clone()
                .calculate_message_hash_for_pagination(),
        )
        .unwrap();

        let last_messages_inbox = db
            .get_last_unread_messages_from_inbox(inbox_name_value.clone().to_string(), 2, None)
            .unwrap();
        assert_eq!(last_messages_inbox.len(), 1);

        // Test permissions
        let subidentity_name = "device1";
        let full_subidentity_name =
            ShinkaiName::from_node_and_profile_names(node1_identity_name.to_string(), subidentity_name.to_string())
                .unwrap();

        let device1_subidentity = StandardIdentity::new(
            full_subidentity_name.clone(),
            None,
            node1_encryption_pk.clone(),
            node1_identity_pk.clone(),
            Some(node1_subencryption_pk),
            Some(node1_subidentity_pk),
            StandardIdentityType::Profile,
            IdentityPermissions::Standard,
        );

        let _ = db.insert_profile(device1_subidentity.clone());
        println!("Inserted profile");
        eprintln!("inbox name: {}", inbox_name_value);

        db.add_permission(&inbox_name_value, &device1_subidentity, InboxPermission::Admin)
            .unwrap();
        assert!(db
            .has_permission(&inbox_name_value, &device1_subidentity, InboxPermission::Admin)
            .unwrap());

        db.remove_permission(&inbox_name_value, &device1_subidentity).unwrap();
        assert!(!db
            .has_permission(&inbox_name_value, &device1_subidentity, InboxPermission::Admin)
            .unwrap());

        let message4 = generate_message_with_text(
            "Hello World 6".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_subencryption_pk,
            "other_inbox".to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.815Z".to_string(),
        );
        let message5 = generate_message_with_text(
            "Hello World 7".to_string(),
            node1_encryption_sk.clone(),
            clone_signature_secret_key(&node1_identity_sk),
            node1_subencryption_pk,
            "yet_another_inbox".to_string(),
            node1_identity_name.to_string(),
            "2023-07-02T20:53:34.816Z".to_string(),
        );
        db.unsafe_insert_inbox_message(&message4, None, None).await.unwrap();
        db.unsafe_insert_inbox_message(&message5, None, None).await.unwrap();

        // Test get_inboxes_for_profile
        let node1_profile_identity = StandardIdentity::new(
            ShinkaiName::from_node_and_profile_names(
                node1_identity_name.to_string(),
                node1_subidentity_name.to_string(),
            )
            .unwrap(),
            None,
            node1_encryption_pk.clone(),
            node1_identity_pk.clone(),
            Some(node1_subencryption_pk),
            Some(node1_subidentity_pk),
            StandardIdentityType::Profile,
            IdentityPermissions::Standard,
        );
        let _ = db.insert_profile(node1_profile_identity.clone());
        let inboxes = db.get_inboxes_for_profile(node1_profile_identity.clone()).unwrap();
        assert_eq!(inboxes.len(), 1);

        let inboxes = db.get_inboxes_for_profile(node1_profile_identity.clone()).unwrap();
        assert_eq!(inboxes.len(), 1);
        assert!(inboxes.contains(&"inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false".to_string()));

        // Test get_smart_inboxes_for_profile
        let smart_inboxes = db
            .get_all_smart_inboxes_for_profile(node1_profile_identity.clone())
            .unwrap();
        assert_eq!(smart_inboxes.len(), 1);

        // Check if smart_inboxes contain the expected results
        let expected_inbox_ids = ["inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false"];

        for smart_inbox in smart_inboxes {
            assert!(expected_inbox_ids.contains(&smart_inbox.inbox_id.as_str()));
            assert_eq!(format!("New Inbox: {}", smart_inbox.inbox_id), smart_inbox.custom_name);

            // Check the last_message of each smart_inbox
            if let Some(last_message) = smart_inbox.last_message {
                match last_message.body {
                    MessageBody::Unencrypted(ref body) => match body.message_data {
                        MessageData::Unencrypted(ref data) => match smart_inbox.inbox_id.as_str() {
                            "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false" => {
                                assert_eq!(data.message_raw_content, "Hello World 5");
                            }
                            "inbox::@@node1.shinkai::@@node1.shinkai/other_inbox::false" => {
                                assert_eq!(data.message_raw_content, "Hello World 6");
                            }
                            "inbox::@@node1.shinkai::@@node1.shinkai/yet_another_inbox::false" => {
                                assert_eq!(data.message_raw_content, "Hello World 7");
                            }
                            _ => panic!("Unexpected inbox_id"),
                        },
                        _ => panic!("Expected unencrypted message data"),
                    },
                    _ => panic!("Expected unencrypted message body"),
                }
                assert_eq!(last_message.external_metadata.sender, "@@node1.shinkai");
                assert_eq!(last_message.external_metadata.recipient, "@@node1.shinkai");
                assert_eq!(last_message.encryption, EncryptionMethod::None);
                assert_eq!(last_message.version, ShinkaiVersion::V1_0);
            }
        }

        // Update the name of one of the inboxes
        let inbox_to_update = "inbox::@@node1.shinkai::@@node1.shinkai/main_profile_node1::false";
        let new_name = "New Inbox Name";
        db.update_smart_inbox_name(inbox_to_update, new_name).unwrap();

        // Get smart_inboxes again
        let updated_smart_inboxes = db.get_all_smart_inboxes_for_profile(node1_profile_identity).unwrap();

        // Check if the name of the updated inbox has been changed
        for smart_inbox in updated_smart_inboxes {
            if smart_inbox.inbox_id == inbox_to_update {
                eprintln!("Smart inbox: {:?}", smart_inbox);
                assert_eq!(smart_inbox.custom_name, new_name);
            }
        }
    }

    // For benchmarking purposes
    // #[tokio::test]
    async fn benchmark_get_all_smart_inboxes_for_profile() {
        let db = setup_test_db();

        let node_identity_name = "@@node.shinkai";
        let subidentity_name = "main";
        let (node_identity_sk, node_identity_pk) = unsafe_deterministic_signature_keypair(0);
        let (node_encryption_sk, node_encryption_pk) = unsafe_deterministic_encryption_keypair(0);

        let (_, node_subencryption_pk) = unsafe_deterministic_encryption_keypair(100);
        let (_, node_subidentity_pk) = unsafe_deterministic_signature_keypair(100);

        // Create a profile identity
        let profile_identity = StandardIdentity::new(
            ShinkaiName::from_node_and_profile_names(node_identity_name.to_string(), subidentity_name.to_string())
                .unwrap(),
            None,
            node_encryption_pk.clone(),
            node_identity_pk.clone(),
            Some(node_subencryption_pk),
            Some(node_subidentity_pk),
            StandardIdentityType::Profile,
            IdentityPermissions::Standard,
        );
        let _ = db.insert_profile(profile_identity.clone());

        // Create 100 inboxes with 100 messages each
        for inbox_index in 0..100 {
            let inbox_name = format!("job_inbox::{}::false", inbox_index);
            for message_index in 0..100 {
                let message_content = format!("Message {} for inbox {}", message_index, inbox_index);
                let message = generate_message_with_text_and_inbox(
                    message_content,
                    node_encryption_sk.clone(),
                    clone_signature_secret_key(&node_identity_sk),
                    node_subencryption_pk,
                    subidentity_name.to_string(),
                    node_identity_name.to_string(),
                    format!("2023-07-02T20:53:34.8{}Z", message_index),
                    inbox_name.clone(),
                );

                db.unsafe_insert_inbox_message(&message, None, None).await.unwrap();
            }
        }

        // Measure the time taken by get_all_smart_inboxes_for_profile
        let start_time = std::time::Instant::now();
        let smart_inboxes = db.get_all_smart_inboxes_for_profile(profile_identity).unwrap();
        let duration = start_time.elapsed();

        println!("Time taken to get all smart inboxes: {:?}", duration);
        println!("Number of smart inboxes retrieved: {}", smart_inboxes.len());
    }
}
