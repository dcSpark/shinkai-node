use super::{db::Topic, db_errors::ShinkaiDBError, db_profile_bound::ProfileBoundWriteBatch, ShinkaiDB};
use rocksdb::{Error, Options};
use serde_json::{from_slice, to_vec};
use shinkai_message_primitives::schemas::{agents::serialized_agent::SerializedAgent, shinkai_name::ShinkaiName};

impl ShinkaiDB {
    // Fetches all agents from the Agents topic
    pub fn get_all_agents(&self) -> Result<Vec<SerializedAgent>, ShinkaiDBError> {
        let cf = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let mut result = Vec::new();
        let prefix = b"agent_";

        let iter = self.db.prefix_iterator_cf(cf, prefix);
        for item in iter {
            match item {
                Ok((_, value)) => {
                    let agent: SerializedAgent = from_slice(value.as_ref()).unwrap();
                    result.push(agent);
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        Ok(result)
    }

    pub fn add_agent(&mut self, agent: SerializedAgent, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Start write batch for atomic operation
        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;

        // Serialize the agent to bytes
        let bytes = to_vec(&agent).unwrap();

        // Get handle to the NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Prefix the agent ID and write it to the NodeAndUsers topic
        let agent_key = format!("agent_{}", &agent.id);
        pb_batch.pb_put_cf(cf_node_and_users, &agent_key, &bytes);

        // Additionally, for each allowed message sender and toolkit permission,
        // you can store them with a specific prefix to indicate their relationship to the agent.
        for profile in &agent.allowed_message_senders {
            let profile_key = format!("agent_{}_profile_{}", &agent.id, profile);
            pb_batch.pb_put_cf(cf_node_and_users, &profile_key, "".as_bytes());
        }
        for toolkit in &agent.toolkit_permissions {
            let toolkit_key = format!("agent_{}_toolkit_{}", &agent.id, toolkit);
            pb_batch.pb_put_cf(cf_node_and_users, &toolkit_key, "".as_bytes());
        }

        // Write the batch
        self.write_pb(pb_batch)?;

        Ok(())
    }

    pub fn remove_agent(&mut self, agent_id: &str, profile: &ShinkaiName) -> Result<(), ShinkaiDBError> {
        // Get cf handle for NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Start write batch for atomic operation
        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;

        // Prefix used to identify all keys related to the agent
        let agent_prefix = format!("agent_{}", agent_id);

        // Iterate over all keys with the agent prefix to remove them
        let iter = self.db.prefix_iterator_cf(cf_node_and_users, agent_prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, _)) => {
                    // Convert the key from bytes to a UTF-8 string
                    let key_str = String::from_utf8(key.to_vec())
                        .map_err(|_| ShinkaiDBError::DataConversionError("UTF-8 conversion error".to_string()))?;
                    pb_batch.pb_delete_cf(cf_node_and_users, &key_str);
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        // Write the batch
        self.write_pb(pb_batch)?;

        Ok(())
    }

    pub fn update_agent_access(
        &mut self,
        agent_id: &str,
        profile: &ShinkaiName,
        new_profiles_with_access: Option<Vec<String>>,
        new_toolkits_accessible: Option<Vec<String>>,
    ) -> Result<(), ShinkaiDBError> {
        // Get handle to the NodeAndUsers topic
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;

        // Start write batch for atomic operation
        let mut pb_batch = ProfileBoundWriteBatch::new(profile)?;

        // Directly update profiles_with_access if provided
        if let Some(profiles) = new_profiles_with_access {
            // Clear existing profiles for this agent and profile
            let existing_profiles_prefix = format!(
                "agent_{}_profile_{}",
                agent_id,
                profile.get_profile_name().unwrap_or_default()
            );
            pb_batch.pb_delete_cf(cf_node_and_users, &existing_profiles_prefix);

            // Add new profiles access
            for profile_access in profiles {
                let profile_key = format!("agent_{}_profile_{}", agent_id, profile_access);
                pb_batch.pb_put_cf(cf_node_and_users, &profile_key, "".as_bytes());
            }
        }

        // Directly update toolkits_accessible if provided
        if let Some(toolkits) = new_toolkits_accessible {
            // Clear existing toolkits for this agent and profile
            let existing_toolkits_prefix = format!(
                "agent_{}_toolkit_{}",
                agent_id,
                profile.get_profile_name().unwrap_or_default()
            );
            pb_batch.pb_delete_cf(cf_node_and_users, &existing_toolkits_prefix);

            // Add new toolkits access
            for toolkit_access in toolkits {
                let toolkit_key = format!("agent_{}_toolkit_{}", agent_id, toolkit_access);
                pb_batch.pb_put_cf(cf_node_and_users, &toolkit_key, "".as_bytes());
            }
        }

        // Write the batch
        self.write_pb(pb_batch)?;

        Ok(())
    }

    pub fn get_agent(&self, agent_id: &str, profile: &ShinkaiName) -> Result<Option<SerializedAgent>, ShinkaiDBError> {
        // Fetch the agent's bytes by their prefixed id from the NodeAndUsers topic
        let prefixed_id = format!("agent_{}", agent_id);
        let agent_bytes = self.pb_topic_get(Topic::NodeAndUsers, &prefixed_id, profile)?;

        // If the agent was found, deserialize the bytes into an agent object and return it
        if !agent_bytes.is_empty() {
            let agent: SerializedAgent = from_slice(agent_bytes.as_slice())?;
            Ok(Some(agent))
        } else {
            Ok(None)
        }
    }

    pub fn get_agent_profiles_with_access(
        &self,
        agent_id: &str,
        profile: &ShinkaiName,
    ) -> Result<Vec<String>, ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let prefix = format!("agent_{}_profile_", agent_id);
        let mut profiles_with_access = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf_node_and_users, prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, _)) => {
                    // Extract profile name from the key
                    let key_str = String::from_utf8(key.to_vec())
                        .map_err(|_| ShinkaiDBError::DataConversionError("UTF-8 conversion error".to_string()))?;
                    if let Some(profile_name) = key_str.split("_").last() {
                        profiles_with_access.push(profile_name.to_string());
                    }
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        Ok(profiles_with_access)
    }

    pub fn get_agent_toolkits_accessible(
        &self,
        agent_id: &str,
        profile: &ShinkaiName,
    ) -> Result<Vec<String>, ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let prefix = format!("agent_{}_toolkit_", agent_id);
        let mut toolkits_accessible = Vec::new();

        let iter = self.db.prefix_iterator_cf(cf_node_and_users, prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, _)) => {
                    // Extract toolkit name from the key
                    let key_str = String::from_utf8(key.to_vec())
                        .map_err(|_| ShinkaiDBError::DataConversionError("UTF-8 conversion error".to_string()))?;
                    if let Some(toolkit_name) = key_str.split("_").last() {
                        toolkits_accessible.push(toolkit_name.to_string());
                    }
                }
                Err(e) => return Err(ShinkaiDBError::RocksDBError(e)),
            }
        }

        Ok(toolkits_accessible)
    }

    pub fn remove_profile_from_agent_access(
        &mut self,
        agent_id: &str,
        profile: &str,
        bounded_profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let profile_key = format!("agent_{}_profile_{}", agent_id, profile);

        // Delete the specific profile access key
        self.pb_delete_cf(cf_node_and_users, &profile_key, bounded_profile)?;

        Ok(())
    }

    pub fn remove_toolkit_from_agent_access(
        &mut self,
        agent_id: &str,
        toolkit: &str,
        bounded_profile: &ShinkaiName,
    ) -> Result<(), ShinkaiDBError> {
        let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
        let toolkit_key = format!("agent_{}_toolkit_{}", agent_id, toolkit);

        // Delete the specific toolkit access key
        self.pb_delete_cf(cf_node_and_users, &toolkit_key, bounded_profile)?;

        Ok(())
    }

    pub fn get_agents_for_profile(&self, profile_name: ShinkaiName) -> Result<Vec<SerializedAgent>, ShinkaiDBError> {
        let profile = profile_name
            .get_profile_name()
            .ok_or(ShinkaiDBError::DataConversionError(
                "Profile name not found".to_string(),
            ))?;
        let mut result = Vec::new();

        // Assuming get_all_agents fetches all agents from the NodeAndUsers topic
        let all_agents = self.get_all_agents()?;

        // Iterate over all agents to check if the profile has access
        for agent in all_agents {
            let access_key = format!("agent_{}_profile_{}", agent.id, profile);
            // Check if the access key exists in the NodeAndUsers topic
            let cf_node_and_users = self.cf_handle(Topic::NodeAndUsers.as_str())?;
            let access_bytes = self.db.get_cf(cf_node_and_users, access_key.as_bytes())?;

            if access_bytes.is_some() {
                // If the profile has access to the agent, add the agent to the result
                result.push(agent);
            } else {
                // Optionally, check for other conditions like creation access
                if profile_name.contains(&agent.full_identity_name) {
                    result.push(agent);
                }
            }
        }

        Ok(result)
    }
}
