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
    shinkai_utils::shinkai_logging::{shinkai_log, ShinkaiLogLevel, ShinkaiLogOption},
};
use shinkai_vector_resources::shinkai_time::ShinkaiStringTime;
use tokio::sync::Mutex;

use crate::{SqliteManager, SqliteManagerError};

impl SqliteManager {
    pub fn create_empty_inbox(&self, inbox_name: String) -> Result<(), SqliteManagerError> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO inboxes (inbox_name, smart_inbox_name) VALUES (?1, ?1)",
            params![inbox_name],
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
            "INSERT INTO inbox_messages (message_id, inbox_name, shinkai_message, parent_message_id, created_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![message.calculate_message_hash_for_pagination(), inbox_name, encoded_message, parent_key],
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
        let mut stmt = conn.prepare("SELECT shinkai_message FROM inbox_messages WHERE message_id = ?1")?;
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
            conn.prepare("SELECT parent_message_id FROM inbox_messages WHERE inbox_name = ?1 AND message_id = ?2")?;
        let mut rows = stmt.query(params![inbox_name, hash_key])?;

        let parent_key: Option<String> = rows.next()?.map(|row| row.get(0)).transpose()?;
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
            "SELECT shinkai_message, parent_message_id FROM inbox_messages
                WHERE inbox_name = ?1
                ORDER BY created_at DESC
                LIMIT ?2",
        )?;

        if let Some(_offset) = &until_offset_hash_key {
            stmt = conn.prepare(
                "SELECT shinkai_message, parent_message_id FROM inbox_messages
                    WHERE inbox_name = ?1 AND created_at <= (SELECT created_at FROM inbox_messages WHERE message_id = ?2)
                    ORDER BY created_at DESC
                    LIMIT ?3",
            )?;
        }

        let mut rows = match &until_offset_hash_key {
            Some(offset) => stmt.query(params![inbox_name, offset, n]),
            None => stmt.query(params![inbox_name, n]),
        }?;

        let mut current_parent_key: Option<String> = None;
        let mut messages = Vec::new();
        while let Some(row) = rows.next()? {
            let mut child_messages = Vec::new();

            let encoded_message: Vec<u8> = row.get(0)?;
            let parent_key: Option<String> = row.get(1)?;
            let message = ShinkaiMessage::decode_message_result(encoded_message)
                .map_err(|e| SqliteManagerError::SomeError(e.to_string()))?;

            if let Some(parent_key) = parent_key {
                if let Some(current_parent_key) = &current_parent_key {
                    if parent_key != *current_parent_key {
                        messages.push(child_messages);
                        child_messages = Vec::new();
                    }
                }
                current_parent_key = Some(parent_key.clone());
            }

            child_messages.push(message);
        }

        Ok(messages)
    }

    pub fn does_inbox_exist(&self, inbox_name: &str) -> Result<bool, SqliteManagerError> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM inboxes WHERE inbox_name = ?1")?;
        let mut rows = stmt.query(params![inbox_name])?;
        let count: i32 = rows.next()?.ok_or(SqliteManagerError::DataNotFound)?.get(0)?;
        Ok(count > 0)
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
        let read_up_to_message_hash: Option<String> = rows.next()?.map(|row| row.get(0)).transpose()?;
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
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO inbox_profile_permissions (inbox_name, profile_name, permission) VALUES (?1, ?2, ?3)",
            params![inbox_name, profile.to_string(), perm.to_string()],
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

        let profile_name = profile_name_identity.full_identity_name.to_string();

        let conn = self.get_connection()?;
        let mut stmt = conn.prepare("SELECT inbox_name FROM inboxes")?;
        let mut rows = stmt.query([])?;

        let mut inboxes = Vec::new();
        while let Some(row) = rows.next()? {
            let inbox_name: String = row.get(0)?;

            if inbox_name.contains(&profile_name) {
                inboxes.push(inbox_name);
            } else {
                // Check if the identity has read permission for the inbox
                if let Ok(has_perm) = self.has_permission(&inbox_name, &profile_name_identity, InboxPermission::Read) {
                    if has_perm {
                        inboxes.push(inbox_name);
                    }
                }
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
                                            match self.get_agent(&agent_id) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use shinkai_vector_resources::model_type::{EmbeddingModelType, OllamaTextEmbeddingsInference};
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn setup_test_db() -> SqliteManager {
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = PathBuf::from(temp_file.path());
        let api_url = String::new();
        let model_type =
            EmbeddingModelType::OllamaTextEmbeddingsInference(OllamaTextEmbeddingsInference::SnowflakeArcticEmbed_M);

        SqliteManager::new(db_path, api_url, model_type).unwrap()
    }
}
